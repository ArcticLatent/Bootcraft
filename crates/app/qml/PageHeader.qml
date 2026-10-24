import Bootcraft
import QtQuick
import QtQuick.Layouts

// A page's title and one line about it.
ColumnLayout {
    property string title
    property string subtitle

    spacing: 8

    Text {
        text: parent.title
        color: Theme.text
        font.pixelSize: 32
        font.weight: Font.Bold
    }

    Text {
        Layout.fillWidth: true
        visible: text !== ""
        text: parent.subtitle
        color: Theme.textMuted
        font.pixelSize: 16
        wrapMode: Text.WordWrap
    }
}
