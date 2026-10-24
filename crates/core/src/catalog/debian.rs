//! Debian 13: contrib/non-free components, sudo for the user who installed
//! with a root password, and the NVIDIA driver from NVIDIA's repository
//! (current cards) or Debian's own 550 build (legacy cards).

use crate::catalog::common::{self, apt};
use crate::edit::Edit;
use crate::system::{Desktop, NvidiaGen, SecureBoot, System};
use crate::task::{Category, Step, Task};

pub const SOURCES: &str = "debian.sources";

/// NVIDIA's keyring package for its Debian 13 repository, pinned by checksum
/// (it installs the signing key ending 8793F200 and the repository).
const CUDA_KEYRING_URL: &str =
    "https://developer.download.nvidia.com/compute/cuda/repos/debian13/x86_64/cuda-keyring_1.1-1_all.deb";
const CUDA_KEYRING_SHA256: &str = "d0d4ef986a44400f9db33c600ef33a985175e7cc63d805a10e1839c7a1e78f5f";
const CUDA_KEYRING_FILE: &str = "/var/cache/bootcraft/downloads/cuda-keyring_1.1-1_all.deb";

pub fn tasks(sys: &System) -> Vec<Task> {
    let mut tasks = Vec::new();
    if let Some(user) = &sys.user {
        tasks.push(sudo(user));
    }
    tasks.extend([sources(), common::apt_update(), common::apt_i386().requires(&[SOURCES]), backports()]);
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
        Task::new("media.codecs", Category::Multimedia, "Media codecs and thumbnails")
            .recommended()
            .description("FFmpeg and GStreamer plugins for common audio/video formats, VA-API decoding, and video thumbnails in file managers.")
            .step(Step::install(&[
                "ffmpeg",
                "gstreamer1.0-libav",
                "gstreamer1.0-plugins-good",
                "gstreamer1.0-plugins-bad",
                "gstreamer1.0-plugins-ugly",
                "gstreamer1.0-vaapi",
                "ffmpegthumbnailer",
            ])),
        common::archive(&["7zip", "unrar", "unzip", "zip"]).requires(&[SOURCES]),
        common::drives(&["ntfs-3g", "exfatprogs"]),
        common::fonts(&["fonts-noto-core", "fonts-noto-cjk", "fonts-noto-color-emoji"]),
        common::ms_fonts_apt().requires(&[SOURCES]),
        common::firmware(),
        common::firewall_ufw(sys.ssh_server),
    ]);
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

/// Debian leaves the first user out of sudo when a root password was set.
fn sudo(user: &str) -> Task {
    Task::new("debian.sudo", Category::System, format!("Let {user} use sudo"))
        .recommended()
        .description("Debian doesn't set this up when a root password was chosen during installation.")
        .step(Step::Install { packages: vec!["sudo".into()], unless: Vec::new() })
        .step(Step::AddToGroup { user: user.into(), group: "sudo".into() })
}

/// contrib and non-free, for firmware, drivers, Steam and Microsoft's fonts.
/// New installs use debian.sources; older ones sources.list.
fn sources() -> Task {
    let components = Edit::apt_components(&["contrib", "non-free", "non-free-firmware"]);
    Task::new(SOURCES, Category::System, "Enable contrib and non-free")
        .recommended()
        .description(
            "Debian's repositories for firmware, the NVIDIA driver, Steam and other non-free software.",
        )
        .step(Step::edit_existing("/etc/apt/sources.list.d/debian.sources", components.clone()))
        .step(Step::edit_existing("/etc/apt/sources.list", components))
}

/// A newer kernel and firmware from trixie-backports, for very new hardware.
fn backports() -> Task {
    Task::new("debian.backports", Category::System, "Newer kernel from backports")
        .description("Linux and firmware from trixie-backports, for hardware newer than Debian 13.")
        .note("Backported kernels move faster; a DKMS driver may need a moment to catch up after an update.")
        .reboot()
        .requires(&[SOURCES])
        .step(Step::write_file(
            "/etc/apt/sources.list.d/trixie-backports.sources",
            "Types: deb\nURIs: http://deb.debian.org/debian\nSuites: trixie-backports\nComponents: main contrib non-free non-free-firmware\nSigned-By: /usr/share/keyrings/debian-archive-keyring.gpg\n",
        ))
        .step(apt(&["update"]))
        .step(apt(&["-t", "trixie-backports", "install", "linux-image-amd64", "linux-headers-amd64", "firmware-misc-nonfree"]))
}

fn nvidia(sys: &System, generation: NvidiaGen) -> Task {
    let mut task = match generation {
        // Debian's 550 still supports Maxwell, Pascal and Volta; NVIDIA's
        // Debian 13 repository starts at 590, without them.
        NvidiaGen::Legacy580 => Task::new("gpu.nvidia", Category::Drivers, "NVIDIA driver (Debian's 550)")
            .description("Debian's own NVIDIA driver, which still supports Maxwell, Pascal and Volta cards.")
            .step(Step::install(&["linux-headers-amd64", "nvidia-kernel-dkms", "nvidia-driver", "firmware-misc-nonfree"])),
        _ => Task::new("gpu.nvidia", Category::Drivers, "NVIDIA driver (open kernel modules)")
            .description("The current driver from NVIDIA's Debian 13 repository, with the open kernel modules for Turing (GTX 16 / RTX 20) and newer.")
            .step(Step::Download {
                url: CUDA_KEYRING_URL.into(),
                path: CUDA_KEYRING_FILE.into(),
                sha256: CUDA_KEYRING_SHA256.into(),
            })
            .step(Step::install_unless(&["cuda-keyring"], &[CUDA_KEYRING_FILE]))
            // The keyring package added a repository.
            .step(apt(&["update"]))
            .step(Step::install(&["linux-headers-amd64", "nvidia-open"])),
    };
    task = task.recommended().reboot().requires(&[SOURCES]);
    if sys.secure_boot == SecureBoot::Enabled {
        task = task.note(
            "Secure Boot is on: enroll DKMS's signing key before restarting (sudo mokutil --import /var/lib/dkms/mok.pub), or the driver won't load.",
        );
    }
    task
}
