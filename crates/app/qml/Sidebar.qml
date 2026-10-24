import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The left column: logo, pages, this computer, Settings and the theme switch.
Rectangle {
    id: sidebar

    signal themeToggled

    // A square icon button next to Settings.
    component SideButton: Rectangle {
        id: button

        property string iconName
        property string tip
        property bool dot: false
        property bool spinning: false

        signal clicked

        implicitWidth: 44
        implicitHeight: 50
        radius: 8
        color: buttonMouse.containsMouse ? Theme.raised : "transparent"

        Icon {
            id: glyph
            anchors.centerIn: parent
            name: button.iconName
            size: 20
            color: button.dot ? Theme.accent : buttonMouse.containsMouse ? Theme.text : Theme.textMuted
        }

        RotationAnimator {
            target: glyph
            running: button.spinning
            from: 0
            to: 360
            duration: 1000
            loops: Animation.Infinite
            onRunningChanged: if (!running)
                glyph.rotation = 0
        }

        Rectangle {
            visible: button.dot
            x: parent.width / 2 + 5
            y: 12
            width: 8
            height: 8
            radius: 4
            color: Theme.success
            border.color: Theme.sidebar
            border.width: 1.5
        }

        MouseArea {
            id: buttonMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: button.clicked()
        }

        ToolTip.visible: buttonMouse.containsMouse
        ToolTip.delay: 500
        ToolTip.text: button.tip
    }

    color: Theme.sidebar
    implicitWidth: 272

    Rectangle {
        anchors.right: parent.right
        width: 1
        height: parent.height
        color: Theme.hairline
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        anchors.topMargin: 30
        spacing: 6

        LogoMark {
            Layout.leftMargin: 12
            Layout.bottomMargin: 30
        }

        NavItem {
            Layout.fillWidth: true
            icon: "house"
            label: "Overview"
            selected: Setup.page === 0
            onClicked: Setup.page = 0
        }

        NavItem {
            Layout.fillWidth: true
            icon: "layout-grid"
            label: "Applications"
            selected: Setup.page === 1
            onClicked: Setup.page = 1
        }

        NavItem {
            Layout.fillWidth: true
            icon: "wrench"
            label: "System setup"
            selected: Setup.page === 2
            onClicked: Setup.page = 2
        }

        NavItem {
            Layout.fillWidth: true
            icon: "history"
            label: "Activity"
            selected: Setup.page === 3
            active: Setup.busy
            onClicked: Setup.page = 3
        }

        Item {
            Layout.fillHeight: true
        }

        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: Theme.hairline
        }

        // This computer
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 12
            Layout.topMargin: 18
            Layout.bottomMargin: 18
            spacing: 16

            Icon {
                Layout.alignment: Qt.AlignTop
                name: "monitor"
                size: 24
                color: Theme.textMuted
            }

            ColumnLayout {
                spacing: 4

                Text {
                    text: "This computer"
                    color: Theme.textFaint
                    font.pixelSize: 13
                }

                Text {
                    text: Setup.system ? Setup.system.name : "Detecting…"
                    color: Theme.text
                    font.pixelSize: 16
                }

                Row {
                    spacing: 8

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 9
                        height: 9
                        radius: 5
                        color: !Setup.system || Setup.system.supported ? (Setup.busy ? Theme.warning : Theme.success) : Theme.error
                    }

                    Text {
                        text: !Setup.system ? "Checking" : !Setup.system.supported ? "Not supported" : Setup.busy ? "Setting up" : "Ready"
                        color: Theme.textMuted
                        font.pixelSize: 14
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            height: 1
            color: Theme.hairline
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 10
            spacing: 2

            NavItem {
                Layout.fillWidth: true
                icon: "settings"
                label: "Settings"
                selected: Setup.page === 4
                onClicked: Setup.page = 4
            }

            // Bootcraft's updates: a dot when one is available; Settings has the details.
            SideButton {
                iconName: Setup.updateAvailable ? "download" : "refresh-cw"
                tip: Setup.updateState === "checking" ? "Checking for updates…" : Setup.updateAvailable ? "Bootcraft " + Setup.updateVersion + " is available" : "Check for updates"
                spinning: Setup.updating
                dot: Setup.updateAvailable
                onClicked: {
                    Setup.page = 4;
                    if (!Setup.updateAvailable)
                        Setup.checkUpdates(false);
                }
            }

            // Light or dark: the icon of the theme it switches to.
            SideButton {
                iconName: Theme.isLight ? "moon" : "sun"
                tip: Theme.isLight ? "Dark theme" : "Light theme"
                onClicked: sidebar.themeToggled()
            }
        }
    }
}
