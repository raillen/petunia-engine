#pragma once

#include <QComboBox>
#include <QEvent>
#include <QObject>

// A combo box changes its selection when the mouse wheel passes over it, which silently edits a
// value while someone is only scrolling a panel. This filter lets the wheel through to the
// scroll area unless the combo box has keyboard focus (a deliberate click or Tab).
class WheelGuard : public QObject {
public:
    // Makes `box` ignore the wheel until it is focused.
    static void protect(QComboBox* box) {
        static WheelGuard guard;
        box->setFocusPolicy(Qt::StrongFocus);
        box->installEventFilter(&guard);
    }

protected:
    bool eventFilter(QObject* watched, QEvent* event) override {
        if (event->type() == QEvent::Wheel) {
            auto* box = qobject_cast<QComboBox*>(watched);
            if (box != nullptr && !box->hasFocus()) {
                event->ignore();
                return true;  // swallowed here; the parent scroll area still gets its own wheel event
            }
        }
        return QObject::eventFilter(watched, event);
    }
};
