//! `opp test` — run the bundled test script and report its result.
//!
//! The script (`.opp/test/test.sh`) runs from the **project root** (the
//! directory that contains `.opp/`), so it sees the implementation at the same
//! paths the design used — e.g. `bash hello.sh`. `OPP_PROJECT_ROOT` is also
//! exported (the absolute project root) for scripts that change directories.

use anyhow::{Context, Result};
use std::process::Command;

use crate::context::Ctx;
use crate::ui;

pub fn run(ctx: &Ctx) -> Result<i32> {
    let Some(script) = ctx.project.test_script() else {
        ui::info("test: no script found (.opp/test/test.sh); nothing to run.");
        return Ok(0);
    };
    // Invoke the script by its path relative to the project root, which is the
    // working directory.
    let rel = script
        .strip_prefix(&ctx.project.root)
        .unwrap_or(&script)
        .to_path_buf();

    if ctx.dry_run {
        println!("# opp test (dry-run)");
        println!(
            "$ bash {}   (cwd: {})",
            rel.display(),
            ctx.project.root.display()
        );
        return Ok(0);
    }

    ui::info(&format!("test: running {}", rel.display()));
    ui::rule_open("test.sh");
    let status = Command::new("bash")
        .arg(&rel)
        .current_dir(&ctx.project.root)
        .env("OPP_PROJECT_ROOT", &ctx.project.root)
        .status()
        .with_context(|| format!("running {}", rel.display()))?;

    let code = status.code().unwrap_or(1);
    if code == 0 {
        ui::rule_close("passed", true);
        Ok(0)
    } else {
        ui::rule_close(&format!("failed (exit {code})"), false);
        Ok(code)
    }
}
