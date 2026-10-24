use crate::catalog::{ARCH_BUILD_TOOLS as BUILD_TOOLS, FLATHUB, FLATHUB_URL, UPDATE};
use crate::edit::Edit;
use crate::system::{Desktop, NvidiaGen, SecureBoot, System};
use crate::task::{Category, Step, Task};

const MULTILIB: &str = "arch.multilib";

/// Kernels in the official repositories, whose `-headers` packages Bootcraft can install.
const OFFICIAL_KERNELS: &[&str] =
    &["linux", "linux-lts", "linux-zen", "linux-hardened", "linux-rt", "linux-rt-lts"];

pub fn tasks(sys: &System) -> Vec<Task> {
    let mut tasks = vec![pacman_tweaks(), multilib(), update(), build_tools()];
    if let Some(generation) = sys.nvidia {
        tasks.extend(nvidia(sys, generation));
        if generation != NvidiaGen::Unsupported {
            tasks.push(nvidia_vaapi());
        }
    }
    tasks.extend([paru(), flathub(), codecs(), archive(), drives(), fonts(), ms_fonts(), firmware()]);
    if sys.desktop == Desktop::Cinnamon {
        tasks.push(cinnamon_mint());
    }
    tasks.push(firewall(sys.ssh_server));
    match sys.desktop {
        Desktop::Gnome => tasks.push(debloat_gnome()),
        Desktop::Kde => tasks.push(debloat_kde()),
        _ => {}
    }
    tasks.push(debloat_libreoffice());
    tasks
}

/// Installing from the repos on Arch is only safe right after a full sync and upgrade.
fn package_task(id: &'static str, category: Category, title: &str) -> Task {
    Task::new(id, category, title).requires(&[UPDATE])
}

fn pacman_tweaks() -> Task {
    Task::new("arch.pacman-tweaks", Category::System, "Colorful pacman output")
        .recommended()
        .description("Turn on pacman's Color and ILoveCandy options.")
        .step(Step::edit("/etc/pacman.conf", Edit::flag("options", "Color")))
        .step(Step::edit("/etc/pacman.conf", Edit::flag("options", "ILoveCandy")))
}

fn multilib() -> Task {
    Task::new(MULTILIB, Category::System, "Enable the multilib repository")
        .recommended()
        .description("32-bit libraries, needed by Steam, Wine and the 32-bit NVIDIA userspace.")
        .requires(&[UPDATE])
        .step(Step::edit("/etc/pacman.conf", Edit::pacman_repo("multilib")))
}

fn update() -> Task {
    Task::new(UPDATE, Category::System, "Update the system")
        .recommended()
        .description("Sync the package databases and upgrade every package.")
        .step(Step::run(&["pacman", "-Syu", "--noconfirm"]))
}

fn build_tools() -> Task {
    package_task(BUILD_TOOLS, Category::Tools, "Build tools (base-devel, git)")
        .recommended()
        .description("Needed to build packages from the AUR.")
        .step(Step::install(&["base-devel", "git"]))
}

