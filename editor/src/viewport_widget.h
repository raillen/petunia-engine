#pragma once

#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonObject>
#include <QOpenGLWidget>
#include <QPoint>
#include <QSet>
#include <QTimer>

class EngineBridge;
class NavCubeWidget;
class ViewportOverlay;

enum class ComponentMode { Vertex, Edge, Face };

// The 3D view. It draws through the engine and turns keyboard and mouse into camera and
// selection operations. Controls (also listed in Ajuda > Atalhos):
//   arrows / PageUp / PageDown move, Shift is faster, mouse wheel moves forward/back,
//   hold the right mouse button to look around AND fly with W A S D Q E,
//   left click selects or grabs a gizmo handle (or a face, with Push/Pull and Extrude),
//   with the terrain tool Space stamps the brush once (no drag: the keyboard path).
class ViewportWidget : public QOpenGLWidget {
    Q_OBJECT

public:
    explicit ViewportWidget(EngineBridge* bridge, QWidget* parent = nullptr);

    // Double tap on G, R or S: the free mode of that tool ("move", "rotate", "scale").
    void startFreeMode(const QString& tool);

    // Ends any drag, stroke or pending path, restoring the values it was previewing.
    void cancelActiveEdit();
    // The window calls this before the engine is destroyed: no callback may reach it after that.
    void setEngineAlive(bool alive);

    bool isEditMode() const { return editMode_; }
    void setEditMode(bool on);
    ComponentMode componentMode() const { return componentMode_; }

    // Play mode: the keyboard walks the player's character (W A S D or arrows, Space jumps),
    // the right mouse button turns the camera, Esc stops. A 60 Hz timer sends `play.tick`.
    bool isPlaying() const { return playing_; }
    void setPlaying(bool on);
    void setComponentMode(ComponentMode mode);

signals:
    void selectionChanged();
    // A gizmo drag is changing a value live: the panels should show it, not rebuild.
    void gizmoPreviewed();

    void cameraChanged();
    void renderFailed(const QString& message);
    // One brush stamp applied by the Space key: the window says what it did in the status bar.
    void terrainStepped(const QJsonObject& result);
    void editModeChanged(bool on);
    void editModeToggleRequested();
    void componentModeChanged(ComponentMode mode);
    // Esc while playing.
    void playStopRequested();
    // What the scripts wrote and which handlers failed in the last frame.
    void playOutput(const QJsonArray& log, const QJsonArray& faults);
    // The "press E to …" line, empty when nothing is in range.
    void playPrompt(const QString& text);

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
    void playTick();
    void pickAt(const QPoint& position);
    void pickComponentAt(const QPoint& position, Qt::KeyboardModifiers modifiers);
    QJsonObject pointerFields(const QPoint& position) const;
    bool hasProject() const;
    QString currentTool() const;
    bool faceTool() const;
    bool sculptTool() const;
    bool naturalTool() const;
    bool naturalPathTool() const;
    bool pathPending() const;
    bool naturalPathPending() const;
    void finishFreeMode(bool confirm);
    void handleFreeKey(QKeyEvent* event);
    void updateHover(const QPoint& position);
    void cancelGizmoDrag();
    // The "step" brush: one stamp at `brushPointer_` (or the centre of the view) and one undo step.
    void applyTerrainStep();

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
    bool naturalDragging_ = false;
    bool freeMode_ = false;
    QString freeTyped_;
    QPoint lastPointer_;
    // Where the terrain brush would land: the last pointer position over the view. Invalid while
    // the pointer has never been here, so the step brush falls back to the centre of the view.
    QPoint brushPointer_{-1, -1};
    QString hoveredHandle_;
    QJsonValue hoveredEntity_;
    QJsonValue hoveredFace_;
    ViewportOverlay* overlay_ = nullptr;
    NavCubeWidget* navCube_ = nullptr;
    bool clickCandidate_ = false;
    bool renderErrorReported_ = false;
    bool engineAlive_ = true;
    bool editMode_ = false;
    bool playing_ = false;
    QTimer playTimer_;
    QElapsedTimer playClock_;
    QSet<int> playKeys_;
    bool playJump_ = false;
    bool playInteract_ = false;
    QString playPrompt_;
    ComponentMode componentMode_ = ComponentMode::Face;
};
