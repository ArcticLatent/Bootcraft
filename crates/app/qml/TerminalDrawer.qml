import Bootcraft
import QtQuick

// The bottom bar: a status line, and a terminal with the setup's live output
// that slides up when a setup starts. Click the bar to open or close it.
Rectangle {
    id: drawer

    property bool open: false
    readonly property int barHeight: 44
    readonly property int terminalHeight: 300
    readonly property bool hasOutput: Setup.log.count > 0 || Setup.busy

    implicitHeight: barHeight + (open ? terminalHeight : 0)
    color: Theme.canvas
    clip: true

    Behavior on implicitHeight {
        NumberAnimation { duration: 240; easing.type: Easing.OutCubic }
    }

    Connections {
        target: Setup

        function onRunStateChanged() {
            if (Setup.busy)
                drawer.open = true;
        }
    }

    Item {
        id: bar
        width: parent.width
        height: drawer.barHeight

        Rectangle {
            width: parent.width
            height: 1
            color: Theme.hairline
        }

        Row {
            anchors.left: parent.left
            anchors.leftMargin: 56
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: Setup.runState === "failed" ? "triangle-alert" : Setup.busy ? "loader-circle" : "info"
                size: 16
                color: Setup.runState === "failed" ? Theme.warning : Setup.busy ? Theme.accent : Theme.textFaint

                RotationAnimation on rotation {
                    running: Setup.busy
                    from: 0
                    to: 360
                    duration: 1000
                    loops: Animation.Infinite
                    onStopped: parent.rotation = 0
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.textFaint
                font.pixelSize: 13
                text: {
                    switch (Setup.runState) {
                    case "authorizing":
                        return "Waiting for your password…";
                    case "running":
                        return "Setting up: " + Setup.finishedCount + " of " + Setup.runTasks.length + " done.";
                    case "done":
                        return Setup.rebootNeeded ? "Setup complete. Restart to finish." : "Setup complete.";
                    case "failed":
                        return Setup.runMessage || "The last setup finished with problems; see Activity.";
                    default:
                        return Setup.loadError || "No changes have been made yet.";
                    }
                }
            }
        }

        Row {
            visible: drawer.hasOutput
            anchors.right: parent.right
            anchors.rightMargin: 24
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "terminal"
                size: 16
                color: barMouse.containsMouse ? Theme.text : Theme.textMuted
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Terminal"
                color: barMouse.containsMouse ? Theme.text : Theme.textMuted
                font.pixelSize: 13
            }

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: drawer.open ? "chevron-down" : "chevron-up"
                size: 16
                color: Theme.textFaint
            }
        }

        MouseArea {
            id: barMouse
            anchors.fill: parent
            enabled: drawer.hasOutput
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: drawer.open = !drawer.open
        }
    }

    Rectangle {
        y: drawer.barHeight
        width: parent.width
        height: drawer.terminalHeight
        color: Theme.terminal

        Rectangle {
            width: parent.width
            height: 1
            color: Theme.hairline
        }

        LogView {
            anchors.fill: parent
            anchors.leftMargin: 56
            anchors.rightMargin: 24
            anchors.topMargin: 14
            anchors.bottomMargin: 10
        }
    }
}
