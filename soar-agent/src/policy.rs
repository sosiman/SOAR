//! Politica de respuesta: modo (enforce/monitor) y lista de bloqueo (CIDR).

use std::path::Path;

use anyhow::{Context as _, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Bloquea de verdad (SK_DROP -> EPERM).
    Enforce,
    /// Solo observa y registra; no bloquea.
    Monitor,
}

fn default_mode() -> Mode {
    Mode::Enforce
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default = "default_mode")]
    pub mode: Mode,
    /// Lista de IPs o CIDR a bloquear.
    #[serde(default)]
    pub blocklist: Vec<String>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            mode: Mode::Enforce,
            blocklist: Vec::new(),
        }
    }
}

impl Policy {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            Ok(serde_yaml::from_str(&text)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, serde_yaml::to_string(self)?)?;
        Ok(())
    }
}

/// Parsea "1.2.3.4" o "1.2.3.0/24" a la clave del LPM trie.
///
/// Devuelve (direccion en bytes de red como entero nativo, prefix_len).
/// El kernel compara los bytes tal cual, por eso NO se usa from_be_bytes aqui.
pub fn parse_cidr(spec: &str) -> anyhow::Result<(u32, u32)> {
    let (ip_s, prefix_s) = match spec.split_once('/') {
        Some((ip, prefix)) => (ip, Some(prefix)),
        None => (spec, None),
    };
    let ip: std::net::Ipv4Addr = ip_s
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
    Ok(((u32::from(ip) & mask).to_be(), prefix))
}
