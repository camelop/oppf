//! The coding-agent abstraction.
//!
//! An [`Agent`] knows how to turn a prompt into a concrete command line that
//! runs the agent non-interactively, with file-write access and without pausing
//! on permission prompts. Agents that emit structured streaming output can also
//! surface live progress (session id, per-step actions) via
//! [`Agent::build_streaming_command`] + [`Agent::parse_events`]. Concrete agents
//! live in submodules; adding one means implementing [`Agent`] and wiring it
//! into [`for_id`].

mod claude;
mod codex;

use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

/// A fully-resolved command line: a program plus its arguments.
pub struct AgentCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl AgentCommand {
    /// Render the command as a copy-pasteable shell line (used by `--dry-run`).
    pub fn display(&self) -> String {
        let mut out = shell_quote(&self.program);
        for arg in &self.args {
            out.push(' ');
            out.push_str(&shell_quote(arg));
        }
        out
    }
}

/// A normalized progress event parsed from an agent's streaming output.
pub enum AgentEvent {
    /// The agent reported its session/conversation id.
    Session { id: String },
    /// An assistant text message (a snippet of the agent's reasoning/output).
    Message(String),
    /// The agent invoked a tool (e.g. wrote a file, ran a command).
    Action {
        tool: String,
        detail: Option<String>,
    },
    /// The run finished; carries the agent's final summary text, if any.
    Done { summary: Option<String> },
}

/// How to continue an agent run as a follow-up to the same session.
pub struct FollowUp {
    /// Command that re-opens the session interactively.
    pub interactive: String,
    /// Command that sends one more instruction to the session headlessly.
    pub headless: String,
}

/// The result of an authentication preflight.
pub enum AuthState {
    /// Authenticated, with an optional human-readable description (account,
    /// method, …).
    LoggedIn(Option<String>),
    /// Not authenticated — the user must log in first.
    LoggedOut,
    /// The agent exposes no way to check; assume it is fine and proceed.
    Unknown,
}

/// A coding agent that can be driven from the command line.
pub trait Agent {
    /// Stable identifier, e.g. `"claude-code"`.
    fn id(&self) -> &'static str;

    /// Build the command that runs `prompt` non-interactively, with file-write
    /// access and no permission prompts. The agent's reply is expected on
    /// stdout.
    fn build_command(&self, prompt: &str) -> AgentCommand;

    /// A command whose stdout is newline-delimited JSON progress events, if the
    /// agent supports structured streaming. `None` means fall back to inheriting
    /// the agent's native stdio.
    fn build_streaming_command(&self, _prompt: &str) -> Option<AgentCommand> {
        None
    }

    /// Parse one line of streaming stdout into zero or more progress events.
    /// Only called when [`Agent::build_streaming_command`] returned `Some`.
    fn parse_events(&self, _line: &str) -> Vec<AgentEvent> {
        Vec::new()
    }

    /// How a user can continue `session_id` after a run, if the agent supports
    /// resuming sessions.
    fn follow_up(&self, _session_id: &str) -> Option<FollowUp> {
        None
    }

    /// Command that reports the agent's authentication status, if it has one.
    /// `None` disables the login preflight for this agent.
    fn auth_check_command(&self) -> Option<AgentCommand> {
        None
    }

    /// Interpret the output of [`Agent::auth_check_command`].
    fn parse_auth(&self, _output: &str, _success: bool) -> AuthState {
        AuthState::Unknown
    }

    /// Lines telling the user how to log in (shown when not authenticated).
    fn login_instructions(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Run the agent's authentication preflight.
pub fn check_auth(agent: &dyn Agent) -> Result<AuthState> {
    let Some(cmd) = agent.auth_check_command() else {
        return Ok(AuthState::Unknown);
    };
    let output = Command::new(&cmd.program)
        .args(&cmd.args)
        .output()
        .map_err(|e| spawn_error(&cmd.program, e))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    Ok(agent.parse_auth(&text, output.status.success()))
}

/// Resolve an agent by its config identifier.
pub fn for_id(id: &str) -> Result<Box<dyn Agent>> {
    match id {
        "claude-code" | "claude" => Ok(Box::new(claude::ClaudeCode)),
        "codex" | "codex-cli" => Ok(Box::new(codex::Codex)),
        other => Err(anyhow!(
            "unknown agent `{other}` (supported: claude-code, codex)"
        )),
    }
}

/// The result of running an agent.
pub struct AgentRun {
    pub success: bool,
    /// The agent's final text output, when available.
    pub stdout: Option<String>,
    /// The session/conversation id, when the agent reported one.
    pub session_id: Option<String>,
}

/// Run `agent` against `prompt` in `cwd`, capturing its output. Used where the
/// caller needs to parse the agent's reply rather than show it live. The agent's
/// own chatter is kept quiet unless the run fails or `verbose` is set, in which
/// case it is shown framed.
pub fn run_captured(
    agent: &dyn Agent,
    prompt: &str,
    cwd: &Path,
    verbose: bool,
) -> Result<AgentRun> {
    let cmd = agent.build_command(prompt);
    let output = Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(cwd)
        .output()
        .map_err(|e| spawn_error(&cmd.program, e))?;

    let success = output.status.success();
    if verbose || !success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            crate::ui::agent_raw(line);
        }
    }

