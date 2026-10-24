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

/// apt-get without questions: keep changed config files, and wait for a lock
/// held by unattended upgrades instead of failing.
pub const APT_GET: &[&str] = &[
    "env",
    "DEBIAN_FRONTEND=noninteractive",
    "apt-get",
    "-y",
    "-o",
    "Dpkg::Options::=--force-confdef",
    "-o",
    "Dpkg::Options::=--force-confold",
    "-o",
    "DPkg::Lock::Timeout=300",
];

/// One unit of work. Steps are data so the full plan can be reviewed before
/// anything runs, and so the helper never executes a shell string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Step {
    /// Install packages from the distribution's repositories.
    Install {
        packages: Vec<String>,
        /// Skip the step when any of these packages or virtual provides is
        /// installed, e.g. a custom driver build providing `NVIDIA-MODULE`.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        unless: Vec<String>,
    },
    /// Remove packages that are installed and skip the rest. A trailing `*`
    /// matches by prefix, e.g. `libreoffice*`.
    Remove { packages: Vec<String> },
    /// Arch: build packages from the AUR as the invoking user, then install
    /// them with pacman. AUR dependencies are resolved and built first.
    AurInstall {
        packages: Vec<String>,
        /// Same as for [`Step::Install`].
        #[serde(skip_serializing_if = "Vec::is_empty")]
        unless: Vec<String>,
    },
    /// Download a package from a release with a signed `linux-release.json`
    /// (see [`crate::release`]), check the signature and the file's SHA-256,
    /// then install it with the package manager.
    SignedPackage { package: String, manifest_url: String, public_key: String },
    /// Add a Flatpak remote system-wide.
    FlatpakRemote { name: String, url: String },
    /// Install Flatpak apps system-wide.
    FlatpakInstall { remote: String, apps: Vec<String> },
    /// Apply a structured edit to a config file.
    Edit {
        path: PathBuf,
        edit: Edit,
        /// The file belongs to an optional program (e.g. LightDM): leave it
        /// alone when it doesn't exist instead of creating it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        if_exists: bool,
    },
    /// Write a small file whole (an apt source, a sysctl setting). The first
    /// original is kept as `<path>.bootcraft.bak`.
    WriteFile { path: PathBuf, contents: String },
    /// Download a file and check it against a pinned SHA-256.
    Download { url: String, path: PathBuf, sha256: String },
    /// Fedora: replace one package with another (`dnf swap`), e.g. Fedora's
    /// patent-free ffmpeg with RPM Fusion's.
    Swap { from: String, to: String },
    /// Add a user to a group, e.g. Debian's `sudo`.
    AddToGroup { user: String, group: String },
    /// Debian family: enable packages of another architecture (i386 for Steam).
    ForeignArch { arch: String },
    /// Ubuntu: remove every snap (leaves before the snaps they depend on).
    RemoveSnaps,
    /// Ubuntu and Mint: install the newest NVIDIA driver `ubuntu-drivers` offers
    /// for this card: the open-module build, or the proprietary one up to
    /// `max_branch` (580 for the legacy cards).
    UbuntuDriver {
        open: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_branch: Option<u32>,
    },
    /// Enable and start a systemd unit.
    EnableService { unit: String },
    /// Run a program. Arguments are passed as-is; there is no shell. Whether
    /// it already ran can't be known, so it runs every time unless one of
    /// `unless` (packages or provides) is installed.
    Run {
        argv: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unless: Vec<String>,
    },
}

impl Step {
    pub fn install(packages: &[&str]) -> Self {
        Self::Install { packages: strings(packages), unless: Vec::new() }
    }

    pub fn remove(packages: &[&str]) -> Self {
        Self::Remove { packages: strings(packages) }
    }

    pub fn aur(packages: &[&str]) -> Self {
        Self::AurInstall { packages: strings(packages), unless: Vec::new() }
    }

    /// An AUR install that is skipped when any of `alternatives` is
    /// installed, e.g. `yay-bin` unless you already use yay or paru.
    pub fn aur_unless(alternatives: &[&str], packages: &[&str]) -> Self {
        Self::AurInstall { packages: strings(packages), unless: strings(alternatives) }
    }

