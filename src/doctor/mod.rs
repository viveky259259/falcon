//! `falcon doctor` — diagnose the project's toolchain and repair what we can.

pub mod checks;
pub mod exec;
pub mod flutter;
pub mod host;
pub mod types;

pub use types::*;