fn nvidia(sys: &System, generation: NvidiaGen) -> Option<Task> {
    let kernels: Vec<&str> = sys.arch_kernels.iter().map(String::as_str).collect();
    let (official, custom): (Vec<&str>, Vec<&str>) =
        kernels.iter().partition(|k| OFFICIAL_KERNELS.contains(k));
    let headers: Vec<String> = official.iter().map(|k| format!("{k}-headers")).collect();

    let mut task = match generation {
        NvidiaGen::Unsupported => return None,
        NvidiaGen::Modern => {
            // Prebuilt modules exist only for the two main kernels; anything else needs DKMS.
            let prebuilt = !kernels.is_empty() && kernels.iter().all(|k| matches!(*k, "linux" | "linux-lts"));
            let modules: Vec<String> = if prebuilt {
                kernels
                    .iter()
                    .map(|k| if *k == "linux" { "nvidia-open" } else { "nvidia-open-lts" }.to_owned())
                    .collect()
            } else {
                std::iter::once("nvidia-open-dkms".to_owned()).chain(headers).collect()
            };
            package_task("gpu.nvidia", Category::Drivers, "NVIDIA driver (open kernel modules)")
                .description(
                    "NVIDIA's open kernel modules, which NVIDIA recommends for Turing (GTX 16 / RTX 20) and newer.",
                )
                // Every NVIDIA kernel-module package (official, DKMS, 580xx, custom
                // builds like -tkg) provides NVIDIA-MODULE; keep whichever is there.
                .step(Step::Install { packages: modules, unless: vec!["NVIDIA-MODULE".into()] })
                .step(Step::install(&["nvidia-utils", "lib32-nvidia-utils"]))
        }
        NvidiaGen::Legacy580 => {
            let mut task =
                package_task("gpu.nvidia", Category::Drivers, "NVIDIA 580 legacy driver (AUR)")
                    .description(
                        "Maxwell, Pascal and Volta cards were dropped from the main driver in 590; Arch moved them to nvidia-580xx in the AUR.",
                    )
                    .note("The 580 branch gets security fixes only.")
                    .requires(&[BUILD_TOOLS])
                    // The 580xx packages conflict with the current driver, which a
                    // GTX 10xx may have been upgraded onto.
                    .step(Step::remove(&[
                        "nvidia-open",
                        "nvidia-open-dkms",
                        "nvidia-open-lts",
                        "nvidia-utils",
                        "lib32-nvidia-utils",
                    ]));
            if !headers.is_empty() {
                task = task.step(Step::Install { packages: headers, unless: Vec::new() });
            }
            task.step(Step::aur(&["nvidia-580xx-utils", "lib32-nvidia-580xx-utils", "nvidia-580xx-dkms"]))
        }
    };
    task = task.recommended().reboot().requires(&[MULTILIB]);
    if !custom.is_empty() {
        task = task.note(format!(
            "DKMS also builds the module for {}, which needs the matching -headers package installed.",
            custom.join(", ")
        ));
    }
    if sys.secure_boot == SecureBoot::Enabled {
        task = task.note(
            "Secure Boot is on. Arch does not sign kernel modules, so the driver will not load until you sign it (for example with sbctl) or turn Secure Boot off.",
        );
    }
    Some(task)
}

fn nvidia_vaapi() -> Task {
    package_task("gpu.nvidia-vaapi", Category::Multimedia, "Hardware video decoding on NVIDIA")
        .recommended()
        .description("VA-API driver so browsers and players can decode video on the GPU.")
        .step(Step::install(&["libva-nvidia-driver"]))
}

fn paru() -> Task {
    Task::new("arch.paru", Category::Tools, "AUR helper (paru)")
        .recommended()
        .description("Install paru (prebuilt) unless you already use an AUR helper, and list search results bottom-up.")
        .requires(&[BUILD_TOOLS])
        .step(Step::aur_unless(&["paru", "yay", "pikaur"], &["paru-bin"]))
        .step(Step::edit_existing("/etc/paru.conf", Edit::flag("options", "BottomUp")))
}

fn flathub() -> Task {
    package_task(FLATHUB, Category::Apps, "Flatpak with Flathub")
        .recommended()
        .step(Step::install(&["flatpak"]))
        .step(Step::FlatpakRemote { name: "flathub".into(), url: FLATHUB_URL.into() })
}

fn codecs() -> Task {
    package_task("media.codecs", Category::Multimedia, "Media codecs and thumbnails")
        .recommended()
        .description("GStreamer plugins for common audio/video formats, VA-API decoding, and video thumbnails in file managers.")
        .step(Step::install(&[
            "gst-libav",
            "gst-plugins-base",
            "gst-plugins-good",
            "gst-plugins-bad",
            "gst-plugins-ugly",
            "gst-plugin-va",
            "ffmpegthumbnailer",
        ]))
}

fn archive() -> Task {
    package_task("tools.archive", Category::Tools, "Archive tools")
        .recommended()
        .description("zip, 7-Zip and RAR support.")
        .step(Step::install(&["tar", "gzip", "zip", "unzip", "7zip", "unrar"]))
}

