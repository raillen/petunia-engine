#pragma once

#include <QWidget>

class QDockWidget;
class QToolButton;

// Compact title bar for a dock: the panel name, then "solta / prende" and "fecha" as small icon
// buttons. Dragging the bar still moves the dock (the label ignores the mouse, so the dock gets it).
class PanelTitleBar : public QWidget {
public:
    explicit PanelTitleBar(QDockWidget* dock);

private:
    void refreshIcons();

    QToolButton* floatButton_;
    QToolButton* closeButton_;
};
