#pragma once

#include <QJsonObject>
#include <QWidget>

class QComboBox;
class QPushButton;
class QToolButton;
class QLabel;

// Strip above the viewport: grid snapping (on/off, the three steps, the target) and the floor.
// The gizmo controls (Global/Local, pivot, edit pivot) live in `transformControls()`, a separate
// widget the main window places in the top bar. It only shows and asks; the engine owns the values.
class ViewportHeader : public QWidget {
    Q_OBJECT

public:
    explicit ViewportHeader(QWidget* parent = nullptr);

    // Global/Local, pivot and edit pivot, for the top bar. Owned by whoever adds it to a layout.
    QWidget* transformControls() const { return transformControls_; }

    // Follows the engine's `snap` object without emitting signals.
    void showSnap(const QJsonObject& snap);
    // Follows the engine's `transform` object (orientation, pivot).
    void showTransform(const QJsonObject& transform);
    // Follows the engine's `floor` object (current floor, list of floors).
    void showFloor(const QJsonObject& floor);
    // Icons depend on the theme color.
    void refreshIcons();
    // Enables or disables both strips (no project open = nothing to edit).
    void setControlsEnabled(bool enabled);

signals:
    void snapRequested(const QJsonObject& fields);
    void transformRequested(const QJsonObject& fields);
    void floorStepRequested(int delta);
    void floorSelectRequested(double entity);
    void viewRequested(const QString& name);

private:
    QComboBox* makeStepBox(const QString& accessibleName, const QList<double>& steps, const QString& suffix);
    void select(QComboBox* box, double value);
    void emitTransform();

    QWidget* transformControls_;
    QToolButton* snapButton_;
    QComboBox* translateBox_;
    QComboBox* rotateBox_;
    QComboBox* scaleBox_;
    QPushButton* globalButton_;
    QPushButton* localButton_;
    QComboBox* pivotBox_;
    QComboBox* targetBox_;
    QToolButton* slopeButton_;
    QToolButton* editPivotButton_;
    QComboBox* floorBox_;
    QToolButton* floorDown_;
    QToolButton* floorUp_;
};
