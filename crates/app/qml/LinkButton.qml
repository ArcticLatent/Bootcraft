import Bootcraft
import QtQuick

// Accent text with an optional icon, before or after it.
Item {
    id: root

    property string label
    property string icon: ""
    property bool iconAfter: false
    property int fontSize: 16

    signal clicked

    implicitWidth: row.implicitWidth
    implicitHeight: Math.max(row.implicitHeight, 24)

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8
        layoutDirection: root.iconAfter ? Qt.RightToLeft : Qt.LeftToRight

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.icon !== ""
            name: root.icon
            size: root.fontSize + 2
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: mouse.containsMouse ? Theme.accentHover : Theme.accent
            font.pixelSize: root.fontSize
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
