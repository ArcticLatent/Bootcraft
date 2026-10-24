//! Applications: what Bootcraft can install, where from, and which source a
//! given system should use.

use serde::{Deserialize, Serialize};

use crate::system::{Distro, System};
use crate::task::{AppImageInstall, Step};

/// Groups on the Applications page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Section {
    Graphics,
    Photo,
    VideoAudio,
    Animation,
    CreativeUtilities,
    Editors,
    DevTools,
    Containers,
    Languages,
    Ai,
    Gaming,
    Everyday,
}

impl Section {
    pub fn title(self) -> &'static str {
        match self {
            Self::Graphics => "Graphics & 3D",
            Self::Photo => "Photography",
            Self::VideoAudio => "Video & audio",
            Self::Animation => "Animation",
            Self::CreativeUtilities => "Creative utilities",
            Self::Editors => "Code editors",
            Self::DevTools => "Developer tools",
            Self::Containers => "Containers",
            Self::Languages => "Languages",
            Self::Ai => "AI & local models",
            Self::Gaming => "Gaming",
            Self::Everyday => "Everyday",
        }
    }
}

/// Where an app comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Flathub, installed system-wide.
    Flatpak,
    /// The distribution's package manager.
    Native,
}

/// The user's global "install apps as" setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preference {
    /// Bootcraft picks per app and per distribution.
    #[default]
    Auto,
    Flatpak,
    Native,
}

/// How an app's own needs override the preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Policy {
    Either,
    /// Needs the host system (editors that run toolchains, Steam with the
    /// GPU stack); only an explicit per-app choice picks the Flatpak.
    NativeFirst,
    /// Upstream supports only the Flatpak (e.g. Bottles).
    FlatpakFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlatpakRef {
    pub id: String,
    /// Published on Flathub by the app's own developers.
    pub verified: bool,
}

/// How the native package is obtained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Via {
    /// The distribution's official repositories.
    Repo,
    /// Fedora: RPM Fusion, which Bootcraft enables.
    RpmFusion,
    /// Arch: built from the AUR.
    Aur,
    /// A package from a signed `linux-release.json` (see [`crate::release`]),
    /// or its AppImage where the package can't be installed.
    SignedRelease { manifest_url: String, public_key: String, appimage: Option<AppImageInstall> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativePkg {
    pub packages: Vec<String>,
    pub via: Via,
    /// systemd units to enable after installing.
    pub services: Vec<String>,
    /// Extra tasks the packages need, e.g. Arch's multilib for `lib32-*`.
    pub requires: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct App {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub section: Section,
    pub flatpak: Option<FlatpakRef>,
    /// The package for the system the catalog was built for, if any.
    pub native: Option<NativePkg>,
    pub policy: Policy,
    pub notes: Vec<String>,
    /// The app's icon (SVG) for apps that aren't on Flathub.
    pub icon_svg: Option<&'static str>,
}

/// The source an app will be installed from, and why, for the UI badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub source: Source,
    pub reason: &'static str,
}

impl App {
    pub fn available(&self) -> bool {
        self.flatpak.is_some() || self.native.is_some()
    }

    pub fn has(&self, source: Source) -> bool {
        match source {
            Source::Flatpak => self.flatpak.is_some(),
            Source::Native => self.native.is_some(),
        }
    }

    /// Pick a source. `explicit` is a per-app choice from the user and wins
    /// whenever that source exists.
    pub fn choose(&self, sys: &System, preference: Preference, explicit: Option<Source>) -> Option<Choice> {
        let pick = |source, reason| Some(Choice { source, reason });
        match (&self.flatpak, &self.native) {
            (None, None) => return None,
            (Some(_), None) => return pick(Source::Flatpak, "only available on Flathub"),
            (None, Some(_)) => return pick(Source::Native, "only available as a system package"),
            (Some(_), Some(_)) => {}
        }
        if let Some(source) = explicit {
            return pick(source, "your choice");
        }
        match self.policy {
            Policy::NativeFirst => return pick(Source::Native, "works best installed on the system"),
            Policy::FlatpakFirst => return pick(Source::Flatpak, "its developers support the Flatpak"),
            Policy::Either => {}
        }
        match preference {
            Preference::Flatpak => pick(Source::Flatpak, "you prefer Flatpak"),
            Preference::Native => pick(Source::Native, "you prefer system packages"),
            Preference::Auto => {
                if self.native.as_ref().is_some_and(|n| n.via == Via::Aur) {
                    return pick(Source::Flatpak, "Flathub build instead of an AUR build");
                }
                match sys.distro() {
                    Some(Distro::Arch | Distro::Fedora) => {
                        pick(Source::Native, "the distribution's package is current")
                    }
                    _ => pick(Source::Flatpak, "the distribution ships an older version"),
                }
            }
        }
    }

    /// Steps to install from `source`. `None` if that source doesn't exist.
    pub fn steps(&self, source: Source) -> Option<Vec<Step>> {
        match source {
            Source::Flatpak => {
                let flatpak = self.flatpak.as_ref()?;
                Some(vec![Step::FlatpakInstall { remote: "flathub".into(), apps: vec![flatpak.id.clone()] }])
            }
            Source::Native => {
                let native = self.native.as_ref()?;
                let packages = native.packages.clone();
                let mut steps = vec![match &native.via {
                    Via::Repo | Via::RpmFusion => Step::Install { packages, unless: Vec::new() },
                    Via::Aur => Step::AurInstall { packages, unless: Vec::new() },
                    Via::SignedRelease { manifest_url, public_key, appimage: None } => Step::SignedPackage {
                        package: packages.join(" "),
                        manifest_url: manifest_url.clone(),
                        public_key: public_key.clone(),
                    },
                    Via::SignedRelease { manifest_url, public_key, appimage: Some(install) } => {
                        Step::SignedAppImage {
                            package: packages.join(" "),
                            manifest_url: manifest_url.clone(),
                            public_key: public_key.clone(),
                            install: install.clone(),
                        }
                    }
                }];
                steps.extend(native.services.iter().map(|unit| Step::EnableService { unit: unit.clone() }));
                Some(steps)
            }
        }
    }
}
