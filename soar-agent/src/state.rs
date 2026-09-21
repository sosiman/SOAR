//! Estado compartido entre el actor eBPF y el panel web.

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Mutex,
    },
    time::Instant,
};

use serde::Serialize;
use tokio::sync::{broadcast, mpsc};

use crate::{
    config::Config,
    policy::{Mode, Policy},
};

/// Evento tal como lo consume el panel.
#[derive(Debug, Clone, Serialize)]
pub struct EventOut {
    pub ts_ms: u64,
    pub pid: u32,
    pub uid: u32,
    pub comm: String,
    pub ip: String,
    pub port: u16,
    pub action: String,
}

/// Ordenes que el panel envia al actor eBPF.
#[derive(Debug)]
pub enum Command {
    AddBlock(String),
    RemoveBlock(String),
    SetEnforce(bool),
    ReplacePolicy(Policy),
    Shutdown,
}

pub struct AppState {
    pub started: Instant,
    pub config: Config,
    /// Token resuelto (CLI/env/config/token_file). None = panel sin autenticacion.
    pub token: Option<String>,
    /// Puerto del panel, para no ensuciar el historial con sus propias peticiones.
    pub panel_port: Option<u16>,
    pub policy: Mutex<Policy>,
    pub cmd_tx: mpsc::Sender<Command>,
    pub events_tx: broadcast::Sender<EventOut>,
    pub history: Mutex<VecDeque<EventOut>>,
    pub allowed: AtomicU64,
    pub blocked: AtomicU64,
    pub enforcing: AtomicBool,
}

impl AppState {
    pub fn new(
        config: Config,
        policy: Policy,
        token: Option<String>,
        cmd_tx: mpsc::Sender<Command>,
        events_tx: broadcast::Sender<EventOut>,
    ) -> Self {
        let enforcing = policy.mode == Mode::Enforce;
        let panel_port = config
            .listen
            .rsplit(':')
            .next()
            .and_then(|p| p.parse::<u16>().ok());
        Self {
            started: Instant::now(),
            config,
            token,
            panel_port,
            policy: Mutex::new(policy),
            cmd_tx,
            events_tx,
            history: Mutex::new(VecDeque::new()),
            allowed: AtomicU64::new(0),
            blocked: AtomicU64::new(0),
            enforcing: AtomicBool::new(enforcing),
        }
    }
}
