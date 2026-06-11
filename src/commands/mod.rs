//! The three OPP-CLI commands: `impl`, `review`, `test`.

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
