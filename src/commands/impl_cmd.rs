//! `opp impl` — read the design and implement what it requires.

use anyhow::Result;

use crate::agent;
use crate::context::Ctx;
use crate::prompts;
use crate::ui;

pub fn run(ctx: &Ctx) -> Result<i32> {
    let design = ctx.project.design_path()?;
    let prompt = prompts::impl_prompt(&ctx.project, &design)?;

    if ctx.dry_run {
        // Prefer the streaming command, since that is what a real run uses.
        let cmd = ctx
            .agent
            .build_streaming_command(&prompt)
            .unwrap_or_else(|| ctx.agent.build_command(&prompt));
        println!("# opp impl (dry-run) — agent: {}", ctx.agent.id());
        println!("$ {}", cmd.display());
        println!("\n--- prompt ---\n{prompt}");
        return Ok(0);
    }

    if let Some(code) = crate::commands::require_login(ctx)? {
        return Ok(code);
    }

    ui::info(&format!(
        "implementing {} with {}",
        ctx.project.root.display(),
        ctx.agent.id()
    ));
    let run = agent::run_with_progress(
        ctx.agent.as_ref(),
        &prompt,
        &ctx.project.root,
        ctx.verbose,
        true,
    )?;

    if run.success {
        print_follow_up(ctx, run.session_id.as_deref());
        Ok(0)
    } else {
        ui::error("the agent exited with a failure status");
        Ok(1)
    }
}

/// Tell the user how to keep iterating in the same agent session.
fn print_follow_up(ctx: &Ctx, session_id: Option<&str>) {
    let Some(id) = session_id else { return };
    let Some(follow_up) = ctx.agent.follow_up(id) else {
        return;
    };
    ui::blank();
    ui::info(&format!("continue this same {} session:", ctx.agent.id()));
    ui::command(
        &follow_up.interactive,
        "pick up where it left off, interactively",
    );
    ui::command(&follow_up.headless, "send one more instruction headlessly");
    ui::info("or re-run `opp review` / `opp test` to check the result.");
}
