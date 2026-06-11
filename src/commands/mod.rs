//! The OPP-CLI commands and small helpers shared between them.

pub mod clear;
pub mod discuss;
pub mod impl_cmd;
pub mod review;
pub mod test;
pub mod upgrade;

use anyhow::Result;

use crate::agent::{self, AuthState};
use crate::context::Ctx;
use crate::ui;

/// Exit code returned when the agent is not authenticated.
pub const EXIT_NOT_LOGGED_IN: i32 = 3;

/// Verify the selected agent is logged in before driving it.
///
/// Returns `Ok(None)` to proceed, or `Ok(Some(code))` if the caller should exit
/// with `code` (the user was told how to log in).
pub fn require_login(ctx: &Ctx) -> Result<Option<i32>> {
    match agent::check_auth(ctx.agent.as_ref())? {
        AuthState::LoggedIn(Some(detail)) => {
            ui::good(&format!("{} — logged in ({detail})", ctx.agent.id()));
            Ok(None)
        }
        AuthState::LoggedIn(None) => {
            ui::good(&format!("{} — logged in", ctx.agent.id()));
            Ok(None)
        }
        AuthState::Unknown => Ok(None),
        AuthState::LoggedOut => {
            ui::error(&format!(
                "not logged in to {} — log in first:",
                ctx.agent.id()
            ));
            for line in ctx.agent.login_instructions() {
                ui::detail(&line);
            }
            Ok(Some(EXIT_NOT_LOGGED_IN))
        }
    }
}

/// Tell the user how to keep iterating in the same agent session, followed by a
/// command-specific `closing` suggestion. No-op when there is no session id or
/// the agent does not support resuming.
pub fn print_session_follow_up(ctx: &Ctx, session_id: Option<&str>, closing: &str) {
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
    if !closing.is_empty() {
        ui::info(closing);
    }
}
