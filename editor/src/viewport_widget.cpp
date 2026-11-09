#include "viewport_widget.h"

#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonObject>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QCursor>
#include <QResizeEvent>
#include <QWheelEvent>

#include "engine_bridge.h"
#include "viewport_overlay.h"

namespace {

constexpr int kFrameMilliseconds = 16;
constexpr int kClickSlopPixels = 4;

// Letters fly the camera only while the right mouse button is held (so Q W E R can pick tools).
bool isLetterMovementKey(int key) {
    switch (key) {
        case Qt::Key_W: case Qt::Key_A: case Qt::Key_S: case Qt::Key_D:
        case Qt::Key_Q: case Qt::Key_E:
            return true;
        default:
            return false;
    }
}

// Arrows and PageUp/PageDown always move the camera.
bool isAlwaysMovementKey(int key) {
    switch (key) {
        case Qt::Key_Up: case Qt::Key_Down: case Qt::Key_Left: case Qt::Key_Right:
        case Qt::Key_PageUp: case Qt::Key_PageDown:
            return true;
        default:
            return false;
    }
}

}  // namespace

ViewportWidget::ViewportWidget(EngineBridge* bridge, QWidget* parent)
    : QOpenGLWidget(parent), bridge_(bridge) {
    setFocusPolicy(Qt::StrongFocus);
    setMouseTracking(true);  // hover highlights the gizmo handle under the pointer
    setMinimumSize(320, 240);
    setAccessibleName(QStringLiteral("Viewport 3D"));
    setAccessibleDescription(QStringLiteral(
        "Vista 3D da cena. Setas movem a câmera. Segure o botão direito do mouse para girar "
        "e voar com W A S D Q E. Clique esquerdo seleciona ou arrasta o gizmo; "
        "com Empurrar/Puxar ou Extrudar, arraste uma face. Com a ferramenta de terreno, "
        "a barra de espaços aplica um carimbo do pincel sem arrastar."));
    overlay_ = new ViewportOverlay(bridge_, this);
    navCube_ = new NavCubeWidget(bridge_, this);
    connect(navCube_, &NavCubeWidget::viewChanged, this, [this] {
        update();
        emit cameraChanged();
    });
    connect(navCube_, &NavCubeWidget::operationFailed, this, &ViewportWidget::renderFailed);
    moveTimer_.setInterval(kFrameMilliseconds);
    connect(&moveTimer_, &QTimer::timeout, this, &ViewportWidget::tick);
    playTimer_.setInterval(kFrameMilliseconds);
    connect(&playTimer_, &QTimer::timeout, this, &ViewportWidget::playTick);
}

void ViewportWidget::initializeGL() {
    const QString error = bridge_->initViewport();
    if (!error.isEmpty()) {
        renderErrorReported_ = true;
        emit renderFailed(QStringLiteral("Não foi possível iniciar o OpenGL: %1").arg(error));
    }
}

void ViewportWidget::paintGL() {
    const qreal ratio = devicePixelRatioF();
    const QString error = bridge_->render(qRound(width() * ratio), qRound(height() * ratio));
    if (!error.isEmpty() && !renderErrorReported_) {
        renderErrorReported_ = true;
        emit renderFailed(QStringLiteral("Erro ao desenhar: %1").arg(error));
    }
    // The overlay and the cube follow every frame (camera, selection, drag).
    overlay_->refresh();
    navCube_->setCamera(overlay_->model().value(QStringLiteral("camera")).toObject());
    navCube_->setVisible(bridge_->hasProject());
}

void ViewportWidget::resizeEvent(QResizeEvent* event) {
    QOpenGLWidget::resizeEvent(event);
    overlay_->setGeometry(rect());
    navCube_->move(width() - navCube_->width() - 6, 6);
    navCube_->raise();
}

void ViewportWidget::setPlaying(bool on) {
    if (playing_ == on) {
        return;
    }
    playing_ = on;
    playKeys_.clear();
    playJump_ = false;
    playInteract_ = false;
    if (!playPrompt_.isEmpty()) {
        playPrompt_.clear();
        emit playPrompt(QString());
    }
    heldKeys_.clear();
    moveTimer_.stop();
    if (on) {
        playClock_.start();
        playTimer_.start();
        setFocus(Qt::OtherFocusReason);
    } else {
        playTimer_.stop();
    }
    update();
}

void ViewportWidget::playTick() {
    if (!playing_) {
        return;
    }
    const double seconds = playClock_.restart() / 1000.0;
    double forward = 0;
    double right = 0;
    for (const int key : std::as_const(playKeys_)) {
        switch (key) {
            case Qt::Key_W: case Qt::Key_Up: forward += 1; break;
            case Qt::Key_S: case Qt::Key_Down: forward -= 1; break;
            case Qt::Key_D: case Qt::Key_Right: right += 1; break;
            case Qt::Key_A: case Qt::Key_Left: right -= 1; break;
            default: break;
        }
    }
    const bool jump = playJump_;
    playJump_ = false;
    const bool interact = playInteract_;
    playInteract_ = false;
    const Reply reply = bridge_->call(QStringLiteral("play.tick"),
                                      QJsonObject{{"seconds", seconds},
                                                  {"forward", forward},
                                                  {"right", right},
                                                  {"jump", jump},
                                                  {"interact", interact}});
    if (!reply.ok) {
        playTimer_.stop();
        emit renderFailed(reply.message);
        emit playStopRequested();
        return;
    }
    const QJsonObject result = reply.result.toObject();
    const QJsonArray log = result.value(QStringLiteral("log")).toArray();
    const QJsonArray faults = result.value(QStringLiteral("faults")).toArray();
    if (!log.isEmpty() || !faults.isEmpty()) {
        emit playOutput(log, faults);
    }
    const QString prompt = result.value(QStringLiteral("prompt")).toString();
    if (prompt != playPrompt_) {
        playPrompt_ = prompt;
        emit playPrompt(prompt);
    }
    update();
    emit cameraChanged();
}

