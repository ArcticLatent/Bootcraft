import Bootcraft
import QtQuick

// A tab over the plan card, with a count.
Item {
    id: root

    property string label
    property int count
    property bool selected

    signal clicked

    implicitWidth: row.implicitWidth + 44
    implicitHeight: 48

    Rectangle {
        anchors.fill: parent
        color: root.selected ? Theme.raised : "transparent"
        radius: 8

        // Square off the bottom so the tab joins the card below it.
        Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: 8
            color: parent.color
        }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 10

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: root.selected ? Theme.accent : Theme.textMuted
            font.pixelSize: 15
            font.weight: root.selected ? Font.Medium : Font.Normal
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(24, countText.implicitWidth + 14)
            height: 24
            radius: 12
            color: Theme.badge

            Text {
                id: countText
                anchors.centerIn: parent
                text: root.count
                color: Theme.text
                font.pixelSize: 13
            }
        }
    }

    Rectangle {
        visible: root.selected
        anchors.bottom: parent.bottom
        width: parent.width
        height: 2
        color: Theme.accent
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
