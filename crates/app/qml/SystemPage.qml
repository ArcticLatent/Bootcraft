import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Every system change Bootcraft offers here, by section. Essentials start
// ticked; optional changes and debloating are off until turned on.
Flickable {
    id: page

    readonly property var order: ["system", "drivers", "multimedia", "apps", "tools", "desktop", "debloat"]
    readonly property var categories: {
        if (!Setup.catalog)
            return [];
        return order.filter(c => Setup.catalog.tasks.some(t => t.category === c));
    }

    contentHeight: content.implicitHeight + 80
    clip: true
    boundsBehavior: Flickable.StopAtBounds

    ScrollBar.vertical: ScrollBar {}

    ColumnLayout {
        id: content
        x: 56
        y: 44
        width: page.width - 112
        spacing: 0

        PageHeader {
            Layout.fillWidth: true
            title: "System setup"
            subtitle: Setup.catalog && Setup.catalog.tasks.length === 0 ? "Bootcraft can't set up " + (Setup.system ? Setup.system.prettyName : "this system") + " yet." : "Essentials are ticked. Optional changes and debloating stay off until you turn them on."
        }

        Repeater {
            model: page.categories

            delegate: ColumnLayout {
                id: group
                required property string modelData
                readonly property var tasks: Setup.catalog.tasks.filter(t => t.category === modelData)
                Layout.topMargin: 30
                Layout.fillWidth: true
                spacing: 12

                Text {
                    text: group.tasks.length ? group.tasks[0].categoryTitle : ""
                    color: Theme.textMuted
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                    font.capitalization: Font.AllUppercase
                    font.letterSpacing: 0.8
                }

                Text {
                    visible: group.modelData === "debloat"
                    Layout.fillWidth: true
                    text: "Removes apps your desktop ships with. Only installed ones are touched."
                    color: Theme.textFaint
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: rows.implicitHeight
                    radius: Theme.radius
                    color: Theme.raised
                    border.color: Theme.border

                    Column {
                        id: rows
                        width: parent.width

                        Repeater {
                            model: group.tasks

                            delegate: ItemRow {
                                required property var modelData
                                required property int index
                                width: rows.width
                                icon: Theme.taskIcon(modelData.id, modelData.category)
                                title: modelData.title
                                subtitle: modelData.description
                                warning: modelData.notes.join(" ")
                                checked: Setup.taskSelected(modelData)
                                status: modelData.status
                                note: modelData.recommended ? "Essentials" : ""
                                last: index === group.tasks.length - 1
                                onToggled: Setup.toggleTask(modelData.id)
                            }
                        }
                    }
                }
            }
        }
    }
}