void ViewportWidget::tick() {
    const double seconds = clock_.restart() / 1000.0;
    double right = 0;
    double up = 0;
    double forward = 0;
    for (const int key : std::as_const(heldKeys_)) {
        switch (key) {
            case Qt::Key_W: case Qt::Key_Up: forward += 1; break;
            case Qt::Key_S: case Qt::Key_Down: forward -= 1; break;
            case Qt::Key_D: case Qt::Key_Right: right += 1; break;
            case Qt::Key_A: case Qt::Key_Left: right -= 1; break;
            case Qt::Key_E: case Qt::Key_PageUp: up += 1; break;
            case Qt::Key_Q: case Qt::Key_PageDown: up -= 1; break;
            default: break;
        }
    }
    if (right == 0 && up == 0 && forward == 0) {
        moveTimer_.stop();
        return;
    }
    const bool boost = QGuiApplication::keyboardModifiers().testFlag(Qt::ShiftModifier);
    bridge_->call(QStringLiteral("camera.advance"),
                  QJsonObject{{"right", right}, {"up", up}, {"forward", forward},
                              {"boost", boost}, {"seconds", seconds}});
    update();
    emit cameraChanged();
}

void ViewportWidget::keyPressEvent(QKeyEvent* event) {
    const int key = event->key();
    const bool plain = !(event->modifiers() & (Qt::ControlModifier | Qt::AltModifier));
    const bool movement = plain && (isAlwaysMovementKey(key) || (looking_ && isLetterMovementKey(key)));
    if (movement) {
        if (!event->isAutoRepeat()) {
            heldKeys_.insert(key);
            if (!moveTimer_.isActive()) {
                clock_.start();
                moveTimer_.start();
            }
        }
        event->accept();
        return;
    }
    QOpenGLWidget::keyPressEvent(event);
}

void ViewportWidget::keyReleaseEvent(QKeyEvent* event) {
    // The live mesh value ends when the key that opened it is let go: a tap applies the value of
    // the bar (as before), holding and moving adjusts it.
    if (!event->isAutoRepeat()
        && (event->key() == Qt::Key_I || event->key() == Qt::Key_E || event->key() == Qt::Key_B
            || event->key() == Qt::Key_R)) {
        emit meshValueKeyReleased();
    }
    if (isAlwaysMovementKey(event->key()) || isLetterMovementKey(event->key())) {
        if (!event->isAutoRepeat()) {
            heldKeys_.remove(event->key());
        }
        event->accept();
        return;
    }
    QOpenGLWidget::keyReleaseEvent(event);
}

namespace {

// Keys free mode keeps for itself (axis letters, the typed number, confirm and cancel).
bool isFreeModeKey(int key) {
    return key == Qt::Key_X || key == Qt::Key_Y || key == Qt::Key_Z || (key >= Qt::Key_0 && key <= Qt::Key_9) ||
           key == Qt::Key_Minus || key == Qt::Key_Comma || key == Qt::Key_Period || key == Qt::Key_Backspace ||
           key == Qt::Key_Return || key == Qt::Key_Enter || key == Qt::Key_Escape;
}

}  // namespace

QString ViewportWidget::currentTool() const {
    return bridge_->snapshot().value(QStringLiteral("tool")).toString();
}

// B (sculpt) and Shift+B (path): the brush ring follows the mouse over the rock.
bool ViewportWidget::sculptTool() const {
    const QString tool = currentTool();
    return tool == QLatin1String("sculpt") || tool == QLatin1String("sculpt_path");
}

bool ViewportWidget::naturalTool() const {
    const QString tool = currentTool();
    return tool == QLatin1String("terrain") || tool == QLatin1String("river_path") || tool == QLatin1String("road_path");
}

bool ViewportWidget::naturalPathTool() const {
    const QString tool = currentTool();
    return tool == QLatin1String("river_path") || tool == QLatin1String("road_path");
}

bool ViewportWidget::naturalPathPending() const {
    return naturalPathTool() &&
           bridge_->snapshot().value(QStringLiteral("natural")).toObject().value(QStringLiteral("path_points")).toInt() > 0;
}

// A tunnel with at least one point waits for Enter (dig), Backspace (undo a point) or Esc (cancel).
bool ViewportWidget::pathPending() const {
    return currentTool() == QLatin1String("sculpt_path") &&
           bridge_->snapshot().value(QStringLiteral("sculpt")).toObject().value(QStringLiteral("path_points")).toInt() > 0;
}

bool ViewportWidget::faceTool() const {
    const QString tool = currentTool();
    return tool == QLatin1String("push_pull") || tool == QLatin1String("extrude");
}

// Double tap on G, R or S (Petunia3D "Modo Livre"): the mouse drives the value right away.
void ViewportWidget::startFreeMode(const QString& tool) {
    QPoint position = mapFromGlobal(QCursor::pos());
    if (!rect().contains(position)) {
        position = rect().center();
    }
    QJsonObject fields = pointerFields(position);
    fields.insert(QStringLiteral("tool"), tool);
    const Reply reply = bridge_->call(QStringLiteral("free.begin"), fields);
    if (!reply.ok) {
        emit renderFailed(reply.message);
        return;
    }
    freeMode_ = true;
    freeTyped_.clear();
    lastPointer_ = position;
    setFocus(Qt::OtherFocusReason);
    setCursor(Qt::SizeAllCursor);
    emit selectionChanged();
    update();
}

