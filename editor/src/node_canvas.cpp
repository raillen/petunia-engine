#include "node_canvas.h"

#include <algorithm>

#include <QBrush>
#include <QFont>
#include <QGraphicsPathItem>
#include <QGraphicsRectItem>
#include <QGraphicsScene>
#include <QGraphicsSimpleTextItem>
#include <QJsonArray>
#include <QJsonValue>
#include <QKeyEvent>
#include <QLineF>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPalette>
#include <QPen>
#include <QScrollBar>
#include <QWheelEvent>

#include "petunia_theme.h"

namespace {

constexpr double kWidth = 240.0;
constexpr double kHeader = 34.0;
constexpr double kRow = 18.0;
constexpr double kPadding = 10.0;
constexpr double kPinRadius = 3.5;
constexpr double kZoomMin = 0.25;
constexpr double kZoomMax = 4.0;
constexpr double kCanvasHalf = 1.0e6;
constexpr double kFitMargin = 60.0;
constexpr double kGroupMargin = 20.0;

// Roles on the graphics items: which node, which pin, whether it is an input, and which group.
constexpr int kBoxRole = 0;
constexpr int kNodeRole = 1;
constexpr int kPinRole = 2;
constexpr int kInputRole = 3;
constexpr int kGroupRole = 4;

int pinCount(const QJsonObject& node, bool input) {
    int count = 0;
    for (const QJsonValue& value : node.value(QStringLiteral("pins")).toArray()) {
        const QJsonObject pin = value.toObject();
        if ((pin.value(QStringLiteral("direction")).toString() == QLatin1String("in")) == input) {
            ++count;
        }
    }
    return count;
}

QString pinKey(int node, const QString& pin, bool input) {
    return QStringLiteral("%1:%2:%3").arg(node).arg(input ? 1 : 0).arg(pin);
}

QString textOf(const QJsonObject& object, const char* key) {
    return object.value(QLatin1String(key)).toString();
}

// A label that fits the box: the full text lives in the tooltip and in the spoken description.
QString fitted(const QFont& font, const QString& text, double room) {
    return QFontMetricsF(font).elidedText(text, Qt::ElideRight, room);
}

void ignoreMouse(QGraphicsItem* item) {
    item->setAcceptedMouseButtons(Qt::NoButton);
}

}  // namespace

NodeCanvas::NodeCanvas(QWidget* parent) : QGraphicsView(parent) {
    setAccessibleName(QStringLiteral("Caixas e fios do grafo"));
    setAccessibleDescription(
        QStringLiteral("A mesma lógica do contorno, desenhada em caixas. Arraste uma caixa para mover; "
                       "arraste de uma saída até uma entrada para ligar; arraste o fundo para mover o "
                       "canvas; Ctrl com a roda dá zoom; Home centraliza. Tab e as setas andam entre as "
                       "caixas, Enter edita, Delete remove."));
    setToolTip(accessibleDescription());
    setFocusPolicy(Qt::StrongFocus);
    setMouseTracking(true);
    setRenderHint(QPainter::Antialiasing, true);
    setDragMode(QGraphicsView::NoDrag);
    setTransformationAnchor(QGraphicsView::AnchorUnderMouse);
    setResizeAnchor(QGraphicsView::AnchorViewCenter);
    setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    scene_ = new QGraphicsScene(this);
    scene_->setSceneRect(-kCanvasHalf, -kCanvasHalf, kCanvasHalf * 2.0, kCanvasHalf * 2.0);
    setScene(scene_);
}

void NodeCanvas::clearMarks() {
    ran_.clear();
    failed_.clear();
    paintSelection();
}

void NodeCanvas::setPlaying(bool playing) {
    if (!playing) {
        clearMarks();
    }
}

QList<int> NodeCanvas::selectedNodes() const {
    QList<int> nodes = selected_.values();
    std::sort(nodes.begin(), nodes.end());
    return nodes;
}

void NodeCanvas::selectAll() {
    selected_.clear();
    for (int id : boxes_.keys()) {
        selected_.insert(id);
    }
    if (!boxes_.isEmpty() && !boxes_.contains(current_)) {
        current_ = boxes_.keys().first();
    }
    paintSelection();
    emit selectionChanged();
    emit message(QStringLiteral("%1 caixa(s) selecionada(s).").arg(selected_.size()));
}

