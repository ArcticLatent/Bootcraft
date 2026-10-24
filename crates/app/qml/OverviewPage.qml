import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The start page: this machine and the workflows to stack beside Start
// setup, then the resulting plan, its rows in two columns to show more at once.
Flickable {
    id: page

    /// 0: Applications tab, 1: System changes tab.
    property int tab: 0

    contentHeight: content.implicitHeight + 64
    clip: true
    boundsBehavior: Flickable.StopAtBounds

    ScrollBar.vertical: ScrollBar {}

    ColumnLayout {
        id: content
        x: 56
        y: 36
        width: page.width - 112
        spacing: 0

        // This machine and the workflows on the left, Start setup on the right.
        RowLayout {
            Layout.fillWidth: true
            spacing: 32

            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                spacing: 0

                SystemStrip {
                    Layout.fillWidth: true
                }

                RowLayout {
                    Layout.topMargin: 22
                    Layout.fillWidth: true

                    Text {
                        text: "Choose your workflow"
                        color: Theme.text
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    LinkButton {
                        label: "Customize"
                        icon: "arrow-right"
                        iconAfter: true
                        fontSize: 15
                        onClicked: Setup.page = 1
                    }
                }

                RowLayout {
                    Layout.topMargin: 12
                    Layout.fillWidth: true
                    spacing: 12

                    Repeater {
                        model: ["essentials", "creative", "development", "gaming", "custom"]

                        delegate: WorkflowCard {
                            required property string modelData
                            readonly property var info: Setup.pack(modelData)
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            icon: Theme.packIcon(modelData)
                            title: info ? info.title : "Custom"
                            subtitle: info ? info.subtitle : "Start with your own picks"
                            selected: Setup.packSelected(modelData)
                            included: modelData === "essentials"
                            onClicked: Setup.togglePack(modelData)
                        }
                    }
                }
            }

            ReadyCard {
                Layout.alignment: Qt.AlignTop
            }
        }

        Repeater {
            model: Setup.system ? Setup.system.warnings : []

            delegate: Row {
                required property string modelData
                Layout.topMargin: 14
                spacing: 10

                Icon {
                    name: "triangle-alert"
                    size: 18
                    color: Theme.warning
                }

                Text {
                    text: modelData
                    color: Theme.warning
                    font.pixelSize: 14
                }
            }
        }

        Text {
            Layout.topMargin: 32
            text: "Your setup plan"
            color: Theme.text
            font.pixelSize: 22
            font.weight: Font.DemiBold
        }

        Text {
            Layout.topMargin: 6
            text: "Review what Bootcraft will install and configure. Untick anything you don't want."
            color: Theme.textMuted
            font.pixelSize: 15
        }

        Row {
            Layout.topMargin: 18

            PlanTab {
                label: "Applications"
                count: Setup.appCount
                selected: page.tab === 0
                onClicked: page.tab = 0
            }

            PlanTab {
                label: "System changes"
                count: Setup.systemCount
                selected: page.tab === 1
                onClicked: page.tab = 1
            }
        }

        // One card; the selected tab's rows flow into two columns.
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: plan.implicitHeight
            radius: Theme.radius
            color: Theme.raised
            border.color: Theme.border

            // Between the two columns.
            Rectangle {
                visible: grid.visibleChildren.length > 1
                x: Math.round(parent.width / 2)
                width: 1
                height: grid.height
                color: Theme.hairline
            }

            ColumnLayout {
                id: plan
                width: parent.width
                spacing: 0

                GridLayout {
                    id: grid
                    Layout.fillWidth: true
                    columns: 2
                    columnSpacing: 0
                    rowSpacing: 0

                    Repeater {
                        model: page.tab === 0 ? Setup.selectedApps : []

                        delegate: ItemRow {
                            required property var modelData
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            app: modelData
                            title: modelData.name
                            subtitle: modelData.summary
                            checked: true
                            status: modelData.status
                            onToggled: Setup.toggleApp(modelData.id)
                        }
                    }

                    // System changes, including ones the plan pulls in.
                    Repeater {
                        model: page.tab === 1 ? Setup.planSystemTasks : []

                        delegate: ItemRow {
                            required property var modelData
                            readonly property var task: Setup.task(modelData.id)
                            readonly property bool chosen: task !== null && Setup.taskSelected(task)
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            icon: Theme.taskIcon(modelData.id, task ? task.category : "")
                            title: modelData.title
                            subtitle: task ? task.description : ""
                            checked: chosen
                            checkable: chosen
                            status: task ? task.status : ""
                            note: !chosen && modelData.requiredBy ? "Needed by " + Setup.titleOf(modelData.requiredBy) : ""
                            onToggled: Setup.toggleTask(modelData.id)
                        }
                    }
                }

                Text {
                    visible: page.tab === 0 && Setup.selectedApps.length === 0
                    Layout.fillWidth: true
                    padding: 20
                    text: "Essentials installs no extra apps. Pick a workflow above, or add applications."
                    color: Theme.textMuted
                    font.pixelSize: 14
                    wrapMode: Text.WordWrap
                }

                LinkButton {
                    Layout.leftMargin: 20
                    Layout.preferredHeight: 54
                    icon: page.tab === 0 ? "plus" : "wrench"
                    label: page.tab === 0 ? "Add applications" : "All system options"
                    fontSize: 15
                    onClicked: Setup.page = page.tab === 0 ? 1 : 2
                }
            }
        }
    }
}