void ViewportWidget::setEditMode(bool on) {
    if (editMode_ == on) {
        return;
    }
    if (!on) {
        if (livePreviewActive_) {
            finishMeshPreview(false);
        }
        if (knifeArmed_) {
            knifeArmed_ = false;
            knifeStart_ = QJsonObject();
            emit knifeArmedChanged(false);
        }
    }
    editMode_ = on;
    if (editMode_) {
        // Editing happens in the 3D view: give it the keyboard so the mesh shortcuts (K, I, E…)
        // work right after choosing an object in the hierarchy.
        setFocus(Qt::OtherFocusReason);
        setCursor(Qt::CrossCursor);
    } else {
        unsetCursor();
    }
    emit editModeChanged(editMode_);
    update();
}

void ViewportWidget::setKnifeArmed(bool armed) {
    knifeArmed_ = armed;
    knifeStart_ = QJsonObject();
    if (armed) {
        setCursor(Qt::CrossCursor);
    } else if (!editMode_) {
        unsetCursor();
    }
    updateKnifeOverlay();
    emit knifeArmedChanged(knifeArmed_);
    update();
}

void ViewportWidget::setLiveMeshOperation(const QString& operation) {
    liveMeshOperation_ = operation;
    if (operation.isEmpty() && livePreviewActive_) {
        finishMeshPreview(false);
    }
}

void ViewportWidget::setLiveMeshValues(const QJsonObject& values) {
    liveMeshValues_ = values;
}

QString ViewportWidget::livePreviewReason(const QJsonObject& result) const {
    const QJsonObject reason = result.value(QStringLiteral("reason")).toObject();
    const QString message = reason.value(QStringLiteral("message")).toString();
    return message.isEmpty() ? QStringLiteral("o valor ainda não funciona") : message;
}

void ViewportWidget::startMeshPreview(const QPoint& position) {
    if (!hasProject() || !bridge_->hasSelection() || liveMeshOperation_.isEmpty()) {
        return;
    }
    livePreviewActive_ = true;
    livePreviewStart_ = position;
    // The bar's fields are the starting values; the drag changes the one that operation uses.
    livePreviewStartValue_ = 0.0;
    if (liveMeshOperation_ == QLatin1String("loop_cut")) {
        livePreviewStartValue_ = liveMeshValues_.value(QStringLiteral("slide")).toDouble();
    } else if (liveMeshOperation_ == QLatin1String("bevel")) {
        livePreviewStartValue_ = liveMeshValues_.value(QStringLiteral("width")).toDouble();
    } else {
        livePreviewStartValue_ = liveMeshValues_.value(QStringLiteral("distance")).toDouble();
    }
    QJsonObject fields = liveMeshValues_;
    fields.insert(QStringLiteral("operation"), liveMeshOperation_);
    fields.insert(QStringLiteral("entity"), static_cast<double>(bridge_->selection()));
    const Reply reply = bridge_->call(QStringLiteral("mesh.preview"), fields);
    if (!reply.ok) {
        livePreviewActive_ = false;
        emit meshStatus(reply.explanation());
        return;
    }
    const QJsonObject result = reply.result.toObject();
    if (!result.value(QStringLiteral("valid")).toBool()) {
        emit meshStatus(QStringLiteral("Prévia: %1").arg(livePreviewReason(result)));
    }
    update();
}

void ViewportWidget::updateMeshPreview(const QPoint& position, Qt::KeyboardModifiers modifiers) {
    if (!livePreviewActive_ || liveMeshOperation_.isEmpty()) {
        return;
    }
    const double pixels = position.x() - livePreviewStart_.x();
    const double step = modifiers.testFlag(Qt::ShiftModifier) ? 0.001 : 0.005;
    QJsonObject fields = liveMeshValues_;
    fields.insert(QStringLiteral("operation"), liveMeshOperation_);
    fields.insert(QStringLiteral("entity"), static_cast<double>(bridge_->selection()));
    if (liveMeshOperation_ == QLatin1String("loop_cut")) {
        fields.insert(QStringLiteral("slide"),
                      qBound(-1.0, livePreviewStartValue_ + pixels * step, 1.0));
    } else {
        const double value = qMax(0.001, livePreviewStartValue_ + pixels * step);
        fields.insert(liveMeshOperation_ == QLatin1String("bevel") ? QStringLiteral("width")
                                                                  : QStringLiteral("distance"),
                      value);
    }
    const Reply reply = bridge_->call(QStringLiteral("mesh.preview"), fields);
    if (!reply.ok) {
        emit meshStatus(reply.explanation());
        return;
    }
    const QJsonObject result = reply.result.toObject();
    if (!result.value(QStringLiteral("valid")).toBool()) {
        emit meshStatus(QStringLiteral("Prévia: %1").arg(livePreviewReason(result)));
    } else {
        emit meshStatus(QStringLiteral("Prévia ao vivo: solte para aplicar, Esc cancela."));
    }
    emit gizmoPreviewed();
    update();
}

void ViewportWidget::finishMeshPreview(bool commit) {
    if (!livePreviewActive_) {
        return;
    }
    livePreviewActive_ = false;
    if (!engineAlive_) {
        return;
    }
    const Reply reply =
        bridge_->call(QStringLiteral("mesh.preview_end"), QJsonObject{{QStringLiteral("commit"), commit}});
    if (!reply.ok) {
        emit meshStatus(reply.explanation());
    } else if (commit && reply.result.toObject().value(QStringLiteral("committed")).toBool()) {
        emit meshStatus(QStringLiteral("Operação aplicada. Ctrl+Z desfaz."));
    } else if (commit) {
        emit meshStatus(QStringLiteral("Nada mudou — ajuste o valor ou selecione uma parte antes de arrastar."));
    } else {
        emit meshStatus(QStringLiteral("Prévia cancelada."));
    }
    emit selectionChanged();
    update();
}

