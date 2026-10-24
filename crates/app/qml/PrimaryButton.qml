import Bootcraft
import QtQuick

// The accent button ("Start setup"); `secondary` for a quiet one.
Rectangle {
    id: root

    property string label
    property string icon: ""
    property bool secondary: false
    property bool compact: false

    signal clicked

    implicitHeight: secondary || compact ? 44 : 54
    implicitWidth: row.implicitWidth + 48
    radius: 8
    color: secondary ? (mouse.containsMouse ? Theme.raisedHover : Theme.raised) : !enabled ? Theme.alpha(Theme.accent, 0.3) : mouse.pressed ? Qt.darker(Theme.accent, 1.1) : mouse.containsMouse ? Theme.accentHover : Theme.accent
    border.color: secondary ? Theme.borderStrong : "transparent"

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 12

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: root.secondary ? Theme.text : Theme.accentText
            font.pixelSize: root.secondary || root.compact ? 15 : 17
            font.weight: Font.DemiBold
        }

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.icon !== ""
            name: root.icon
            size: 20
            color: root.secondary ? Theme.text : Theme.accentText
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.clicked()
    }
}
