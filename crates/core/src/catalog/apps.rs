//! The curated app list and the workflow packs built from it.
//!
//! Sources were checked in October 2026 against Flathub (existence and
//! verification), Arch, Fedora 44, Debian 13 and Ubuntu 24.04/26.04. A native
//! package is listed only where the distribution's own repositories (or RPM
//! Fusion, or the AUR) have it on every supported release.

use crate::apps::{App, FlatpakRef, NativePkg, Policy, Section, Via};
use crate::catalog::{COMFYPILOT_MANIFEST, COMFYPILOT_PUBLIC_KEY};
use crate::system::{Distro, GpuVendor, System};

/// A workflow card. Packs stack on top of Essentials; `apps` lists
/// `(app id, ticked by default)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pack {
    pub id: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub apps: &'static [(&'static str, bool)],
}

impl Pack {
    pub fn ticked(&self) -> impl Iterator<Item = &'static str> {
        self.apps.iter().filter(|(_, ticked)| *ticked).map(|(id, _)| *id)
    }
}

pub const PACKS: &[Pack] = &[
    Pack { id: "essentials", title: "Essentials", subtitle: "A clean, everyday setup", apps: &[] },
    Pack {
        id: "creative",
        title: "Creative",
        subtitle: "Design, video & 3D",
        apps: &[
            ("blender", true),
            ("krita", true),
            ("gimp", true),
            ("inkscape", true),
            ("kdenlive", true),
            ("obs-studio", true),
            ("ffmpeg", true),
            ("darktable", false),
            ("digikam", false),
            ("shotcut", false),
            ("handbrake", false),
            ("audacity", false),
            ("ardour", false),
            ("davinci-resolve", false),
            ("synfig", false),
            ("glaxnimate", false),
            ("opentoonz", false),
            ("freecad", false),
            ("scribus", false),
            ("upscayl", false),
            ("video-trimmer", false),
            ("losslesscut", false),
            ("eyedropper", false),
            ("switcheroo", false),
        ],
    },
    Pack {
        id: "development",
        title: "Development",
        subtitle: "Build, test & ship",
        apps: &[
            ("vscodium", true),
            ("git", true),
            ("github-cli", true),
            ("cli-tools", true),
            ("podman", true),
            ("vscode", false),
            ("zed", false),
            ("neovim", false),
            ("jetbrains-toolbox", false),
            ("build-tools", false),
            ("rust", false),
            ("python", false),
            ("nodejs", false),
            ("go", false),
            ("docker", false),
            ("podman-desktop", false),
            ("boxbuddy", false),
            ("bruno", false),
            ("dbeaver", false),
            ("meld", false),
            ("android-studio", false),
            ("godot", false),
            ("kitty", false),
            ("ghostty", false),
            ("comfypilot", false),
            ("ollama", false),
            ("alpaca", false),
            ("cuda", false),
        ],
    },
    Pack {
        id: "gaming",
        title: "Gaming",
        subtitle: "Steam, launchers & tools",
        apps: &[
            ("steam", true),
            ("heroic", true),
            ("protonplus", true),
            ("mangohud", true),
            ("gamemode", true),
            ("lutris", false),
            ("bottles", false),
            ("faugus", false),
            ("discord", false),
            ("prismlauncher", false),
            ("retroarch", false),
            ("moonlight", false),
        ],
    },
];

pub fn pack(id: &str) -> Option<&'static Pack> {
    PACKS.iter().find(|p| p.id == id)
}

/// Distribution families as package names see them; Mint uses Ubuntu's
/// archive plus its own packages.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Platform {
    Arch,
    Fedora,
    Debian,
    Ubuntu,
    Mint,
}

struct Builder {
    app: App,
    platform: Option<Platform>,
}

fn app(
    sys: &System,
    id: &'static str,
    name: &'static str,
    summary: &'static str,
    section: Section,
) -> Builder {
    let platform = sys.distro().map(|d| match d {
        Distro::Arch => Platform::Arch,
        Distro::Fedora => Platform::Fedora,
        Distro::Debian => Platform::Debian,
        Distro::Ubuntu => Platform::Ubuntu,
        Distro::Mint => Platform::Mint,
    });
    Builder {
        app: App {
            id,
            name,
            summary,
            section,
            flatpak: None,
            native: None,
            policy: Policy::Either,
            notes: Vec::new(),
        },
        platform,
    }
}

