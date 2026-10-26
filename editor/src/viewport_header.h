#pragma once

#include <QJsonObject>
#include <QWidget>

class QAction;
class QActionGroup;
class QButtonGroup;
class QCheckBox;
class QComboBox;
class QFrame;
class QMenu;
class QPushButton;
class QToolButton;
enum class ComponentMode;

// Strip above the viewport, the "tool settings" of the scene (Unity and Blender put them here):
// axes (Global/Local), pivot, edit pivot, snapping and the floor. Snapping is one toggle that shows
// the current step; its steps, target and slope option open in a small panel (the ⌄ next to it).
// It only shows and asks; the engine owns the values.
class ViewportHeader : public QWidget {
    Q_OBJECT

public:
    explicit ViewportHeader(QWidget* parent = nullptr);

    // Follows the engine's `snap` object without emitting signals.
    void showSnap(const QJsonObject& snap);
    // Follows the engine's `transform` object (orientation, pivot).
    void showTransform(const QJsonObject& transform);
    // Follows the engine's `floor` object (current floor, list of floors).
    void showFloor(const QJsonObject& floor);
    void showEditMode(bool on);
    void showComponentMode(ComponentMode mode);
    // Icons depend on the theme color.
    void refreshIcons();
    void setControlsEnabled(bool enabled) { setEnabled(enabled); }

signals:
    void snapRequested(const QJsonObject& fields);
    void transformRequested(const QJsonObject& fields);
    void floorStepRequested(int delta);
    void floorSelectRequested(double entity);
    void viewRequested(const QString& name);
    void editModeRequested(bool on);
    void componentModeRequested(ComponentMode mode);

private:
    QComboBox* makeStepBox(QWidget* parent, const QString& accessibleName, const QList<double>& steps, const QString& suffix);
    void select(QComboBox* box, double value);
    void emitTransform();
    void emitSnap();
    void updateSnapText();

    QButtonGroup* modeGroup_ = nullptr;
    QPushButton* objectModeButton_ = nullptr;
    QPushButton* editModeButton_ = nullptr;
    QButtonGroup* componentGroup_ = nullptr;
    QPushButton* vertexButton_ = nullptr;
    QPushButton* edgeButton_ = nullptr;
    QPushButton* faceButton_ = nullptr;
    QFrame* componentSeparator_ = nullptr;

    QPushButton* globalButton_;
    QPushButton* localButton_;
    QToolButton* pivotButton_;
    QActionGroup* pivotGroup_;
    QToolButton* editPivotButton_;
    QToolButton* snapButton_;
    QToolButton* snapOptionsButton_;
    QFrame* snapPanel_;
    QComboBox* translateBox_;
    QComboBox* rotateBox_;
    QComboBox* scaleBox_;
    QComboBox* targetBox_;
    QCheckBox* slopeBox_;
    QComboBox* floorBox_;
    QToolButton* floorDown_;
    QToolButton* floorUp_;
    QToolButton* viewButton_;
};