void ViewportWidget::updateKnifeOverlay() {
    if (overlay_ == nullptr) {
        return;
    }
    overlay_->setKnifeLine(knifeArmed_ && !knifeStart_.isEmpty(), knifeStartScreen_, knifeHoverScreen_);
}

void ViewportWidget::setComponentMode(ComponentMode mode) {
    if (componentMode_ == mode) {
        return;
    }
    componentMode_ = mode;
    emit componentModeChanged(componentMode_);
    update();
}

void ViewportWidget::finishFreeMode(bool confirm) {
    freeMode_ = false;
    freeTyped_.clear();
    bridge_->call(confirm ? QStringLiteral("gizmo.end") : QStringLiteral("gizmo.cancel"));
    unsetCursor();
    emit selectionChanged();
    update();
}

void ViewportWidget::handleFreeKey(QKeyEvent* event) {
    const int key = event->key();
    QJsonObject fields = pointerFields(lastPointer_);
    if (key == Qt::Key_Escape) {
        finishFreeMode(false);
    } else if (key == Qt::Key_Return || key == Qt::Key_Enter) {
        finishFreeMode(true);
    } else if (key == Qt::Key_X || key == Qt::Key_Y || key == Qt::Key_Z) {
        fields.insert(QStringLiteral("axis"), key - Qt::Key_X);
        bridge_->call(QStringLiteral("free.axis"), fields);
    } else {
        if (key == Qt::Key_Backspace) {
            freeTyped_.chop(1);
        } else {
            freeTyped_ += event->text();
        }
        fields.insert(QStringLiteral("text"), freeTyped_);
        bridge_->call(QStringLiteral("free.type"), fields);
    }
    update();
    emit gizmoPreviewed();
}

// While flying, W A S D Q E must reach us, not the tool shortcuts. While dragging, Esc cancels
// the drag instead of clearing the selection. In free mode, axis letters and digits are ours.
bool ViewportWidget::event(QEvent* event) {
    if (playing_ && (event->type() == QEvent::ShortcutOverride || event->type() == QEvent::KeyPress)) {
        // While playing, the game owns the keyboard: S is "backward", not the Scale tool.
        auto* key = static_cast<QKeyEvent*>(event);
        const bool plain = !(key->modifiers() & (Qt::ControlModifier | Qt::AltModifier));
        const int code = key->key();
        const bool gameKey = plain && (code == Qt::Key_W || code == Qt::Key_A || code == Qt::Key_S || code == Qt::Key_D ||
                                       code == Qt::Key_Up || code == Qt::Key_Down || code == Qt::Key_Left ||
                                       code == Qt::Key_Right || code == Qt::Key_Space || code == Qt::Key_E ||
                                       code == Qt::Key_Escape);
        if (gameKey) {
            if (event->type() == QEvent::ShortcutOverride) {
                event->accept();
                return true;
            }
            if (!key->isAutoRepeat()) {
                if (code == Qt::Key_Escape) {
                    emit playStopRequested();
                } else if (code == Qt::Key_Space) {
                    playJump_ = true;
                } else if (code == Qt::Key_E) {
                    playInteract_ = true;
                } else {
                    playKeys_.insert(code);
                }
            }
            return true;
        }
    }
    if (playing_ && event->type() == QEvent::KeyRelease) {
        auto* key = static_cast<QKeyEvent*>(event);
        if (!key->isAutoRepeat()) {
            playKeys_.remove(key->key());
        }
    }
    if (event->type() == QEvent::ShortcutOverride) {
        auto* key = static_cast<QKeyEvent*>(event);
        const bool plain = !(key->modifiers() & (Qt::ControlModifier | Qt::AltModifier));
        if ((plain && key->key() == Qt::Key_Tab) ||
            (looking_ && plain && isLetterMovementKey(key->key())) ||
            ((gizmoDragging_ || faceDragging_ || colliderDragging_) &&
             key->key() == Qt::Key_Escape) ||
            ((livePreviewActive_ || knifeArmed_) && key->key() == Qt::Key_Escape) ||
            (freeMode_ && plain && isFreeModeKey(key->key())) ||
            (plain && key->key() == Qt::Key_Space && hasProject() &&
             currentTool() == QLatin1String("terrain")) ||
            ((pathPending() || naturalPathPending()) &&
             (key->key() == Qt::Key_Escape || key->key() == Qt::Key_Return || key->key() == Qt::Key_Enter ||
              key->key() == Qt::Key_Backspace))) {
            key->accept();
            return true;
        }
    }
    if (event->type() == QEvent::KeyPress) {
        auto* key = static_cast<QKeyEvent*>(event);
        if (key->key() == Qt::Key_Escape && livePreviewActive_) {
            finishMeshPreview(false);
            return true;
        }
        if (key->key() == Qt::Key_Escape && knifeArmed_) {
            setKnifeArmed(false);
            emit meshStatus(QStringLiteral("Faca cancelada."));
            return true;
        }
        if (key->key() == Qt::Key_Tab && !(key->modifiers() & (Qt::ControlModifier | Qt::AltModifier))) {
            emit editModeToggleRequested();
            return true;
        }
        // The keyboard path of the terrain brush: one press, one stamp, no drag at all.
        if (key->key() == Qt::Key_Space && !(key->modifiers() & (Qt::ControlModifier | Qt::AltModifier)) &&
            hasProject() && currentTool() == QLatin1String("terrain") && !naturalDragging_) {
            applyTerrainStep();
            return true;
        }
        if (freeMode_ && isFreeModeKey(key->key())) {
            handleFreeKey(key);
            return true;
        }
        if ((gizmoDragging_ || colliderDragging_) && key->key() == Qt::Key_Escape) {
            cancelGizmoDrag();
            return true;
        }
        if (naturalPathPending()) {
            const int code = key->key();
            if (code == Qt::Key_Return || code == Qt::Key_Enter) {
                const Reply reply = bridge_->call(QStringLiteral("natural.commit"));
                if (!reply.ok) {
                    emit renderFailed(reply.explanation());
                }
                emit selectionChanged();
                update();
                return true;
            }
            if (code == Qt::Key_Backspace || code == Qt::Key_Escape) {
                bridge_->call(code == Qt::Key_Backspace ? QStringLiteral("natural.pop") : QStringLiteral("natural.cancel"));
                emit selectionChanged();
                update();
                return true;
            }
        }
        if (pathPending()) {
            const int code = key->key();
            if (code == Qt::Key_Return || code == Qt::Key_Enter) {
                const Reply reply = bridge_->call(QStringLiteral("path.commit"));
                if (!reply.ok) {
                    emit renderFailed(reply.explanation());
                }
                emit selectionChanged();
                update();
                return true;
            }
            if (code == Qt::Key_Backspace || code == Qt::Key_Escape) {
                bridge_->call(code == Qt::Key_Backspace ? QStringLiteral("path.pop") : QStringLiteral("path.cancel"));
                emit selectionChanged();
                update();
                return true;
            }
        }
        if (sculptDragging_ && key->key() == Qt::Key_Escape) {
            sculptDragging_ = false;
            bridge_->call(QStringLiteral("sculpt.cancel"));
            update();
            return true;
        }
        if (naturalDragging_ && key->key() == Qt::Key_Escape) {
            naturalDragging_ = false;
            bridge_->call(QStringLiteral("terrain.cancel"));
            update();
            return true;
        }
        if (faceDragging_ && key->key() == Qt::Key_Escape) {
            faceDragging_ = false;
            bridge_->call(QStringLiteral("face.cancel"));
            emit selectionChanged();
            update();
            return true;
        }
    }
    return QOpenGLWidget::event(event);
}

