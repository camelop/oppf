//! `opp clear` — remove everything the agent generated, reverting the project
//! to its pre-`impl` state.
//!
//! Deletes every top-level entry in the project root except the things that
//! define the project rather than result from it: the `.opp/` directory, any
//! `exclude` paths from the config, and `.git/` (a hard safety guard so version
//! control is never destroyed). Asks for confirmation unless `--yes` is given.
//!
//! With `--move <DIR>`, the would-be-deleted entries are moved into `<DIR>`
//! instead of being deleted.

use anyhow::{anyhow, Context, Result};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::context::Ctx;
use crate::project::Project;
use crate::ui;

pub fn run(ctx: &Ctx, yes: bool, move_to: Option<PathBuf>) -> Result<i32> {
    let mut targets = removable_entries(&ctx.project)?;

    // In move mode, resolve the destination and drop any target that is the
    // destination itself or sits on its path (so we never move a dir into
    // itself).
    let dest = match &move_to {
        Some(dir) => {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            let dest_abs = dir
                .canonicalize()
                .with_context(|| format!("resolving {}", dir.display()))?;
            targets.retain(|t| match t.canonicalize() {
                Ok(t_abs) => !(dest_abs.starts_with(&t_abs) || t_abs.starts_with(&dest_abs)),
                Err(_) => true,
            });
            Some(dest_abs)
        }
        None => None,
    };

    if targets.is_empty() {
        ui::info("clear: nothing to clear — only .opp/ and protected paths are present.");
        return Ok(0);
    }

    let root = &ctx.project.root;

    if ctx.dry_run {
        match &move_to {
            Some(dir) => ui::info(&format!(
                "clear (dry-run): would move {} item(s) to {}:",
                targets.len(),
                dir.display()
            )),
            None => ui::info(&format!(
                "clear (dry-run): would remove {} item(s) from {}:",
                targets.len(),
                root.display()
            )),
        }
        for p in &targets {
            ui::detail(&rel(root, p));
        }
        return Ok(0);
    }

    if !yes {
        match &move_to {
            Some(dir) => ui::info(&format!(
                "clear: about to move {} item(s) from {} to {}:",
                targets.len(),
                root.display(),
                dir.display()
            )),
            None => ui::info(&format!(
                "clear: about to delete {} item(s) from {}:",
                targets.len(),
                root.display()
            )),
        }
        for p in &targets {
            ui::detail(&rel(root, p));
        }
        ui::info("preserved: .opp/, .git/, and excluded paths.");
        let prompt = if move_to.is_some() {
            "move these out of the project? [y/N] "
        } else {
            "delete these and revert to the pre-generation state? [y/N] "
        };
        if !confirm(prompt)? {
            ui::info("clear: aborted; nothing was changed.");
            return Ok(0);
        }
    }

    let mut count = 0;
    for p in &targets {
        match &dest {
            Some(dest_dir) => {
                move_into(p, dest_dir).with_context(|| format!("moving {}", p.display()))?;
            }
            None => {
                remove_path(p).with_context(|| format!("removing {}", p.display()))?;
            }
        }
        count += 1;
    }

    match &move_to {
        Some(dir) => ui::good(&format!(
            "clear: moved {count} item(s) to {}",
            dir.display()
        )),
        None => ui::good(&format!(
            "clear: removed {count} item(s); the project is back to its .opp/ design."
        )),
    }
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

/// Move `src` into the directory `dest_dir` (keeping its name). Falls back to a
/// recursive copy + remove when a plain rename can't cross filesystems.
fn move_into(src: &Path, dest_dir: &Path) -> Result<()> {
    let name = src
        .file_name()
        .ok_or_else(|| anyhow!("cannot move {}", src.display()))?;
    let dest = dest_dir.join(name);
    if dest.exists() {
        return Err(anyhow!(
            "{} already exists; refusing to overwrite",
            dest.display()
        ));
    }
    if std::fs::rename(src, &dest).is_ok() {
        return Ok(());
    }
    copy_recursive(src, &dest)?;
    remove_path(src)?;
    Ok(())
}

/// Copy a file, directory tree, or symlink target from `src` to `dest`.
fn copy_recursive(src: &Path, dest: &Path) -> Result<()> {
    let meta = std::fs::symlink_metadata(src)?;
    if meta.is_dir() {
        std::fs::create_dir_all(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dest)?;
    }
    Ok(())
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

    #[test]
    fn move_into_relocates_file() {
        let base = std::env::temp_dir().join(format!("opp-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src_dir = base.join("proj");
        let dest_dir = base.join("backup");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&dest_dir).unwrap();
        let src = src_dir.join("gen.txt");
        std::fs::write(&src, "x").unwrap();

        move_into(&src, &dest_dir).unwrap();

        assert!(!src.exists());
        assert!(dest_dir.join("gen.txt").exists());
        std::fs::remove_dir_all(&base).ok();
    }
}
