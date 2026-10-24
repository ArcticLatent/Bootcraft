//! Check whether a task's steps are already in effect, for the "already done"
//! badges and for skipping work on re-runs. Package state is read from the
//! package manager's database files, so no commands are run.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::system::Distro;
use crate::task::Step;

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
    db: Option<PackageDb>,
}

/// Installed package names, and those names plus everything they provide.
#[derive(Default)]
struct PackageDb {
    names: HashSet<String>,
    provided: HashSet<String>,
}

impl Probe {
    pub fn new(root: &Path, distro: Distro) -> Self {
        let db = match distro {
            Distro::Arch => pacman_db(root),
            Distro::Ubuntu | Distro::Mint | Distro::Debian => dpkg_db(root),
            Distro::Fedora => rpm_db(root),
        };
        Self { root: root.to_owned(), db }
    }

    /// Whether every step is already in effect.
    pub fn status(&self, steps: &[Step]) -> Status {
        let mut status = Status::Done;
        for step in steps {
            match self.step_done(step) {
                Some(false) => return Status::Pending,
                None => status = Status::Unknown,
                Some(true) => {}
            }
        }
        status
    }

    /// Whether `package` is installed or provided by an installed package.
    /// `None` when the package database can't be read.
    pub fn is_installed(&self, package: &str) -> Option<bool> {
        Some(self.db.as_ref()?.provided.contains(package))
    }

    /// Real names of installed packages matching `pattern` (a name, or a
    /// prefix ending in `*`), sorted. Removal needs real names, not provides.
    pub fn installed_matching(&self, pattern: &str) -> Option<Vec<String>> {
        let names = &self.db.as_ref()?.names;
        let mut matches: Vec<String> = match pattern.strip_suffix('*') {
            Some(prefix) => names.iter().filter(|n| n.starts_with(prefix)).cloned().collect(),
            None => names.get(pattern).cloned().into_iter().collect(),
        };
        matches.sort();
        Some(matches)
    }

    /// An absolute path inside the probed root.
    fn rooted(&self, path: &Path) -> PathBuf {
        self.root.join(path.strip_prefix("/").unwrap_or(path))
    }

    fn read(&self, path: &Path) -> Option<String> {
        fs::read_to_string(self.rooted(path)).ok()
    }

    fn step_done(&self, step: &Step) -> Option<bool> {
        match step {
            Step::Install { unless, .. } | Step::AurInstall { unless, .. } | Step::Run { unless, .. }
                if unless.iter().any(|p| self.is_installed(p) == Some(true)) =>
            {
                Some(true)
            }
            Step::Install { packages, .. } | Step::AurInstall { packages, .. } => {
                packages.iter().try_fold(true, |acc, p| Some(acc && self.is_installed(p)?))
            }
            Step::Remove { packages } => {
                packages.iter().try_fold(true, |acc, p| Some(acc && self.installed_matching(p)?.is_empty()))
            }
            Step::FlatpakRemote { name, .. } => {
                let config = fs::read_to_string(self.root.join("var/lib/flatpak/repo/config")).ok()?;
                Some(config.contains(&format!("[remote \"{name}\"]")))
            }
            Step::FlatpakInstall { apps, .. } => {
                Some(apps.iter().all(|app| self.root.join("var/lib/flatpak/app").join(app).is_dir()))
            }
            Step::Edit { path, edit, if_exists } => {
                match fs::read_to_string(self.root.join(path.strip_prefix("/").unwrap_or(path))) {
                    Ok(text) => Some(edit.is_applied(&text)),
                    Err(_) if *if_exists => Some(true),
                    Err(_) => Some(false),
                }
            }
            Step::EnableService { unit } => {
                let wants = fs::read_dir(self.root.join("etc/systemd/system")).ok()?;
                Some(wants.flatten().any(|dir| dir.path().join(unit).exists()))
            }
            Step::SignedPackage { package, .. } | Step::Swap { to: package, .. } => {
                self.is_installed(package)
            }
            Step::SignedAppImage { install, .. } => Some(self.rooted(&install.path).is_file()),
            Step::WriteFile { path, contents } => Some(self.read(path).is_some_and(|text| &text == contents)),
            Step::Download { path, sha256, .. } => Some(
                crate::release::sha256_file(&self.rooted(path))
                    .is_ok_and(|sum| sum.eq_ignore_ascii_case(sha256)),
            ),
            Step::AddToGroup { user, group } => {
                let groups = self.read(Path::new("/etc/group"))?;
                Some(group_members(&groups, group).iter().any(|member| member == user))
            }
            Step::ForeignArch { arch } => {
                let arches = self.read(Path::new("/var/lib/dpkg/arch")).unwrap_or_default();
                Some(arches.lines().any(|line| line.trim() == arch))
            }
            Step::RemoveSnaps => {
                if self.is_installed("snapd")? {
                    let snaps = fs::read_dir(self.root.join("var/lib/snapd/snaps")).ok()?;
                    Some(!snaps.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".snap")))
                } else {
                    Some(true)
                }
            }
            // Any NVIDIA driver from the Ubuntu archive counts.
            Step::UbuntuDriver { .. } => Some(!self.installed_matching("nvidia-driver-*")?.is_empty()),
            Step::Run { .. } => None,
        }
    }
}

