//! soar-agent: agente SOAR autonomo con enforcement eBPF (cgroup/connect4) y panel web.

mod actor;
mod api;
mod config;
mod policy;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use clap::Parser;
use log::warn;
use tokio::sync::{broadcast, mpsc};

use crate::{
    actor::Actor,
    config::Config,
    policy::{Mode, Policy},
    state::{AppState, Command, EventOut},
};

#[derive(Debug, Parser)]
#[command(
    name = "soar-agent",
    version,
    about = "Agente SOAR autonomo: enforcement eBPF + panel web de control"
)]
struct Cli {
    /// Fichero de configuracion YAML.
    #[arg(short, long, default_value = "/etc/soar-agent/agent.yaml")]
    config: PathBuf,
    /// Escucha del panel (ej. 127.0.0.1:8787).
    #[arg(long)]
    listen: Option<String>,
    /// Token de acceso al panel (vacio = sin autenticacion).
    #[arg(long)]
    token: Option<String>,
    /// Cgroup al que enganchar el hook.
    #[arg(long)]
    cgroup: Option<PathBuf>,
    /// Arrancar en modo monitor (no bloquea, solo observa).
    #[arg(long)]
    monitor: bool,
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

    let mut config = match Config::load(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "config {} ilegible ({e}); usando valores por defecto",
                cli.config.display()
            );
            Config::default()
        }
    };
    if let Some(listen) = cli.listen {
        config.listen = listen;
    }
    if let Some(cgroup) = cli.cgroup {
        config.cgroup = cgroup;
    }

    let token = resolve_token(cli.token.clone(), &config);
    if config.panel && token.is_none() && !is_loopback_listen(&config.listen) {
        warn!(
            "panel en {} SIN token: exponlo solo en localhost o define token/token_file",
            config.listen
        );
    }

    let mut policy = Policy::load(&config.policy).unwrap_or_default();
    if cli.monitor {
        policy.mode = Mode::Monitor;
    }

    bump_memlock_rlimit();

    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(64);
    let (events_tx, _events_rx) = broadcast::channel::<EventOut>(2048);
    let state = Arc::new(AppState::new(
        config.clone(),
        policy,
        token,
        cmd_tx,
        events_tx,
    ));

    let actor = Actor::setup(state.clone(), cmd_rx).context("inicializando el agente eBPF")?;

    if config.panel {
        let api_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = api::serve(api_state).await {
                log::error!("panel web: {e}");
            }
        });
    } else {
        log::info!("Panel web deshabilitado (panel: false); enforcement activo sin interfaz");
    }

    actor.run().await;

    // Persistir la politica por si el panel la modifico en caliente.
    let snapshot = state.policy.lock().unwrap().clone();
    if let Err(e) = snapshot.save(&state.config.policy) {
        warn!("no se pudo guardar la politica: {e}");
    }
    log::info!("Adios");
    Ok(())
}

/// Resuelve el token del panel por prioridad: CLI > entorno > config > fichero.
fn resolve_token(cli: Option<String>, config: &Config) -> Option<String> {
    if let Some(t) = cli {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    if let Ok(t) = std::env::var("SOAR_AGENT_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let t = config.token.trim().to_string();
    if !t.is_empty() {
        return Some(t);
    }
    if let Some(path) = &config.token_file {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let t = text.trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
            Err(e) => warn!("no se pudo leer token_file {}: {e}", path.display()),
        }
    }
    None
}

/// True si la escucha esta limitada a loopback.
fn is_loopback_listen(listen: &str) -> bool {
    let host = listen.rsplit_once(':').map(|(h, _)| h).unwrap_or(listen);
    host == "127.0.0.1" || host == "localhost" || host == "::1" || host == "[::1]"
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
