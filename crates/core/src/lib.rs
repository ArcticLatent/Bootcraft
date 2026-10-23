//! Bootcraft core: system detection, the task catalog and the planner.
//!
//! Nothing here needs root or Qt. The GUI uses this crate to show the scan and
//! build a selection; the privileged helper uses it to re-detect the system and
//! turn a selection of task IDs into concrete steps. Steps are plain data, so a
//! plan can be shown in full before anything runs.

pub mod catalog;
pub mod detect;
pub mod edit;
pub mod plan;
pub mod probe;
pub mod system;
pub mod task;

pub use plan::{Plan, PlanError, plan};
pub use system::System;
pub use task::{Category, Preset, Step, Task};
