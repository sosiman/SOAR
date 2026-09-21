//! Configuracion del agente (fichero YAML + overrides por CLI).

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Direccion de escucha del panel web, por ejemplo 127.0.0.1:8787 .
    #[serde(default = "default_listen")]
    pub listen: String,
    /// Token de acceso al panel. Vacio = sin autenticacion (solo recomendado en localhost).
    #[serde(default)]
    pub token: String,
    /// Cgroup al que enganchar el hook (por defecto la raiz: todo el sistema).
    #[serde(default = "default_cgroup")]
    pub cgroup: PathBuf,
    /// Fichero de politica (YAML) que se guarda y recarga.
    #[serde(default = "default_policy")]
    pub policy: PathBuf,
    /// Fichero JSONL donde se anexan los eventos. Vacio = no persistir.
    #[serde(default)]
    pub log: Option<PathBuf>,
    /// Numero de eventos recientes que se conservan en memoria para el panel.
    #[serde(default = "default_history")]
    pub history: usize,
}

fn default_listen() -> String {
    "127.0.0.1:8787".to_string()
}
fn default_cgroup() -> PathBuf {
    PathBuf::from("/sys/fs/cgroup")
}
fn default_policy() -> PathBuf {
    PathBuf::from("/etc/soar-agent/policy.yaml")
}
fn default_history() -> usize {
    500
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            token: String::new(),
            cgroup: default_cgroup(),
            policy: default_policy(),
            log: None,
            history: default_history(),
        }
    }
}

impl Config {
    /// Carga el fichero si existe; si no, devuelve la configuracion por defecto.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            Ok(serde_yaml::from_str(&text)?)
        } else {
            Ok(Self::default())
        }
    }
}
