//! Claude Code (`claude`) agent adapter.

use serde_json::Value;

use super::{truncate, Agent, AgentCommand, AgentEvent, AuthState, FollowUp};

pub struct ClaudeCode;

impl Agent for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn build_command(&self, prompt: &str) -> AgentCommand {
        // `--print` runs headlessly and prints the final response to stdout;
        // `--dangerously-skip-permissions` lets the agent edit files without
        // pausing for confirmation.
        AgentCommand {
            program: "claude".to_string(),
            args: vec![
                "--dangerously-skip-permissions".to_string(),
                "--print".to_string(),
                prompt.to_string(),
            ],
        }
    }

    fn build_streaming_command(&self, prompt: &str) -> Option<AgentCommand> {
        // `stream-json` emits one JSON object per line: an `init` event carrying
        // the session id, `assistant` events as the agent works, and a final
        // `result` event. `--verbose` is required to stream in `--print` mode.
        Some(AgentCommand {
            program: "claude".to_string(),
            args: vec![
                "--dangerously-skip-permissions".to_string(),
                "--verbose".to_string(),
                "--output-format".to_string(),
                "stream-json".to_string(),
                "--print".to_string(),
                prompt.to_string(),
            ],
        })
    }

    fn parse_events(&self, line: &str) -> Vec<AgentEvent> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };

        match value.get("type").and_then(Value::as_str) {
            Some("system") => {
                if value.get("subtype").and_then(Value::as_str) == Some("init") {
                    if let Some(id) = value.get("session_id").and_then(Value::as_str) {
                        return vec![AgentEvent::Session { id: id.to_string() }];
                    }
                }
                Vec::new()
            }
            Some("assistant") => assistant_events(&value),
            Some("result") => {
                let summary = value
                    .get("result")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                vec![AgentEvent::Done { summary }]
            }
            _ => Vec::new(),
        }
    }

    fn follow_up(&self, session_id: &str) -> Option<FollowUp> {
        Some(FollowUp {
            interactive: format!("claude --resume {session_id}"),
            headless: format!("claude --resume {session_id} -p \"<your next instruction>\""),
        })
    }

    fn auth_check_command(&self) -> Option<AgentCommand> {
        // `claude auth status` prints a JSON object with a `loggedIn` field.
        Some(AgentCommand {
            program: "claude".to_string(),
            args: vec!["auth".to_string(), "status".to_string()],
        })
    }

    fn parse_auth(&self, output: &str, _success: bool) -> AuthState {
        let Ok(value) = serde_json::from_str::<Value>(output.trim()) else {
            return AuthState::Unknown;
        };
        if value.get("loggedIn").and_then(Value::as_bool) != Some(true) {
            return AuthState::LoggedOut;
        }
        let method = value.get("authMethod").and_then(Value::as_str);
        let email = value.get("email").and_then(Value::as_str);
        let detail = match (method, email) {
            (Some(m), Some(e)) => Some(format!("{m}, {e}")),
            (Some(m), None) => Some(m.to_string()),
            (None, Some(e)) => Some(e.to_string()),
            (None, None) => None,
        };
        AuthState::LoggedIn(detail)
    }

    fn login_instructions(&self) -> Vec<String> {
        vec![
            "claude auth login                # sign in to your Anthropic account".to_string(),
            "(or export ANTHROPIC_API_KEY=…   to use an API key)".to_string(),
        ]
    }
}

/// Turn an `assistant` event's content blocks into progress events.
fn assistant_events(value: &Value) -> Vec<AgentEvent> {
    let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut events = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        events.push(AgentEvent::Message(text.to_string()));
                    }
                }
            }
            Some("tool_use") => {
                let tool = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool")
                    .to_string();
                let detail = block.get("input").and_then(summarize_tool_input);
                events.push(AgentEvent::Action { tool, detail });
            }
            _ => {}
        }
    }
    events
}

/// Pick a short, human-readable summary of a tool call's input.
fn summarize_tool_input(input: &Value) -> Option<String> {
    let obj = input.as_object()?;
    for key in [
        "file_path",
        "path",
        "command",
        "pattern",
        "url",
        "query",
        "description",
    ] {
        if let Some(v) = obj.get(key).and_then(Value::as_str) {
            if !v.trim().is_empty() {
                return Some(truncate(v, 120));
            }
        }
    }
    None
}
