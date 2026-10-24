import Bootcraft
import QtQuick

// A sidebar row; the selected one is tinted and marked with an accent bar.
Rectangle {
    id: root

    property string icon
    property string label
    property bool selected
    /// Shows a small pulsing dot (a setup is running).
    property bool active: false

    signal clicked

    implicitHeight: 50
    radius: 8
    color: selected ? Theme.navSelected : mouse.containsMouse ? Theme.raised : "transparent"

    Rectangle {
        visible: root.selected
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: 3
        height: 26
        radius: 2
        color: Theme.accent
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: 20
        anchors.verticalCenter: parent.verticalCenter
        spacing: 16

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon
            size: 22
            color: root.selected ? Theme.text : Theme.textMuted
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: root.selected ? Theme.text : Theme.textNav
            font.pixelSize: 16
            font.weight: root.selected ? Font.Medium : Font.Normal
        }
    }

    Rectangle {
        visible: root.active
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        width: 8
        height: 8
        radius: 4
        color: Theme.accent

        SequentialAnimation on opacity {
            running: root.active
            loops: Animation.Infinite
            NumberAnimation { to: 0.3; duration: 700 }
            NumberAnimation { to: 1; duration: 700 }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
