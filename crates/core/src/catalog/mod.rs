//! What Bootcraft offers on each distribution: system tasks in execution
//! order, the curated apps, and the workflow packs that tick them.

pub mod apps;
mod arch;
mod common;
mod debian;
mod fedora;
mod ubuntu;

pub use apps::{PACKS, Pack, pack};

use crate::apps::{App, Source, Via};
use crate::system::{Distro, System};
use crate::task::Task;

pub const FLATHUB_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

pub const COMFYPILOT_MANIFEST: &str =
    "https://github.com/ArcticLatent/ComfyPilot/releases/latest/download/linux-release.json";
/// ComfyPilot's release-signing key, as pinned in its own updater
/// (`src/update_signing.rs`).
pub const COMFYPILOT_PUBLIC_KEY: &str = "rrs0a3FucFu27aAz2rQ1N2kZhvP/BEgNTHKqZK9nS54=";

/// Task IDs that apps depend on.
pub const UPDATE: &str = "system.update";
pub const FLATHUB: &str = "flatpak.flathub";
pub const ARCH_BUILD_TOOLS: &str = "arch.build-tools";
pub const FEDORA_RPMFUSION: &str = "fedora.rpmfusion";

pub struct Catalog {
    pub tasks: Vec<Task>,
    pub apps: Vec<App>,
}

impl Catalog {
    pub fn task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn app(&self, id: &str) -> Option<&App> {
        self.apps.iter().find(|a| a.id == id)
    }
}

/// The catalog for this system; empty for unsupported distributions.
pub fn catalog(sys: &System) -> Catalog {
    let tasks = match sys.distro() {
        Some(Distro::Arch) => arch::tasks(sys),
        Some(Distro::Fedora) => fedora::tasks(sys),
        Some(Distro::Debian) => debian::tasks(sys),
        Some(Distro::Ubuntu | Distro::Mint) => ubuntu::tasks(sys),
        None => Vec::new(),
    };
    let apps = if sys.distro().is_some() { apps::apps(sys) } else { Vec::new() };
    Catalog { tasks, apps }
}

/// Tasks an app installed from `source` depends on.
pub fn app_requires(distro: Distro, app: &App, source: Source) -> Vec<&'static str> {
    let Some(native) = app.native.as_ref().filter(|_| source == Source::Native) else {
        return vec![FLATHUB];
    };
    let mut requires = match (&native.via, distro) {
        (Via::RpmFusion, _) => vec![FEDORA_RPMFUSION],
        (Via::Aur, _) => vec![UPDATE, ARCH_BUILD_TOOLS],
        // Arch installs only from freshly synced databases; dnf refreshes
        // its metadata itself and the executor refreshes apt's lists.
        (Via::Repo | Via::SignedRelease { .. }, Distro::Arch) => vec![UPDATE],
        (Via::Repo | Via::SignedRelease { .. }, _) => Vec::new(),
    };
    requires.extend_from_slice(&native.requires);
    requires
}
