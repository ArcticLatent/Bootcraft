//! Turn a selection of task IDs into an ordered plan.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde::Serialize;
use thiserror::Error;

use crate::system::{Distro, System};
use crate::task::{Preset, Step, Task};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("{0} is not a supported distribution")]
    UnsupportedSystem(String),
    #[error("unknown task `{0}`")]
    UnknownTask(String),
    #[error("task `{task}` requires `{missing}`, which is not available on this system")]
    MissingRequirement { task: &'static str, missing: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub distro: Distro,
    pub tasks: Vec<PlannedTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedTask {
    pub id: &'static str,
    pub title: String,
    pub steps: Vec<Step>,
    pub reboot: bool,
    /// Set when the task was not selected but another task requires it.
    pub required_by: Option<&'static str>,
}

/// IDs of the tasks that start ticked.
pub fn recommended(catalog: &[Task]) -> Vec<&'static str> {
    catalog.iter().filter(|t| t.preset == Preset::Recommended).map(|t| t.id).collect()
}

/// Resolve `selected` against `catalog`, pull in requirements and keep catalog
/// order, which is the execution order.
pub fn plan<S: AsRef<str>>(sys: &System, catalog: &[Task], selected: &[S]) -> Result<Plan, PlanError> {
    let distro = sys.distro().ok_or_else(|| PlanError::UnsupportedSystem(sys.os.pretty_name.clone()))?;
    let find = |id: &str| catalog.iter().find(|t| t.id == id);

    let mut chosen: Vec<(&Task, Option<&'static str>)> = Vec::new();
    let mut seen = HashSet::new();
    for id in selected {
        let task = find(id.as_ref()).ok_or_else(|| PlanError::UnknownTask(id.as_ref().to_owned()))?;
        if seen.insert(task.id) {
            chosen.push((task, None));
        }
    }
    let mut i = 0;
    while i < chosen.len() {
        let task = chosen[i].0;
        for &req in &task.requires {
            let dep = find(req).ok_or(PlanError::MissingRequirement { task: task.id, missing: req })?;
            if seen.insert(dep.id) {
                chosen.push((dep, Some(task.id)));
            }
        }
        i += 1;
    }

    let position = |id| catalog.iter().position(|t| t.id == id);
    chosen.sort_by_key(|(task, _)| position(task.id));
    Ok(Plan {
        distro,
        tasks: chosen
            .into_iter()
            .map(|(task, required_by)| PlannedTask {
                id: task.id,
                title: task.title.clone(),
                steps: task.steps.clone(),
                reboot: task.reboot,
                required_by,
            })
            .collect(),
    })
}

impl Plan {
    pub fn reboot_required(&self) -> bool {
        self.tasks.iter().any(|t| t.reboot)
    }

    /// Plain-text dry run, as shown by `bootcraft-helper plan`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let total = self.tasks.len();
        let _ = writeln!(out, "Bootcraft plan for {} ({total} tasks)", self.distro.name());
        for (n, task) in self.tasks.iter().enumerate() {
            let _ = write!(out, "\n[{}/{total}] {}  ({})", n + 1, task.title, task.id);
            if let Some(by) = task.required_by {
                let _ = write!(out, "  [required by {by}]");
            }
            out.push('\n');
            for step in &task.steps {
                let _ = writeln!(out, "    $ {}", step.describe(self.distro));
            }
        }
        if self.reboot_required() {
            out.push_str("\nA reboot is required afterwards.\n");
        }
        out
    }
}
