//! Actor que posee el estado eBPF. Toda mutacion de mapas pasa por aqui, en un solo
//! hilo logico, para no compartir el Ebpf entre tareas.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    mem::size_of,
    ptr,
    sync::{atomic::Ordering, Arc},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;
use aya::{
    Ebpf,
    maps::{
        lpm_trie::{Key, LpmTrie},
        Array, MapData, RingBuf,
    },
    programs::{CgroupAttachMode, CgroupSockAddr},
};
use log::{info, warn};
use tokio::{io::unix::AsyncFd, signal, sync::mpsc};

use soar_agent_common::ConnectEvent;

use crate::{
    policy::{parse_cidr, Mode, Policy},
    state::{AppState, Command, EventOut},
};

pub struct Actor {
    pub ebpf: Ebpf,
    pub blocklist: LpmTrie<MapData, u32, u8>,
    pub control: Array<MapData, u32>,
    pub ring: AsyncFd<RingBuf<MapData>>,
    pub state: Arc<AppState>,
    pub cmd_rx: mpsc::Receiver<Command>,
}

impl Actor {
    /// Carga el eBPF, rellena la blocklist desde la politica, pone el interruptor segun el
    /// modo y engancha el hook. Devuelve el actor listo para ejecutar run().
    pub fn setup(state: Arc<AppState>, cmd_rx: mpsc::Receiver<Command>) -> anyhow::Result<Self> {
        let mut ebpf = Ebpf::load(aya::include_bytes_aligned!(concat!(
            env!("OUT_DIR"),
            "/soar-agent"
        )))
        .context("cargando el objeto eBPF")?;

        let blocklist_map = ebpf
            .take_map("BLOCKLIST")
            .context("mapa BLOCKLIST no encontrado")?;
        let mut blocklist: LpmTrie<MapData, u32, u8> =
            LpmTrie::try_from(blocklist_map).context("construyendo LpmTrie BLOCKLIST")?;

        let control_map = ebpf
            .take_map("CONTROL")
            .context("mapa CONTROL no encontrado")?;
        let mut control: Array<MapData, u32> =
            Array::try_from(control_map).context("construyendo Array CONTROL")?;

        let (mode, entries) = {
            let p = state.policy.lock().unwrap();
            (p.mode, p.blocklist.clone())
        };
        for spec in &entries {
            let (ip, prefix) = parse_cidr(spec)?;
            blocklist
                .insert(&Key::new(prefix, ip), &1u8, 0)
                .with_context(|| format!("insertando {spec} en la blocklist"))?;
        }
        control.set(0, &(if mode == Mode::Enforce { 1u32 } else { 0 }), 0)?;

        {
            let program: &mut CgroupSockAddr = ebpf
                .program_mut("connect4")
                .context("programa connect4 no encontrado")?
                .try_into()?;
            program.load().context("cargando connect4 en el kernel")?;
            let cgroup = File::open(&state.config.cgroup)
                .with_context(|| format!("abriendo cgroup {}", state.config.cgroup.display()))?;
            program
                .attach(cgroup, CgroupAttachMode::default())
                .context("enganchando connect4 al cgroup")?;
        }

        let ring_map = ebpf
            .take_map("EVENTS")
            .context("mapa EVENTS no encontrado")?;
        let ring = AsyncFd::new(RingBuf::try_from(ring_map).context("construyendo RingBuf")?)
            .context("registrando RingBuf en tokio")?;

        Ok(Self {
            ebpf,
            blocklist,
            control,
            ring,
            state,
            cmd_rx,
        })
    }