int NodeCanvas::groupOf(int node) const {
    for (auto it = groups_.cbegin(); it != groups_.cend(); ++it) {
        if (it.value().contains(node)) {
            return it.key();
        }
    }
    return 0;
}

void NodeCanvas::setGraph(const QJsonObject& canvas) {
    const int keep = current_;
    const QTransform previousTransform = transform();
    const QPointF previousCenter = mapToScene(viewport()->rect().center());
    const bool restore = hasGraph_;

    canvas_ = canvas;
    scene_->clear();
    boxes_.clear();
    boxSizes_.clear();
    nodes_.clear();
    places_.clear();
    pinLocal_.clear();
    pins_.clear();
    pinPlaces_.clear();
    edges_.clear();
    wireItems_.clear();
    tempWire_ = nullptr;
    groupFrames_.clear();
    groupLabels_.clear();
    groups_.clear();
    pending_ = Pin();
    wireFrom_ = Pin();
    wireDragging_ = false;
    wireMoved_ = false;
    dragging_ = false;
    dragMoved_ = false;
    dragNode_ = 0;
    dragGroup_ = 0;
    dragOrigins_.clear();
    current_ = 0;
    scene_->setSceneRect(-kCanvasHalf, -kCanvasHalf, kCanvasHalf * 2.0, kCanvasHalf * 2.0);
    drawNodes(canvas);
    drawGroups(canvas);
    drawEdges(canvas);
    if (!boxes_.isEmpty()) {
        current_ = boxes_.contains(keep) ? keep : boxes_.keys().first();
    }
    for (auto it = selected_.begin(); it != selected_.end();) {
        if (!boxes_.contains(*it)) {
            it = selected_.erase(it);
        } else {
            ++it;
        }
    }
    paintSelection();
    if (restore) {
        setTransform(previousTransform);
        centerOn(previousCenter);
        zoom_ = previousTransform.m11() > 0.0 ? previousTransform.m11() : 1.0;
    } else {
        fitGraph();
    }
    hasGraph_ = true;
    emit zoomChanged(qRound(zoom_ * 100.0));
}

void NodeCanvas::drawGroups(const QJsonObject& canvas) {
    PetuniaTheme& theme = PetuniaTheme::instance();
    const QColor border = theme.borderSubtle();
    QFont labelFont = font();
    labelFont.setBold(true);
    for (const QJsonValue& value : canvas.value(QStringLiteral("groups")).toArray()) {
        const QJsonObject group = value.toObject();
        const int id = group.value(QStringLiteral("id")).toInt();
        const QString label = textOf(group, "label");
        QList<int> nodes;
        for (const QJsonValue& node : group.value(QStringLiteral("nodes")).toArray()) {
            nodes.append(node.toInt());
        }
        groups_.insert(id, nodes);
        auto* frame = scene_->addRect(QRectF(), QPen(border, 1.2, Qt::DashLine), QBrush(Qt::NoBrush));
        frame->setZValue(-2.0);
        frame->setData(kGroupRole, id);
        ignoreMouse(frame);
        groupFrames_.insert(id, frame);
        auto* text = scene_->addSimpleText(label, labelFont);
        text->setZValue(-1.5);
        text->setBrush(theme.textMuted());
        ignoreMouse(text);
        groupLabels_.insert(id, text);
    }
    updateGroupFrames();
}

