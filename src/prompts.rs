//! Prompt rendering for the agent-driven commands.
//!
//! The prompt text itself lives in the `templates/` directory as minijinja
//! templates, not in this source file. The templates are embedded into the
//! binary at build time and rendered with the project's data. They are
//! deliberately agent-agnostic: the same text works for any agent wired into
//! [`crate::agent`].

use anyhow::{Context, Result};
use minijinja::{context, Environment};
use std::path::Path;
use std::sync::OnceLock;

use crate::project::Project;

const IMPL_TEMPLATE: &str = include_str!("../templates/impl.md.jinja");
const REVIEW_TEMPLATE: &str = include_str!("../templates/review.md.jinja");

/// The shared, lazily-built template environment.
fn environment() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        env.add_template("impl", IMPL_TEMPLATE)
            .expect("templates/impl.md.jinja is a valid template");
        env.add_template("review", REVIEW_TEMPLATE)
            .expect("templates/review.md.jinja is a valid template");
        env
    })
}

/// Prompt for `opp impl`: implement the design into the project root.
pub fn impl_prompt(project: &Project, design_path: &Path) -> Result<String> {
    environment()
        .get_template("impl")
        .expect("impl template is registered")
        .render(context! {
            project_root => project.root.display().to_string(),
            design_path => design_path.display().to_string(),
            design_dir => project.design_dir().map(|d| d.display().to_string()),
            exclude => &project.config.exclude,
        })
        .context("rendering the impl prompt")
}

/// Prompt for `opp review`: check a single acceptance property.
pub fn review_prompt(project: &Project, property_path: &Path) -> Result<String> {
    environment()
        .get_template("review")
        .expect("review template is registered")
        .render(context! {
            project_root => project.root.display().to_string(),
            property_path => property_path.display().to_string(),
            exclude => &project.config.exclude,
        })
        .context("rendering the review prompt")
}
