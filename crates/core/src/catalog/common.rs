//! Tasks Fedora and the Debian family offer in the same shape; only package
//! names differ. (Arch has its own, which also require a full upgrade first.)

use crate::catalog::{FLATHUB, FLATHUB_URL};
use crate::system::Desktop;
use crate::task::{APT_GET, Category, Step, Task};

pub const APT_I386: &str = "apt.i386";

/// `apt-get` with Bootcraft's non-interactive options, then `args`.
pub fn apt(args: &[&str]) -> Step {
    Step::Run { argv: APT_GET.iter().chain(args).map(|a| (*a).to_owned()).collect(), unless: Vec::new() }
}

pub fn apt_update() -> Task {
    Task::new(crate::catalog::UPDATE, Category::System, "Update the system")
        .recommended()
        .description("Refresh the package lists and upgrade every package.")
        .step(apt(&["update"]))
        .step(apt(&["full-upgrade"]))
}

/// Steam needs 32-bit libraries.
pub fn apt_i386() -> Task {
    Task::new(APT_I386, Category::System, "32-bit packages")
        .description("Lets 32-bit libraries be installed alongside 64-bit ones, which Steam and Wine need.")
        .step(Step::ForeignArch { arch: "i386".into() })
}

/// Flatpak, Flathub, and the plugin that lets the desktop's software store install Flatpaks.
pub fn flathub(desktop: &Desktop, gnome_plugin: &str, kde_plugin: &str) -> Task {
    let mut packages = vec!["flatpak"];
    match desktop {
        Desktop::Gnome if !gnome_plugin.is_empty() => packages.push(gnome_plugin),
        Desktop::Kde if !kde_plugin.is_empty() => packages.push(kde_plugin),
        _ => {}
    }
    Task::new(FLATHUB, Category::Apps, "Flatpak with Flathub")
        .recommended()
        .step(Step::install(&packages))
        .step(Step::FlatpakRemote { name: "flathub".into(), url: FLATHUB_URL.into() })
}

pub fn archive(packages: &[&str]) -> Task {
    Task::new("tools.archive", Category::Tools, "Archive tools")
        .recommended()
        .description("zip, 7-Zip and RAR support.")
        .step(Step::install(packages))
}

pub fn drives(packages: &[&str]) -> Task {
    Task::new("tools.drives", Category::Tools, "NTFS and exFAT drives")
        .recommended()
        .description("Read and write Windows-formatted disks and SD cards.")
        .step(Step::install(packages))
}

pub fn fonts(packages: &[&str]) -> Task {
    Task::new("system.fonts", Category::System, "Fonts for every language and emoji")
        .recommended()
        .description(
            "Noto fonts, including Chinese, Japanese, Korean and color emoji, so no text shows as boxes.",
        )
        .step(Step::install(packages))
}

pub fn firmware() -> Task {
    Task::new("system.firmware", Category::System, "Firmware updates")
        .recommended()
        .description("fwupd, which updates device firmware from the Linux Vendor Firmware Service.")
        .step(Step::install(&["fwupd"]))
}

/// Debian's installer for Microsoft's fonts asks to accept their license;
/// answer it ahead, since Bootcraft runs without a terminal. Ticking the
/// task is accepting it. Not needed once the installer is in.
pub fn mscorefonts_license() -> Vec<Step> {
    const PRESEED: &str = "/var/cache/bootcraft/mscorefonts.debconf";
    vec![
        Step::write_file(
            PRESEED,
            "ttf-mscorefonts-installer msttcorefonts/accepted-mscorefonts-eula boolean true\n",
        ),
        Step::run_unless(&["ttf-mscorefonts-installer"], &["debconf-set-selections", PRESEED]),
    ]
}

pub fn ms_fonts_apt() -> Task {
    let mut task = Task::new("system.ms-fonts", Category::System, "Microsoft core fonts")
        .description("Arial, Times New Roman and friends, for documents made on Windows.")
        .note("Installing them means accepting Microsoft's font license.");
    for step in mscorefonts_license() {
        task = task.step(step);
    }
    task.step(Step::install(&["ttf-mscorefonts-installer"]))
}

pub fn firewall_ufw(ssh_server: bool) -> Task {
    let mut task = Task::new("system.firewall", Category::System, "Firewall (ufw)")
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

pub fn debloat_gnome(packages: &[&str]) -> Task {
    Task::new("debloat.gnome", Category::Debloat, "Remove GNOME extras")
        .description("Maps, Snapshot, Document Scanner, the video player, htop and vim.")
        .step(Step::remove(packages))
}

pub fn debloat_kde(packages: &[&str]) -> Task {
    Task::new("debloat.kde", Category::Debloat, "Remove KDE extras")
        .description("Games, KolourPaint, Skanpage, Akregator, KMail, remote-desktop tools, NeoChat, Dragon, Elisa, Kamoso and Qrca.")
        .step(Step::remove(packages))
}

pub fn debloat_libreoffice() -> Task {
    Task::new("debloat.libreoffice", Category::Debloat, "Remove LibreOffice")
        .step(Step::remove(&["libreoffice*"]))
}

pub const KDE_EXTRAS: &[&str] = &[
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
    "dragonplayer",
    "dragon",
    "elisa",
    "elisa-player",
    "kamoso",
    "qrca",
];