void NodeCanvas::drawNodes(const QJsonObject& canvas) {
    PetuniaTheme& theme = PetuniaTheme::instance();
    const QColor text = theme.textPrimary();
    const QColor faint = theme.textMuted();
    const QColor fill = theme.bgSurface();
    const QColor border = theme.borderSubtle();
    const double room = kWidth - kPadding * 2.0;
    for (const QJsonValue& value : canvas.value(QStringLiteral("nodes")).toArray()) {
        const QJsonObject node = value.toObject();
        const int id = node.value(QStringLiteral("node")).toInt();
        const int inputs = pinCount(node, true);
        const int outputs = pinCount(node, false);
        const int problems = node.value(QStringLiteral("problems")).toArray().size();
        const double height = kHeader + kPadding + qMax(inputs, outputs) * kRow + problems * 16.0 + 8.0;
        const QPointF place(node.value(QStringLiteral("x")).toDouble(), node.value(QStringLiteral("y")).toDouble());

        auto* box = scene_->addRect(QRectF(0.0, 0.0, kWidth, height), QPen(border, 1.2), QBrush(fill));
        box->setPos(place);
        // Moving is handled by the view: child labels and pins must not steal the drag.
        box->setFlag(QGraphicsItem::ItemIsMovable, false);
        box->setFlag(QGraphicsItem::ItemIsSelectable, false);
        box->setData(kBoxRole, true);
        box->setData(kNodeRole, id);
        box->setToolTip(QStringLiteral("%1 — %2").arg(textOf(node, "title"), textOf(node, "summary")));
        boxes_.insert(id, box);
        boxSizes_.insert(id, QSizeF(kWidth, height));
        nodes_.insert(id, node);
        places_.insert(id, place);

        QFont title = font();
        if (title.pointSizeF() > 0.0) {
            title.setPointSizeF(title.pointSizeF() + 0.5);
        } else {
            title.setPixelSize(qMax(1, title.pixelSize() + 1));
        }
        title.setBold(true);
        auto* label = scene_->addSimpleText(fitted(title, textOf(node, "title"), room), title);
        label->setBrush(text);
        label->setParentItem(box);
        label->setPos(kPadding, 5.0);
        ignoreMouse(label);

        QFont small = font();
        if (small.pointSizeF() > 0.0) {
            small.setPointSizeF(qMax(6.5, small.pointSizeF() - 1.5));
        } else {
            small.setPixelSize(qMax(9, small.pixelSize() - 1));
        }
        auto* kind = scene_->addSimpleText(fitted(small, textOf(node, "class"), room), small);
        kind->setBrush(faint);
        kind->setParentItem(box);
        kind->setPos(kPadding, 5.0 + QFontMetricsF(title).height());
        ignoreMouse(kind);

        int inputRow = 0;
        int outputRow = 0;
        for (const QJsonValue& pinValue : node.value(QStringLiteral("pins")).toArray()) {
            const QJsonObject pin = pinValue.toObject();
            const bool input = pin.value(QStringLiteral("direction")).toString() == QLatin1String("in");
            const QString name = textOf(pin, "name");
            const double y = kHeader + kPadding + (input ? inputRow : outputRow) * kRow;
            if (input) {
                ++inputRow;
            } else {
                ++outputRow;
            }
            const double x = input ? 8.0 : kWidth - 8.0;
            const QColor color = pin.value(QStringLiteral("exec")).toBool() ? theme.accentPrimary() : faint;
            auto* dot = scene_->addEllipse(
                QRectF(x - kPinRadius, y - kPinRadius, kPinRadius * 2.0, kPinRadius * 2.0),
                QPen(color, 1.2), QBrush(color));
            dot->setParentItem(box);
            dot->setData(kNodeRole, id);
            dot->setData(kPinRole, name);
            dot->setData(kInputRole, input);
            dot->setToolTip(input ? QStringLiteral("Entrada %1 (%2)").arg(textOf(pin, "label"), textOf(pin, "type"))
                                  : QStringLiteral("Saída %1 (%2)").arg(textOf(pin, "label"), textOf(pin, "type")));
            ignoreMouse(dot);
            const QPointF local(x, y);
            const QString key = pinKey(id, name, input);
            pinLocal_.insert(key, local);
            pins_.insert(key, place + local);
            pinPlaces_.append(Pin{id, name, input, place + local});

            auto* pinLabel = scene_->addSimpleText(fitted(small, textOf(pin, "label"), room * 0.5), small);
            pinLabel->setBrush(text);
            pinLabel->setParentItem(box);
            pinLabel->setPos(input ? x + 7.0 : x - 7.0 - pinLabel->boundingRect().width(), y - 7.0);
            ignoreMouse(pinLabel);
        }
        double problemY = kHeader + kPadding + qMax(inputs, outputs) * kRow;
        for (const QJsonValue& problemValue : node.value(QStringLiteral("problems")).toArray()) {
            const QJsonObject problem = problemValue.toObject();
            const QString message = textOf(problem, "message");
            auto* warning = scene_->addSimpleText(QStringLiteral("⚠ ") + fitted(small, message, room - 12.0), small);
            warning->setBrush(theme.statusWarning());
            warning->setParentItem(box);
            warning->setToolTip(message);
            warning->setPos(kPadding, problemY);
            ignoreMouse(warning);
            problemY += 16.0;
        }
    }
}

