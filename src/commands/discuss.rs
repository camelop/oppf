//! `opp discuss` — have the agent read the design and raise implementation
//! uncertainties (conflicts, blocking questions, unspecified design decisions)
//! before any code is written, without touching the project.

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::agent;
use crate::context::Ctx;
use crate::prompts;
use crate::ui;

/// `level` is `"blocking"`, `"major"`, or `"all"`. `output`, when set, writes the
/// discussion to that file instead of the terminal. `focus`, when set, is extra
/// guidance from the user that is added to the prompt.
pub fn run(ctx: &Ctx, level: &str, output: Option<PathBuf>, focus: Option<&str>) -> Result<i32> {
    let design = ctx.project.design_path()?;
    let prompt = prompts::discuss_prompt(&ctx.project, &design, level, focus)?;

    if ctx.dry_run {
        let cmd = ctx
            .agent
            .build_streaming_command(&prompt)
            .unwrap_or_else(|| ctx.agent.build_command(&prompt));
        println!(
            "# opp discuss (dry-run) — agent: {}, level: {level}",
            ctx.agent.id()
        );
        println!("$ {}", cmd.display());
        println!("\n--- prompt ---\n{prompt}");
        return Ok(0);
    }

    if let Some(code) = crate::commands::require_login(ctx)? {
        return Ok(code);
    }

    ui::info(&format!(
        "discuss: reviewing the design with {} (level: {level}) ...",
        ctx.agent.id()
    ));
    // Stream progress, but capture the discussion instead of rendering the final
    // block in the frame — we present it ourselves (to the terminal or a file).
    let run = agent::run_with_progress(
        ctx.agent.as_ref(),
        &prompt,
        &ctx.project.root,
        ctx.verbose,
        false,
    )?;
    let discussion = run.stdout.unwrap_or_default();
    let discussion = discussion.trim();

    if discussion.is_empty() {
        ui::error("discuss: the agent returned no discussion.");
        return Ok(1);
    }

    match output {
        Some(path) => {
            let mut body = discussion.to_string();
            body.push('\n');
            std::fs::write(&path, &body).with_context(|| format!("writing {}", path.display()))?;
            ui::good(&format!(
                "discuss: wrote the discussion to {}",
                path.display()
            ));
        }
        None => {
            ui::blank();
            println!("{discussion}");
        }
    }

    if run.success {
        crate::commands::print_session_follow_up(
            ctx,
            run.session_id.as_deref(),
            "or run `opp impl` once the open questions are settled.",
        );
        Ok(0)
    } else {
        Ok(1)
    }
}
