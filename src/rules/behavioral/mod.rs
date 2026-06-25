//! Behavioral rule pack — catches runtime/correctness bugs common in
//! AI-generated Flutter/Dart code (async lifecycle gaps, swallowed errors,
//! state-after-dispose, leaked providers, etc.).
//!
//! Some rules in this pack require cross-file resolver context to be precise.
//! `dispose-not-called` uses the first resolver-backed class index slice;
//! `set-state-after-dispose` and `riverpod-scope-leak` remain registered
//! no-op stubs until richer flow and provider resolution lands.

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
