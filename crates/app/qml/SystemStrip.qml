import Bootcraft
import QtQuick

// Distribution, graphics, memory and free space, as detected. Wraps onto
// a second line when the window is narrow.
Flow {
    id: strip

    readonly property var info: Setup.system

    spacing: 0

    component Cell: Row {
        property string iconName
        property color iconColor: Theme.textMuted
        property string label
        /// A divider before every cell but the first.
        property bool first: false

        height: 32
        spacing: 10

        // The divider, with room on both sides.
        Rectangle {
            visible: !parent.first
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: 22
            color: Theme.border
        }

        Item {
            visible: !parent.first
            width: 6
            height: 1
        }

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: parent.iconName
            size: 20
            color: parent.iconColor
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: parent.label
            color: Theme.textMuted
            font.pixelSize: 14
        }

        Item {
            width: 6
            height: 1
        }
    }

    Cell {
        first: true
        iconName: strip.info && strip.info.logo ? "logos/" + strip.info.logo : "monitor"
        iconColor: strip.info && strip.info.logoColor ? strip.info.logoColor : Theme.textMuted
        label: strip.info ? strip.info.name : "Detecting…"
    }

    Cell {
        iconName: "gpu"
        label: strip.info ? strip.info.graphics : ""
    }

    Cell {
        visible: strip.info !== null && strip.info.memory !== null
        iconName: "memory-stick"
        label: strip.info && strip.info.memory ? strip.info.memory : ""
    }

    Cell {
        visible: strip.info !== null && strip.info.diskFree !== null
        iconName: "hard-drive"
        label: strip.info && strip.info.diskFree ? strip.info.diskFree : ""
    }
}
