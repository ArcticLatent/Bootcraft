import Bootcraft
import QtQuick

// An app's own icon from Flathub once it is cached, else the Lucide icon
// for its section on a tile. Also used for task icons (`icon` set, no `app`).
Item {
    id: root

    property var app: null
    property string icon: ""
    property int size: 36
    readonly property string url: app ? (Setup.appIcons[app.id] || "") : ""

    implicitWidth: size
    implicitHeight: size

    Image {
        anchors.fill: parent
        visible: root.url !== ""
        source: root.url
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true
    }

    Rectangle {
        anchors.fill: parent
        visible: root.url === ""
        radius: 8
        color: Theme.badge
        border.color: Theme.border

        Icon {
            anchors.centerIn: parent
            name: root.icon || (root.app ? Theme.sectionIcon(root.app.section) : "box")
            size: Math.round(root.size * 0.5)
            color: Theme.textMuted
        }
    }
}
