//! soar-agent: agente de ejecucion de acciones SOAR a nivel de kernel.
//!
//! MVP (fase 1): engancha un programa eBPF a cgroup/connect4, lo que permite DENEGAR
//! llamadas connect() IPv4 directamente en el kernel (retorno EPERM) consultando una lista
//! de bloqueo con soporte de CIDR, y reporta cada intento a la consola via ring buffer.

use std::{fs::File, net::Ipv4Addr, path::PathBuf, ptr};

use anyhow::{Context as _, anyhow};
use aya::{
    Ebpf,
    maps::{
        RingBuf,
        lpm_trie::{Key as LpmKey, LpmTrie},
    },
    programs::{CgroupAttachMode, CgroupSockAddr},
};
use clap::Parser;
use log::{info, warn};
use soar_agent_common::ConnectEvent;
use tokio::{io::unix::AsyncFd, signal};

#[derive(Debug, Parser)]
#[command(
    name = "soar-agent",
    version,
    about = "Agente SOAR: observa y bloquea conexiones TCP salientes en el kernel (eBPF)"
)]
struct Cli {
    /// IP o CIDR a bloquear (repetible). Ej: --block 1.2.3.4 --block 10.0.0.0/8
    #[arg(long = "block", value_name = "CIDR")]
    block: Vec<String>,

    /// Cgroup al que enganchar el programa. Por defecto la raiz (todo el sistema).
    #[arg(long, value_name = "PATH", default_value = "/sys/fs/cgroup")]
    cgroup: PathBuf,

    /// Registrar tambien las conexiones permitidas (por defecto solo las bloqueadas).
    #[arg(long)]
    log_allowed: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();

    if unsafe { libc::geteuid() } != 0 {
        return Err(anyhow!(
            "este agente carga programas eBPF y necesita root; ejecutalo con sudo"
        ));
    }

    bump_memlock_rlimit();

    let mut ebpf = Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/soar-agent"
    )))
    .context("cargando el objeto eBPF")?;

    // 1) Poblar la lista de bloqueo (LPM trie: soporta CIDR).
    {
        let mut blocklist: LpmTrie<_, u32, u8> = LpmTrie::try_from(
            ebpf.map_mut("BLOCKLIST")
                .ok_or_else(|| anyhow!("mapa BLOCKLIST no encontrado"))?,
        )
        .context("construyendo LpmTrie BLOCKLIST")?;

        for spec in &cli.block {
            let (ip_be, prefix) = parse_cidr(spec)?;
            blocklist
                .insert(&LpmKey::new(prefix, ip_be), &1u8, 0)
                .with_context(|| format!("insertando {spec} en la blocklist"))?;
            info!("Bloqueando {spec}");
        }
        if cli.block.is_empty() {
            warn!("No hay IPs bloqueadas; solo se observaran conexiones (usa --block)");
        }
    }

    // 2) Cargar y enganchar el hook cgroup/connect4.
    {
        let program: &mut CgroupSockAddr = ebpf
            .program_mut("connect4")
            .ok_or_else(|| anyhow!("programa connect4 no encontrado"))?
            .try_into()?;
        program.load().context("cargando connect4 en el kernel")?;

        let cgroup = File::open(&cli.cgroup)
            .with_context(|| format!("abriendo cgroup {}", cli.cgroup.display()))?;
        program
            .attach(cgroup, CgroupAttachMode::default())
            .context("enganchando connect4 al cgroup")?;
        info!(
            "Hook cgroup/connect4 activo en {} (bloqueo via kernel, retorno EPERM)",
            cli.cgroup.display()
        );
    }

    // 3) Escuchar eventos del ring buffer.
    let ring_map = ebpf
        .take_map("EVENTS")
        .ok_or_else(|| anyhow!("mapa EVENTS no encontrado"))?;
    let ring = RingBuf::try_from(ring_map).context("construyendo RingBuf")?;
    let mut ring = AsyncFd::new(ring).context("registrando RingBuf en el runtime de tokio")?;

    info!("Observando conexiones... pulsa Ctrl-C para salir.");
    loop {
        tokio::select! {
            ready = ring.readable_mut() => {
                let mut guard = ready?;
                while let Some(item) = guard.get_inner_mut().next() {
                    if item.len() < core::mem::size_of::<ConnectEvent>() {
                        continue;
                    }
                    // SAFETY: el eBPF solo publica ConnectEvent y ya hemos comprobado el tamano.
                    let event = unsafe {
                        ptr::read_unaligned(item.as_ptr().cast::<ConnectEvent>())
                    };
                    if event.blocked == 0 && !cli.log_allowed {
                        continue;
                    }
                    report(&event);
                }
                guard.clear_ready();
            }
            _ = signal::ctrl_c() => {
                info!("Saliendo...");
                break;
            }
        }
    }

    Ok(())
}

/// Eleva RLIMIT_MEMLOCK (necesario en kernels antiguos; inocuo en modernos).
fn bump_memlock_rlimit() {
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) } != 0 {
        warn!("no se pudo elevar RLIMIT_MEMLOCK (normalmente irrelevante en kernels modernos)");
    }
}

/// Parsea 1.2.3.4 o 1.2.3.0/24 a la clave del LPM trie.
///
/// Devuelve (direccion_en_bytes_de_red, prefix_len).
fn parse_cidr(spec: &str) -> anyhow::Result<(u32, u32)> {
    let (ip_s, prefix_s) = match spec.split_once('/') {
        Some((ip, prefix)) => (ip, Some(prefix)),
        None => (spec, None),
    };
    let ip: Ipv4Addr = ip_s
        .trim()
        .parse()
        .with_context(|| format!("IP invalida: {ip_s}"))?;
    let prefix: u32 = match prefix_s {
        Some(p) => p
            .trim()
            .parse()
            .with_context(|| format!("prefijo invalido: {p}"))?,
        None => 32,
    };
    if prefix > 32 {
        return Err(anyhow!("prefijo fuera de rango (0-32): {prefix}"));
    }
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let host = u32::from(ip) & mask;
    // El kernel compara los bytes tal cual: usamos la representacion de red como entero nativo.
    Ok((host.to_be(), prefix))
}

fn report(event: &ConnectEvent) {
    let ip = Ipv4Addr::from(event.dst_ip4);
    let comm = String::from_utf8_lossy(&event.comm);
    let comm = comm.trim_end_matches(char::from(0));
    let tag = if event.blocked != 0 { "BLOCK" } else { "ALLOW" };
    println!(
        "{tag} pid={:<6} uid={:<5} comm={:<16} dst={ip}:{}",
        event.pid, event.uid, comm, event.dst_port
    );
}