impl Builder {
    /// Flathub app published by its own developers.
    fn flathub(mut self, id: &str) -> Self {
        self.app.flatpak = Some(FlatpakRef { id: id.into(), verified: true });
        self
    }

    /// Flathub app packaged by the community.
    fn flathub_community(mut self, id: &str) -> Self {
        self.app.flatpak = Some(FlatpakRef { id: id.into(), verified: false });
        self
    }

    /// Set the native package when this system is one of `platforms`.
    /// Later calls override earlier ones, so `.ubuntu(..).mint(..)` works.
    fn on(mut self, platforms: &[Platform], via: Via, packages: &[&str]) -> Self {
        if self.platform.is_some_and(|p| platforms.contains(&p)) {
            self.app.native = Some(NativePkg {
                packages: packages.iter().map(|p| (*p).to_owned()).collect(),
                via,
                services: Vec::new(),
                requires: Vec::new(),
            });
        }
        self
    }

    fn everywhere(self, packages: &[&str]) -> Self {
        use Platform::*;
        self.on(&[Arch, Fedora, Debian, Ubuntu, Mint], Via::Repo, packages)
    }
    fn arch(self, packages: &[&str]) -> Self {
        self.on(&[Platform::Arch], Via::Repo, packages)
    }
    fn aur(self, packages: &[&str]) -> Self {
        self.on(&[Platform::Arch], Via::Aur, packages)
    }
    fn fedora(self, packages: &[&str]) -> Self {
        self.on(&[Platform::Fedora], Via::Repo, packages)
    }
    fn rpmfusion(self, packages: &[&str]) -> Self {
        self.on(&[Platform::Fedora], Via::RpmFusion, packages)
    }
    fn debian(self, packages: &[&str]) -> Self {
        self.on(&[Platform::Debian], Via::Repo, packages)
    }
    /// Ubuntu and Mint.
    fn ubuntu(self, packages: &[&str]) -> Self {
        self.on(&[Platform::Ubuntu, Platform::Mint], Via::Repo, packages)
    }
    fn mint(self, packages: &[&str]) -> Self {
        self.on(&[Platform::Mint], Via::Repo, packages)
    }
    /// Debian, Ubuntu and Mint.
    fn deb(self, packages: &[&str]) -> Self {
        use Platform::*;
        self.on(&[Debian, Ubuntu, Mint], Via::Repo, packages)
    }

    fn signed_release(self, package: &str, manifest_url: &str, public_key: &str) -> Self {
        use Platform::*;
        let via = Via::SignedRelease { manifest_url: manifest_url.into(), public_key: public_key.into() };
        self.on(&[Arch, Fedora, Debian, Ubuntu, Mint], via, &[package])
    }

