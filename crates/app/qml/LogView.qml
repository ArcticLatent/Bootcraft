import Bootcraft
import QtQuick
import QtQuick.Controls

// The live output of a setup, terminal style. Follows new lines unless
// scrolled up.
ListView {
    id: log

    property bool following: true

    clip: true
    model: Setup.log
    onMovementEnded: following = atYEnd
    onCountChanged: if (following)
        positionViewAtEnd()

    ScrollBar.vertical: ScrollBar {}

    delegate: Text {
        required property string kind
        required property string line

        width: log.width
        topPadding: kind === "task" ? 8 : 0
        // Steps are descriptions; the commands they run follow as `$ …` lines.
        text: kind === "task" ? "▸ " + line : kind === "step" ? "  → " + line : "    " + line
        color: kind === "task" ? Theme.text : kind === "step" ? Theme.accent : kind === "error" ? Theme.error : kind === "stderr" ? Theme.warning : "#b4bac0"
        font.family: Theme.monoFamily
        // Show tool output as printed: no `==>` turned into an arrow.
        font.features: {
            "calt": 0,
            "liga": 0
        }
        font.pixelSize: 13
        font.weight: kind === "task" ? Font.Bold : Font.Normal
        wrapMode: Text.WrapAnywhere
    }
}
