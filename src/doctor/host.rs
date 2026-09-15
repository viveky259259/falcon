//! Host platform detection.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub home: Option<PathBuf>,
    pub shell_rc: Option<PathBuf>,
    pub package_managers: Vec<String>,
}
