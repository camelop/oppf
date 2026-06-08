//! Parsing of `.opp/config.toml` (the OPP-CONFIG).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Configuration declared in `.opp/config.toml`. All fields are optional; a
/// missing file yields [`OppConfig::default`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OppConfig {
    /// The coding agent used to implement and review the project.
    #[serde(default = "default_agent")]
    pub agent: String,

    /// External paths treated as read-only resources: not generated, checked or
    /// modified by the agent.
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_agent() -> String {
    "claude-code".to_string()
}

impl Default for OppConfig {
    fn default() -> Self {
        Self {
            agent: default_agent(),
            exclude: Vec::new(),
        }
    }
}

impl OppConfig {
    /// Load the config from `<opp_dir>/config.toml`, or the default if absent.
    pub fn load(opp_dir: &Path) -> Result<Self> {
        let path = opp_dir.join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }
}
