import Bootcraft
import QtQuick
import QtQuick.Controls.impl

// A Lucide icon (`name: "house"` → icons/house.svg) or a brand logo
// (`name: "logos/fedora"`), tinted with `color`; the SVGs are single-colour.
Item {
    id: root

    property string name
    property int size: 20
    property color color: Theme.textMuted

    implicitWidth: size
    implicitHeight: size
    width: size
    height: size

    IconImage {
        anchors.fill: parent
        sourceSize: Qt.size(root.size, root.size)
        source: !root.name ? "" : root.name.indexOf("/") >= 0 ? root.name + ".svg" : "icons/" + root.name + ".svg"
        color: root.color
    }
}
