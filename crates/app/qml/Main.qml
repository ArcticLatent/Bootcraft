import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window

    /// Development: `--screenshot=<file>` saves the window once loaded and quits;
    /// `--page=<n>` opens a page first.
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
        const page = parseInt(argument("--page="));
        if (!isNaN(page))
            Setup.page = page;
        const packs = argument("--packs=");
        if (packs)
            Setup.packs = packs.split(",");
        Setup.load();
        if (Qt.application.arguments.indexOf("--demo-run") >= 0)
            demoRun.start();
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