void NodeCanvas::drawEdges(const QJsonObject& canvas) {
    PetuniaTheme& theme = PetuniaTheme::instance();
    const QColor exec = theme.accentPrimary();
    const QColor data = theme.textMuted();
    edges_.clear();
    for (const QJsonValue& value : canvas.value(QStringLiteral("edges")).toArray()) {
        const QJsonObject edge = value.toObject();
        edges_.append(edge);
        const QJsonObject from = edge.value(QStringLiteral("from")).toObject();
        const QJsonObject to = edge.value(QStringLiteral("to")).toObject();
        const QPointF start = pinPoint(from.value(QStringLiteral("node")).toInt(), textOf(from, "pin"), false);
        const QPointF end = pinPoint(to.value(QStringLiteral("node")).toInt(), textOf(to, "pin"), true);
        if (start.isNull() || end.isNull()) {
            continue;
        }
        QPainterPath path(start);
        const double reach = qMax(40.0, qAbs(end.x() - start.x()) / 2.0);
        path.cubicTo(start + QPointF(reach, 0.0), end - QPointF(reach, 0.0), end);
        const bool isExec = textOf(edge, "kind") == QLatin1String("exec");
        auto* wire = scene_->addPath(path, QPen(isExec ? exec : data, isExec ? 2.0 : 1.4));
        wire->setZValue(-1.0);
        wire->setToolTip(textOf(edge, "kind") == QLatin1String("exec")
                             ? QStringLiteral("Ligação de execução")
                             : QStringLiteral("Ligação de valor (%1)").arg(textOf(edge, "type")));
        wireItems_.append(wire);
    }
}

void NodeCanvas::redrawEdges() {
    for (QGraphicsPathItem* wire : std::as_const(wireItems_)) {
        scene_->removeItem(wire);
        delete wire;
    }
    wireItems_.clear();
    drawEdges(canvas_);
}

QPointF NodeCanvas::pinPoint(int node, const QString& pin, bool input) const {
    return pins_.value(pinKey(node, pin, input));
}

int NodeCanvas::nodeAt(const QPointF& position) const {
    for (QGraphicsItem* item : scene_->items(position)) {
        for (QGraphicsItem* walk = item; walk != nullptr; walk = walk->parentItem()) {
            if (walk->data(kBoxRole).toBool()) {
                return walk->data(kNodeRole).toInt();
            }
        }
    }
    return 0;
}

int NodeCanvas::groupAt(const QPointF& position) const {
    if (nodeAt(position) != 0) {
        return 0;
    }
    for (auto it = groupFrames_.cbegin(); it != groupFrames_.cend(); ++it) {
        if (it.value()->isVisible() && it.value()->rect().translated(it.value()->pos()).contains(position)) {
            return it.key();
        }
    }
    return 0;
}

// Pins are small, so the nearest one within reach wins rather than whatever is drawn on top.
NodeCanvas::Pin NodeCanvas::pinAt(const QPointF& position) const {
    Pin best;
    double nearest = 10.0;
    for (const Pin& pin : pinPlaces_) {
        const double distance = QLineF(pin.at, position).length();
        if (distance <= nearest) {
            nearest = distance;
            best = pin;
        }
    }
    return best;
}

void NodeCanvas::paintSelection() {
    PetuniaTheme& theme = PetuniaTheme::instance();
    const QColor border = theme.borderSubtle();
    const QColor fill = theme.bgSurface();
    for (auto it = boxes_.cbegin(); it != boxes_.cend(); ++it) {
        it.value()->setPen(QPen(border, 1.2));
        it.value()->setBrush(QBrush(fill));
        it.value()->setZValue(0.0);
    }
    for (int node : std::as_const(ran_)) {
        if (QGraphicsRectItem* box = boxes_.value(node)) {
            box->setPen(QPen(theme.accentPrimary(), 2.2));
            box->setBrush(QBrush(theme.bgSurfaceHover()));
        }
    }
    for (int node : std::as_const(failed_)) {
        if (QGraphicsRectItem* box = boxes_.value(node)) {
            box->setPen(QPen(theme.statusWarning(), 2.6));
            box->setBrush(QBrush(theme.bgSurfaceHover()));
        }
    }
    for (int node : std::as_const(selected_)) {
        if (QGraphicsRectItem* box = boxes_.value(node)) {
            box->setPen(QPen(theme.accentSelectionActive(), 2.4));
        }
    }
    if (QGraphicsRectItem* box = boxes_.value(current_)) {
        box->setPen(QPen(theme.accentSelectionActive(), 3.0));
        box->setZValue(1.0);
    }
}

