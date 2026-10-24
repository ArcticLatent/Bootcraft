import Bootcraft
import QtCore
import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts

// Top right of the Overview: what the plan adds up to, and Start setup.
Rectangle {
    id: root

    implicitWidth: 320
    implicitHeight: layout.implicitHeight + 36
    radius: Theme.radius
    color: Theme.raised
    border.color: Theme.border

    ColumnLayout {
        id: layout
        anchors.fill: parent
        anchors.margins: 18
        spacing: 10

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            Text {
                Layout.fillWidth: true
                text: "Ready when you are"
                color: Theme.text
                font.pixelSize: 17
                font.weight: Font.DemiBold
            }

            Icon {
                name: Setup.planError ? "triangle-alert" : "shield-check"
                size: 18
                color: Setup.planError ? Theme.warning : Theme.textFaint
            }
        }

        Text {
            Layout.fillWidth: true
            text: {
                if (Setup.planError)
                    return Setup.planError;
                const apps = Setup.appCount === 1 ? "1 app" : Setup.appCount + " apps";
                const changes = Setup.systemCount === 1 ? "1 system change" : Setup.systemCount + " system changes";
                const restart = Setup.plan && Setup.plan.reboot ? " · restart likely" : "";
                return apps + " · " + changes + restart;
            }
            color: Setup.planError ? Theme.warning : Theme.textMuted
            font.pixelSize: 14
            wrapMode: Text.WordWrap
        }

        PrimaryButton {
            Layout.fillWidth: true
            Layout.topMargin: 4
            compact: true
            label: Setup.busy ? "Setting up…" : "Start setup"
            icon: Setup.busy ? "" : "arrow-right"
            enabled: Setup.plan !== null && !Setup.busy
            onClicked: Setup.start()
        }

        RowLayout {
            Layout.fillWidth: true

            LinkButton {
                icon: "file-code"
                label: "Export plan"
                fontSize: 13
                enabled: Setup.plan !== null
                onClicked: saveDialog.open()
            }

            Item {
                Layout.fillWidth: true
            }

            Text {
                id: note
                text: "Nothing changes until you start."
                color: Theme.textFaint
                font.pixelSize: 12
                elide: Text.ElideRight
            }
        }
    }

    FileDialog {
        id: saveDialog
        title: "Export setup plan"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "txt"
        currentFile: StandardPaths.writableLocation(StandardPaths.HomeLocation) + "/bootcraft-plan.txt"
        nameFilters: ["Text files (*.txt)"]
        onAccepted: {
            const path = decodeURIComponent(selectedFile.toString().replace(/^file:\/\//, ""));
            Setup.exportPlan(path).then(r => note.text = "Saved to " + r.path).catch(e => note.text = String(e));
        }
    }
}
