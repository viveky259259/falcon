//! Behavioral rule pack — catches runtime/correctness bugs common in
//! AI-generated Flutter/Dart code (async lifecycle gaps, swallowed errors,
//! state-after-dispose, leaked providers, etc.).
//!
//! Some rules in this pack require the upcoming cross-file resolver
//! (EPIC 3.1) to be precise. Those rules are present, registered, and named
//! so they are plumbed end-to-end, but ship as no-op stubs that return
//! `vec![]` until the resolver lands.

pub mod dispose_not_called;
pub mod fake_mounted_check;
pub mod riverpod_scope_leak;
pub mod set_state_after_dispose;
pub mod silent_catch;
pub mod unawaited_future_in_build;

pub use dispose_not_called::DisposeNotCalled;
pub use fake_mounted_check::FakeMountedCheck;
pub use riverpod_scope_leak::RiverpodScopeLeak;
pub use set_state_after_dispose::SetStateAfterDispose;
pub use silent_catch::SilentCatch;
pub use unawaited_future_in_build::UnawaitedFutureInBuild;