void ViewportWidget::cancelGizmoDrag() {
    if (colliderDragging_) {
        colliderDragging_ = false;
        bridge_->call(QStringLiteral("collider.handle_cancel"));
    } else {
        gizmoDragging_ = false;
        bridge_->call(QStringLiteral("gizmo.cancel"));
    }
    unsetCursor();
    emit selectionChanged();  // refresh everything: the value is back to what it was
    update();
}

// The "step" brush (docs/04-acessibilidade/01-editor.md, Pilar 1: "esculpir/pintar sem arrastar"):
// one press stamps the current mode, radius and strength where the pointer is, without holding the
// button or dragging. With the pointer outside the view the stamp lands in the middle of the
// screen, so the keyboard alone is enough (the arrows already move the camera).
void ViewportWidget::applyTerrainStep() {
    const QPoint position = rect().contains(brushPointer_) ? brushPointer_ : rect().center();
    brushPointer_ = position;
    const Reply reply = bridge_->call(QStringLiteral("terrain.step"), pointerFields(position));
    if (!reply.ok) {
        emit renderFailed(reply.explanation());
        return;
    }
    emit terrainStepped(reply.result.toObject());
    emit selectionChanged();  // one new undo step: history and buttons refresh
    update();
}

void ViewportWidget::setEngineAlive(bool alive) {
    engineAlive_ = alive;
}

void ViewportWidget::cancelActiveEdit() {
    auto clearLocalState = [this] {
        freeMode_ = false;
        gizmoDragging_ = false;
        faceDragging_ = false;
        colliderDragging_ = false;
        sculptDragging_ = false;
        naturalDragging_ = false;
        freeTyped_.clear();
        heldKeys_.clear();
        looking_ = false;
        livePreviewActive_ = false;
        knifeArmed_ = false;
        knifeStart_ = QJsonObject();
        hoveredHandle_.clear();
        hoveredEntity_ = QJsonValue();
        hoveredFace_ = QJsonValue();
        hoveredCollider_ = false;
        clickCandidate_ = false;
        unsetCursor();
        updateKnifeOverlay();
    };
    if (!engineAlive_ || !hasProject()) {
        clearLocalState();
        return;
    }
    if (livePreviewActive_) {
        finishMeshPreview(false);
    }
    if (knifeArmed_) {
        setKnifeArmed(false);
    }
    if (freeMode_) {
        finishFreeMode(false);
    } else if (gizmoDragging_ || colliderDragging_) {
        cancelGizmoDrag();
    }
    if (faceDragging_) {
        faceDragging_ = false;
        bridge_->call(QStringLiteral("face.cancel"));
        emit selectionChanged();
    }
    if (sculptDragging_) {
        sculptDragging_ = false;
        bridge_->call(QStringLiteral("sculpt.cancel"));
    }
    if (naturalDragging_) {
        naturalDragging_ = false;
        bridge_->call(QStringLiteral("terrain.cancel"));
    }
    if (pathPending()) {
        bridge_->call(QStringLiteral("path.cancel"));
    }
    if (naturalPathPending()) {
        bridge_->call(QStringLiteral("natural.cancel"));
    }
    clearLocalState();
    update();
}

