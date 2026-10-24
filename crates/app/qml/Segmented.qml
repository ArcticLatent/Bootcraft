import Bootcraft
import QtQuick

// A row of mutually exclusive options. `options`: [{value, label}].
Rectangle {
    id: root

    property var options: []
    property string value

    signal picked(string value)

    implicitHeight: 40
    implicitWidth: row.implicitWidth + 8
    radius: 8
    color: Theme.well
    border.color: Theme.border

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2

        Repeater {
            model: root.options

            delegate: Rectangle {
                required property var modelData
                readonly property bool current: modelData.value === root.value
                width: label.implicitWidth + 28
                height: 32
                radius: 6
                color: current ? Theme.navSelected : option.containsMouse ? Theme.raised : "transparent"
                border.color: current ? Theme.alpha(Theme.accent, 0.5) : "transparent"

                Text {
                    id: label
                    anchors.centerIn: parent
                    text: modelData.label
                    color: parent.current ? Theme.accent : Theme.textMuted
                    font.pixelSize: 14
                }

                MouseArea {
                    id: option
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.picked(modelData.value)
                }
            }
        }
    }
}
