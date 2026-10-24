//! Fedora: RPM Fusion for codecs and the NVIDIA driver, dnf for everything.

use crate::catalog::common;
use crate::catalog::{FEDORA_RPMFUSION, UPDATE};
use crate::edit::Edit;
use crate::system::{Desktop, GpuVendor, NvidiaGen, SecureBoot, System};
use crate::task::{Category, Step, Task};

pub fn tasks(sys: &System) -> Vec<Task> {
    let version = sys.os.version_id.clone().unwrap_or_default();
    let mut tasks = vec![dnf_tweaks(), update(), rpmfusion(&version)];
    if let Some(generation) = sys.nvidia
        && generation != NvidiaGen::Unsupported
    {
        tasks.push(nvidia(sys, generation));
        tasks.push(
            Task::new("gpu.nvidia-vaapi", Category::Multimedia, "Hardware video decoding on NVIDIA")
                .recommended()
                .description("VA-API driver so browsers and players can decode video on the GPU.")
                .step(Step::install(&["libva-nvidia-driver"])),
        );
    }
    let amd = sys.gpus.iter().any(|g| g.vendor == GpuVendor::Amd);
    let intel = sys.gpus.iter().any(|g| g.vendor == GpuVendor::Intel);
    if amd || intel {
        tasks.push(mesa_freeworld(amd, intel));
    }
    tasks.extend([
        common::flathub(&sys.desktop, "", ""),
        codecs(),
        common::archive(&["7zip", "unrar", "unzip", "zip"]).requires(&[FEDORA_RPMFUSION]),
        common::drives(&["ntfs-3g", "exfatprogs"]),
        common::fonts(&[
            "google-noto-sans-fonts",
            "google-noto-serif-fonts",
            "google-noto-sans-cjk-fonts",
            "google-noto-color-emoji-fonts",
        ]),
        common::firmware(),
    ]);
    match sys.desktop {
        Desktop::Gnome => tasks.push(common::debloat_gnome(&[
            "gnome-maps",
            "snapshot",
            "simple-scan",
            "showtime",
            "totem",
            "htop",
            "vim-enhanced",
        ])),
        Desktop::Kde => tasks.push(common::debloat_kde(common::KDE_EXTRAS)),
        _ => {}
    }
    tasks.push(common::debloat_libreoffice());
    tasks
}

fn dnf_tweaks() -> Task {
    Task::new("fedora.dnf-tweaks", Category::System, "Faster downloads in dnf")
        .recommended()
        .description("Download up to 10 packages at once.")
        .step(Step::edit("/etc/dnf/dnf.conf", Edit::set_key("main", "max_parallel_downloads", "10")))
}

fn update() -> Task {
    Task::new(UPDATE, Category::System, "Update the system")
        .recommended()
        .description("Refresh the repositories and upgrade every package.")
        .step(Step::run(&["dnf", "-y", "upgrade", "--refresh"]))
}

/// The free and nonfree repositories, from their release packages.
fn rpmfusion(version: &str) -> Task {
    let release = |kind: &str| {
        format!("https://mirrors.rpmfusion.org/{kind}/fedora/rpmfusion-{kind}-release-{version}.noarch.rpm")
    };
    let (free, nonfree) = (release("free"), release("nonfree"));
    Task::new(FEDORA_RPMFUSION, Category::System, "RPM Fusion repositories")
        .recommended()
        .description("Codecs, the NVIDIA driver, Steam and other software Fedora can't ship.")
        .step(Step::install_unless(&["rpmfusion-free-release"], &[free.as_str()]))
        .step(Step::install_unless(&["rpmfusion-nonfree-release"], &[nonfree.as_str()]))
}

fn nvidia(sys: &System, generation: NvidiaGen) -> Task {
    let mut task = match generation {
        NvidiaGen::Legacy580 => Task::new("gpu.nvidia", Category::Drivers, "NVIDIA 580 legacy driver")
            .description("RPM Fusion's 580 branch, the last for Maxwell, Pascal and Volta cards.")
            .note("The 580 branch gets security fixes only.")
            .step(Step::install(&["akmod-nvidia-580xx", "xorg-x11-drv-nvidia-580xx", "xorg-x11-drv-nvidia-580xx-cuda"])),
        _ => Task::new("gpu.nvidia", Category::Drivers, "NVIDIA driver (open kernel modules)")
            .description("RPM Fusion's driver, with NVIDIA's open kernel modules for Turing (GTX 16 / RTX 20) and newer, and CUDA/NVENC support.")
            .step(Step::install(&["akmod-nvidia", "xorg-x11-drv-nvidia-cuda"])),
    };
    task = task
        .recommended()
        .reboot()
        .requires(&[FEDORA_RPMFUSION])
        .note("The kernel module is built in the background after installing; wait a few minutes before restarting.");
    if sys.secure_boot == SecureBoot::Enabled {
        task = task.note(
            "Secure Boot is on: enroll the akmods signing key before restarting (sudo mokutil --import /etc/pki/akmods/certs/public_key.der), or the driver won't load.",
        );
    }
    task
}

/// Fedora's Mesa can't decode H.264/H.265; RPM Fusion's build can.
fn mesa_freeworld(amd: bool, intel: bool) -> Task {
    let mut task =
        Task::new("gpu.mesa-freeworld", Category::Multimedia, "Hardware video decoding (AMD/Intel)")
            .recommended()
            .description("RPM Fusion's video drivers, which decode H.264 and H.265 on the GPU.")
            .requires(&[FEDORA_RPMFUSION]);
    if amd {
        task =
            task.step(Step::Swap { from: "mesa-va-drivers".into(), to: "mesa-va-drivers-freeworld".into() });
    }
    if intel {
        task = task.step(Step::install(&["intel-media-driver"]));
    }
    task
}

fn codecs() -> Task {
    Task::new("media.codecs", Category::Multimedia, "Media codecs and thumbnails")
        .recommended()
        .description("RPM Fusion's full FFmpeg in place of Fedora's patent-free build, GStreamer plugins for common formats, and video thumbnails in file managers.")
        .requires(&[FEDORA_RPMFUSION])
        .step(Step::Swap { from: "ffmpeg-free".into(), to: "ffmpeg".into() })
        .step(Step::install(&[
            "gstreamer1-plugins-bad-freeworld",
            "gstreamer1-plugins-ugly",
            "gstreamer1-plugin-libav",
            "gstreamer1-plugin-openh264",
            "ffmpegthumbnailer",
        ]))
}