void NodeCanvas::setMarks(const QJsonObject& debug) {
    ran_.clear();
    failed_.clear();
    for (const QJsonValue& value : debug.value(QStringLiteral("marks")).toArray()) {
        const QJsonObject mark = value.toObject();
        const int node = mark.value(QStringLiteral("node")).toInt();
        if (mark.value(QStringLiteral("kind")).toString() == QLatin1String("ran")) {
            if (!ran_.contains(node)) {
                ran_.append(node);
            }
        } else if (mark.value(QStringLiteral("kind")).toString() == QLatin1String("check")
                   && !mark.value(QStringLiteral("value")).toBool()) {
            if (!failed_.contains(node)) {
                failed_.append(node);
            }
        }
    }
    paintSelection();
}

QString NodeCanvas::titleOf(int node) const {
    return textOf(nodes_.value(node), "title");
}

void NodeCanvas::selectNode(int node) {
    if (!boxes_.contains(node)) {
        return;
    }
    current_ = node;
    selected_ = QSet<int>{node};
    paintSelection();
    if (QGraphicsRectItem* box = boxes_.value(node)) {
        ensureVisible(box);
    }
    describeCurrent();
    emit selectionChanged();
}

void NodeCanvas::clearSelection() {
    selected_.clear();
    paintSelection();
    emit selectionChanged();
}

void NodeCanvas::describeCurrent() {
    const QJsonObject node = nodes_.value(current_);
    if (node.isEmpty()) {
        return;
    }
    QString sentence = QStringLiteral("%1. %2").arg(textOf(node, "title"), textOf(node, "summary")).trimmed();
    const QJsonArray problems = node.value(QStringLiteral("problems")).toArray();
    for (const QJsonValue& value : problems) {
        sentence += QStringLiteral(" Atenção: %1.").arg(textOf(value.toObject(), "message"));
    }
    if (ran_.contains(current_)) {
        sentence += QStringLiteral(" Rodou no último quadro.");
    }
    if (failed_.contains(current_)) {
        sentence += QStringLiteral(" Esta condição deu falso no último quadro.");
    }
    emit message(sentence);
}

void NodeCanvas::zoomBy(double factor, bool underMouse) {
    const double wanted = qBound(kZoomMin, zoom_ * factor, kZoomMax);
    if (qFuzzyCompare(wanted, zoom_)) {
        return;
    }
    setTransformationAnchor(underMouse ? QGraphicsView::AnchorUnderMouse : QGraphicsView::AnchorViewCenter);
    const double applied = wanted / zoom_;
    zoom_ = wanted;
    scale(applied, applied);
    emit zoomChanged(qRound(zoom_ * 100.0));
    emit message(QStringLiteral("Zoom %1 %.").arg(qRound(zoom_ * 100.0)));
}

void NodeCanvas::zoomIn() {
    zoomBy(1.25, false);
}

void NodeCanvas::zoomOut() {
    zoomBy(1.0 / 1.25, false);
}

void NodeCanvas::resetZoom() {
    setTransform(QTransform());
    zoom_ = 1.0;
    const QRectF bounds = scene_->itemsBoundingRect();
    centerOn(bounds.isEmpty() ? QPointF(0.0, 0.0) : bounds.center());
    emit zoomChanged(100);
    emit message(QStringLiteral("Zoom 100 %."));
}

