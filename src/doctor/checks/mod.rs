//! Toolchain checks. Each one probes the host and, when it can, offers a fix.

pub mod dart;
pub mod flutter;

use crate::doctor::exec::CommandRunner;
use crate::doctor::flutter::releases::ReleaseManifest;
use crate::doctor::host::{Arch, HostInfo};
use crate::doctor::types::CheckResult;
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
}
