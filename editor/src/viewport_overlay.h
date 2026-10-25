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

// Navigation cube in the top-right corner (Petunia3D's PetuniaNavCube, copied unchanged): click a
// face, edge or corner to look from there, a compass letter to turn, the house to go home.
class NavCubeWidget : public QWidget {
    Q_OBJECT

public:
    explicit NavCubeWidget(EngineBridge* bridge, QWidget* parent = nullptr);

    // Camera basis from the overlay model (right, up, forward).
    void setCamera(const QJsonObject& camera);

signals:
    void viewChanged();

protected:
    void paintEvent(QPaintEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void leaveEvent(QEvent* event) override;

private:
    void layoutCube();

    EngineBridge* bridge_;
    PetuniaNavCube cube_;
    PetuniaNavCube::Hit hover_;
};
