#pragma once

#include <QJsonObject>
#include <QWidget>

#include "petunia_nav_cube.h"

class EngineBridge;

// Paints everything that sits on top of the 3D view, from the engine's `viewport.overlay` model:
// the transform gizmo (ported from Petunia3D's screen-space gizmo), axis guides while dragging,
// the pivot and 3D cursor markers, axis letters, and the heads-up card of the current gesture.
// It never takes the mouse: clicks go to the viewport underneath.
class ViewportOverlay : public QWidget {
    Q_OBJECT

public:
    explicit ViewportOverlay(EngineBridge* bridge, QWidget* parent = nullptr);

    // Fetches a fresh model and schedules a repaint.
    void refresh();
    const QJsonObject& model() const { return model_; }

protected:
    void paintEvent(QPaintEvent* event) override;

private:
    EngineBridge* bridge_;
    QJsonObject model_;
};

// Navigation cube in the top-right corner: click a preset, drag to orbit, or use the
// projection menu. Arrow keys orbit; Space/Return open the menu.
class NavCubeWidget : public QWidget {
    Q_OBJECT

public:
    explicit NavCubeWidget(EngineBridge* bridge, QWidget* parent = nullptr);

    // Camera basis from the overlay model (right, up, forward).
    void setCamera(const QJsonObject& camera);

signals:
    void viewChanged();
    void operationFailed(const QString& message);

protected:
    void paintEvent(QPaintEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    void keyReleaseEvent(QKeyEvent* event) override;
    void focusInEvent(QFocusEvent* event) override;
    void focusOutEvent(QFocusEvent* event) override;
    void leaveEvent(QEvent* event) override;

private:
    void layout_cube();
    void show_projection_menu(const QPoint& global_position);
    void handle_hit(const PetuniaNavCube::Hit& hit, const QPoint& global_position);
    void invoke_command(const QString& operation, const QJsonObject& fields = {});

    EngineBridge* bridge_ = nullptr;
    PetuniaNavCube cube_;
    PetuniaNavCube::Hit hover_;
    PetuniaNavCube::Hit pressed_hit_;
    QPointF press_position_;
    QPointF last_drag_position_;
    bool has_left_press_ = false;
    bool is_dragging_ = false;
};