void ViewportWidget::mousePressEvent(QMouseEvent* event) {
    setFocus(Qt::MouseFocusReason);
    if (playing_ && event->button() != Qt::RightButton) {
        event->accept();  // no selecting or dragging while the game runs
        return;
    }
    if (freeMode_) {
        finishFreeMode(event->button() == Qt::LeftButton);  // left confirms, right cancels
        event->accept();
        return;
    }
    if (event->button() == Qt::RightButton) {
        looking_ = true;
        lastMouse_ = event->pos();
        setCursor(Qt::ClosedHandCursor);
    } else if (event->button() == Qt::LeftButton) {
        if (hasProject() && editMode_) {
            if (faceTool()) {
                const Reply reply = bridge_->call(QStringLiteral("face.begin"), pointerFields(event->pos()));
                if (reply.ok && reply.result.toObject().value(QStringLiteral("started")).toBool()) {
                    faceDragging_ = true;
                    clickCandidate_ = false;
                    emit selectionChanged();
                    update();
                    event->accept();
                    return;
                }
                if (!reply.ok) {
                    emit renderFailed(reply.message);
                }
            }
            pressPosition_ = event->pos();
            if (!liveMeshOperation_.isEmpty() && !knifeArmed_) {
                clickCandidate_ = false;
                startMeshPreview(event->pos());
            } else {
                clickCandidate_ = true;
            }
            event->accept();
            return;
        }
        if (hasProject() && currentTool() == QLatin1String("sculpt")) {
            QJsonObject fields = pointerFields(event->pos());
            fields.insert(QStringLiteral("invert"), event->modifiers().testFlag(Qt::ControlModifier));
            const Reply reply = bridge_->call(QStringLiteral("sculpt.begin"), fields);
            if (!reply.ok) {
                emit renderFailed(reply.explanation());
            } else {
                sculptDragging_ = true;
                clickCandidate_ = false;
                emit selectionChanged();
                update();
            }
            event->accept();
            return;
        }
        if (hasProject() && currentTool() == QLatin1String("sculpt_path")) {
            pressPosition_ = event->pos();
            clickCandidate_ = true;  // a click (not a drag) adds a point of the tunnel
            event->accept();
            return;
        }
        if (hasProject() && currentTool() == QLatin1String("terrain")) {
            const Reply reply = bridge_->call(QStringLiteral("terrain.begin"), pointerFields(event->pos()));
            brushPointer_ = event->pos();
            if (!reply.ok) {
                emit renderFailed(reply.explanation());
            } else {
                naturalDragging_ = true;
                clickCandidate_ = false;
                emit selectionChanged();
                update();
            }
            event->accept();
            return;
        }
        if (hasProject() && naturalPathTool()) {
            pressPosition_ = event->pos();
            clickCandidate_ = true;  // a click adds a point of the river/road
            event->accept();
            return;
        }
        if (hasProject() && faceTool()) {
            const Reply reply = bridge_->call(QStringLiteral("face.begin"), pointerFields(event->pos()));
            if (!reply.ok) {
                emit renderFailed(reply.message);
            } else if (reply.result.toObject().value(QStringLiteral("started")).toBool()) {
                faceDragging_ = true;
                clickCandidate_ = false;
                emit selectionChanged();
                update();
                event->accept();
                return;
            }
        } else if (hasProject()) {
            // The gizmo goes first: it is the object's own control and must never lose a click
            // to a small collider handle that happens to sit near it.
            const Reply reply = bridge_->call(QStringLiteral("gizmo.begin"), pointerFields(event->pos()));
            if (reply.ok && reply.result.toObject().value(QStringLiteral("started")).toBool()) {
                gizmoDragging_ = true;
                clickCandidate_ = false;
                update();
                event->accept();
                return;
            }
            const Reply handle = bridge_->call(QStringLiteral("collider.handle_begin"), pointerFields(event->pos()));
            if (handle.ok && handle.result.toObject().value(QStringLiteral("started")).toBool()) {
                colliderDragging_ = true;
                clickCandidate_ = false;
                emit selectionChanged();
                update();
                event->accept();
                return;
            }
        }
        pressPosition_ = event->pos();
        clickCandidate_ = true;
    }
    event->accept();
}

void ViewportWidget::mouseMoveEvent(QMouseEvent* event) {
    const bool invert = event->modifiers().testFlag(Qt::ControlModifier);
    if (knifeArmed_ && !knifeStart_.isEmpty()) {
        knifeHoverScreen_ = event->pos();
        updateKnifeOverlay();
    }
    if (livePreviewActive_) {
        updateMeshPreview(event->pos(), event->modifiers());
        return;
    }
    if (naturalDragging_) {
        bridge_->call(QStringLiteral("terrain.drag"), pointerFields(event->pos()));
        update();
        return;
    }
    if (sculptDragging_) {
        bridge_->call(QStringLiteral("sculpt.drag"), pointerFields(event->pos()));
        update();
        return;
    }
    if (!looking_ && event->buttons() == Qt::NoButton && hasProject() && (sculptTool() || naturalTool())) {
        // The brush ring follows the mouse over the cave.
        brushPointer_ = event->pos();
        bridge_->call(QStringLiteral("pointer.hover"), pointerFields(event->pos()));
        update();
        return;
    }
    if (freeMode_ || gizmoDragging_ || faceDragging_ || colliderDragging_) {
        QJsonObject fields = pointerFields(event->pos());
        fields.insert(QStringLiteral("invert_snap"), invert);
        lastPointer_ = event->pos();
        if (freeMode_ && !freeTyped_.isEmpty()) {
            return;  // a typed value wins over the mouse
        }
        const QString operation = colliderDragging_ ? QStringLiteral("collider.handle_drag")
                                  : faceDragging_ ? QStringLiteral("face.drag")
                                                  : QStringLiteral("gizmo.drag");
        bridge_->call(operation, fields);
        update();
        emit gizmoPreviewed();
        return;
    }
    if (clickCandidate_ && (event->pos() - pressPosition_).manhattanLength() > kClickSlopPixels) {
        clickCandidate_ = false;
    }
    if (!looking_) {
        if (event->buttons() == Qt::NoButton) {
            updateHover(event->pos());
        }
        return;
    }
    const QPoint delta = event->pos() - lastMouse_;
    lastMouse_ = event->pos();
    bridge_->call(QStringLiteral("camera.look"), QJsonObject{{"dx", delta.x()}, {"dy", delta.y()}});
    update();
    emit cameraChanged();
}

