import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Where apps come from, the NVIDIA driver choice, and credits.
Flickable {
    id: page

    contentHeight: content.implicitHeight + 80
    clip: true
    boundsBehavior: Flickable.StopAtBounds

    ScrollBar.vertical: ScrollBar {}

    component Section: Rectangle {
        default property alias content: inner.data
        property string title
        property string description

        Layout.fillWidth: true
        Layout.topMargin: 24
        implicitHeight: inner.implicitHeight + 48
        radius: Theme.radius
        color: Theme.raised
        border.color: Theme.border

        ColumnLayout {
            id: inner
            anchors.fill: parent
            anchors.margins: 24
            spacing: 12

            Text {
                text: parent.parent.title
                color: Theme.text
                font.pixelSize: 18
                font.weight: Font.DemiBold
            }

            Text {
                Layout.fillWidth: true
                text: parent.parent.description
                color: Theme.textMuted
                font.pixelSize: 14
                wrapMode: Text.WordWrap
            }
        }
    }

    ColumnLayout {
        id: content
        x: 56
        y: 44
        width: Math.min(page.width - 112, 860)
        spacing: 0

        PageHeader {
            Layout.fillWidth: true
            title: "Settings"
        }

        Section {
            title: "Install apps as"
            description: {
                switch (Setup.preference) {
                case "flatpak":
                    return "Flathub builds wherever they exist. Apps that need the host system (code editors, Steam, command-line tools) still come from the system's packages.";
                case "native":
                    return "The distribution's packages wherever they exist, including the AUR on Arch. Apps only on Flathub still come from Flathub.";
                default:
                    return "Bootcraft picks per app: system packages where the distribution keeps them current (Arch, Fedora), Flathub where it freezes them for years (Ubuntu, Mint, Debian), and a verified Flathub build over an AUR build.";
                }
            }

            Segmented {
                Layout.topMargin: 6
                value: Setup.preference
                options: [
                    {
                        "value": "auto",
                        "label": "Automatic"
                    },
                    {
                        "value": "flatpak",
                        "label": "Prefer Flatpak"
                    },
                    {
                        "value": "native",
                        "label": "Prefer system packages"
                    }
                ]
                onPicked: value => Setup.preference = value
            }
        }

        Section {
            title: "NVIDIA driver"
            description: {
                const gpu = Setup.system ? Setup.system.gpus.find(g => g.vendor === "nvidia") : null;
                if (!gpu)
                    return "No NVIDIA card was detected.";
                return "Detected " + (gpu.name || "an NVIDIA card") + ": " + (gpu.driver || "") + ". Change this only if detection got your card wrong.";
            }

            Segmented {
                Layout.topMargin: 6
                value: Setup.nvidia
                options: [
                    {
                        "value": "",
                        "label": "As detected"
                    },
                    {
                        "value": "modern",
                        "label": "Current (open modules)"
                    },
                    {
                        "value": "legacy",
                        "label": "580 legacy"
                    },
                    {
                        "value": "none",
                        "label": "Don't install"
                    }
                ]
                onPicked: value => Setup.nvidia = value
            }
        }

        Section {
            title: "Updates"
            description: {
                if (Setup.updateMessage !== "")
                    return Setup.updateMessage;
                if (Setup.updateAvailable)
                    return "Bootcraft " + Setup.updateVersion + " is available (you have " + Qt.application.version + "). "
                        + (Setup.updateKind === "manual" ? "This copy wasn't installed from a package or an AppImage; get it from the release page."
                            : "Bootcraft downloads it, checks its signature and restarts." + (Setup.updateKind === "package" ? " Installing it asks for your password." : ""));
                return "Bootcraft " + Qt.application.version + ". New versions are checked for when Bootcraft starts, and only installed when you choose to.";
            }

            RowLayout {
                Layout.topMargin: 6
                spacing: 16

                PrimaryButton {
                    visible: Setup.updateAvailable
                    compact: true
                    enabled: !Setup.updating && !Setup.busy
                    label: Setup.updateState === "installing" ? "Installing…" : Setup.updateKind === "manual" ? "Open the release page" : "Install " + Setup.updateVersion
                    icon: Setup.updateKind === "manual" ? "arrow-right" : "download"
                    onClicked: Setup.installUpdate()
                }

                PrimaryButton {
                    secondary: true
                    enabled: !Setup.updating
                    label: Setup.updateState === "checking" ? "Checking…" : "Check for updates"
                    icon: "refresh-cw"
                    onClicked: Setup.checkUpdates(false)
                }

                LinkButton {
                    visible: Setup.updatePage !== ""
                    label: "What's new"
                    icon: "arrow-right"
                    iconAfter: true
                    fontSize: 14
                    onClicked: Qt.openUrlExternally(Setup.updatePage)
                }
            }
        }

        Section {
            title: "About Bootcraft"
            description: "Version " + Qt.application.version + ". Setup logs are kept in /var/log/bootcraft; app icons are cached in ~/.cache/bootcraft/icons."

            Text {
                Layout.fillWidth: true
                text: "Icons by Lucide (ISC). Distribution and GPU logos from Simple Icons (CC0); they are trademarks of their owners. Fonts: Geist and JetBrains Mono (SIL Open Font License)."
                color: Theme.textFaint
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }
        }
    }
}
