import QtQuick
import "../services"

// eDEX-style instrument frame: thin outline, angular corner brackets, mono
// uppercase header with a red notch. Content goes in `contentItem`.
Item {
    id: root

    property string title: ""
    default property alias contentData: content.data
    // Optional right-aligned content on the header line (e.g. a legend).
    property alias headerData: headerExtra.data

    Rectangle {
        anchors.fill: parent
        color: Theme.surface
        border.width: 1
        border.color: Theme.line
    }

    // Corner brackets.
    Repeater {
        model: 4

        Item {
            readonly property bool isRight: index % 2 === 1
            readonly property bool isBottom: index >= 2

            x: isRight ? root.width - 10 : 0
            y: isBottom ? root.height - 10 : 0
            width: 10
            height: 10

            Rectangle {
                width: 10
                height: 2
                y: parent.isBottom ? 8 : 0
                color: Theme.redDim
            }

            Rectangle {
                width: 2
                height: 10
                x: parent.isRight ? 8 : 0
                color: Theme.redDim
            }
        }
    }

    Row {
        id: header

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.topMargin: 8
        anchors.leftMargin: 12
        spacing: 8

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 4
            height: 12
            color: Theme.red
        }

        Text {
            text: root.title
            font.family: Theme.monoFamily
            font.pixelSize: Theme.fontSize - 1
            font.letterSpacing: 2
            color: Theme.textMuted
        }
    }

    Item {
        id: headerExtra

        anchors.top: parent.top
        anchors.right: parent.right
        anchors.topMargin: 8
        anchors.rightMargin: 12
        height: header.height
        width: childrenRect.width
    }

    Item {
        id: content

        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 12
        anchors.topMargin: 8
        clip: true
    }
}