//! Terminal styling that keeps `opp`'s own output visually distinct from the
//! coding agent's output.
//!
//! Convention:
//! - opp's own messages carry a bold-cyan `opp` badge and go to stderr.
//! - the agent's live activity is framed in a dim gutter (`╭ │ ╰`).
//! - colors auto-disable when stderr is not a TTY or `NO_COLOR` is set.

use std::io::IsTerminal;
use std::sync::OnceLock;

fn colored() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal())
}

/// Wrap `s` in an SGR sequence when color is enabled.
fn sgr(codes: &str, s: &str) -> String {
    if colored() {
        format!("\x1b[{codes}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn dim(s: &str) -> String {
    sgr("2", s)
}
fn bold(s: &str) -> String {
    sgr("1", s)
}
fn badge() -> String {
    sgr("1;36", "opp")
}

// --- opp's own chrome -------------------------------------------------------

/// A neutral opp status line.
pub fn info(msg: &str) {
    eprintln!("{} {msg}", badge());
}

/// A successful opp status line.
pub fn good(msg: &str) {
    eprintln!("{} {} {msg}", badge(), sgr("32", "✓"));
}

/// An error from opp.
pub fn error(msg: &str) {
    eprintln!("{} {} {msg}", badge(), sgr("31", "✗"));
}

/// An indented continuation line under an opp message.
pub fn detail(msg: &str) {
    eprintln!("      {}", dim(msg));
}

/// An indented command suggestion with a dim explanatory note.
pub fn command(cmd: &str, note: &str) {
    eprintln!("      {}   {}", sgr("36", cmd), dim(&format!("# {note}")));
}

/// A blank spacer line.
pub fn blank() {
    eprintln!();
}

// --- list items (e.g. review verdicts) -------------------------------------

/// A passing list item.
pub fn item_pass(name: &str) {
    eprintln!("  {}  {name}", sgr("1;32", "✓"));
}

/// A failing list item with a dim reason.
pub fn item_fail(name: &str, reason: &str) {
    eprintln!("  {}  {name} {}", sgr("1;31", "✗"), dim(&format!("— {reason}")));
}

/// An inconclusive list item with a dim note.
pub fn item_unknown(name: &str, note: &str) {
    eprintln!("  {}  {name} {}", sgr("33", "?"), dim(&format!("— {note}")));
}

// --- the agent's framed activity region ------------------------------------

/// Open the agent frame with its identity and process id.
pub fn agent_open(agent_id: &str, pid: u32) {
    eprintln!("{} {} {}", dim("╭─"), bold(agent_id), dim(&format!("· pid {pid}")));
}

/// The agent's session id, inside the frame.
pub fn agent_session(id: &str) {
    eprintln!("{} {}", dim("│"), dim(&format!("session {id}")));
}

/// A tool the agent invoked, numbered.
pub fn agent_action(step: u32, tool: &str, detail_text: Option<&str>) {
    let num = sgr("36", &format!("{step:>2}"));
    let body = match detail_text {
        Some(d) => format!("{} {}", bold(tool), dim(d)),
        None => bold(tool),
    };
    eprintln!("{} {num} {body}", dim("│"));
}

/// An interstitial assistant message (dimmed).
pub fn agent_message(text: &str) {
    eprintln!("{} {} {}", dim("│"), dim("·"), dim(text));
}

/// A raw streaming line (only shown with `--verbose`).
pub fn agent_raw(text: &str) {
    eprintln!("{} {}", dim("│"), dim(text));
}

/// The agent's final answer, highlighted within the frame.
pub fn agent_final(text: &str) {
    eprintln!("{}", dim("│"));
    for line in text.lines() {
        eprintln!("{} {} {line}", dim("│"), sgr("36", "▌"));
    }
}

/// Close the agent frame after a successful run.
pub fn agent_close(secs: u64, steps: u32) {
    eprintln!("{} {}", dim("╰─"), dim(&format!("done in {secs}s · {steps} steps")));
}

/// Close the agent frame after a failed run.
pub fn agent_close_failed(secs: u64, steps: u32) {
    eprintln!(
        "{} {}",
        dim("╰─"),
        sgr("31", &format!("failed after {secs}s · {steps} steps"))
    );
}

// --- a generic framed region (e.g. test output) ----------------------------

/// Open a labelled frame.
pub fn rule_open(label: &str) {
    eprintln!("{} {}", dim("╭─"), bold(label));
}

/// Close a labelled frame with a colored verdict.
pub fn rule_close(msg: &str, ok: bool) {
    let painted = if ok { sgr("32", msg) } else { sgr("31", msg) };
    eprintln!("{} {painted}", dim("╰─"));
}