void NodeCanvas::fitGraph() {
    QRectF bounds;
    bool first = true;
    for (auto it = boxes_.cbegin(); it != boxes_.cend(); ++it) {
        const QRectF box(places_.value(it.key()), boxSizes_.value(it.key()));
        bounds = first ? box : bounds.united(box);
        first = false;
    }
    if (bounds.isEmpty()) {
        setTransform(QTransform());
        zoom_ = 1.0;
        centerOn(QPointF(0.0, 0.0));
        emit zoomChanged(100);
        return;
    }
    bounds.adjust(-kFitMargin, -kFitMargin, kFitMargin, kFitMargin);
    const QSize view = viewport()->size();
    const double horizontal = view.width() > 0 ? view.width() / bounds.width() : 1.0;
    const double vertical = view.height() > 0 ? view.height() / bounds.height() : 1.0;
    const double wanted = qBound(kZoomMin, qMin(horizontal, vertical), kZoomMax);
    zoom_ = wanted;
    setTransform(QTransform::fromScale(zoom_, zoom_));
    centerOn(bounds.center());
    emit zoomChanged(qRound(zoom_ * 100.0));
}

void NodeCanvas::walk(int dx, int dy) {
    if (boxes_.isEmpty()) {
        return;
    }
    const QPointF from = places_.value(current_);
    int best = 0;
    double bestScore = 0.0;
    for (auto it = places_.cbegin(); it != places_.cend(); ++it) {
        if (it.key() == current_) {
            continue;
        }
        const double along = (it.value().x() - from.x()) * dx + (it.value().y() - from.y()) * dy;
        if (along <= 1.0) {
            continue;
        }
        const double across = qAbs((it.value().x() - from.x()) * dy) + qAbs((it.value().y() - from.y()) * dx);
        const double score = along + across * 4.0;
        if (best == 0 || score < bestScore) {
            best = it.key();
            bestScore = score;
        }
    }
    if (best != 0) {
        selectNode(best);
        emit nodeSelected(best);
    }
}

void NodeCanvas::keyPressEvent(QKeyEvent* event) {
    switch (event->key()) {
    case Qt::Key_A:
        if (event->modifiers().testFlag(Qt::ControlModifier)) {
            selectAll();
            event->accept();
            return;
        }
        break;
    case Qt::Key_Escape:
        if (wireDragging_ || pending_.node != 0) {
            wireDragging_ = false;
            wireMoved_ = false;
            wireFrom_ = Pin();
            pending_ = Pin();
            if (tempWire_ != nullptr) {
                scene_->removeItem(tempWire_);
                delete tempWire_;
                tempWire_ = nullptr;
            }
            emit message(QStringLiteral("Ligação cancelada."));
        } else {
            clearSelection();
        }
        event->accept();
        return;
    case Qt::Key_Home:
    case Qt::Key_F:
        fitGraph();
        event->accept();
        return;
    case Qt::Key_0:
        resetZoom();
        event->accept();
        return;
    case Qt::Key_Tab:
    case Qt::Key_Right:
        walk(1, 0);
        event->accept();
        return;
    case Qt::Key_Backtab:
    case Qt::Key_Left:
        walk(-1, 0);
        event->accept();
        return;
    case Qt::Key_Down:
        walk(0, 1);
        event->accept();
        return;
    case Qt::Key_Up:
        walk(0, -1);
        event->accept();
        return;
    case Qt::Key_Return:
    case Qt::Key_Enter:
        if (current_ != 0) {
            emit nodeActivated(current_);
        }
        event->accept();
        return;
    case Qt::Key_Delete:
        if (current_ != 0) {
            emit nodeRemoveRequested(current_);
        }
        event->accept();
        return;
    case Qt::Key_Plus:
    case Qt::Key_Equal:
        zoomIn();
        event->accept();
        return;
    case Qt::Key_Minus:
        zoomOut();
        event->accept();
        return;
    default:
        break;
    }
    QGraphicsView::keyPressEvent(event);
}

void NodeCanvas::wheelEvent(QWheelEvent* event) {
    if (event->modifiers().testFlag(Qt::ControlModifier)) {
        zoomBy(event->angleDelta().y() > 0 ? 1.15 : (1.0 / 1.15), true);
        event->accept();
        return;
    }
    QGraphicsView::wheelEvent(event);
}

