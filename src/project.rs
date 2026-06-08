//! Discovery and layout of an OPPF project.
//!
//! Layout (see `.notes/guidelines.md`):
//!
//! ```text
//! .opp/
//!   design/index.md   (or a single design.md)
//!   config.toml       (optional)
//!   review/*.md       (optional)
//!   test/test.sh      (optional)
//! <implementation files>
//! ```

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::config::OppConfig;

/// A located OPPF project: the directory containing `.opp`, plus its config.
pub struct Project {
    /// The project root (the directory that contains `.opp`).
    pub root: PathBuf,
    /// The `.opp` directory itself.
    pub opp_dir: PathBuf,
    /// Parsed `config.toml`.
    pub config: OppConfig,
}

impl Project {
    /// Find the nearest project by walking up from `start` (or the current
    /// directory) until a `.opp` directory is found.
    pub fn discover(start: Option<&Path>) -> Result<Self> {
        let start = match start {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir().context("getting current directory")?,
        };
        let start = start
            .canonicalize()
            .with_context(|| format!("resolving {}", start.display()))?;

        let mut dir = start.as_path();
        loop {
            let candidate = dir.join(".opp");
            if candidate.is_dir() {
                let config = OppConfig::load(&candidate)?;
                return Ok(Self {
                    root: dir.to_path_buf(),
                    opp_dir: candidate,
                    config,
                });
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => {
                    return Err(anyhow!(
                        "no `.opp` directory found in {} or any parent directory",
                        start.display()
                    ))
                }
            }
        }
    }

    /// Resolve the design entry point: `.opp/design/index.md`, falling back to a
    /// single `.opp/design.md`.
    pub fn design_path(&self) -> Result<PathBuf> {
        let folder_index = self.opp_dir.join("design").join("index.md");
        if folder_index.is_file() {
            return Ok(folder_index);
        }
        let single = self.opp_dir.join("design.md");
        if single.is_file() {
            return Ok(single);
        }
        Err(anyhow!(
            "no design found: expected {} or {}",
            folder_index.display(),
            single.display()
        ))
    }

    /// The `.opp/design` directory, if the project uses the folder form.
    pub fn design_dir(&self) -> Option<PathBuf> {
        let dir = self.opp_dir.join("design");
        dir.is_dir().then_some(dir)
    }

    /// All review property documents (`.opp/review/*.md`), sorted by path.
    pub fn review_properties(&self) -> Result<Vec<PathBuf>> {
        let dir = self.opp_dir.join("review");
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut props: Vec<PathBuf> = std::fs::read_dir(&dir)
            .with_context(|| format!("reading {}", dir.display()))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
            .collect();
        props.sort();
        Ok(props)
    }

    /// The test entry point (`.opp/test/test.sh`), if present.
    pub fn test_script(&self) -> Option<PathBuf> {
        let script = self.opp_dir.join("test").join("test.sh");
        script.is_file().then_some(script)
    }
}
