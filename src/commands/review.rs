//! `opp review` — have the coding agent check each review property and report a
//! pass/fail verdict per property.

use anyhow::Result;

use crate::agent;
use crate::context::Ctx;
use crate::prompts;
use crate::ui;

/// The outcome of reviewing a single property.
enum Verdict {
    Pass,
    Fail(String),
    /// The agent did not emit a parseable verdict line.
    Unknown,
}

pub fn run(ctx: &Ctx) -> Result<i32> {
    let properties = ctx.project.review_properties()?;
    if properties.is_empty() {
        ui::info("review: no properties found (.opp/review/*.md); nothing to check.");
        return Ok(0);
    }

    if !ctx.dry_run {
        if let Some(code) = crate::commands::require_login(ctx)? {
            return Ok(code);
        }
    }

    let mut results = Vec::with_capacity(properties.len());
    for property in &properties {
        let name = property
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| property.display().to_string());
        let prompt = prompts::review_prompt(&ctx.project, property)?;

        if ctx.dry_run {
            let cmd = ctx.agent.build_command(&prompt);
            println!(
                "# review property `{name}` (dry-run) — agent: {}",
                ctx.agent.id()
            );
            println!("$ {}\n", cmd.display());
            continue;
        }

        ui::info(&format!(
            "review: checking `{name}` with {} ...",
            ctx.agent.id()
        ));
        let run = agent::run_captured(ctx.agent.as_ref(), &prompt, &ctx.project.root, ctx.verbose)?;
        let verdict = parse_verdict(run.stdout.as_deref().unwrap_or_default());
        results.push((name, verdict));
    }

    if ctx.dry_run {
        return Ok(0);
    }

    ui::blank();
    ui::info(&format!("review summary — {} properties", results.len()));
    let mut failed = 0;
    for (name, verdict) in &results {
        match verdict {
            Verdict::Pass => ui::item_pass(name),
            Verdict::Fail(reason) => {
                failed += 1;
                ui::item_fail(name, reason);
            }
            Verdict::Unknown => {
                failed += 1;
                ui::item_unknown(name, "agent returned no verdict");
            }
        }
    }
    ui::blank();

    if failed == 0 {
        ui::good(&format!("all {} properties passed", results.len()));
        Ok(0)
    } else {
        ui::error(&format!(
            "{failed} of {} properties did not pass",
            results.len()
        ));
        Ok(1)
    }
}

/// Extract the verdict from the agent's reply: the last line beginning with
/// `OPP_REVIEW:`.
fn parse_verdict(text: &str) -> Verdict {
    for line in text.lines().rev() {
        let Some(rest) = line.trim().strip_prefix("OPP_REVIEW:") else {
            continue;
        };
        let rest = rest.trim();
        let upper = rest.to_uppercase();
        if upper.starts_with("PASS") {
            return Verdict::Pass;
        }
        if upper.starts_with("FAIL") {
            let reason = rest[4..]
                .trim_start_matches(|c: char| {
                    c.is_whitespace() || matches!(c, '—' | '-' | ':' | '–')
                })
                .trim();
            let reason = if reason.is_empty() {
                "no reason given".to_string()
            } else {
                reason.to_string()
            };
            return Verdict::Fail(reason);
        }
    }
    Verdict::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pass() {
        assert!(matches!(
            parse_verdict("blah\nOPP_REVIEW: PASS"),
            Verdict::Pass
        ));
    }

    #[test]
    fn parses_fail_with_reason() {
        match parse_verdict("OPP_REVIEW: FAIL — missing handler") {
            Verdict::Fail(r) => assert_eq!(r, "missing handler"),
            _ => panic!("expected fail"),
        }
    }

    #[test]
    fn takes_last_verdict() {
        assert!(matches!(
            parse_verdict("OPP_REVIEW: FAIL — x\nOPP_REVIEW: PASS"),
            Verdict::Pass
        ));
    }

    #[test]
    fn unknown_when_absent() {
        assert!(matches!(parse_verdict("no verdict here"), Verdict::Unknown));
    }
}
