import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// An app in the Applications grid. Click anywhere to pick it.
Rectangle {
    id: tile

    property var app
    readonly property bool checked: Setup.appSelected(app.id)

    implicitHeight: 108
    radius: Theme.radius
    color: checked ? Theme.accentSoft : mouse.containsMouse ? Theme.raisedHover : Theme.raised
    border.color: checked ? Theme.alpha(Theme.accent, 0.7) : Theme.border

    Behavior on color {
        ColorAnimation { duration: Theme.motion }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: Setup.toggleApp(tile.app.id)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            spacing: 14

            AppIcon {
                Layout.alignment: Qt.AlignTop
                app: tile.app
                size: 36
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                spacing: 3

                Text {
                    Layout.fillWidth: true
                    text: tile.app.name
                    color: Theme.text
                    font.pixelSize: 15
                    font.weight: Font.Medium
                    elide: Text.ElideRight
                }

                Text {
                    Layout.fillWidth: true
                    text: tile.app.summary
                    color: Theme.textMuted
                    font.pixelSize: 13
                    elide: Text.ElideRight
                }
            }

            CheckMark {
                Layout.alignment: Qt.AlignTop
                checked: tile.checked
                onToggled: Setup.toggleApp(tile.app.id)
            }
        }

        Item {
            Layout.fillHeight: true
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            SourceBadge {
                app: tile.app
            }

            Item {
                Layout.fillWidth: true
            }

            Icon {
                visible: tile.app.notes.length > 0
                name: "info"
                size: 16
                color: Theme.textFaint
            }

            // Compact: a pill next to the fixed-width badge won't fit a tile.
            Row {
                visible: tile.app.status === "done"
                spacing: 4

                Icon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "check"
                    size: 14
                    color: Theme.accent
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Installed"
                    color: Theme.accent
                    font.pixelSize: 12
                }
            }
        }
    }

    ToolTip {
        visible: mouse.containsMouse && tile.app.notes.length > 0
        delay: 500
        width: Math.min(360, implicitWidth)
        contentItem: Text {
            text: tile.app.notes.join("\n")
            color: Theme.text
            font.pixelSize: 13
            wrapMode: Text.WordWrap
        }
        background: Rectangle {
            color: Theme.raised
            border.color: Theme.borderStrong
            radius: 6
        }
    }
}
