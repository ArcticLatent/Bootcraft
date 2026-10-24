import Bootcraft
import QtQuick
import QtQuick.Layouts

// One app or system change in a list: check box, icon, name and
// description, status and source.
Rectangle {
    id: root

    property var app: null
    property string icon: ""
    property string title
    property string subtitle
    property string warning: ""
    property bool checked
    /// Required tasks can't be unticked on their own.
    property bool checkable: true
    /// "done": already in effect ("Installed" for apps).
    property string status: ""
    /// Right-aligned note, e.g. "Needed by Steam".
    property string note: ""
    property bool last: false

    signal toggled

    implicitHeight: Math.max(66, layout.implicitHeight + 22)
    color: rowMouse.containsMouse ? Theme.alpha(Theme.text, 0.025) : "transparent"

    MouseArea {
        id: rowMouse
        anchors.fill: parent
        hoverEnabled: true
        enabled: root.checkable
        cursorShape: root.checkable ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.toggled()
    }

    RowLayout {
        id: layout
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: 20
        anchors.rightMargin: 20
        spacing: 16

        CheckMark {
            visible: root.checkable
            checked: root.checked
            onToggled: root.toggled()
        }

        Item {
            visible: !root.checkable
            implicitWidth: 22
            implicitHeight: 22

            Icon {
                anchors.centerIn: parent
                name: "check"
                size: 16
                color: Theme.textFaint
            }
        }

        AppIcon {
            app: root.app
            icon: root.icon
            size: 36
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 3

            Text {
                Layout.fillWidth: true
                text: root.title
                color: Theme.text
                font.pixelSize: 15
                font.weight: Font.Medium
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                visible: text !== ""
                text: root.subtitle
                color: Theme.textMuted
                font.pixelSize: 13
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
            }

            Text {
                Layout.fillWidth: true
                visible: text !== ""
                text: root.warning
                color: Theme.warning
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }
        }

        Text {
            visible: root.note !== ""
            text: root.note
            color: Theme.textFaint
            font.pixelSize: 13
        }

        Pill {
            visible: root.status === "done"
            label: root.app ? "Installed" : "Done"
            icon: "check"
            textColor: Theme.accent
        }

        SourceBadge {
            visible: root.app !== null
            app: root.app
        }
    }

    Rectangle {
        visible: !root.last
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: 1
        color: Theme.hairline
    }
}
