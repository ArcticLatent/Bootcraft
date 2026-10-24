import Bootcraft
import QtQuick

// The Bootcraft mark: a power symbol with a spark, and the name.
Row {
    spacing: 12

    Item {
        width: 44
        height: 44
        anchors.verticalCenter: parent.verticalCenter

        Icon {
            anchors.centerIn: parent
            name: "power"
            size: 40
            color: Theme.accent
        }

        Icon {
            x: 32
            y: -3
            name: "sparkle"
            size: 15
            color: Theme.sparkle
        }
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: "Bootcraft"
        color: Theme.text
        font.pixelSize: 30
        font.weight: Font.Bold
    }
}