/// pacman keeps one directory per package with a `desc` file listing its
/// name and what it provides, e.g. `nvidia-open-dkms-tkg` provides
/// `NVIDIA-MODULE`.
fn pacman_db(root: &Path) -> Option<PackageDb> {
    let entries = fs::read_dir(root.join("var/lib/pacman/local")).ok()?;
    let mut db = PackageDb::default();
    for entry in entries.flatten() {
        let Ok(desc) = fs::read_to_string(entry.path().join("desc")) else {
            continue;
        };
        let mut section = "";
        for line in desc.lines().map(str::trim) {
            if line.starts_with('%') && line.ends_with('%') {
                section = line;
            } else if !line.is_empty() && section == "%NAME%" {
                db.names.insert(line.to_owned());
                db.provided.insert(line.to_owned());
            } else if !line.is_empty() && section == "%PROVIDES%" {
                db.provided.insert(strip_version(line).to_owned());
            }
        }
    }
    Some(db)
}

fn dpkg_db(root: &Path) -> Option<PackageDb> {
    let status = fs::read_to_string(root.join("var/lib/dpkg/status")).ok()?;
    let mut db = PackageDb::default();
    for stanza in status.split("\n\n") {
        let field = |name: &str| stanza.lines().find_map(|l| l.strip_prefix(name)).map(str::trim);
        if field("Status:") != Some("install ok installed") {
            continue;
        }
        if let Some(name) = field("Package:") {
            db.names.insert(name.to_owned());
            db.provided.insert(name.to_owned());
        }
        if let Some(provides) = field("Provides:") {
            db.provided.extend(provides.split(',').map(|p| strip_version(p).to_owned()));
        }
    }
    Some(db)
}

/// The rpm database isn't a plain file; ask rpm for every package and what it provides.
fn rpm_db(root: &Path) -> Option<PackageDb> {
    let out = std::process::Command::new("rpm")
        .arg("--root")
        .arg(root)
        .args(["-qa", "--qf", "N %{NAME}\\n[P %{PROVIDENAME}\\n]"])
        .output()
        .ok()?;
    out.status.success().then(|| parse_rpm_query(&String::from_utf8_lossy(&out.stdout)))
}

/// Output of `rpm -qa --qf 'N %{NAME}\n[P %{PROVIDENAME}\n]'`.
fn parse_rpm_query(text: &str) -> PackageDb {
    let mut db = PackageDb::default();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("N ") {
            db.names.insert(name.to_owned());
            db.provided.insert(name.to_owned());
        } else if let Some(provide) = line.strip_prefix("P ") {
            db.provided.insert(strip_version(provide).to_owned());
        }
    }
    db
}

/// Members of `group` in an /etc/group file.
fn group_members(groups: &str, group: &str) -> Vec<String> {
    groups
        .lines()
        .find_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            (fields.len() >= 4 && fields[0] == group).then(|| fields[3].to_owned())
        })
        .map(|members| members.split(',').filter(|m| !m.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// `foo=1.2`, `foo>=1` or `foo (= 1.2)` -> `foo`.
pub fn strip_version(dep: &str) -> &str {
    dep.split(['=', '<', '>', ' ', '(']).next().unwrap_or(dep).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpm_query_output() {
        let db =
            parse_rpm_query("N akmod-nvidia\nP akmod-nvidia\nP nvidia-kmod-common = 3:615\nN ffmpeg-free\n");
        assert!(db.names.contains("akmod-nvidia") && db.names.contains("ffmpeg-free"));
        assert!(db.provided.contains("nvidia-kmod-common"));
        assert!(!db.names.contains("nvidia-kmod-common"));
    }

    #[test]
    fn group_membership() {
        let groups = "root:x:0:\nsudo:x:27:alice,bob\nusers:x:100:\n";
        assert_eq!(group_members(groups, "sudo"), ["alice", "bob"]);
        assert!(group_members(groups, "users").is_empty());
        assert!(group_members(groups, "wheel").is_empty());
    }
}
