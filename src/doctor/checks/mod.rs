//! Toolchain checks. Each one probes the host and, when it can, offers a fix.

pub mod cocoapods;
pub mod dart;
pub mod flutter;

use crate::doctor::exec::CommandRunner;
use crate::doctor::flutter::releases::ReleaseManifest;
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::{CheckResult, Plan};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct CheckContext {
    pub root: PathBuf,
    pub host: HostInfo,
    pub arch: Arch,
    /// `None` when the release manifest could not be fetched — fixes then
    /// degrade to `Manual` rather than guessing.
    pub manifest: Option<ReleaseManifest>,
    pub runner: Box<dyn CommandRunner>,
}

pub trait Check {
    fn id(&self) -> &'static str;
    fn probe(&self, ctx: &CheckContext) -> CheckResult;

    /// Turn answered questions into an executable plan. Checks whose fix is
    /// deferred to another check (like `dart`) keep the default empty plan.
    fn plan(
        &self,
        _ctx: &CheckContext,
        _decisions: &HashMap<String, String>,
    ) -> Result<Plan, String> {
        Ok(Plan {
            check_id: self.id().to_string(),
            steps: vec![],
        })
    }
}
