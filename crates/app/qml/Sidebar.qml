import Bootcraft
import QtQuick
import QtQuick.Layouts

// The left column: logo, pages, this computer and Settings.
Rectangle {
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
                        color: !Setup.system || Setup.system.supported ? (Setup.busy ? Theme.warning : "#3ecf7a") : Theme.error
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

        NavItem {
            Layout.fillWidth: true
            Layout.topMargin: 10
            icon: "settings"
            label: "Settings"
            selected: Setup.page === 4
            onClicked: Setup.page = 4
        }
    }
}