    /// Tasks the native package needs on Arch, e.g. multilib for `lib32-*`.
    fn arch_requires(mut self, tasks: &[&'static str]) -> Self {
        if self.platform == Some(Platform::Arch)
            && let Some(native) = &mut self.app.native
        {
            native.requires.extend_from_slice(tasks);
        }
        self
    }

    fn service(mut self, unit: &str) -> Self {
        if let Some(native) = &mut self.app.native {
            native.services.push(unit.into());
        }
        self
    }

    fn native_first(mut self) -> Self {
        self.app.policy = Policy::NativeFirst;
        self
    }

    fn flatpak_first(mut self) -> Self {
        self.app.policy = Policy::FlatpakFirst;
        self
    }

    fn note(mut self, text: &str) -> Self {
        self.app.notes.push(text.into());
        self
    }

    fn build(self) -> App {
        self.app
    }
}

/// Every app, with native packages resolved for `sys`. Apps with no source
/// on this system are included but unavailable (`App::available`).
#[allow(clippy::too_many_lines)] // One entry per app reads best as a flat list.
pub fn apps(sys: &System) -> Vec<App> {
    use Section::*;
    let nvidia = sys.nvidia.is_some();
    let amd = sys.gpus.iter().any(|g| g.vendor == GpuVendor::Amd);
    let multilib = &["arch.multilib"];

    vec![
        // Creative
        app(sys, "blender", "Blender", "3D creation", Graphics)
            .flathub_community("org.blender.Blender")
            .everywhere(&["blender"])
            .build(),
        app(sys, "krita", "Krita", "Painting & illustration", Graphics)
            .flathub("org.kde.krita")
            .everywhere(&["krita"])
            .build(),
        app(sys, "gimp", "GIMP", "Photo editing", Graphics).flathub("org.gimp.GIMP").everywhere(&["gimp"]).build(),
        app(sys, "inkscape", "Inkscape", "Vector graphics", Graphics)
            .flathub("org.inkscape.Inkscape")
            .everywhere(&["inkscape"])
            .build(),
        app(sys, "freecad", "FreeCAD", "3D CAD modelling", Graphics)
            .flathub("org.freecad.FreeCAD")
            .arch(&["freecad"])
            .deb(&["freecad"])
            .build(),
        app(sys, "scribus", "Scribus", "Desktop publishing", Graphics)
            .flathub_community("net.scribus.Scribus")
            .everywhere(&["scribus"])
            .build(),
        app(sys, "darktable", "darktable", "RAW photo editing", Photo)
            .flathub_community("org.darktable.Darktable")
            .everywhere(&["darktable"])
            .build(),
        app(sys, "digikam", "digiKam", "Photo library", Photo)
            .flathub("org.kde.digikam")
            .everywhere(&["digikam"])
            .build(),
        app(sys, "kdenlive", "Kdenlive", "Video editing", VideoAudio)
            .flathub("org.kde.kdenlive")
            .everywhere(&["kdenlive"])
            .build(),
        app(sys, "shotcut", "Shotcut", "Simple video editing", VideoAudio)
            .flathub("org.shotcut.Shotcut")
            .everywhere(&["shotcut"])
            .build(),
        app(sys, "davinci-resolve", "DaVinci Resolve", "Professional editing & color", VideoAudio)
            .aur(&["davinci-resolve"])
            .note("Proprietary. The free version can't decode H.264/H.265 on Linux and needs a working GPU driver.")
            .build(),
        app(sys, "obs-studio", "OBS Studio", "Recording & streaming", VideoAudio)
            .flathub("com.obsproject.Studio")
            .everywhere(&["obs-studio"])
            .build(),
        app(sys, "ffmpeg", "FFmpeg", "Media tools", VideoAudio)
            .arch(&["ffmpeg"])
            .rpmfusion(&["ffmpeg"])
            .deb(&["ffmpeg"])
            .build(),
        app(sys, "handbrake", "HandBrake", "Video transcoding", VideoAudio)
            .flathub("fr.handbrake.ghb")
            .arch(&["handbrake"])
            .rpmfusion(&["HandBrake-gui"])
            .deb(&["handbrake"])
            .build(),
        app(sys, "audacity", "Audacity", "Audio editing", VideoAudio)
            .flathub_community("org.audacityteam.Audacity")
            .everywhere(&["audacity"])
            .build(),
        app(sys, "ardour", "Ardour", "Digital audio workstation", VideoAudio)
            .flathub_community("org.ardour.Ardour")
            .arch(&["ardour"])
            .fedora(&["ardour9"])
            .deb(&["ardour"])
            .build(),
        app(sys, "synfig", "Synfig Studio", "2D animation", Animation).flathub("org.synfig.SynfigStudio").build(),
        app(sys, "glaxnimate", "Glaxnimate", "Motion graphics", Animation)
            .flathub("org.kde.glaxnimate")
            .arch(&["glaxnimate"])
            .fedora(&["glaxnimate"])
            .build(),
        app(sys, "opentoonz", "OpenToonz", "Cel animation", Animation)
            .flathub_community("io.github.OpenToonz")
            .arch(&["opentoonz"])
            .fedora(&["opentoonz"])
            .build(),
        app(sys, "upscayl", "Upscayl", "AI image upscaling", CreativeUtilities).flathub("org.upscayl.Upscayl").build(),
        app(sys, "video-trimmer", "Video Trimmer", "Quick video trimming", CreativeUtilities)
            .flathub("org.gnome.gitlab.YaLTeR.VideoTrimmer")
            .build(),
        app(sys, "losslesscut", "LosslessCut", "Lossless video cutting", CreativeUtilities)
            .flathub_community("no.mifi.losslesscut")
            .build(),
        app(sys, "eyedropper", "Eyedropper", "Color picker", CreativeUtilities)
            .flathub("com.github.finefindus.eyedropper")
            .build(),
        app(sys, "switcheroo", "Switcheroo", "Image converter", CreativeUtilities)
            .flathub("io.gitlab.adhami3310.Converter")
            .build(),
        // Development
        app(sys, "vscodium", "VSCodium", "Code editor", Editors)
            .flathub("com.vscodium.codium")
            .aur(&["vscodium-bin"])
            .native_first()
            .note("VS Code without Microsoft's telemetry, using the Open VSX extension registry.")
            .build(),
        app(sys, "vscode", "Visual Studio Code", "Code editor by Microsoft", Editors)
            .flathub_community("com.visualstudio.code")
            .aur(&["visual-studio-code-bin"])
            .native_first()
            .note("Microsoft's build with the official extension marketplace. The Flatpak is unofficial and can't see host SDKs or Docker without workarounds.")
            .build(),
        app(sys, "zed", "Zed", "Fast collaborative editor", Editors)
            .flathub_community("dev.zed.Zed")
            .arch(&["zed"])
            .native_first()
            .build(),
        app(sys, "neovim", "Neovim", "Terminal editor", Editors).everywhere(&["neovim"]).build(),
        app(sys, "jetbrains-toolbox", "JetBrains Toolbox", "Installs IntelliJ, PyCharm and more", Editors)
            .aur(&["jetbrains-toolbox"])
            .build(),
        app(sys, "git", "Git", "Version control, with Git LFS", DevTools).everywhere(&["git", "git-lfs"]).build(),
        app(sys, "github-cli", "GitHub CLI", "GitHub from the terminal", DevTools)
            .arch(&["github-cli"])
            .fedora(&["gh"])
            .deb(&["gh"])
            .build(),
        app(sys, "cli-tools", "Modern CLI tools", "ripgrep, fd, bat, fzf, eza, zoxide, jq, btop, tmux", DevTools)
            .arch(&["ripgrep", "fd", "bat", "fzf", "eza", "zoxide", "jq", "btop", "tmux", "lazygit"])
            .fedora(&["ripgrep", "fd-find", "bat", "fzf", "eza", "zoxide", "jq", "btop", "tmux"])
            .debian(&["ripgrep", "fd-find", "bat", "fzf", "eza", "zoxide", "jq", "btop", "tmux", "lazygit"])
            .ubuntu(&["ripgrep", "fd-find", "bat", "fzf", "eza", "zoxide", "jq", "btop", "tmux"])
            .build(),
        app(sys, "bruno", "Bruno", "API client", DevTools)
            .flathub("com.usebruno.Bruno")
            .aur(&["bruno-bin"])
            .build(),
        app(sys, "dbeaver", "DBeaver", "Database tool", DevTools)
            .flathub_community("io.dbeaver.DBeaverCommunity")
            .arch(&["dbeaver"])
            .build(),
        app(sys, "meld", "Meld", "Diff & merge", DevTools)
            .flathub_community("org.gnome.meld")
            .everywhere(&["meld"])
            .build(),
        app(sys, "android-studio", "Android Studio", "Android development", DevTools)
            .flathub_community("com.google.AndroidStudio")
            .aur(&["android-studio"])
            .build(),
        app(sys, "godot", "Godot", "Game engine", DevTools)
            .flathub_community("org.godotengine.Godot")
            .arch(&["godot"])
            .fedora(&["godot"])
            .build(),
        app(sys, "kitty", "kitty", "GPU-accelerated terminal", DevTools).everywhere(&["kitty"]).build(),
        app(sys, "ghostty", "Ghostty", "Fast native terminal", DevTools).arch(&["ghostty"]).build(),
        app(sys, "podman", "Podman & Distrobox", "Rootless containers and dev boxes", Containers)
            .everywhere(&["podman", "distrobox"])
            .build(),
        app(sys, "docker", "Docker Engine", "Docker containers", Containers)
            .arch(&["docker"])
            .fedora(&["moby-engine"])
            .deb(&["docker.io"])
            .service("docker.socket")
            .note("Members of the docker group effectively have root access, so Bootcraft doesn't add you to it.")
            .build(),
        app(sys, "podman-desktop", "Podman Desktop", "Container GUI", Containers)
            .flathub("io.podman_desktop.PodmanDesktop")
            .arch(&["podman-desktop"])
            .build(),
        app(sys, "boxbuddy", "BoxBuddy", "Distrobox GUI", Containers).flathub("io.github.dvlv.boxbuddyrs").build(),
        app(sys, "build-tools", "C/C++ build tools", "Compilers, make and friends", Languages)
            .arch(&["base-devel"])
            .fedora(&["@development-tools"])
            .deb(&["build-essential"])
            .build(),
        app(sys, "rust", "Rust", "rustup toolchain manager", Languages)
            .everywhere(&["rustup"])
            .note("Run `rustup default stable` once to install a toolchain.")
            .build(),
        app(sys, "python", "Python", "Python 3, with uv where packaged", Languages)
            .arch(&["python", "uv"])
            .fedora(&["python3", "uv"])
            .deb(&["python3", "python3-venv", "python3-pip"])
            .build(),
        app(sys, "nodejs", "Node.js", "JavaScript runtime and npm", Languages)
            .arch(&["nodejs", "npm"])
            .deb(&["nodejs", "npm"])
            .build(),
        app(sys, "go", "Go", "Go toolchain", Languages)
            .arch(&["go"])
            .fedora(&["golang"])
            .deb(&["golang-go"])
            .build(),
        app(sys, "comfypilot", "ComfyPilot", "Install and run ComfyUI the easy way", Ai)
            .signed_release("comfypilot", COMFYPILOT_MANIFEST, COMFYPILOT_PUBLIC_KEY)
            .note("By Arctic Latent. Installed from its signed GitHub release; it updates itself.")
            .build(),
        app(sys, "ollama", "Ollama", "Run local language models", Ai)
            .arch(&[if nvidia {
                "ollama-cuda"
            } else if amd {
                "ollama-rocm"
            } else {
                "ollama"
            }])
            .fedora(&["ollama"])
            .service("ollama.service")
            .build(),
        app(sys, "alpaca", "Alpaca", "Chat with local models", Ai).flathub("com.jeffser.Alpaca").build(),
        {
            let cuda = app(sys, "cuda", "CUDA toolkit", "NVIDIA GPU computing", Ai);
            if nvidia { cuda.arch(&["cuda"]) } else { cuda }.build()
        },
        // Gaming
        app(sys, "steam", "Steam", "Valve's game store and Proton", Gaming)
            .flathub_community("com.valvesoftware.Steam")
            .arch(&["steam"])
            .arch_requires(multilib)
            .rpmfusion(&["steam"])
            .deb(&["steam-installer"])
            .native_first()
            .build(),
        app(sys, "heroic", "Heroic", "Epic, GOG and Amazon games", Gaming)
            .flathub("com.heroicgameslauncher.hgl")
            .aur(&["heroic-games-launcher-bin"])
            .build(),
        app(sys, "protonplus", "ProtonPlus", "Manage Proton-GE and Wine versions", Gaming)
            .flathub("com.vysp3r.ProtonPlus")
            .aur(&["protonplus"])
            .build(),
        app(sys, "mangohud", "MangoHud", "In-game performance overlay", Gaming)
            .arch(&["mangohud", "lib32-mangohud"])
            .arch_requires(multilib)
            .fedora(&["mangohud"])
            .deb(&["mangohud"])
            .note("For Flatpak games, also install the MangoHud Vulkan layer from Flathub.")
            .build(),
        app(sys, "gamemode", "GameMode", "Performance tweaks while gaming", Gaming)
            .arch(&["gamemode", "lib32-gamemode"])
            .arch_requires(multilib)
            .fedora(&["gamemode"])
            .deb(&["gamemode"])
            .build(),
        app(sys, "lutris", "Lutris", "Game manager for every platform", Gaming)
            .flathub("net.lutris.Lutris")
            .everywhere(&["lutris"])
            .build(),
        app(sys, "bottles", "Bottles", "Run Windows software", Gaming)
            .flathub("com.usebottles.bottles")
            .aur(&["bottles"])
            .fedora(&["bottles"])
            .flatpak_first()
            .build(),
        app(sys, "faugus", "Faugus Launcher", "Simple launcher for Windows games", Gaming)
            .flathub("io.github.Faugus.faugus-launcher")
            .aur(&["faugus-launcher"])
            .build(),
        app(sys, "discord", "Discord", "Voice & chat", Gaming)
            .flathub("com.discordapp.Discord")
            .arch(&["discord"])
            .rpmfusion(&["discord"])
            .build(),
        app(sys, "prismlauncher", "Prism Launcher", "Minecraft launcher", Gaming)
            .flathub("org.prismlauncher.PrismLauncher")
            .arch(&["prismlauncher"])
            .build(),
        app(sys, "retroarch", "RetroArch", "Emulator frontend", Gaming)
            .flathub("org.libretro.RetroArch")
            .everywhere(&["retroarch"])
            .build(),
        app(sys, "moonlight", "Moonlight", "Game streaming client", Gaming)
            .flathub("com.moonlight_stream.Moonlight")
            .build(),
        // Everyday: not in any pack; offered through "Add applications".
        app(sys, "firefox", "Firefox", "Web browser", Everyday)
            .flathub("org.mozilla.firefox")
            .arch(&["firefox"])
            .fedora(&["firefox"])
            .debian(&["firefox-esr"])
            .mint(&["firefox"])
            .note("On Ubuntu the firefox package only installs the Snap, so Bootcraft uses Flathub.")
            .build(),
        app(sys, "thunderbird", "Thunderbird", "Email & calendar", Everyday)
            .flathub("org.mozilla.Thunderbird")
            .arch(&["thunderbird"])
            .fedora(&["thunderbird"])
            .debian(&["thunderbird"])
            .mint(&["thunderbird"])
            .build(),
        app(sys, "libreoffice", "LibreOffice", "Office suite", Everyday)
            .flathub("org.libreoffice.LibreOffice")
            .arch(&["libreoffice-fresh"])
            .fedora(&["libreoffice"])
            .deb(&["libreoffice"])
            .build(),
        app(sys, "onlyoffice", "ONLYOFFICE", "Office suite, Microsoft-compatible", Everyday)
            .flathub("org.onlyoffice.desktopeditors")
            .build(),
        app(sys, "keepassxc", "KeePassXC", "Offline password manager", Everyday)
            .flathub("org.keepassxc.KeePassXC")
            .everywhere(&["keepassxc"])
            .build(),
        app(sys, "bitwarden", "Bitwarden", "Password manager", Everyday).flathub("com.bitwarden.desktop").build(),
        app(sys, "localsend", "LocalSend", "Share files with nearby devices", Everyday)
            .flathub("org.localsend.localsend_app")
            .build(),
        app(sys, "obsidian", "Obsidian", "Notes", Everyday).flathub("md.obsidian.Obsidian").build(),
        app(sys, "signal", "Signal", "Private messenger", Everyday).flathub_community("org.signal.Signal").build(),
        app(sys, "vlc", "VLC", "Video player", Everyday)
            .flathub_community("org.videolan.VLC")
            .everywhere(&["vlc"])
            .build(),
        app(sys, "mpv", "mpv", "Minimal video player", Everyday)
            .flathub_community("io.mpv.Mpv")
            .everywhere(&["mpv"])
            .build(),
        app(sys, "celluloid", "Celluloid", "Video player for GNOME", Everyday)
            .flathub("io.github.celluloid_player.Celluloid")
            .everywhere(&["celluloid"])
            .build(),
        app(sys, "mpc-qt", "MPC-QT", "Video player for KDE", Everyday)
            .flathub("io.github.mpc_qt.mpc-qt")
            .aur(&["mpc-qt-bin"])
            .build(),
        app(sys, "showtime", "Showtime", "GNOME's video player", Everyday)
            .flathub("org.gnome.Showtime")
            .arch(&["showtime"])
            .build(),
        app(sys, "flatseal", "Flatseal", "Manage Flatpak permissions", Everyday)
            .flathub("com.github.tchx84.Flatseal")
            .build(),
        app(sys, "warehouse", "Warehouse", "Manage Flatpak apps", Everyday).flathub("io.github.flattool.Warehouse").build(),
        app(sys, "mission-center", "Mission Center", "System monitor", Everyday)
            .flathub("io.missioncenter.MissionCenter")
            .build(),
        app(sys, "pika-backup", "Pika Backup", "Simple backups", Everyday).flathub("org.gnome.World.PikaBackup").build(),
    ]
}
