// QML test file for syntax highlighting
import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtQuick.Window 2.15

ApplicationWindow {
    id: mainWindow
    visible: true
    width: 800
    height: 600
    title: qsTr("QML Syntax Test")

    // Custom properties
    property int counter: 0
    property color themeColor: "#2196F3"
    property real animationDuration: 250

    // JavaScript function
    function incrementCounter() {
        counter++
        console.log("Counter:", counter)
    }

    // Background gradient
    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#f5f5f5" }
            GradientStop { position: 1.0; color: "#e0e0e0" }
        }
    }

    // Main content
    ColumnLayout {
        anchors.centerIn: parent
        spacing: 20

        // Custom component
        Component {
            id: customButton

            Rectangle {
                width: 200
                height: 50
                radius: 25
                color: mouseArea.pressed ? Qt.darker(themeColor, 1.2) : themeColor

                Behavior on color {
                    ColorAnimation { duration: animationDuration }
                }

                Text {
                    anchors.centerIn: parent
                    text: "Click Me!"
                    color: "white"
                    font {
                        pixelSize: 16
                        bold: true
                    }
                }

                MouseArea {
                    id: mouseArea
                    anchors.fill: parent
                    onClicked: incrementCounter()
                }
            }
        }

        // Loader for dynamic component
        Loader {
            sourceComponent: customButton
        }

        // Display counter
        Text {
            text: "Count: " + counter
            font.pixelSize: 24
            color: "#333333"
            Layout.alignment: Qt.AlignHCenter
        }

        // ListView example
        ListView {
            width: 300
            height: 200
            model: ListModel {
                ListElement { name: "Item 1"; value: 10 }
                ListElement { name: "Item 2"; value: 20 }
                ListElement { name: "Item 3"; value: 30 }
            }

            delegate: Rectangle {
                width: ListView.view.width
                height: 40
                color: index % 2 == 0 ? "#ffffff" : "#f0f0f0"

                Row {
                    anchors.centerIn: parent
                    spacing: 20

                    Text {
                        text: model.name
                        font.pixelSize: 14
                    }

                    Text {
                        text: "Value: " + model.value
                        font.pixelSize: 12
                        color: "#666666"
                    }
                }
            }
        }

        // State and transitions
        Rectangle {
            id: stateRect
            width: 100
            height: 100
            color: "red"

            states: [
                State {
                    name: "expanded"
                    PropertyChanges { target: stateRect; width: 200; color: "blue" }
                }
            ]

            transitions: Transition {
                NumberAnimation { properties: "width"; duration: 500 }
                ColorAnimation { duration: 500 }
            }

            MouseArea {
                anchors.fill: parent
                onClicked: stateRect.state = stateRect.state === "" ? "expanded" : ""
            }
        }
    }

    // Timer example
    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            console.log("Timer tick at:", new Date().toTimeString())
        }
    }
}
