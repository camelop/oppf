//! Shared execution context passed to every command.

use crate::agent::Agent;
use crate::project::Project;

/// Everything a command needs: the located project, the selected agent, and the
/// global flags.
pub struct Ctx {
    pub project: Project,
    pub agent: Box<dyn Agent>,
    pub dry_run: bool,
    pub verbose: bool,
}
