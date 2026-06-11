//! `opp upgrade` — self-update by running the published install script.
//!
//! Fetches `https://oppf.dirp.dev/install.sh` and pipes it into `sh`, installing
//! the latest release into the directory of the currently-running binary (so it
//! upgrades this `opp` in place). The installer is idempotent, so this is a
//! no-op when already up to date. Needs no `.opp/` project and no agent login.

use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};

use crate::ui;

const INSTALL_URL: &str = "https://oppf.dirp.dev/install.sh";

pub fn run(dry_run: bool) -> Result<i32> {
    // Upgrade in place: install into the running executable's directory.
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if dry_run {
        println!("# opp upgrade (dry-run)");
        match &dir {
            Some(d) => println!(
                "$ curl -fsSL {INSTALL_URL} | sh -s -- --dir {}",
                d.display()
            ),
            None => println!("$ curl -fsSL {INSTALL_URL} | sh"),
        }
        return Ok(0);
    }

    ui::info("upgrade: fetching and running the installer ...");

    let (mut fetcher, prog) = spawn_fetcher()?;
    let pipe = fetcher.stdout.take().expect("fetcher stdout was piped");

    let mut sh = Command::new("sh");
    sh.arg("-s").stdin(Stdio::from(pipe));
    if let Some(d) = &dir {
        sh.arg("--").arg("--dir").arg(d);
    }
    let sh_status = sh.status().map_err(|e| anyhow!("failed to run sh: {e}"))?;
    let fetch_ok = fetcher.wait()?.success();

    if !fetch_ok {
        return Err(anyhow!(
            "failed to download the installer from {INSTALL_URL} (via {prog})"
        ));
    }
    if sh_status.success() {
        Ok(0)
    } else {
        ui::error("upgrade: the installer reported a failure");
        Ok(1)
    }
}

/// Start `curl` (or `wget`) streaming the install script to stdout.
fn spawn_fetcher() -> Result<(std::process::Child, &'static str)> {
    let candidates: [(&str, [&str; 2]); 2] = [
        ("curl", ["-fsSL", INSTALL_URL]),
        ("wget", ["-qO-", INSTALL_URL]),
    ];
    for (prog, args) in candidates {
        match Command::new(prog).args(args).stdout(Stdio::piped()).spawn() {
            Ok(child) => return Ok((child, prog)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(anyhow!("failed to run {prog}: {e}")),
        }
    }
    Err(anyhow!("`opp upgrade` needs curl or wget on your PATH"))
}