void NodeCanvas::refreshTempWire(const QPointF& scenePoint) {
    if (wireFrom_.node == 0) {
        return;
    }
    if (tempWire_ == nullptr) {
        tempWire_ = scene_->addPath(QPainterPath(),
                                    QPen(PetuniaTheme::instance().accentPrimary(), 2.0, Qt::DashLine));
        tempWire_->setZValue(2.0);
    }
    QPainterPath path(wireFrom_.at);
    const double reach = qMax(40.0, qAbs(scenePoint.x() - wireFrom_.at.x()) / 2.0);
    path.cubicTo(wireFrom_.at + QPointF(reach, 0.0), scenePoint - QPointF(reach, 0.0), scenePoint);
    tempWire_->setPath(path);
}

void NodeCanvas::moveNodeTo(int node, const QPointF& place) {
    if (!places_.contains(node)) {
        return;
    }
    const QPointF delta = place - places_.value(node);
    places_.insert(node, place);
    if (QGraphicsRectItem* box = boxes_.value(node)) {
        box->setPos(place);
    }
    for (Pin& pin : pinPlaces_) {
        if (pin.node == node) {
            pin.at += delta;
            pins_.insert(pinKey(pin.node, pin.name, pin.input), pin.at);
        }
    }
}

void NodeCanvas::updateGroupFrames() {
    for (auto it = groupFrames_.cbegin(); it != groupFrames_.cend(); ++it) {
        const int id = it.key();
        QRectF bounds;
        bool first = true;
        for (int node : groups_.value(id)) {
            if (!places_.contains(node) || !boxSizes_.contains(node)) {
                continue;
            }
            const QRectF box(places_.value(node), boxSizes_.value(node));
            bounds = first ? box : bounds.united(box);
            first = false;
        }
        QGraphicsSimpleTextItem* label = groupLabels_.value(id);
        if (first) {
            it.value()->hide();
            if (label != nullptr) {
                label->hide();
            }
            continue;
        }
        bounds.adjust(-kGroupMargin, -kGroupMargin - 18.0, kGroupMargin, kGroupMargin);
        it.value()->setRect(bounds);
        it.value()->show();
        if (label != nullptr) {
            const QString name = label->text();
            label->setPos(bounds.topLeft() + QPointF(10.0, 6.0));
            label->setToolTip(name);
            label->show();
        }
    }
}

QList<int> NodeCanvas::nodesInGroup(int group) const {
    return groups_.value(group);
}

void NodeCanvas::mousePressEvent(QMouseEvent* event) {
    setFocus(Qt::MouseFocusReason);
    const QPointF scenePoint = mapToScene(event->position().toPoint());
    if (event->button() == Qt::MiddleButton) {
        panning_ = true;
        lastPan_ = event->position().toPoint();
        setCursor(Qt::ClosedHandCursor);
        event->accept();
        return;
    }
    if (event->button() != Qt::LeftButton) {
        QGraphicsView::mousePressEvent(event);
        return;
    }

    const Pin pin = pinAt(scenePoint);
    if (pin.node != 0) {
        if (!pin.input) {
            wireFrom_ = pin;
            wireDragging_ = true;
            wireMoved_ = false;
            pending_ = Pin();
            refreshTempWire(scenePoint);
            emit message(QStringLiteral("Arraste até a entrada que vai receber (ou clique nela). Esc cancela."));
        } else if (pending_.node != 0 && pending_.node != pin.node) {
            emit connectRequested(QJsonObject{{"node", pending_.node}, {"pin", pending_.name}},
                                  QJsonObject{{"node", pin.node}, {"pin", pin.name}});
            pending_ = Pin();
        } else {
            emit message(QStringLiteral("Para ligar, comece pela saída de um nó."));
        }
        event->accept();
        return;
    }

    const int node = nodeAt(scenePoint);
    if (node != 0) {
        if (event->modifiers().testFlag(Qt::ControlModifier)) {
            if (selected_.contains(node)) {
                selected_.remove(node);
                if (current_ == node) {
                    current_ = selected_.isEmpty() ? 0 : *selected_.begin();
                }
            } else {
                selected_.insert(node);
                current_ = node;
            }
        } else {
            if (!selected_.contains(node)) {
                selected_.clear();
                selected_.insert(node);
            }
            current_ = node;
        }
        dragNode_ = node;
        dragGroup_ = 0;
        dragStart_ = scenePoint;
        dragging_ = true;
        dragMoved_ = false;
        dragOrigins_.clear();
        for (int id : std::as_const(selected_)) {
            dragOrigins_.insert(id, places_.value(id));
        }
        if (!dragOrigins_.contains(node)) {
            dragOrigins_.insert(node, places_.value(node));
            selected_.insert(node);
        }
        paintSelection();
        describeCurrent();
        emit nodeSelected(node);
        emit selectionChanged();
        event->accept();
        return;
    }

    const int group = groupAt(scenePoint);
    if (group != 0) {
        selected_.clear();
        dragOrigins_.clear();
        for (int id : groups_.value(group)) {
            if (boxes_.contains(id)) {
                selected_.insert(id);
                dragOrigins_.insert(id, places_.value(id));
            }
        }
        current_ = selected_.isEmpty() ? 0 : *selected_.begin();
        dragNode_ = 0;
        dragGroup_ = group;
        dragStart_ = scenePoint;
        dragging_ = true;
        dragMoved_ = false;
        paintSelection();
        emit selectionChanged();
        emit message(QStringLiteral("Grupo selecionado. Arraste para mover as caixas juntas."));
        event->accept();
        return;
    }

    pending_ = Pin();
    selected_.clear();
    current_ = 0;
    paintSelection();
    emit selectionChanged();
    panning_ = true;
    lastPan_ = event->position().toPoint();
    setCursor(Qt::ClosedHandCursor);
    event->accept();
}

