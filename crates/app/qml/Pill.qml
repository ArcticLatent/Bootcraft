import Bootcraft
import QtQuick

// A small label: source badges, "Installed", counts.
Rectangle {
    id: root

    property string label
    property string icon: ""
    property color textColor: Theme.textMuted

    implicitHeight: 28
    implicitWidth: row.implicitWidth + 20
    radius: 6
    color: Theme.badge
    border.color: Theme.border

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.icon !== ""
            name: root.icon
            size: 14
            color: root.textColor
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: root.textColor
            font.pixelSize: 13
        }
    }
}