void ViewportWidget::mouseReleaseEvent(QMouseEvent* event) {
    if (event->button() == Qt::RightButton) {
        looking_ = false;
        unsetCursor();
        // Fly keys only count while looking; do not leave the camera drifting.
        for (const int key : QList<int>(heldKeys_.begin(), heldKeys_.end())) {
            if (isLetterMovementKey(key)) {
                heldKeys_.remove(key);
            }
        }
    } else if (event->button() == Qt::LeftButton) {
        if (livePreviewActive_) {
            finishMeshPreview(true);
        } else if (naturalDragging_) {
            naturalDragging_ = false;
            const Reply reply = bridge_->call(QStringLiteral("terrain.end"));
            if (!reply.ok) {
                emit renderFailed(reply.explanation());
            }
            emit selectionChanged();  // refresh: one new undo step
            update();
        } else if (sculptDragging_) {
            sculptDragging_ = false;
            const Reply reply = bridge_->call(QStringLiteral("sculpt.end"));
            if (!reply.ok) {
                emit renderFailed(reply.explanation());
            }
            emit selectionChanged();  // refresh: one new undo step
            update();
        } else if (gizmoDragging_ || faceDragging_ || colliderDragging_) {
            const QString operation = colliderDragging_ ? QStringLiteral("collider.handle_end")
                                      : faceDragging_ ? QStringLiteral("face.end")
                                                      : QStringLiteral("gizmo.end");
            bridge_->call(operation);
            gizmoDragging_ = false;
            faceDragging_ = false;
            colliderDragging_ = false;
            unsetCursor();
            hoveredHandle_.clear();
            emit selectionChanged();  // refresh: one new undo step, new values
            update();
        } else if (clickCandidate_ && currentTool() == QLatin1String("sculpt_path")) {
            clickCandidate_ = false;
            QJsonObject fields = pointerFields(event->pos());
            fields.insert(QStringLiteral("invert"), event->modifiers().testFlag(Qt::ControlModifier));
            const Reply reply = bridge_->call(QStringLiteral("path.point"), fields);
            if (!reply.ok) {
                emit renderFailed(reply.explanation());
            }
            emit selectionChanged();
            update();
        } else if (clickCandidate_ && naturalPathTool()) {
            clickCandidate_ = false;
            const Reply reply = bridge_->call(QStringLiteral("natural.point"), pointerFields(event->pos()));
            if (!reply.ok) {
                emit renderFailed(reply.explanation());
            }
            emit selectionChanged();
            update();
        } else if (clickCandidate_) {
            clickCandidate_ = false;
            if (editMode_) {
                pickComponentAt(event->pos(), event->modifiers());
            } else {
                pickAt(event->pos());
            }
        }
    }
    event->accept();
}

void ViewportWidget::leaveEvent(QEvent* event) {
    if ((!hoveredHandle_.isEmpty() || !hoveredEntity_.isNull() || hoveredCollider_) &&
        !gizmoDragging_ && !faceDragging_ && !colliderDragging_) {
        bridge_->call(QStringLiteral("pointer.hover"), QJsonObject{{"x", -10000}, {"y", -10000}, {"width", width()}, {"height", height()}});
        hoveredHandle_.clear();
        hoveredEntity_ = QJsonValue();
        hoveredCollider_ = false;
        update();
    }
    QOpenGLWidget::leaveEvent(event);
}

void ViewportWidget::focusOutEvent(QFocusEvent* event) {
    cancelActiveEdit();
    heldKeys_.clear();
    looking_ = false;
    unsetCursor();
    QOpenGLWidget::focusOutEvent(event);
}

QJsonObject ViewportWidget::pointerFields(const QPoint& position) const {
    return QJsonObject{{"x", position.x()}, {"y", position.y()}, {"width", width()}, {"height", height()}};
}

bool ViewportWidget::hasProject() const {
    return bridge_->hasProject();
}

// Pre-selection: the gizmo handle under the mouse, the face under it (face tools), or else the
// object under it.
void ViewportWidget::updateHover(const QPoint& position) {
    if (!hasProject() || playing_) {
        return;
    }
    if (editMode_) {
        setCursor(Qt::CrossCursor);
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("pointer.hover"), pointerFields(position));
    const QJsonObject result = reply.result.toObject();
    const QString handle = result.value(QStringLiteral("handle")).toString();
    const QJsonValue entity = result.value(QStringLiteral("entity"));
    const QJsonValue face = result.value(QStringLiteral("face"));
    const bool collider = result.value(QStringLiteral("collider")).toBool();
    if (handle != hoveredHandle_ || entity != hoveredEntity_ || face != hoveredFace_ ||
        collider != hoveredCollider_) {
        hoveredHandle_ = handle;
        hoveredEntity_ = entity;
        hoveredFace_ = face;
        hoveredCollider_ = collider;
        const bool hot = !handle.isEmpty() || !face.isNull() || collider;
        // A collider handle resizes: horizontal resize cursor reads as a handle without
        // promising a direction the handle does not have.
        setCursor(collider ? Qt::SizeHorCursor : hot ? Qt::PointingHandCursor : Qt::ArrowCursor);
        update();
    }
}