fn drives() -> Task {
    package_task("tools.drives", Category::Tools, "NTFS and exFAT drives")
        .recommended()
        .description("Read and write Windows-formatted disks and SD cards.")
        .step(Step::install(&["ntfs-3g", "exfatprogs"]))
}

fn fonts() -> Task {
    package_task("system.fonts", Category::System, "Fonts for every language and emoji")
        .recommended()
        .description(
            "Noto fonts, including Chinese, Japanese, Korean and color emoji, so no text shows as boxes.",
        )
        .step(Step::install(&["noto-fonts", "noto-fonts-cjk", "noto-fonts-emoji"]))
}

fn ms_fonts() -> Task {
    Task::new("system.ms-fonts", Category::System, "Microsoft core fonts")
        .description("Arial, Times New Roman and friends, for documents made on Windows.")
        .note("Built from the AUR; installing them means accepting Microsoft's font license.")
        .requires(&[UPDATE, BUILD_TOOLS])
        .step(Step::aur(&["ttf-ms-fonts"]))
}

fn firmware() -> Task {
    package_task("system.firmware", Category::System, "Firmware updates")
        .recommended()
        .description("fwupd, which updates device firmware from the Linux Vendor Firmware Service.")
        .step(Step::install(&["fwupd"]))
}

fn cinnamon_mint() -> Task {
    package_task("desktop.cinnamon-mint", Category::Desktop, "Linux Mint look for Cinnamon")
        .description("Mint's themes, icons, sounds and login screen, plus the usual Cinnamon desktop apps.")
        .note("mint-artwork and its theme dependencies are large AUR builds and take a while.")
        .requires(&[BUILD_TOOLS])
        .step(Step::install(&[
            "ristretto",
            "papers",
            "gedit",
            "gnome-calculator",
            "papirus-icon-theme",
            "file-roller",
            "nemo-fileroller",
        ]))
        // nemo-fileroller replaces engrampa as the archive handler.
        .step(Step::remove(&["engrampa"]))
        .step(Step::aur(&["mint-artwork"]))
        .step(Step::edit_existing(
            "/etc/lightdm/lightdm.conf",
            Edit::set_key("Seat:*", "greeter-session", "lightdm-slick-greeter"),
        ))
}

fn firewall(ssh_server: bool) -> Task {
    let mut task = package_task("system.firewall", Category::System, "Firewall (ufw)")
        .description(if ssh_server {
            "Block incoming connections except SSH; allow outgoing."
        } else {
            "Block incoming connections; allow outgoing."
        })
        .step(Step::install(&["ufw"]))
        .step(Step::run(&["ufw", "default", "deny", "incoming"]))
        .step(Step::run(&["ufw", "default", "allow", "outgoing"]));
    if ssh_server {
        task = task.step(Step::run(&["ufw", "allow", "22/tcp"]));
    }
    task.step(Step::run(&["ufw", "--force", "enable"]))
        .step(Step::EnableService { unit: "ufw.service".into() })
}

fn debloat_gnome() -> Task {
    Task::new("debloat.gnome", Category::Debloat, "Remove GNOME extras")
        .description("Web, Maps, Snapshot, Document Scanner, Showtime, htop and vim.")
        .step(Step::remove(&["epiphany", "gnome-maps", "snapshot", "simple-scan", "showtime", "htop", "vim"]))
}

fn debloat_kde() -> Task {
    Task::new("debloat.kde", Category::Debloat, "Remove KDE extras")
        .description("Games, KolourPaint, Skanpage, Akregator, KMail, remote-desktop tools, NeoChat, Dragon, Elisa, Kamoso and Qrca.")
        .step(Step::remove(&[
            "kmahjongg",
            "kmines",
            "kpat",
            "kolourpaint",
            "skanpage",
            "akregator",
            "kmail",
            "krdc",
            "krdp",
            "neochat",
            "krfb",
            "ktnef",
            "dragon",
            "elisa",
            "kamoso",
            "qrca",
        ]))
}

fn debloat_libreoffice() -> Task {
    Task::new("debloat.libreoffice", Category::Debloat, "Remove LibreOffice")
        .step(Step::remove(&["libreoffice*"]))
}