    Ok(AgentRun {
        success,
        stdout: Some(String::from_utf8_lossy(&output.stdout).into_owned()),
        session_id: None,
    })
}

/// Run `agent` against `prompt` in `cwd`, surfacing live progress in a framed
/// region that is visually distinct from opp's own output.
///
/// If the agent exposes structured streaming, this shows the process id, the
/// session id, and a numbered feed of the agent's actions as they happen.
/// Otherwise it falls back to inheriting the agent's native output, still
/// framing the region and reporting the process id.
pub fn run_with_progress(
    agent: &dyn Agent,
    prompt: &str,
    cwd: &Path,
    verbose: bool,
) -> Result<AgentRun> {
    match agent.build_streaming_command(prompt) {
        Some(cmd) => run_json_stream(agent, &cmd, cwd, verbose),
        None => run_inherited(agent, &agent.build_command(prompt), cwd),
    }
}

fn run_json_stream(
    agent: &dyn Agent,
    cmd: &AgentCommand,
    cwd: &Path,
    verbose: bool,
) -> Result<AgentRun> {
    let mut child = Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| spawn_error(&cmd.program, e))?;

    crate::ui::agent_open(agent.id(), child.id());

    let stdout = child
        .stdout
        .take()
        .expect("child stdout was requested via Stdio::piped");
    let reader = BufReader::new(stdout);

    let started = Instant::now();
    let mut step = 0u32;
    let mut summary = None;
    let mut session_id = None;
    // The newest assistant message is held back and only shown once a later
    // event arrives, so the final message (which agents also repeat as the
    // result summary) is superseded by the highlighted summary block instead of
    // being printed twice.
    let mut pending_message: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let events = agent.parse_events(&line);
        if events.is_empty() && verbose {
            crate::ui::agent_raw(&line);
            continue;
        }
        for event in events {
            match event {
                AgentEvent::Session { id } => {
                    flush_pending(&mut pending_message);
                    crate::ui::agent_session(&id);
                    session_id = Some(id);
                }
                AgentEvent::Action { tool, detail } => {
                    flush_pending(&mut pending_message);
                    step += 1;
                    crate::ui::agent_action(step, &tool, detail.as_deref());
                }
                AgentEvent::Message(text) => {
                    flush_pending(&mut pending_message);
                    let snippet = truncate(&text, 200);
                    if !snippet.is_empty() {
                        pending_message = Some(snippet);
                    }
                }
                AgentEvent::Done { summary: s } => summary = s,
            }
        }
    }

    let status = child.wait()?;
    let secs = started.elapsed().as_secs();

    match summary.as_deref().map(str::trim) {
        Some(text) if !text.is_empty() => crate::ui::agent_final(text),
        // No final summary — surface the last held message instead.
        _ => flush_pending(&mut pending_message),
    }
    if status.success() {
        crate::ui::agent_close(secs, step);
    } else {
        crate::ui::agent_close_failed(secs, step);
    }

    Ok(AgentRun {
        success: status.success(),
        stdout: summary,
        session_id,
    })
}

/// Print and clear a held-back assistant message, if any.
fn flush_pending(pending: &mut Option<String>) {
    if let Some(message) = pending.take() {
        crate::ui::agent_message(&message);
    }
}

fn run_inherited(agent: &dyn Agent, cmd: &AgentCommand, cwd: &Path) -> Result<AgentRun> {
    let mut child = Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(cwd)
        .spawn()
        .map_err(|e| spawn_error(&cmd.program, e))?;

    crate::ui::agent_open(agent.id(), child.id());

    let started = Instant::now();
    let status = child.wait()?;
    let secs = started.elapsed().as_secs();
    if status.success() {
        crate::ui::agent_close(secs, 0);
    } else {
        crate::ui::agent_close_failed(secs, 0);
    }

    Ok(AgentRun {
        success: status.success(),
        stdout: None,
        session_id: None,
    })
}

fn spawn_error(program: &str, err: std::io::Error) -> anyhow::Error {
    if err.kind() == std::io::ErrorKind::NotFound {
        anyhow!(
            "could not run `{program}`: command not found. Is the agent CLI installed and on PATH?"
        )
    } else {
        anyhow!("failed to run `{program}`: {err}")
    }
}

/// Collapse whitespace and clip a string to `max` characters for display.
pub(crate) fn truncate(s: &str, max: usize) -> String {
    let collapsed: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        collapsed
    } else {
        let mut clipped: String = collapsed.chars().take(max).collect();
        clipped.push('…');
        clipped
    }
}

/// Minimal POSIX shell quoting for display purposes.
fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '=' | ':'))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}