void NodeCanvas::mouseMoveEvent(QMouseEvent* event) {
    const QPointF scenePoint = mapToScene(event->position().toPoint());
    if (panning_) {
        const QPoint delta = event->position().toPoint() - lastPan_;
        horizontalScrollBar()->setValue(horizontalScrollBar()->value() - delta.x());
        verticalScrollBar()->setValue(verticalScrollBar()->value() - delta.y());
        lastPan_ = event->position().toPoint();
        event->accept();
        return;
    }
    if (wireDragging_) {
        if (QLineF(wireFrom_.at, scenePoint).length() > 3.0) {
            wireMoved_ = true;
        }
        refreshTempWire(scenePoint);
        event->accept();
        return;
    }
    if (dragging_) {
        const QPointF delta = scenePoint - dragStart_;
        if (delta.manhattanLength() > 0.1) {
            dragMoved_ = true;
        }
        for (auto it = dragOrigins_.cbegin(); it != dragOrigins_.cend(); ++it) {
            moveNodeTo(it.key(), it.value() + delta);
        }
        updateGroupFrames();
        redrawEdges();
        update();
        event->accept();
        return;
    }
    QGraphicsView::mouseMoveEvent(event);
}

void NodeCanvas::mouseReleaseEvent(QMouseEvent* event) {
    if (panning_) {
        panning_ = false;
        unsetCursor();
        event->accept();
        return;
    }
    if (wireDragging_) {
        const Pin target = pinAt(mapToScene(event->position().toPoint()));
        const Pin from = wireFrom_;
        const bool moved = wireMoved_;
        wireDragging_ = false;
        wireMoved_ = false;
        wireFrom_ = Pin();
        if (tempWire_ != nullptr) {
            scene_->removeItem(tempWire_);
            delete tempWire_;
            tempWire_ = nullptr;
        }
        if (target.input && target.node != 0 && target.node != from.node) {
            emit connectRequested(QJsonObject{{"node", from.node}, {"pin", from.name}},
                                  QJsonObject{{"node", target.node}, {"pin", target.name}});
        } else if (!moved && target.node == from.node && !target.input) {
            pending_ = from;
            emit message(QStringLiteral("Saída escolhida: %1. Clique na entrada que vai receber, ou Esc para cancelar.")
                             .arg(from.name));
        }
        update();
        event->accept();
        return;
    }
    if (dragging_) {
        dragging_ = false;
        dragNode_ = 0;
        dragGroup_ = 0;
        if (dragMoved_) {
            QJsonArray places;
            for (auto it = dragOrigins_.cbegin(); it != dragOrigins_.cend(); ++it) {
                const QPointF place = places_.value(it.key());
                places.append(QJsonObject{{"node", it.key()}, {"x", place.x()}, {"y", place.y()}});
            }
            emit nodesMoved(places);
        }
        dragOrigins_.clear();
        dragMoved_ = false;
        event->accept();
        return;
    }
    QGraphicsView::mouseReleaseEvent(event);
}
