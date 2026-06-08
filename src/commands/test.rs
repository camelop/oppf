//! `opp test` — run the bundled test script and report its result.
//!
//! The script (`.opp/test/test.sh`) runs with its own directory as the working
//! directory and with `OPP_PROJECT_ROOT` pointing at the project root, so it can
//! locate the implementation regardless of where `opp` was invoked from.

use anyhow::{Context, Result};
use std::process::Command;

use crate::context::Ctx;
use crate::ui;

pub fn run(ctx: &Ctx) -> Result<i32> {
    let Some(script) = ctx.project.test_script() else {
        ui::info("test: no script found (.opp/test/test.sh); nothing to run.");
        return Ok(0);
    };
    let test_dir = script
        .parent()
        .expect("test script always has a parent directory");

    if ctx.dry_run {
        println!("# opp test (dry-run)");
        println!(
            "$ OPP_PROJECT_ROOT={} bash {}   (cwd: {})",
            ctx.project.root.display(),
            script.display(),
            test_dir.display()
        );
        return Ok(0);
    }

    ui::info(&format!("test: running {}", script.display()));
    ui::rule_open("test.sh");
    let status = Command::new("bash")
        .arg(&script)
        .current_dir(test_dir)
        .env("OPP_PROJECT_ROOT", &ctx.project.root)
        .status()
        .with_context(|| format!("running {}", script.display()))?;

    let code = status.code().unwrap_or(1);
    if code == 0 {
        ui::rule_close("passed", true);
        Ok(0)
    } else {
        ui::rule_close(&format!("failed (exit {code})"), false);
        Ok(code)
    }
}
