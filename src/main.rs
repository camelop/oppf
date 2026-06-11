//! `opp` — reference CLI for the Open-Prompt Project Format (OPPF).
//!
//! OPPF describes a project entirely through its prompts (design), acceptance
//! criteria (review) and tests, so that a generative coding agent can reproduce
//! the implementation and the acceptance process is explicit. This binary drives
//! that lifecycle: `impl` implements the design, `review` checks each property,
//! and `test` runs the bundled test suite.

mod agent;
mod commands;
mod config;
mod context;
mod project;
mod prompts;
mod ui;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use context::Ctx;
use project::Project;

#[derive(Parser)]
#[command(
    name = "opp",
    version,
    about = "Reference CLI for the Open-Prompt Project Format (OPPF)",
    long_about = None,
)]
struct Cli {
    /// Path to the OPPF project. Defaults to discovering `.opp` from the current
    /// directory upwards.
    #[arg(long, short, global = true, value_name = "DIR")]
    path: Option<PathBuf>,

    /// Override the coding agent from config (e.g. `claude-code`, `codex`).
    #[arg(long, global = true, value_name = "AGENT")]
    agent: Option<String>,

    /// Print the agent commands and prompts that would run, without executing
    /// them.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Show the agent's raw streaming output in addition to the parsed progress.
    #[arg(long, short, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Read the design and implement what it requires.
    Impl,
    /// Have the coding agent check each review property.
    Review,
    /// Run the test suite and report results.
    Test,
    /// Remove all generated files, reverting to the pre-generation state.
    Clear {
        /// Delete without asking for confirmation.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Read the design and discuss implementation uncertainties before coding.
    Discuss {
        /// Lowest severity to include: `blocking`, `major`, or `all`.
        #[arg(long, value_enum, default_value_t = DiscussLevel::All)]
        level: DiscussLevel,
        /// Write the discussion to this file instead of the terminal.
        #[arg(long, short = 'o', value_name = "FILE")]
        output: Option<PathBuf>,
    },
}

/// Severity threshold for `opp discuss`.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum DiscussLevel {
    /// Only blocking issues: conflicts, must-resolve questions, clear flaws.
    Blocking,
    /// Blocking issues plus major, hard-to-change design decisions.
    Major,
    /// Everything, down to minor choices worth confirming first.
    All,
}

impl DiscussLevel {
    fn as_str(self) -> &'static str {
        match self {
            DiscussLevel::Blocking => "blocking",
            DiscussLevel::Major => "major",
            DiscussLevel::All => "all",
        }
    }
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            ui::error(&format!("{err:#}"));
            std::process::exit(2);
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    let project = Project::discover(cli.path.as_deref())?;
    let agent_id = cli
        .agent
        .clone()
        .unwrap_or_else(|| project.config.agent.clone());
    let agent = agent::for_id(&agent_id)?;
    let ctx = Ctx {
        project,
        agent,
        dry_run: cli.dry_run,
        verbose: cli.verbose,
    };

    match cli.command {
        Commands::Impl => commands::impl_cmd::run(&ctx),
        Commands::Review => commands::review::run(&ctx),
        Commands::Test => commands::test::run(&ctx),
        Commands::Clear { yes } => commands::clear::run(&ctx, yes),
        Commands::Discuss { level, output } => commands::discuss::run(&ctx, level.as_str(), output),
    }
}
