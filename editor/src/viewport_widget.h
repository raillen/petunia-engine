#pragma once

#include <QElapsedTimer>
#include <QJsonObject>
#include <QOpenGLWidget>
#include <QPoint>
#include <QSet>
#include <QTimer>

class EngineBridge;
class NavCubeWidget;
class ViewportOverlay;

// The 3D view. It draws through the engine and turns keyboard and mouse into camera and
// selection operations. Controls (also listed in Ajuda > Atalhos):
//   arrows / PageUp / PageDown move, Shift is faster, mouse wheel moves forward/back,
//   hold the right mouse button to look around AND fly with W A S D Q E,
//   left click selects or grabs a gizmo handle (or a face, with Push/Pull and Extrude).
class ViewportWidget : public QOpenGLWidget {
    Q_OBJECT

public:
    explicit ViewportWidget(EngineBridge* bridge, QWidget* parent = nullptr);

    // Double tap on G, R or S: the free mode of that tool ("move", "rotate", "scale").
    void startFreeMode(const QString& tool);

signals:
    void selectionChanged();
    // A gizmo drag is changing a value live: the panels should show it, not rebuild.
    void gizmoPreviewed();

    void cameraChanged();
    void renderFailed(const QString& message);

protected:
    void initializeGL() override;
    void paintGL() override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void wheelEvent(QWheelEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    void keyReleaseEvent(QKeyEvent* event) override;
    void focusOutEvent(QFocusEvent* event) override;
    void leaveEvent(QEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
    bool event(QEvent* event) override;

private:
    void tick();
    void pickAt(const QPoint& position);
    QJsonObject pointerFields(const QPoint& position) const;
    bool hasProject() const;
    QString currentTool() const;
    bool faceTool() const;
    bool sculptTool() const;
    bool pathPending() const;
    void finishFreeMode(bool confirm);
    void handleFreeKey(QKeyEvent* event);
    void updateHover(const QPoint& position);
    void cancelGizmoDrag();

    EngineBridge* bridge_;
    QTimer moveTimer_;
    QElapsedTimer clock_;
    QSet<int> heldKeys_;
    QPoint lastMouse_;
    QPoint pressPosition_;
    bool looking_ = false;
    bool gizmoDragging_ = false;
    bool faceDragging_ = false;
    bool sculptDragging_ = false;
    bool freeMode_ = false;
    QString freeTyped_;
    QPoint lastPointer_;
    QString hoveredHandle_;
    QJsonValue hoveredEntity_;
    QJsonValue hoveredFace_;
    ViewportOverlay* overlay_ = nullptr;
    NavCubeWidget* navCube_ = nullptr;
    bool clickCandidate_ = false;
    bool renderErrorReported_ = false;
};
