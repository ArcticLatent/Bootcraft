//! Check whether a task's steps are already in effect, for the "already done"
//! badges and for skipping work on re-runs. Package state is read from the
//! package manager's database files, so no commands are run.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::system::Distro;
use crate::task::{Step, Task};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Every step is already in effect.
    Done,
    /// At least one step still has work to do.
    Pending,
    /// Some steps (plain commands, or a package database we can't read) can't
    /// be checked, and nothing checkable is pending.
    Unknown,
}

pub struct Probe {
    root: PathBuf,
    installed: Option<HashSet<String>>,
}

impl Probe {
    pub fn new(root: &Path, distro: Distro) -> Self {
        let installed = match distro {
            Distro::Arch => pacman_installed(root),
            Distro::Ubuntu | Distro::Mint | Distro::Debian => dpkg_installed(root),
            // The rpm database is not a plain file; read via rpm once Fedora is ported.
            Distro::Fedora => None,
        };
        Self { root: root.to_owned(), installed }
    }

    pub fn status(&self, task: &Task) -> Status {
        let mut status = Status::Done;
        for step in &task.steps {
            match self.step_done(step) {
                Some(false) => return Status::Pending,
                None => status = Status::Unknown,
                Some(true) => {}
            }
        }
        status
    }

    fn is_installed(&self, package: &str) -> Option<bool> {
        let installed = self.installed.as_ref()?;
        Some(match package.strip_suffix('*') {
            Some(prefix) => installed.iter().any(|p| p.starts_with(prefix)),
            None => installed.contains(package),
        })
    }

    fn step_done(&self, step: &Step) -> Option<bool> {
        let all = |packages: &[String], want: bool| {
            packages.iter().try_fold(true, |acc, p| Some(acc && self.is_installed(p)? == want))
        };
        match step {
            Step::Install { unless: Some(provide), .. } if self.is_installed(provide)? => Some(true),
            Step::Install { packages, .. } | Step::AurInstall { packages } => all(packages, true),
            Step::Remove { packages } => all(packages, false),
            Step::FlatpakRemote { name, .. } => {
                let config = fs::read_to_string(self.root.join("var/lib/flatpak/repo/config")).ok()?;
                Some(config.contains(&format!("[remote \"{name}\"]")))
            }
            Step::FlatpakInstall { apps, .. } => {
                Some(apps.iter().all(|app| self.root.join("var/lib/flatpak/app").join(app).is_dir()))
            }
            Step::Edit { path, edit } => {
                let text = fs::read_to_string(self.root.join(path.strip_prefix("/").unwrap_or(path)))
                    .unwrap_or_default();
                Some(edit.is_applied(&text))
            }
            Step::EnableService { unit } => {
                let wants = fs::read_dir(self.root.join("etc/systemd/system")).ok()?;
                Some(wants.flatten().any(|dir| dir.path().join(unit).exists()))
            }
            Step::Run { .. } => None,
        }
    }
}

/// pacman keeps one directory per package with a `desc` file listing its
/// name and what it provides, e.g. `nvidia-open-dkms-tkg` provides
/// `nvidia-open-dkms`. Both count as installed.
fn pacman_installed(root: &Path) -> Option<HashSet<String>> {
    let entries = fs::read_dir(root.join("var/lib/pacman/local")).ok()?;
    let mut installed = HashSet::new();
    for entry in entries.flatten() {
        let Ok(desc) = fs::read_to_string(entry.path().join("desc")) else {
            continue;
        };
        let mut section = "";
        for line in desc.lines().map(str::trim) {
            if line.starts_with('%') && line.ends_with('%') {
                section = line;
            } else if !line.is_empty() && matches!(section, "%NAME%" | "%PROVIDES%") {
                installed.insert(strip_version(line).to_owned());
            }
        }
    }
    Some(installed)
}

/// Installed packages and everything they provide.
fn dpkg_installed(root: &Path) -> Option<HashSet<String>> {
    let status = fs::read_to_string(root.join("var/lib/dpkg/status")).ok()?;
    let mut installed = HashSet::new();
    for stanza in status.split("\n\n") {
        let field = |name: &str| stanza.lines().find_map(|l| l.strip_prefix(name)).map(str::trim);
        if field("Status:") != Some("install ok installed") {
            continue;
        }
        installed.extend(field("Package:").map(str::to_owned));
        if let Some(provides) = field("Provides:") {
            installed.extend(provides.split(',').map(|p| strip_version(p).to_owned()));
        }
    }
    Some(installed)
}

/// `foo=1.2`, `foo>=1` or `foo (= 1.2)` -> `foo`.
fn strip_version(dep: &str) -> &str {
    dep.split(['=', '<', '>', ' ', '(']).next().unwrap_or(dep).trim()
}
