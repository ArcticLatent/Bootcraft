pragma Singleton
import QtQuick

// Colours, sizes and icon choices, from the Bootcraft mockup: a near-black
// canvas, slightly lighter cards with thin borders, and a mint accent.
QtObject {
    // ---- surfaces --------------------------------------------------------------
    readonly property color canvas: "#16191d"
    readonly property color sidebar: "#121518"
    readonly property color raised: "#1b1f23"
    readonly property color raisedHover: "#20252a"
    readonly property color well: "#131619"
    readonly property color border: "#2a2f35"
    readonly property color borderStrong: "#383f46"
    readonly property color hairline: "#22272c"
    readonly property color navSelected: "#1b2926"
    // ---- text ------------------------------------------------------------------
    readonly property color text: "#eceef0"
    readonly property color textMuted: "#a3aab1"
    readonly property color textFaint: "#737b83"
    // ---- accent and states -----------------------------------------------------
    readonly property color accent: "#5ee0b5"
    readonly property color accentHover: "#7be9c4"
    /// Text and marks on an accent fill.
    readonly property color accentText: "#0b1813"
    /// A selected card's fill.
    readonly property color accentSoft: "#15302a"
    readonly property color badge: "#22272c"
    readonly property color warning: "#f2b25c"
    readonly property color error: "#f27474"
    readonly property color sparkle: "#f5b544"
    // ---- type and shape --------------------------------------------------------
    readonly property string fontFamily: "Geist"
    readonly property string monoFamily: "JetBrains Mono"
    readonly property int radius: 10
    readonly property int radiusSmall: 6
    readonly property int motion: 140

    function alpha(c, a) {
        return Qt.rgba(c.r, c.g, c.b, a);
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
            "desktop.cinnamon-mint": "palette"
        };
        if (byId[id])
            return byId[id];
        return category === "debloat" ? "trash-2" : "wrench";
    }
}
