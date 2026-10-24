import Bootcraft
import QtQuick
import QtQuick.Controls

// A workflow to stack on the setup, compact: icon and name, with the
// description on hover. Essentials is always included.
Rectangle {
    id: root

    property string icon
    property string title
    property string subtitle
    property bool selected
    /// Essentials: part of every setup, not a toggle.
    property bool included: false

    signal clicked

    implicitHeight: 84
    radius: Theme.radius
    color: selected && !included ? Theme.accentSoft : mouse.containsMouse && !included ? Theme.raisedHover : Theme.raised
    border.color: selected && !included ? Theme.accent : Theme.border
    border.width: selected && !included ? 1.5 : 1

    Behavior on color {
        ColorAnimation { duration: Theme.motion }
    }

    Column {
        anchors.centerIn: parent
        width: parent.width - 16
        spacing: 6

        Icon {
            anchors.horizontalCenter: parent.horizontalCenter
            name: root.icon
            size: 24
            color: root.selected && !root.included ? Theme.accent : Theme.text
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.title
            color: Theme.text
            font.pixelSize: 15
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }

        Text {
            visible: root.included
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: "Always included"
            color: Theme.accent
            font.pixelSize: 11
        }
    }

    Rectangle {
        visible: root.selected && !root.included
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 8
        width: 18
        height: 18
        radius: 9
        color: Theme.accent

        Icon {
            anchors.centerIn: parent
            name: "check"
            size: 12
            color: Theme.accentText
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.included ? Qt.ArrowCursor : Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    ToolTip {
        visible: mouse.containsMouse
        delay: 400
        contentItem: Text {
            text: root.subtitle
            color: Theme.text
            font.pixelSize: 13
        }
        background: Rectangle {
            color: Theme.raised
            border.color: Theme.borderStrong
            radius: 6
        }
    }
}
