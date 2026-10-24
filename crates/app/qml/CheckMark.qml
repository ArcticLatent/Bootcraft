import Bootcraft
import QtQuick

// A square check box in the accent colour.
Rectangle {
    id: root

    property bool checked

    signal toggled

    implicitWidth: 22
    implicitHeight: 22
    radius: 5
    color: checked ? Theme.accent : "transparent"
    border.color: checked ? Theme.accent : Theme.borderStrong
    border.width: 1.5
    opacity: enabled ? 1 : 0.4

    Icon {
        anchors.centerIn: parent
        visible: root.checked
        name: "check"
        size: 15
        color: Theme.accentText
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