    /// Bucle principal: atiende ordenes del panel y drena eventos del kernel.
    pub async fn run(self) {
        // Se destructura para que los prestamos de los campos sean disjuntos dentro de select!.
        let Actor {
            ebpf: _ebpf,
            mut blocklist,
            mut control,
            mut ring,
            state,
            mut cmd_rx,
        } = self;

        if state.config.panel {
            info!(
                "Agente en marcha (version {}). Panel en http://{}",
                env!("CARGO_PKG_VERSION"),
                state.config.listen
            );
        } else {
            info!(
                "Agente en marcha (version {}). Panel deshabilitado (solo enforcement)",
                env!("CARGO_PKG_VERSION")
            );
        }

        loop {
            tokio::select! {
                maybe = cmd_rx.recv() => {
                    match maybe {
                        Some(cmd) => {
                            if !handle_command(cmd, &mut blocklist, &mut control, &state) {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                ready = ring.readable_mut() => {
                    match ready {
                        Ok(mut guard) => {
                            while let Some(item) = guard.get_inner_mut().next() {
                                if item.len() >= size_of::<ConnectEvent>() {
                                    // SAFETY: el eBPF solo publica ConnectEvent y ya se comprobo el tamano.
                                    let raw = unsafe {
                                        ptr::read_unaligned(item.as_ptr().cast::<ConnectEvent>())
                                    };
                                    emit(raw, &state);
                                }
                            }
                            guard.clear_ready();
                        }
                        Err(e) => warn!("ring buffer: {e}"),
                    }
                }
                _ = signal::ctrl_c() => {
                    info!("Senal de parada recibida");
                    break;
                }
            }
        }

        info!("Agente detenido");
    }
}

/// El loopback nunca se bloquea: protege el acceso al propio panel.
fn is_loopback_spec(spec: &str) -> bool {
    spec.split('/')
        .next()
        .unwrap_or(spec)
        .trim()
        .parse::<std::net::Ipv4Addr>()
        .map(|a| a.is_loopback())
        .unwrap_or(false)
}

/// Devuelve false cuando hay que terminar el bucle (apagado solicitado).
fn handle_command(
    cmd: Command,
    blocklist: &mut LpmTrie<MapData, u32, u8>,
    control: &mut Array<MapData, u32>,
    state: &AppState,
) -> bool {
    match cmd {
        Command::AddBlock(spec) => match parse_cidr(&spec) {
            Ok(_) if is_loopback_spec(&spec) => {
                warn!("ignorado {spec}: es loopback (protege el acceso al panel)");
            }
            Ok((ip, prefix)) => match blocklist.insert(&Key::new(prefix, ip), &1u8, 0) {
                Ok(()) => {
                    info!("Bloqueado {spec}");
                    mutate_policy(state, |p| {
                        if !p.blocklist.iter().any(|x| x == &spec) {
                            p.blocklist.push(spec.clone());
                        }
                    });
                }
                Err(e) => warn!("No se pudo bloquear {spec}: {e}"),
            },
            Err(e) => warn!("CIDR invalido {spec}: {e}"),
        },
        Command::RemoveBlock(spec) => match parse_cidr(&spec) {
            Ok((ip, prefix)) => {
                let _ = blocklist.remove(&Key::new(prefix, ip));
                mutate_policy(state, |p| p.blocklist.retain(|x| x != &spec));
                info!("Desbloqueado {spec}");
            }
            Err(e) => warn!("CIDR invalido {spec}: {e}"),
        },
        Command::SetEnforce(on) => {
            if let Err(e) = control.set(0, &(if on { 1u32 } else { 0 }), 0) {
                warn!("no se pudo escribir CONTROL: {e}");
            }
            state.enforcing.store(on, Ordering::SeqCst);
            mutate_policy(state, |p| {
                p.mode = if on { Mode::Enforce } else { Mode::Monitor };
            });
            info!("Enforcement {}", if on { "ACTIVO" } else { "pausado (monitor)" });
        }
        Command::ReplacePolicy(new_policy) => {
            let keys: Vec<Key<u32>> = blocklist.keys().filter_map(|k| k.ok()).collect();
            for key in keys {
                let _ = blocklist.remove(&key);
            }
            for spec in &new_policy.blocklist {
                if is_loopback_spec(spec) {
                    warn!("ignorado en la politica: {spec} es loopback");
                    continue;
                }
                if let Ok((ip, prefix)) = parse_cidr(spec) {
                    let _ = blocklist.insert(&Key::new(prefix, ip), &1u8, 0);
                } else {
                    warn!("CIDR invalido en la politica: {spec}");
                }
            }
            let on = matches!(new_policy.mode, Mode::Enforce);
            let _ = control.set(0, &(if on { 1u32 } else { 0 }), 0);
            state.enforcing.store(on, Ordering::SeqCst);
            mutate_policy(state, |p| *p = new_policy);
            info!("Politica reemplazada");
        }
        Command::Shutdown => {
            info!("Apagado solicitado desde el panel");
            return false;
        }
    }
    true
}

/// Aplica un cambio a la politica y la persiste en disco.
fn mutate_policy(state: &AppState, f: impl FnOnce(&mut Policy)) {
    let snapshot = {
        let mut p = state.policy.lock().unwrap();
        f(&mut p);
        p.clone()
    };
    if let Err(e) = snapshot.save(&state.config.policy) {
        warn!(
            "no se pudo guardar la politica en {}: {e}",
            state.config.policy.display()
        );
    }
}

/// Publica un evento: contadores, historial, SSE y log JSONL.
fn emit(raw: ConnectEvent, state: &AppState) {
    let ip = std::net::Ipv4Addr::from(raw.dst_ip4);
    // No registrar las peticiones al propio panel (ruido y fuga de su trafico).
    if ip.is_loopback() && Some(raw.dst_port) == state.panel_port {
        return;
    }

    let blocked = raw.blocked != 0;
    if blocked {
        state.blocked.fetch_add(1, Ordering::Relaxed);
    } else {
        state.allowed.fetch_add(1, Ordering::Relaxed);
    }

    let comm = String::from_utf8_lossy(&raw.comm)
        .trim_end_matches(char::from(0))
        .to_string();
    let out = EventOut {
        ts_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        pid: raw.pid,
        uid: raw.uid,
        comm,
        ip: ip.to_string(),
        port: raw.dst_port,
        action: if blocked { "block" } else { "allow" }.to_string(),
    };

    if let Ok(mut h) = state.history.lock() {
        h.push_back(out.clone());
        while h.len() > state.config.history {
            h.pop_front();
        }
    }
    let _ = state.events_tx.send(out.clone());

    if let Some(path) = &state.config.log {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{}", serde_json::to_string(&out).unwrap_or_default());
        }
    }
}
