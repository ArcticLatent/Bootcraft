import Bootcraft
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Every app available on this system, by section, with search and the
// Flatpak/system preference.
Flickable {
    id: page

    property string query: ""
    /// As many ~250px tiles per row as fit.
    readonly property int columns: Math.max(2, Math.floor((content.width + 14) / (250 + 14)))
    readonly property real tileWidth: (content.width - (columns - 1) * 14) / columns
    readonly property var apps: Setup.catalog ? Setup.catalog.apps.filter(a => a.available && matches(a)) : []
    readonly property var sections: {
        const seen = [];
        for (const a of apps) {
            if (seen.indexOf(a.section) < 0)
                seen.push(a.section);
        }
        return seen;
    }

    function matches(a) {
        const q = query.trim().toLowerCase();
        return q === "" || a.name.toLowerCase().indexOf(q) >= 0 || a.summary.toLowerCase().indexOf(q) >= 0;
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
            title: "Applications"
            subtitle: "Pick the apps to install. Each badge shows where an app comes from; click it to choose another source."
        }

        RowLayout {
            Layout.topMargin: 26
            Layout.fillWidth: true
            spacing: 16

            TextField {
                id: search
                Layout.preferredWidth: 340
                placeholderText: "Search applications"
                placeholderTextColor: Theme.textFaint
                color: Theme.text
                font.pixelSize: 15
                leftPadding: 42
                onTextChanged: page.query = text
                // Development: `--search=<text>` starts with a search (screenshots).
                Component.onCompleted: {
                    const arg = Qt.application.arguments.find(a => a.startsWith("--search="));
                    if (arg)
                        text = arg.slice("--search=".length);
                }
                background: Rectangle {
                    implicitHeight: 42
                    radius: 8
                    color: Theme.well
                    border.color: search.activeFocus ? Theme.accent : Theme.border

                    Icon {
                        anchors.left: parent.left
                        anchors.leftMargin: 14
                        anchors.verticalCenter: parent.verticalCenter
                        name: "search"
                        size: 18
                        color: Theme.textFaint
                    }
                }
            }

            Item {
                Layout.fillWidth: true
            }

            Text {
                text: "Install apps as"
                color: Theme.textMuted
                font.pixelSize: 14
            }

            Segmented {
                value: Setup.preference
                options: [
                    {
                        "value": "auto",
                        "label": "Automatic"
                    },
                    {
                        "value": "flatpak",
                        "label": "Flatpak"
                    },
                    {
                        "value": "native",
                        "label": "System packages"
                    }
                ]
                onPicked: value => Setup.preference = value
            }
        }

        Repeater {
            model: page.sections

            delegate: ColumnLayout {
                id: section
                required property string modelData
                readonly property var sectionApps: page.apps.filter(a => a.section === modelData)
                Layout.topMargin: 28
                Layout.fillWidth: true
                spacing: 14

                Text {
                    text: section.sectionApps.length ? section.sectionApps[0].sectionTitle : ""
                    color: Theme.textMuted
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                    font.capitalization: Font.AllUppercase
                    font.letterSpacing: 0.8
                }

                GridLayout {
                    Layout.fillWidth: true
                    columns: page.columns
                    columnSpacing: 14
                    rowSpacing: 14

                    Repeater {
                        model: section.sectionApps

                        delegate: AppTile {
                            required property var modelData
                            Layout.preferredWidth: page.tileWidth
                            app: modelData
                        }
                    }
                }
            }
        }

        Text {
            Layout.topMargin: 30
            visible: page.apps.length === 0
            text: Setup.catalog ? "No applications match “" + page.query + "”." : "Loading…"
            color: Theme.textMuted
            font.pixelSize: 15
        }
    }
}
