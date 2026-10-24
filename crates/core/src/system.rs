//! What Bootcraft knows about the machine it runs on.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Distro {
    Fedora,
    Arch,
    Ubuntu,
    Mint,
    Debian,
}

impl Distro {
    /// Match on the exact `ID` from os-release. Derivatives (Manjaro, Pop!_OS,
    /// Nobara, ...) have their own repositories and driver tooling, so they are
    /// reported as unsupported rather than run through a flow built for the parent.
    pub fn from_os_release_id(id: &str) -> Option<Self> {
        match id {
            "fedora" => Some(Self::Fedora),
            "arch" => Some(Self::Arch),
            "ubuntu" => Some(Self::Ubuntu),
            "linuxmint" => Some(Self::Mint),
            "debian" => Some(Self::Debian),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Fedora => "Fedora",
            Self::Arch => "Arch Linux",
            Self::Ubuntu => "Ubuntu",
            Self::Mint => "Linux Mint",
            Self::Debian => "Debian",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OsInfo {
    pub id: String,
    pub id_like: Vec<String>,
    pub version_id: Option<String>,
    pub pretty_name: String,
}

impl OsInfo {
    pub fn distro(&self) -> Option<Distro> {
        Distro::from_os_release_id(&self.id)
    }

    /// Whether this release is one Bootcraft is built and tested for.
    pub fn version_supported(&self) -> bool {
        let Some(distro) = self.distro() else {
            return false;
        };
        let version = self.version_id.as_deref().unwrap_or("");
        let major = version.split('.').next().and_then(|m| m.parse::<u32>().ok());
        match distro {
            Distro::Arch => true,
            Distro::Fedora => major.is_some_and(|m| m >= 44),
            Distro::Ubuntu => matches!(version, "24.04" | "26.04"),
            Distro::Mint => major.is_some_and(|m| m >= 22),
            Distro::Debian => major == Some(13),
        }
    }

    /// Ubuntu 24.04 and Mint 22 have Qt 6.4, older than the .deb packages of
    /// Arctic Latent's apps need (they're built on Debian 13): those apps
    /// come as AppImages there.
    pub fn qt_too_old_for_debs(&self) -> bool {
        let version = self.version_id.as_deref().unwrap_or("");
        let major = version.split('.').next().and_then(|m| m.parse::<u32>().ok());
        match self.distro() {
            Some(Distro::Ubuntu) => major.is_some_and(|m| m < 26),
            Some(Distro::Mint) => major.is_some_and(|m| m < 23),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Desktop {
    Gnome,
    Kde,
    Cinnamon,
    Other(String),
    Unknown,
}

impl Desktop {
    /// Parse `XDG_CURRENT_DESKTOP` / logind's `Desktop`, e.g. `ubuntu:GNOME`,
    /// `KDE` or `X-Cinnamon`.
    pub fn parse(raw: &str) -> Self {
        let tokens: Vec<String> =
            raw.split(':').map(|t| t.trim().to_ascii_lowercase()).filter(|t| !t.is_empty()).collect();
        let has = |name: &str| tokens.iter().any(|t| t.contains(name));
        if has("gnome") {
            Self::Gnome
        } else if has("kde") || has("plasma") {
            Self::Kde
        } else if has("cinnamon") {
            Self::Cinnamon
        } else if let Some(first) = tokens.first() {
            Self::Other(first.clone())
        } else {
            Self::Unknown
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE Plasma",
            Self::Cinnamon => "Cinnamon",
            Self::Other(name) => name,
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    Wayland,
    X11,
    Tty,
    Unknown,
}

impl SessionType {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "wayland" => Self::Wayland,
            "x11" => Self::X11,
            "tty" => Self::Tty,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

impl GpuVendor {
    pub fn from_pci_id(vendor_id: u16) -> Self {
        match vendor_id {
            0x10de => Self::Nvidia,
            0x1002 => Self::Amd,
            0x8086 => Self::Intel,
            _ => Self::Other,
        }
    }
}

/// Which NVIDIA driver branch a card can use. Ordered oldest first, so the
/// minimum over several cards is the branch that supports all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NvidiaGen {
    /// Kepler and older: no driver branch the distros still package.
    Unsupported,
    /// Maxwell, Pascal and Volta: the 580 legacy branch, proprietary modules.
    Legacy580,
    /// Turing (GTX 16 / RTX 20) and newer: current branch, open kernel modules.
    Modern,
}

impl NvidiaGen {
    /// NVIDIA hands out PCI device IDs roughly in chip order. Turing (TU102)
    /// starts at 0x1E00 and every later chip sits above it; Maxwell (GM108)
    /// starts at 0x1340 and Maxwell/Pascal/Volta all stay below 0x1E00.
    pub fn from_device_id(device_id: u16) -> Self {
        match device_id {
            0x1e00.. => Self::Modern,
            0x1340..=0x1dff => Self::Legacy580,
            _ => Self::Unsupported,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Modern => "current driver, open kernel modules",
            Self::Legacy580 => "580 legacy driver",
            Self::Unsupported => "no supported driver (Kepler or older)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gpu {
    /// PCI address, e.g. `0000:01:00.0`.
    pub slot: String,
    pub vendor: GpuVendor,
    pub vendor_id: u16,
    pub device_id: u16,
    /// Marketing name from pci.ids, when the database is installed.
    pub name: Option<String>,
    pub boot_vga: bool,
}

impl Gpu {
    pub fn nvidia_gen(&self) -> Option<NvidiaGen> {
        (self.vendor == GpuVendor::Nvidia).then(|| NvidiaGen::from_device_id(self.device_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecureBoot {
    Enabled,
    Disabled,
    NotEfi,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct System {
    pub os: OsInfo,
    pub desktop: Desktop,
    pub session: SessionType,
    pub gpus: Vec<Gpu>,
    /// Driver branch to install, derived from `gpus` but overridable by the
    /// user. `None` means no NVIDIA driver work.
    pub nvidia: Option<NvidiaGen>,
    pub secure_boot: SecureBoot,
    pub kernel_release: String,
    /// Arch only: `pkgbase` of every installed kernel, from `/usr/lib/modules/*/pkgbase`.
    pub arch_kernels: Vec<String>,
    /// An SSH server unit is installed, so a firewall must keep port 22 open.
    pub ssh_server: bool,
    /// Installed RAM, from /proc/meminfo.
    pub memory_bytes: Option<u64>,
    /// Space available to the user on the root filesystem.
    pub disk_free_bytes: Option<u64>,
    /// The person setting up the machine (not root), when known.
    pub user: Option<String>,
    /// Their home folder, from the passwd file.
    pub home: Option<PathBuf>,
}

impl System {
    pub fn distro(&self) -> Option<Distro> {
        self.os.distro()
    }

    /// Problems the user should see on the scan screen before choosing tasks.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        match self.distro() {
            None => warnings.push(format!(
                "{} is not supported. Bootcraft supports Fedora, Arch Linux, Ubuntu, Linux Mint and Debian.",
                self.os.pretty_name
            )),
            Some(_) if !self.os.version_supported() => warnings.push(format!(
                "{} is not a release Bootcraft is tested on; some packages may be missing.",
                self.os.pretty_name
            )),
            Some(_) => {}
        }
        for gpu in &self.gpus {
            if gpu.nvidia_gen() == Some(NvidiaGen::Unsupported) {
                warnings.push(format!(
                    "{} predates Maxwell and no packaged NVIDIA driver supports it; keep the open-source nouveau driver.",
                    gpu.name.as_deref().unwrap_or("This NVIDIA card")
                ));
            }
        }
        if self.desktop == Desktop::Unknown {
            warnings
                .push("Could not detect the desktop environment; desktop-specific tasks are hidden.".into());
        }
        warnings
    }
}
