import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A run as it happens: each task's state, the live log, and what to do next.
Item {
    id: page

    readonly property bool idle: Setup.runState === "idle"

    function stateIcon(state) {
        return ({
            "pending": "circle-dashed",
            "running": "loader-circle",
            "done": "circle-check",
            "already-done": "circle-check",
            "failed": "circle-x",
            "skipped": "circle-minus",
            "cancelled": "circle-slash"
        })[state] || "circle-dashed";
    }

    function stateColor(state) {
        return ({
            "running": Theme.accent,
            "done": Theme.accent,
            "failed": Theme.error,
            "skipped": Theme.warning
        })[state] || Theme.textFaint;
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 56
        anchors.rightMargin: 56
        anchors.topMargin: 44
        anchors.bottomMargin: 28
        spacing: 0

        PageHeader {
            Layout.fillWidth: true
            title: ({
                    "authorizing": "Waiting for your password",
                    "running": "Setting up",
                    "done": "Setup complete",
                    "failed": "Setup finished with problems"
                })[Setup.runState] || "Activity"
            subtitle: {
                switch (Setup.runState) {
                case "authorizing":
                    return "Bootcraft needs administrator access to install software. Nothing has changed yet.";
                case "running":
                    return Setup.finishedCount + " of " + Setup.runTasks.length + " done. You can keep using your computer.";
                case "done":
                    return Setup.rebootNeeded ? "Restart to finish setting up drivers and the kernel." : "Everything in the plan is in place.";
                case "failed":
                    return Setup.runMessage;
                default:
                    return "";
                }
            }
        }

        // Nothing has run yet.
        Item {
            visible: page.idle
            Layout.fillWidth: true
            Layout.fillHeight: true

            Column {
                anchors.centerIn: parent
                spacing: 14

                Icon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: "history"
                    size: 48
                    color: Theme.textFaint
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Nothing has run yet"
                    color: Theme.text
                    font.pixelSize: 18
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Start setup from the Overview to follow it here."
                    color: Theme.textMuted
                    font.pixelSize: 14
                }
            }
        }

        // Progress
        Rectangle {
            visible: !page.idle
            Layout.topMargin: 24
            Layout.fillWidth: true
            height: 6
            radius: 3
            color: Theme.badge

            Rectangle {
                width: Setup.runTasks.length ? parent.width * Setup.finishedCount / Setup.runTasks.length : 0
                height: parent.height
                radius: 3
                color: Setup.runState === "failed" ? Theme.warning : Theme.accent

                Behavior on width {
                    NumberAnimation { duration: 300; easing.type: Easing.OutCubic }
                }
            }
        }

        RowLayout {
            visible: !page.idle
            Layout.topMargin: 22
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 24

            // Tasks; their output is in the terminal at the bottom.
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: Theme.radius
                color: Theme.raised
                border.color: Theme.border

                ListView {
                    id: tasks
                    anchors.fill: parent
                    anchors.margins: 8
                    clip: true
                    model: Setup.runTasks
                    spacing: 2

                    ScrollBar.vertical: ScrollBar {}

                    delegate: Rectangle {
                        required property var modelData
                        width: tasks.width
                        height: Math.max(48, taskColumn.implicitHeight + 20)
                        radius: 6
                        color: modelData.state === "running" ? Theme.accentSoft : "transparent"

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 12

                            Icon {
                                Layout.alignment: Qt.AlignVCenter
                                name: page.stateIcon(modelData.state)
                                size: 20
                                color: page.stateColor(modelData.state)

                                RotationAnimation on rotation {
                                    running: modelData.state === "running"
                                    from: 0
                                    to: 360
                                    duration: 1000
                                    loops: Animation.Infinite
                                }
                            }

                            ColumnLayout {
                                id: taskColumn
                                Layout.fillWidth: true
                                spacing: 2

                                Text {
                                    Layout.fillWidth: true
                                    text: modelData.title
                                    color: modelData.state === "pending" ? Theme.textMuted : Theme.text
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: text !== ""
                                    text: modelData.state === "already-done" ? "Already done" : modelData.message
                                    color: modelData.state === "failed" ? Theme.error : Theme.textFaint
                                    font.pixelSize: 12
                                    wrapMode: Text.WordWrap
                                }
                            }
                        }
                    }
                }
            }
        }

        RowLayout {
            visible: !page.idle
            Layout.topMargin: 20
            Layout.fillWidth: true
            spacing: 14

            Text {
                Layout.fillWidth: true
                visible: Setup.logPath !== ""
                text: "Full log: " + Setup.logPath
                color: Theme.textFaint
                font.pixelSize: 13
                elide: Text.ElideMiddle
            }

            Item {
                Layout.fillWidth: Setup.logPath === ""
            }

            PrimaryButton {
                visible: Setup.runState === "running"
                secondary: true
                label: Setup.cancelRequested ? "Stopping after this step…" : "Stop after this step"
                enabled: !Setup.cancelRequested
                onClicked: Setup.cancel()
            }

            PrimaryButton {
                visible: Setup.runState === "done" || Setup.runState === "failed"
                secondary: true
                label: "Back to overview"
                onClicked: Setup.page = 0
            }

            PrimaryButton {
                visible: Setup.runState === "done" && Setup.rebootNeeded
                label: "Restart now"
                icon: "rotate-cw"
                onClicked: Setup.reboot()
            }
        }
    }
}
