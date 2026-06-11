//! Codex CLI (`codex`) agent adapter.

use serde_json::Value;

use super::{truncate, Agent, AgentCommand, AgentEvent, AuthState, FollowUp};

pub struct Codex;

impl Agent for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn build_command(&self, prompt: &str) -> AgentCommand {
        // `codex exec` is the non-interactive entry point; the bypass flag lets
        // it edit files without approval prompts or sandboxing.
        AgentCommand {
            program: "codex".to_string(),
            args: vec![
                "exec".to_string(),
                "--dangerously-bypass-approvals-and-sandbox".to_string(),
                prompt.to_string(),
            ],
        }
    }

    fn build_streaming_command(&self, prompt: &str) -> Option<AgentCommand> {
        // `--json` prints thread/turn/item events as JSONL. The `thread.started`
        // event carries the session (thread) id.
        Some(AgentCommand {
            program: "codex".to_string(),
            args: vec![
                "exec".to_string(),
                "--json".to_string(),
                "--dangerously-bypass-approvals-and-sandbox".to_string(),
                prompt.to_string(),
            ],
        })
    }

    fn parse_events(&self, line: &str) -> Vec<AgentEvent> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };

        match value.get("type").and_then(Value::as_str) {
            Some("thread.started") => value
                .get("thread_id")
                .and_then(Value::as_str)
                .map(|id| vec![AgentEvent::Session { id: id.to_string() }])
                .unwrap_or_default(),
            // Items (command runs, file edits, messages) are reported once on
            // completion; ignore the `started`/`updated` phases to avoid noise.
            Some("item.completed") => value.get("item").map(item_events).unwrap_or_default(),
            Some("error") => value
                .get("message")
                .and_then(Value::as_str)
                .map(|m| vec![AgentEvent::Message(format!("error: {m}"))])
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn follow_up(&self, session_id: &str) -> Option<FollowUp> {
        Some(FollowUp {
            interactive: format!("codex resume {session_id}"),
            headless: format!("codex exec resume {session_id} \"<your next instruction>\""),
        })
    }

    fn auth_check_command(&self) -> Option<AgentCommand> {
        // `codex login status` prints "Not logged in" or "Logged in using …".
        // It exits 0 either way, so the verdict comes from the text.
        Some(AgentCommand {
            program: "codex".to_string(),
            args: vec!["login".to_string(), "status".to_string()],
        })
    }

    fn parse_auth(&self, output: &str, _success: bool) -> AuthState {
        if output.contains("Not logged in") {
            return AuthState::LoggedOut;
        }
        if let Some(line) = output.lines().find(|l| l.contains("Logged in")) {
            return AuthState::LoggedIn(Some(line.trim().to_string()));
        }
        AuthState::Unknown
    }

    fn login_instructions(&self) -> Vec<String> {
        vec![
            "codex login                      # sign in with your ChatGPT account".to_string(),
            "codex login --with-api-key       # or pipe an API key via stdin".to_string(),
        ]
    }
}

/// Turn a completed `item` object into a progress event.
fn item_events(item: &Value) -> Vec<AgentEvent> {
    match item.get("type").and_then(Value::as_str) {
        Some("agent_message") => item
            .get("text")
            .and_then(Value::as_str)
            // The final assistant message is the run's summary.
            .map(|text| {
                vec![AgentEvent::Done {
                    summary: Some(text.to_string()),
                }]
            })
            .unwrap_or_default(),
        Some("command_execution") => {
            let detail = item
                .get("command")
                .and_then(Value::as_str)
                .map(|c| truncate(c, 120));
            vec![AgentEvent::Action {
                tool: "Run".to_string(),
                detail,
            }]
        }
        Some("file_change") => {
            let detail = item.get("changes").and_then(summarize_changes);
            vec![AgentEvent::Action {
                tool: "Edit".to_string(),
                detail,
            }]
        }
        Some("mcp_tool_call") => {
            let server = item.get("server").and_then(Value::as_str).unwrap_or("mcp");
            let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            vec![AgentEvent::Action {
                tool: format!("{server}.{tool}"),
                detail: None,
            }]
        }
        Some("web_search") => {
            let detail = item
                .get("query")
                .and_then(Value::as_str)
                .map(|q| truncate(q, 120));
            vec![AgentEvent::Action {
                tool: "Search".to_string(),
                detail,
            }]
        }
        _ => Vec::new(),
    }
}

/// Summarize a `file_change` item's list of changed paths.
fn summarize_changes(changes: &Value) -> Option<String> {
    let array = changes.as_array()?;
    let paths: Vec<&str> = array
        .iter()
        .filter_map(|c| c.get("path").and_then(Value::as_str))
        .collect();
    if paths.is_empty() {
        None
    } else {
        Some(truncate(&paths.join(", "), 120))
    }
}
