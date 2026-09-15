//! `falcon doctor` — diagnose the project's toolchain and repair what we can.

pub mod exec;
pub mod host;
pub mod types;

pub use types::*;
