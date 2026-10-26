#pragma once

#include <QFrame>
#include <QList>
#include <QPointer>

class QAction;
class QLineEdit;
class QListWidget;
class QLabel;
class QMenu;

// Ctrl+K: finds any command of the window by name and runs it. The list is built from the menus,
// so every command that has a menu entry (and its shortcut) is here with no extra registration.
// Matching forgives typos of order and accents ("cvr" finds "Caverna"); matched letters are bold.
// Adapted from Petunia3D's PetuniaCommandPalette, which searched through the Rust bridge.
class CommandPalette : public QFrame {
    Q_OBJECT

public:
    CommandPalette(QWidget* window, QList<QMenu*> menus);

    // Shows the palette centered under the top of the window, with the text field focused.
    void open();

protected:
    bool eventFilter(QObject* watched, QEvent* event) override;

private:
    struct Entry {
        QPointer<QAction> action;
        QString label;
        QString path;
        QString searchable;
    };

    void collect();
    void refresh();
    void runCurrent();

    QWidget* window_;
    QList<QMenu*> menus_;
    QList<Entry> entries_;
    QLineEdit* query_;
    QListWidget* results_;
    QLabel* empty_;
};
