//! Ubuntu and Linux Mint, which share Ubuntu's archive: the restricted
//! codecs, `ubuntu-drivers` for NVIDIA, and on Ubuntu, optionally, Snap
//! replaced by Flatpak with Firefox from Mozilla's own repository.

use crate::catalog::FLATHUB;
use crate::catalog::common::{self, apt};
use crate::edit::Edit;
use crate::system::{Desktop, Distro, NvidiaGen, SecureBoot, System};
use crate::task::{Category, Step, Task};

pub const MULTIVERSE: &str = "ubuntu.multiverse";

/// Mozilla's APT signing key, embedded rather than downloaded at install
/// time (fingerprint 35BA A0B3 3E9E B396 F59C A838 C0BA 5CE6 DC63 15A3).
const MOZILLA_KEY: &str = include_str!("keys/packages.mozilla.org.asc");

pub fn tasks(sys: &System) -> Vec<Task> {
    let mint = sys.distro() == Some(Distro::Mint);
    let mut tasks = Vec::new();
    // Mint's sources already include universe and multiverse.
    if !mint {
        tasks.push(multiverse());
    }
    tasks.extend([common::apt_update(), common::apt_i386()]);
    if !mint {
        tasks.push(replace_snap(&sys.desktop));
    }
    if let Some(generation) = sys.nvidia
        && generation != NvidiaGen::Unsupported
    {
        tasks.push(nvidia(sys, generation));
        tasks.push(
            Task::new("gpu.nvidia-vaapi", Category::Multimedia, "Hardware video decoding on NVIDIA")
                .recommended()
                .description("VA-API driver so browsers and players can decode video on the GPU.")
                .step(Step::install(&["nvidia-vaapi-driver"])),
        );
    }
    tasks.extend([
        common::flathub(&sys.desktop, "gnome-software-plugin-flatpak", "plasma-discover-backend-flatpak"),
        codecs(mint),
        archive(mint),
        common::drives(&["ntfs-3g", "exfatprogs"]),
        common::fonts(&["fonts-noto-core", "fonts-noto-cjk", "fonts-noto-color-emoji"]),
        common::ms_fonts_apt(),
        common::firmware(),
        common::firewall_ufw(sys.ssh_server),
    ]);
    if mint {
        tasks.push(swappiness());
    }
    match sys.desktop {
        Desktop::Gnome => tasks.push(common::debloat_gnome(&[
            "gnome-maps",
            "gnome-snapshot",
            "simple-scan",
            "showtime",
            "totem",
            "htop",
            "vim",
        ])),
        Desktop::Kde => tasks.push(common::debloat_kde(common::KDE_EXTRAS)),
        _ => {}
    }
    tasks.push(common::debloat_libreoffice());
    tasks
}

/// universe and multiverse: codecs, unrar, Steam. Ubuntu 24.04+ keeps its
/// sources in ubuntu.sources; sources.list is only a pointer to it.
fn multiverse() -> Task {
    let components = Edit::apt_components(&["universe", "multiverse"]);
    Task::new(MULTIVERSE, Category::System, "Enable universe and multiverse")
        .recommended()
        .description("Ubuntu's community and restricted repositories, for codecs, RAR support and Steam.")
        .step(Step::edit_existing("/etc/apt/sources.list.d/ubuntu.sources", components.clone()))
        .step(Step::edit_existing("/etc/apt/sources.list", components))
}