void ViewportWidget::wheelEvent(QWheelEvent* event) {
    if (playing_) {
        event->accept();
        return;
    }
    const double notches = event->angleDelta().y() / 120.0;
    const bool boost = event->modifiers().testFlag(Qt::ShiftModifier);
    bridge_->call(QStringLiteral("camera.advance"),
                  QJsonObject{{"forward", notches}, {"boost", boost}, {"seconds", 0.1}});
    update();
    emit cameraChanged();
    event->accept();
}

void ViewportWidget::pickAt(const QPoint& position) {
    if (width() <= 0 || height() <= 0) {
        return;
    }
    const double ndcX = 2.0 * (position.x() + 0.5) / width() - 1.0;
    const double ndcY = 1.0 - 2.0 * (position.y() + 0.5) / height();
    const double aspect = static_cast<double>(width()) / height();
    const Reply reply = bridge_->call(QStringLiteral("selection.pick"),
                                      QJsonObject{{"x", ndcX}, {"y", ndcY}, {"aspect", aspect}});
    if (reply.ok) {
        emit selectionChanged();
    }
}

QJsonObject ViewportWidget::pickMeshPartAt(const QPoint& position) {
    if (width() <= 0 || height() <= 0 || !hasProject() || !bridge_->hasSelection()) {
        return QJsonObject();
    }
    QJsonObject fields = pointerFields(position);
    fields.insert(QStringLiteral("entity"), static_cast<double>(bridge_->selection()));
    const Reply reply = bridge_->call(QStringLiteral("mesh.pick"), fields);
    if (!reply.ok || !reply.result.isObject()) {
        return QJsonObject();
    }
    return reply.result.toObject();
}

void ViewportWidget::pickComponentAt(const QPoint& position, Qt::KeyboardModifiers modifiers) {
    if (width() <= 0 || height() <= 0 || !hasProject() || !bridge_->hasSelection()) {
        return;
    }
    QJsonObject fields = pointerFields(position);
    fields.insert(QStringLiteral("entity"), static_cast<double>(bridge_->selection()));
    const Reply pickReply = bridge_->call(QStringLiteral("mesh.pick"), fields);

    if (knifeArmed_) {
        // Two clicks, each a point on an edge; the second one cuts the face between them.
        if (!pickReply.ok || !pickReply.result.isObject()) {
            knifeHoverScreen_ = position;
            updateKnifeOverlay();
            emit meshStatus(QStringLiteral("Faca: clique sobre uma aresta da malha selecionada."));
            return;
        }
        const QJsonObject hit = pickReply.result.toObject();
        if (!hit.contains(QStringLiteral("edge"))) {
            knifeHoverScreen_ = position;
            updateKnifeOverlay();
            emit meshStatus(QStringLiteral("Faca: o ponto precisa estar sobre uma aresta."));
            return;
        }
        const QJsonObject point{
            {QStringLiteral("edge"), hit.value(QStringLiteral("edge"))},
            {QStringLiteral("fraction"), hit.value(QStringLiteral("edge_fraction")).toDouble(0.5)},
        };
        if (knifeStart_.isEmpty()) {
            knifeStart_ = point;
            knifeStartScreen_ = position;
            knifeHoverScreen_ = position;
            updateKnifeOverlay();
            emit meshStatus(QStringLiteral("Faca: primeiro ponto marcado; clique no segundo ponto da mesma face."));
            return;
        }
        const Reply cut = bridge_->call(
            QStringLiteral("mesh.knife"),
            QJsonObject{
                {QStringLiteral("entity"), static_cast<double>(bridge_->selection())},
                {QStringLiteral("start"), knifeStart_},
                {QStringLiteral("end"), point},
            });
        knifeStart_ = QJsonObject();
        updateKnifeOverlay();
        if (cut.ok) {
            knifeArmed_ = false;
            emit knifeArmedChanged(false);
            emit meshStatus(QStringLiteral("Faca: corte feito. Ctrl+Z desfaz."));
            emit selectionChanged();
            update();
        } else {
            emit meshStatus(cut.explanation());
        }
        return;
    }

    QString selectMode = QStringLiteral("replace");
    if (modifiers.testFlag(Qt::ControlModifier)) {
        selectMode = QStringLiteral("toggle");
    } else if (modifiers.testFlag(Qt::ShiftModifier)) {
        selectMode = QStringLiteral("add");
    }

    if (!pickReply.ok || pickReply.result.isNull() || !pickReply.result.isObject()) {
        if (selectMode == QLatin1String("replace")) {
            bridge_->call(QStringLiteral("mesh.select"), QJsonObject{
                {QStringLiteral("entity"), static_cast<double>(bridge_->selection())},
                {QStringLiteral("mode"), QStringLiteral("clear")}
            });
            update();
        }
        return;
    }

    const QJsonObject hit = pickReply.result.toObject();
    QJsonObject selectFields{
        {QStringLiteral("entity"), static_cast<double>(bridge_->selection())},
        {QStringLiteral("mode"), selectMode}
    };

    if (componentMode_ == ComponentMode::Vertex) {
        if (hit.contains(QStringLiteral("vertex"))) {
            const int vertex = hit.value(QStringLiteral("vertex")).toInt();
            selectFields.insert(QStringLiteral("vertices"), QJsonArray{vertex});
        }
    } else if (componentMode_ == ComponentMode::Edge) {
        if (hit.contains(QStringLiteral("edge"))) {
            selectFields.insert(QStringLiteral("edges"), QJsonArray{hit.value(QStringLiteral("edge")).toArray()});
        }
    } else {
        if (hit.contains(QStringLiteral("face"))) {
            const int face = hit.value(QStringLiteral("face")).toInt();
            selectFields.insert(QStringLiteral("faces"), QJsonArray{face});
        }
    }

    bridge_->call(QStringLiteral("mesh.select"), selectFields);
    update();
}
