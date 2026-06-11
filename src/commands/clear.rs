//! `opp clear` — remove everything the agent generated, reverting the project
//! to its pre-`impl` state.
//!
//! Deletes every top-level entry in the project root except the things that
//! define the project rather than result from it: the `.opp/` directory, any
//! `exclude` paths from the config, and `.git/` (a hard safety guard so version
//! control is never destroyed). Asks for confirmation unless `--yes` is given.

use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::context::Ctx;
use crate::project::Project;
use crate::ui;

pub fn run(ctx: &Ctx, yes: bool) -> Result<i32> {
    let targets = removable_entries(&ctx.project)?;

    if targets.is_empty() {
        ui::info("clear: nothing to remove — only .opp/ and protected paths are present.");
        return Ok(0);
    }

    if ctx.dry_run {
        ui::info(&format!(
            "clear (dry-run): would remove {} item(s) from {}:",
            targets.len(),
            ctx.project.root.display()
        ));
        for p in &targets {
            ui::detail(&rel(&ctx.project.root, p));
        }
        return Ok(0);
    }

    if !yes {
        ui::info(&format!(
            "clear: about to delete {} item(s) from {}:",
            targets.len(),
            ctx.project.root.display()
        ));
        for p in &targets {
            ui::detail(&rel(&ctx.project.root, p));
        }
        ui::info("preserved: .opp/, .git/, and excluded paths.");
        if !confirm("delete these and revert to the pre-generation state? [y/N] ")? {
            ui::info("clear: aborted; nothing was deleted.");
            return Ok(0);
        }
    }

    let mut removed = 0;
    for p in &targets {
        remove_path(p).with_context(|| format!("removing {}", p.display()))?;
        removed += 1;
    }
    ui::good(&format!(
        "clear: removed {removed} item(s); the project is back to its .opp/ design."
    ));
    Ok(0)
}

/// Top-level entries in the project root that `clear` may delete.
fn removable_entries(project: &Project) -> Result<Vec<PathBuf>> {
    let protected = protected_names(project);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&project.root)
        .with_context(|| format!("reading {}", project.root.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        if protected.contains(name.to_string_lossy().as_ref()) {
            continue;
        }
        out.push(entry.path());
    }
    out.sort();
    Ok(out)
}

/// Names of top-level entries that must never be deleted.
fn protected_names(project: &Project) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    set.insert(".opp".to_string());
    set.insert(".git".to_string());
    for exclude in &project.config.exclude {
        if let Some(top) = top_component(exclude) {
            set.insert(top);
        }
    }
    set
}

/// The first real path component of an `exclude` entry (relative to root), e.g.
/// `./vendor/cache` -> `vendor`. Returns `None` for paths that escape the root.
fn top_component(p: &str) -> Option<String> {
    let trimmed = p.trim().trim_start_matches("./");
    let first = trimmed.split('/').find(|s| !s.is_empty() && *s != ".")?;
    if first == ".." {
        return None;
    }
    Some(first.to_string())
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

fn remove_path(p: &Path) -> std::io::Result<()> {
    if p.is_dir() && !p.is_symlink() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}

/// Ask the user to confirm on the terminal. A non-`y` answer (including a closed
/// or piped stdin) is treated as "no".
fn confirm(prompt: &str) -> Result<bool> {
    eprint!("opp {prompt}");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line)? == 0 {
        return Ok(false);
    }
    let answer = line.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_component_normalizes() {
        assert_eq!(top_component("./vendor"), Some("vendor".to_string()));
        assert_eq!(top_component("vendor/cache"), Some("vendor".to_string()));
        assert_eq!(top_component("  ./a/b/c "), Some("a".to_string()));
        assert_eq!(top_component("../outside"), None);
        assert_eq!(top_component(""), None);
    }

    #[test]
    fn removable_skips_protected_and_excluded() {
        use crate::config::OppConfig;

        let root = std::env::temp_dir().join(format!("opp-clear-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".opp")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("vendor")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("main.rs"), "").unwrap();

        let project = Project {
            root: root.clone(),
            opp_dir: root.join(".opp"),
            config: OppConfig {
                agent: "claude-code".to_string(),
                exclude: vec!["./vendor".to_string()],
            },
        };

        let mut names: Vec<String> = removable_entries(&project)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();

        std::fs::remove_dir_all(&root).ok();
        assert_eq!(names, vec!["main.rs".to_string(), "src".to_string()]);
    }
}