/// Remove every snap and snapd, keep snapd from coming back, and install
/// Firefox (a snap on Ubuntu) from Mozilla's repository instead.
fn replace_snap(desktop: &Desktop) -> Task {
    let store = match desktop {
        Desktop::Gnome => Some(["gnome-software", "gnome-software-plugin-flatpak"]),
        Desktop::Kde => Some(["plasma-discover", "plasma-discover-backend-flatpak"]),
        _ => None,
    };
    let mut task = Task::new("ubuntu.replace-snap", Category::Apps, "Replace Snap with Flatpak")
        .description("Removes every snap and snapd, then installs Firefox from Mozilla's own repository and a software store that installs Flatpaks.")
        .note("All snaps are removed, including their data in ~/snap. Reinstall the apps you need from Flathub.")
        .requires(&[FLATHUB])
        .step(Step::RemoveSnaps)
        .step(Step::remove(&["snapd"]))
        .step(Step::write_file(
            "/etc/apt/preferences.d/no-snap.pref",
            "# Bootcraft: keep snapd from being reinstalled.\nPackage: snapd\nPin: release a=*\nPin-Priority: -1\n",
        ))
        .step(Step::write_file("/etc/apt/keyrings/packages.mozilla.org.asc", MOZILLA_KEY))
        .step(Step::write_file(
            "/etc/apt/sources.list.d/mozilla.sources",
            "Types: deb\nURIs: https://packages.mozilla.org/apt\nSuites: mozilla\nComponents: main\nSigned-By: /etc/apt/keyrings/packages.mozilla.org.asc\n",
        ))
        // Mozilla's firefox over Ubuntu's, which only installs the snap.
        .step(Step::write_file(
            "/etc/apt/preferences.d/mozilla.pref",
            "Package: *\nPin: origin packages.mozilla.org\nPin-Priority: 1000\n",
        ))
        .step(apt(&["update"]))
        .step(apt(&["--allow-downgrades", "install", "firefox"]));
    if let Some(store) = store {
        task = task.step(Step::install(&store));
    }
    task
}

fn nvidia(sys: &System, generation: NvidiaGen) -> Task {
    let (title, description, driver) = match generation {
        NvidiaGen::Legacy580 => (
            "NVIDIA 580 legacy driver",
            "The newest driver ubuntu-drivers offers up to the 580 branch, the last for Maxwell, Pascal and Volta.",
            Step::UbuntuDriver { open: false, max_branch: Some(580) },
        ),
        _ => (
            "NVIDIA driver (open kernel modules)",
            "The newest open-module driver ubuntu-drivers offers, which NVIDIA recommends for Turing (GTX 16 / RTX 20) and newer.",
            Step::UbuntuDriver { open: true, max_branch: None },
        ),
    };
    let mut task = Task::new("gpu.nvidia", Category::Drivers, title)
        .recommended()
        .reboot()
        .description(description)
        .step(Step::install(&["ubuntu-drivers-common"]))
        .step(driver);
    if sys.secure_boot == SecureBoot::Enabled {
        task = task.note(
            "Secure Boot is on: if the driver doesn't load after restarting, run sudo update-secureboot-policy --enroll-key and confirm the key in the blue screen at the next boot.",
        );
    }
    task
}

fn codecs(mint: bool) -> Task {
    let task = Task::new("media.codecs", Category::Multimedia, "Media codecs and thumbnails")
        .recommended()
        .description("The restricted codecs (H.264, H.265, AAC, MP3), VA-API decoding, and video thumbnails in file managers.")
        .note("Includes Microsoft's core fonts; installing them means accepting Microsoft's font license.");
    let task = common::mscorefonts_license().into_iter().fold(task, Task::step);
    if mint {
        task.step(Step::install(&["mint-meta-codecs", "gstreamer1.0-vaapi", "ffmpegthumbnailer"]))
    } else {
        task.requires(&[MULTIVERSE]).step(Step::install(&[
            "ubuntu-restricted-extras",
            "gstreamer1.0-vaapi",
            "ffmpegthumbnailer",
        ]))
    }
}

fn archive(mint: bool) -> Task {
    let task = common::archive(&["7zip", "unrar", "unzip", "zip"]);
    if mint { task } else { task.requires(&[MULTIVERSE]) }
}

/// Mint's own recommendation for desktops with plenty of RAM.
fn swappiness() -> Task {
    Task::new("mint.swappiness", Category::System, "Prefer RAM over swap")
        .description("Sets vm.swappiness to 10, so the system swaps only when memory runs low.")
        .step(Step::write_file("/etc/sysctl.d/99-bootcraft-swappiness.conf", "vm.swappiness=10\n"))
        .step(Step::run(&["sysctl", "-w", "vm.swappiness=10"]))
}
