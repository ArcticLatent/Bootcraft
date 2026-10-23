//! Tasks and the steps they are made of.

use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

use crate::edit::Edit;
use crate::system::Distro;

/// Sections of the task list, in display and execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    System,
    Drivers,
    Multimedia,
    Apps,
    Tools,
    Desktop,
    Debloat,
}

impl Category {
    pub fn title(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Drivers => "Drivers",
            Self::Multimedia => "Multimedia",
            Self::Apps => "Apps",
            Self::Tools => "Tools",
            Self::Desktop => "Desktop",
            Self::Debloat => "Debloat",
        }
    }
}

/// Whether a task starts ticked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Recommended,
    Optional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    pub id: &'static str,
    pub category: Category,
    pub title: String,
    pub description: String,
    pub preset: Preset,
    /// Tasks that must run too; the planner pulls them in automatically.
    pub requires: Vec<&'static str>,
    pub reboot: bool,
    /// Caveats shown next to the task, e.g. Secure Boot signing.
    pub notes: Vec<String>,
    pub steps: Vec<Step>,
}

impl Task {
    pub fn new(id: &'static str, category: Category, title: impl Into<String>) -> Self {
        Self {
            id,
            category,
            title: title.into(),
            description: String::new(),
            preset: Preset::Optional,
            requires: Vec::new(),
            reboot: false,
            notes: Vec::new(),
            steps: Vec::new(),
        }
    }

    pub fn recommended(mut self) -> Self {
        self.preset = Preset::Recommended;
        self
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = text.into();
        self
    }

    pub fn requires(mut self, ids: &[&'static str]) -> Self {
        self.requires.extend_from_slice(ids);
        self
    }

    pub fn reboot(mut self) -> Self {
        self.reboot = true;
        self
    }

    pub fn note(mut self, text: impl Into<String>) -> Self {
        self.notes.push(text.into());
        self
    }

    pub fn step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }
}

/// One unit of work. Steps are data so the full plan can be reviewed before
/// anything runs, and so the helper never executes a shell string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Step {
    /// Install packages from the distribution's repositories.
    Install {
        packages: Vec<String>,
        /// Skip the step when this package or virtual provide is already
        /// installed, e.g. a custom driver build providing `NVIDIA-MODULE`.
        #[serde(skip_serializing_if = "Option::is_none")]
        unless: Option<String>,
    },
    /// Remove packages that are installed and skip the rest. A trailing `*`
    /// matches by prefix, e.g. `libreoffice*`.
    Remove { packages: Vec<String> },
    /// Arch: build packages from the AUR as the invoking user, then install
    /// them with pacman. AUR dependencies are resolved and built first.
    AurInstall { packages: Vec<String> },
    /// Add a Flatpak remote system-wide.
    FlatpakRemote { name: String, url: String },
    /// Install Flatpak apps system-wide.
    FlatpakInstall { remote: String, apps: Vec<String> },
    /// Apply a structured edit to a config file.
    Edit { path: PathBuf, edit: Edit },
    /// Enable and start a systemd unit.
    EnableService { unit: String },
    /// Run a program. Arguments are passed as-is; there is no shell.
    Run { argv: Vec<String> },
}

impl Step {
    pub fn install(packages: &[&str]) -> Self {
        Self::Install { packages: strings(packages), unless: None }
    }

    pub fn remove(packages: &[&str]) -> Self {
        Self::Remove { packages: strings(packages) }
    }

    pub fn aur(packages: &[&str]) -> Self {
        Self::AurInstall { packages: strings(packages) }
    }

    pub fn edit(path: &str, edit: Edit) -> Self {
        Self::Edit { path: path.into(), edit }
    }

    pub fn run(argv: &[&str]) -> Self {
        Self::Run { argv: strings(argv) }
    }

    /// The command line (or a description, for edits and AUR builds) shown on
    /// the review screen.
    pub fn describe(&self, distro: Distro) -> String {
        match self {
            Self::Install { packages, unless } => {
                let pm = match distro {
                    Distro::Arch => "pacman -S --needed --noconfirm",
                    Distro::Fedora => "dnf install -y",
                    Distro::Ubuntu | Distro::Mint | Distro::Debian => "apt-get install -y",
                };
                let mut line = format!("{pm} {}", packages.join(" "));
                if let Some(provide) = unless {
                    line.push_str(&format!("  (skipped if {provide} is already installed)"));
                }
                line
            }
            Self::Remove { packages } => {
                let pm = match distro {
                    Distro::Arch => "pacman -Rns --noconfirm",
                    Distro::Fedora => "dnf remove -y",
                    Distro::Ubuntu | Distro::Mint | Distro::Debian => "apt-get remove -y",
                };
                format!("{pm} {} (installed ones only)", packages.join(" "))
            }
            Self::AurInstall { packages } => {
                format!("build from the AUR as your user, then pacman -U: {}", packages.join(" "))
            }
            Self::FlatpakRemote { name, url } => {
                format!("flatpak remote-add --if-not-exists --system {name} {url}")
            }
            Self::FlatpakInstall { remote, apps } => {
                format!("flatpak install -y --noninteractive --system {remote} {}", apps.join(" "))
            }
            Self::Edit { path, edit } => format!("edit {}: {edit}", path.display()),
            Self::EnableService { unit } => format!("systemctl enable --now {unit}"),
            Self::Run { argv } => argv.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" "),
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn shell_quote(arg: &str) -> String {
    let safe = !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@+,".contains(c));
    if safe { arg.to_owned() } else { format!("'{}'", arg.replace('\'', r"'\''")) }
}
