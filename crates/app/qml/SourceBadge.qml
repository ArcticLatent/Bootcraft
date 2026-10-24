import Bootcraft
import QtQuick
import QtQuick.Controls

// Where an app comes from (Flatpak, System, AUR...). When both sources
// exist it opens a menu to choose one for this app.
Rectangle {
    id: root

    property var app
    readonly property bool switchable: app !== null && app.flatpak !== null && app.native !== null
    readonly property string source: app ? Setup.sourceOf(app) : ""

    // One width for every badge, so a column of them lines up.
    implicitHeight: 28
    implicitWidth: 136
    radius: 6
    color: mouse.containsMouse && switchable ? Theme.raisedHover : Theme.badge
    border.color: menu.opened ? Theme.borderStrong : Theme.border

    Row {
        id: row
        anchors.left: parent.left
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.source === "flatpak" && root.app && root.app.flatpak && root.app.flatpak.verified
            name: "badge-check"
            size: 14
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.app ? Setup.sourceLabel(root.app) : ""
            color: Theme.textMuted
            font.pixelSize: 13
        }
    }

    Icon {
        anchors.right: parent.right
        anchors.rightMargin: 9
        anchors.verticalCenter: parent.verticalCenter
        visible: root.switchable
        name: "chevron-down"
        size: 14
        color: Theme.textFaint
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.switchable
        cursorShape: Qt.PointingHandCursor
        onClicked: menu.open()
    }

    Popup {
        id: menu
        y: root.height + 6
        x: root.width - width
        width: 300
        padding: 6
        background: Rectangle {
            color: Theme.raised
            border.color: Theme.borderStrong
            radius: 8
        }

        contentItem: Column {
            spacing: 2

            Repeater {
                model: root.switchable ? [
                    {
                        "source": "flatpak",
                        "title": "Flatpak",
                        "detail": root.app.flatpak.verified ? "Flathub, published by its developers" : "Flathub, packaged by the community"
                    },
                    {
                        "source": "native",
                        "title": Setup.sourceLabel(Object.assign({}, root.app, { "choice": { "source": "native" } })),
                        "detail": ({
                                "aur": "Built from the AUR",
                                "rpm-fusion": "From RPM Fusion",
                                "signed-release": "Signed GitHub release"
                            })[root.app.native.via] || "From the distribution's packages"
                    }
                ] : []

                delegate: Rectangle {
                    required property var modelData
                    width: 288
                    height: 50
                    radius: 6
                    color: option.containsMouse ? Theme.raisedHover : "transparent"

                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2

                        Text {
                            text: modelData.title
                            color: Theme.text
                            font.pixelSize: 14
                        }

                        Text {
                            text: modelData.detail
                            color: Theme.textFaint
                            font.pixelSize: 12
                        }
                    }

                    Icon {
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        visible: root.source === modelData.source
                        name: "check"
                        size: 16
                        color: Theme.accent
                    }

                    MouseArea {
                        id: option
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            Setup.setSource(root.app.id, modelData.source);
                            menu.close();
                        }
                    }
                }
            }

            Text {
                visible: root.app !== null && root.app.choice !== null
                width: 288
                leftPadding: 12
                topPadding: 6
                bottomPadding: 6
                wrapMode: Text.WordWrap
                text: root.app && root.app.choice ? "Automatic: " + root.app.choice.reason + "." : ""
                color: Theme.textFaint
                font.pixelSize: 12
            }
        }
    }
}
