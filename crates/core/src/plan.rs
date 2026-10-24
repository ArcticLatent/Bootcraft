//! Turn a selection of tasks and apps into an ordered plan.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::apps::{Preference, Source, Via};
use crate::catalog::{Catalog, app_requires, pack};
use crate::system::{Distro, System};
use crate::task::{Category, Preset, Step, Task};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("{0} is not a supported distribution")]
    UnsupportedSystem(String),
    #[error("unknown task `{0}`")]
    UnknownTask(String),
    #[error("unknown app `{0}`")]
    UnknownApp(String),
    #[error("unknown workflow `{0}`")]
    UnknownPack(String),
    #[error("{app} can't be installed from {from} on this system")]
    Unavailable { app: String, from: &'static str },
    #[error("`{task}` requires `{missing}`, which is not available on this system")]
    MissingRequirement { task: String, missing: String },
}

/// What the user picked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// System task IDs.
    pub tasks: Vec<String>,
    pub apps: Vec<AppPick>,
    #[serde(default)]
    pub preference: Preference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPick {
    pub id: String,
    /// A per-app source chosen by the user; otherwise the preference decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

impl Selection {
    /// Essentials (the recommended tasks) plus the ticked apps of `packs`.
    /// Apps without any source on this system are left out.
    pub fn from_packs(catalog: &Catalog, packs: &[impl AsRef<str>]) -> Result<Self, PlanError> {
        let mut selection = Self { tasks: recommended(&catalog.tasks), ..Self::default() };
        for id in packs {
            let pack = pack(id.as_ref()).ok_or_else(|| PlanError::UnknownPack(id.as_ref().to_owned()))?;
            for app in pack.ticked() {
                let available = catalog.app(app).is_some_and(crate::apps::App::available);
                if available && !selection.apps.iter().any(|p| p.id == app) {
                    selection.apps.push(AppPick { id: app.to_owned(), source: None });
                }
            }
        }
        Ok(selection)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub distro: Distro,
    pub tasks: Vec<PlannedTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedTask {
    /// A task ID, or `app.<id>` for an app.
    pub id: String,
    pub title: String,
    pub steps: Vec<Step>,
    pub reboot: bool,
    pub requires: Vec<String>,
    /// Set when the task was not selected but another one requires it.
    pub required_by: Option<String>,
}

/// IDs of the tasks that start ticked: the Essentials.
pub fn recommended(tasks: &[Task]) -> Vec<String> {
    tasks.iter().filter(|t| t.preset == Preset::Recommended).map(|t| t.id.to_owned()).collect()
}

/// Resolve `selection` against the catalog and pull in requirements.
/// Order: system tasks in catalog order, then apps, then debloat tasks, so
/// nothing is removed before new apps are in place.
pub fn plan(sys: &System, catalog: &Catalog, selection: &Selection) -> Result<Plan, PlanError> {
    let distro = sys.distro().ok_or_else(|| PlanError::UnsupportedSystem(sys.os.pretty_name.clone()))?;

    let mut apps: Vec<PlannedTask> = Vec::new();
    for pick in &selection.apps {
        let app = catalog.app(&pick.id).ok_or_else(|| PlanError::UnknownApp(pick.id.clone()))?;
        if let Some(source) = pick.source.filter(|s| !app.has(*s)) {
            let from = match source {
                Source::Flatpak => "Flathub",
                Source::Native => "the system's packages",
            };
            return Err(PlanError::Unavailable { app: app.name.to_owned(), from });
        }
        let choice = app
            .choose(sys, selection.preference, pick.source)
            .ok_or(PlanError::Unavailable { app: app.name.to_owned(), from: "any source" })?;
        let id = format!("app.{}", app.id);
        if apps.iter().any(|a| a.id == id) {
            continue;
        }
        let label = match (choice.source, app.native.as_ref().map(|n| &n.via)) {
            (Source::Flatpak, _) => "Flatpak",
            (Source::Native, Some(Via::Aur)) => "AUR",
            (Source::Native, Some(Via::SignedRelease { .. })) => "signed release",
            (Source::Native, _) => "system package",
        };
        apps.push(PlannedTask {
            id,
            title: format!("{} ({label})", app.name),
            steps: app.steps(choice.source).unwrap_or_default(),
            reboot: false,
            requires: app_requires(distro, app, choice.source).into_iter().map(str::to_owned).collect(),
            required_by: None,
        });
    }

    // Apps follow catalog order, which groups them by section.
    let app_position = |id: &str| catalog.apps.iter().position(|a| id.strip_prefix("app.") == Some(a.id));
    apps.sort_by_key(|a| app_position(&a.id));

    // System tasks: the selection plus everything required, transitively.
    let mut chosen: Vec<(&Task, Option<String>)> = Vec::new();
    let mut seen = HashSet::new();
    for id in &selection.tasks {
        let task = catalog.task(id).ok_or_else(|| PlanError::UnknownTask(id.clone()))?;
        if seen.insert(task.id) {
            chosen.push((task, None));
        }
    }
    let mut pending: Vec<(String, String)> =
        apps.iter().flat_map(|a| a.requires.iter().map(|r| (a.id.clone(), r.clone()))).collect();
    let mut i = 0;
    loop {
        if let Some((by, req)) = pending.pop() {
            let dep = catalog
                .task(&req)
                .ok_or_else(|| PlanError::MissingRequirement { task: by.clone(), missing: req.clone() })?;
            if seen.insert(dep.id) {
                chosen.push((dep, Some(by)));
            }
            continue;
        }
        let Some((task, _)) = chosen.get(i) else { break };
        pending.extend(task.requires.iter().map(|r| (task.id.to_owned(), (*r).to_owned())));
        i += 1;
    }

    let position = |id| catalog.tasks.iter().position(|t| t.id == id);
    chosen.sort_by_key(|(task, _)| position(task.id));
    let planned = |(task, required_by): (&Task, Option<String>)| PlannedTask {
        id: task.id.to_owned(),
        title: task.title.clone(),
        steps: task.steps.clone(),
        reboot: task.reboot,
        requires: task.requires.iter().map(|r| (*r).to_owned()).collect(),
        required_by,
    };
    let (debloat, system): (Vec<_>, Vec<_>) =
        chosen.into_iter().partition(|(task, _)| task.category == Category::Debloat);
    let tasks = system.into_iter().map(planned).chain(apps).chain(debloat.into_iter().map(planned)).collect();
    Ok(Plan { distro, tasks })
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
            if let Some(by) = &task.required_by {
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