    pub fn edit(path: &str, edit: Edit) -> Self {
        Self::Edit { path: path.into(), edit, if_exists: false }
    }

    /// An edit to the config file of an optional program; see `if_exists`.
    pub fn edit_existing(path: &str, edit: Edit) -> Self {
        Self::Edit { path: path.into(), edit, if_exists: true }
    }

    pub fn run(argv: &[&str]) -> Self {
        Self::Run { argv: strings(argv), unless: Vec::new() }
    }

    /// A command only needed until one of `alternatives` is installed, e.g.
    /// answering a package's license question before installing it.
    pub fn run_unless(alternatives: &[&str], argv: &[&str]) -> Self {
        Self::Run { argv: strings(argv), unless: strings(alternatives) }
    }

    pub fn write_file(path: &str, contents: &str) -> Self {
        Self::WriteFile { path: path.into(), contents: contents.into() }
    }

    /// An install skipped when any of `alternatives` is installed.
    pub fn install_unless(alternatives: &[&str], packages: &[&str]) -> Self {
        Self::Install { packages: strings(packages), unless: strings(alternatives) }
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
                format!("{pm} {}{}", packages.join(" "), unless_note(unless))
            }
            Self::Remove { packages } => {
                let pm = match distro {
                    Distro::Arch => "pacman -Rns --noconfirm",
                    Distro::Fedora => "dnf remove -y",
                    Distro::Ubuntu | Distro::Mint | Distro::Debian => "apt-get remove -y",
                };
                format!("{pm} {} (installed ones only)", packages.join(" "))
            }
            Self::AurInstall { packages, unless } => format!(
                "build from the AUR as your user, then pacman -U: {}{}",
                packages.join(" "),
                unless_note(unless)
            ),
            Self::SignedPackage { package, manifest_url, .. } => {
                format!("download {package} from its signed release ({manifest_url}), verify it, install it")
            }
            Self::FlatpakRemote { name, url } => {
                format!("flatpak remote-add --if-not-exists --system {name} {url}")
            }
            Self::FlatpakInstall { remote, apps } => {
                format!("flatpak install -y --noninteractive --system {remote} {}", apps.join(" "))
            }
            Self::Edit { path, edit, if_exists } => {
                let only = if *if_exists { " (if the file exists)" } else { "" };
                format!("edit {}: {edit}{only}", path.display())
            }
            Self::WriteFile { path, .. } => format!("write {}", path.display()),
            Self::Download { url, path, .. } => {
                format!("download {url} to {} and check its SHA-256", path.display())
            }
            Self::Swap { from, to } => format!("dnf swap -y {from} {to} --allowerasing"),
            Self::AddToGroup { user, group } => format!("usermod -aG {group} {user}"),
            Self::ForeignArch { arch } => format!("dpkg --add-architecture {arch}"),
            Self::RemoveSnaps => "snap remove --purge every installed snap".into(),
            Self::UbuntuDriver { open: true, .. } => {
                "apt-get install -y the newest nvidia-driver-<branch>-open that ubuntu-drivers lists".into()
            }
            Self::UbuntuDriver { open: false, max_branch } => format!(
                "apt-get install -y the newest nvidia-driver-<branch> that ubuntu-drivers lists{}",
                max_branch.map(|m| format!(", up to {m}")).unwrap_or_default()
            ),
            Self::EnableService { unit } => format!("systemctl enable --now {unit}"),
            Self::Run { argv, unless } => format!(
                "{}{}",
                argv.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" "),
                unless_note(unless)
            ),
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

fn unless_note(unless: &[String]) -> String {
    match unless {
        [] => String::new(),
        [one] => format!("  (skipped if {one} is already installed)"),
        [rest @ .., last] => format!("  (skipped if {} or {last} is already installed)", rest.join(", ")),
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

pub(crate) fn shell_quote(arg: &str) -> String {
    let safe = !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@+,".contains(c));
    if safe { arg.to_owned() } else { format!("'{}'", arg.replace('\'', r"'\''")) }
}
