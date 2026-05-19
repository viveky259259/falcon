//! CLI-side helpers (deprecation notices, command grouping, etc.).
//!
//! This module is intentionally small: it holds presentation-only helpers
//! that are shared between the top-level command dispatch and any
//! aliased / experimental command paths (`falcon x ...`).

pub mod deprecation;
