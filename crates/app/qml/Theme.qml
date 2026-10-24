pragma Singleton
import QtQuick

// Colours, sizes and icon choices, from the Bootcraft mockup: a near-black
// canvas (or a soft white one in the light theme), cards a step off it with
// thin borders, and the accent of the distribution Bootcraft runs on.
QtObject {
    id: theme

    /// "dark" or "light": the switcher in the sidebar (Main keeps it).
    property string mode: "dark"
    readonly property bool isLight: mode === "light"
    /// The detected distribution (Main sets it), which picks the accent.
    property string distro: ""

    // ---- surfaces --------------------------------------------------------------
    readonly property color canvas: isLight ? "#f5f6f8" : "#16191d"
    readonly property color sidebar: isLight ? "#eceef1" : "#121518"
    readonly property color raised: isLight ? "#ffffff" : "#1b1f23"
    readonly property color raisedHover: isLight ? "#f9fafb" : "#20252a"
    readonly property color well: isLight ? "#eef0f3" : "#131619"
    readonly property color border: isLight ? "#dde1e5" : "#2a2f35"
    readonly property color borderStrong: isLight ? "#c4cad0" : "#383f46"
    readonly property color hairline: isLight ? "#e2e5e9" : "#22272c"
    /// The selected sidebar page and segment: a whisper of the accent.
    readonly property color navSelected: blend(sidebar, accent, isLight ? 0.12 : 0.09)
    /// The terminal under the pages.
    readonly property color terminal: isLight ? "#eceef1" : "#0b0d10"
    // ---- text ------------------------------------------------------------------
    readonly property color text: isLight ? "#1a1d21" : "#eceef0"
    /// Sidebar labels, a little quieter than text.
    readonly property color textNav: isLight ? "#363c43" : "#cfd4d8"
    readonly property color textMuted: isLight ? "#59616a" : "#a3aab1"
    readonly property color textFaint: isLight ? "#878e96" : "#737b83"
    /// Tool output in the terminal.
    readonly property color textOutput: isLight ? "#4a5159" : "#b4bac0"
    // ---- accent and states -----------------------------------------------------
    /// Each distribution's own colour, brighter on the dark theme and deeper
    /// on the light one (for contrast); the mockup's mint elsewhere.
    readonly property var accents: ({
        "ubuntu": ["#f07240", "#d4471a"],   // Ubuntu orange (#E95420)
        "arch": ["#38aee8", "#1077b0"],     // Arch blue (#1793D1)
        "fedora": ["#5aa9e0", "#3c6eb4"],   // Fedora blues (#51A2DA, #3C6EB4)
        "mint": ["#8ccf4d", "#4c8a1c"],     // Mint green (#87CF3E)
        "debian": ["#f0507e", "#c70a4c"],   // Debian red (#D70A53)
        "": ["#5ee0b5", "#13906a"]
    })
    readonly property color accent: (accents[distro] || accents[""])[isLight ? 1 : 0]
    readonly property color accentHover: blend(accent, isLight ? "#000000" : "#ffffff", 0.15)
    /// Text and marks on an accent fill.
    readonly property color accentText: !isLight && luminance(accent) > 0.55 ? "#0b1310" : "#ffffff"
    /// A selected card's fill.
    readonly property color accentSoft: blend(raised, accent, isLight ? 0.1 : 0.13)
    readonly property color badge: isLight ? "#e9ecef" : "#22272c"
    readonly property color success: isLight ? "#1f9d55" : "#3ecf7a"
    readonly property color warning: isLight ? "#b86e0b" : "#f2b25c"
    readonly property color error: isLight ? "#cf3f3f" : "#f27474"
    readonly property color sparkle: isLight ? "#e09a12" : "#f5b544"
    // ---- type and shape --------------------------------------------------------
    readonly property string fontFamily: "Geist"
    readonly property string monoFamily: "JetBrains Mono"
    readonly property int radius: 10
    readonly property int radiusSmall: 6
    readonly property int motion: 140

    function alpha(c, a) {
        return Qt.rgba(c.r, c.g, c.b, a);
    }

    function blend(a, b, t) {
        return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, 1);
    }

    function luminance(c) {
        return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
    }

    // ---- icons (Lucide names, see icons/) ---------------------------------------
    function packIcon(id) {
        return ({
            "essentials": "laptop",
            "creative": "brush",
            "development": "code",
            "gaming": "gamepad-2",
            "custom": "sliders-horizontal"
        })[id] || "box";
    }

    /// Stand-in for apps without a Flathub icon (or offline).
    function sectionIcon(section) {
        return ({
            "graphics": "pen-tool",
            "photo": "camera",
            "video-audio": "clapperboard",
            "animation": "film",
            "creative-utilities": "sparkles",
            "editors": "code",
            "dev-tools": "terminal",
            "containers": "container",
            "languages": "braces",
            "ai": "bot",
            "gaming": "gamepad-2",
            "everyday": "app-window"
        })[section] || "box";
    }

    function taskIcon(id, category) {
        const byId = {
            "system.update": "refresh-cw",
            "gpu.nvidia": "gpu",
            "gpu.nvidia-vaapi": "film",
            "flatpak.flathub": "package",
            "media.codecs": "clapperboard",
            "tools.archive": "archive",
            "tools.drives": "hard-drive",
            "system.fonts": "type",
            "system.ms-fonts": "type",
            "system.firmware": "microchip",
            "system.firewall": "shield",
            "arch.pacman-tweaks": "palette",
            "arch.multilib": "layers",
            "arch.build-tools": "hammer",
            "arch.aur-helper": "package",
            "desktop.cinnamon-mint": "palette",
            "fedora.dnf-tweaks": "download",
            "fedora.rpmfusion": "layers",
            "gpu.mesa-freeworld": "film",
            "debian.sudo": "shield-check",
            "debian.sources": "layers",
            "debian.backports": "refresh-cw",
            "apt.i386": "cpu",
            "ubuntu.multiverse": "layers",
            "ubuntu.replace-snap": "package",
            "mint.swappiness": "memory-stick"
        };
        if (byId[id])
            return byId[id];
        return category === "debloat" ? "trash-2" : "wrench";
    }
}
