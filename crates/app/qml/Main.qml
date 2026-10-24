import Bootcraft
import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window

    /// Development: `--screenshot=<file>` saves the window once loaded and quits;
    /// `--page=<n>` opens a page first, `--theme=light|dark` picks the theme
    /// and `--accent=<distro>` another distribution's colour.
    readonly property string screenshotPath: argument("--screenshot=")

    function argument(prefix) {
        const arg = Qt.application.arguments.find(a => a.startsWith(prefix));
        return arg ? arg.slice(prefix.length) : "";
    }

    width: 1440
    height: 940
    minimumWidth: 1120
    minimumHeight: 720
    visible: true
    title: "Bootcraft"
    color: Theme.canvas
    font.family: Theme.fontFamily
    font.pixelSize: 14
    // Basic-style defaults (tool tips, menus, scroll bars) in the theme's colours.
    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.raised
    palette.alternateBase: Theme.well
    palette.text: Theme.text
    palette.button: Theme.raised
    palette.buttonText: Theme.text
    palette.brightText: Theme.text
    palette.highlight: Theme.accent
    palette.highlightedText: Theme.accentText
    palette.placeholderText: Theme.textFaint
    palette.toolTipBase: Theme.raised
    palette.toolTipText: Theme.text
    palette.light: Theme.raisedHover
    palette.midlight: Theme.border
    palette.mid: Theme.borderStrong
    palette.dark: Theme.textFaint
    palette.shadow: "#000000"

    // The window's own state, kept between runs.
    Settings {
        id: windowSettings
        category: "window"

        /// "dark" or "light" (the sidebar's switcher).
        property string theme: "dark"
    }

    Binding {
        target: Theme
        property: "mode"
        value: windowSettings.theme === "light" ? "light" : "dark"
    }

    Binding {
        target: Theme
        property: "distro"
        value: window.argument("--accent=") || (Setup.system && Setup.system.distro ? Setup.system.distro : "")
    }

    // The canvas is drawn here too, so screenshots of `root` aren't transparent.
    Rectangle {
        id: root
        anchors.fill: parent
        color: Theme.canvas

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Sidebar {
            Layout.fillHeight: true
            onThemeToggled: windowSettings.theme = Theme.isLight ? "dark" : "light"
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: Setup.page

                OverviewPage {}
                ApplicationsPage {}
                SystemPage {}
                ActivityPage {}
                SettingsPage {}
            }

            TerminalDrawer {
                Layout.fillWidth: true
            }
        }
    }
    }

    Component.onCompleted: {
        const theme = argument("--theme=");
        if (theme)
            windowSettings.theme = theme;
        const page = parseInt(argument("--page="));
        if (!isNaN(page))
            Setup.page = page;
        const packs = argument("--packs=");
        if (packs)
            Setup.packs = packs.split(",");
        Setup.load();
        // Screenshots skip the update check, unless `--check-updates` asks for it now.
        if (Qt.application.arguments.indexOf("--check-updates") >= 0)
            Setup.checkUpdates(false);
        else if (screenshotPath === "")
            updateCheck.start();
        if (Qt.application.arguments.indexOf("--demo-run") >= 0)
            demoRun.start();
    }

    // A quiet update check a little after startup.
    Timer {
        id: updateCheck
        interval: 4000
        onTriggered: Setup.checkUpdates(true)
    }

    // Development: `--demo-run` plays sample helper events through the real
    // handler, to work on the Activity page without changing the system.
    Timer {
        id: demoRun
        interval: 400
        onTriggered: {
            const events = [
                { "event": "plan", "tasks": [
                    { "id": "system.update", "title": "Update the system" },
                    { "id": "media.codecs", "title": "Media codecs and thumbnails" },
                    { "id": "app.gimp", "title": "GIMP (Flatpak)" },
                    { "id": "app.vscodium", "title": "VSCodium (AUR)" },
                    { "id": "debloat.kde", "title": "Remove KDE extras" } ] },
                { "event": "task-started", "id": "system.update" },
                { "event": "step", "id": "system.update", "description": "pacman -Syu --noconfirm" },
                { "event": "output", "line": "$ pacman -Syu --noconfirm" },
                { "event": "output", "line": ":: Synchronizing package databases..." },
                { "event": "output", "line": " there is nothing to do" },
                { "event": "task-finished", "id": "system.update", "outcome": "done" },
                { "event": "task-finished", "id": "media.codecs", "outcome": "already-done" },
                { "event": "task-started", "id": "app.gimp" },
                { "event": "step", "id": "app.gimp", "description": "flatpak install -y --noninteractive --system flathub org.gimp.GIMP" },
                { "event": "output", "line": "Installing org.gimp.GIMP… 48%" },
                { "event": "task-finished", "id": "app.gimp", "outcome": "failed", "message": "flatpak failed: Unable to connect to dl.flathub.org" },
                { "event": "task-started", "id": "app.vscodium" },
                { "event": "step", "id": "app.vscodium", "description": "build from the AUR as your user, then pacman -U: vscodium-bin" },
                { "event": "output", "line": "==> Making package: vscodium-bin 1.140.06880-1" }
            ];
            Setup.runTasks = [];
            Setup.log.clear();
            events.forEach(e => Setup._onHelper(e));
            Setup.page = 3;
        }
    }

    Timer {
        // Give detection, the plan and the icon cache a moment.
        running: window.screenshotPath !== "" && Setup.ready && Setup.plan !== null
        interval: 1500
        onTriggered: root.grabToImage(result => {
            result.saveToFile(window.screenshotPath);
            Qt.quit();
        })
    }
}
