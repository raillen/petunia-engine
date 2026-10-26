#include "node_canvas.h"

#include <QBrush>
#include <QFont>
#include <QGraphicsPathItem>
#include <QGraphicsRectItem>
#include <QGraphicsScene>
#include <QGraphicsSimpleTextItem>
#include <QJsonArray>
#include <QKeyEvent>
#include <QLineF>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPalette>
#include <QPen>
#include <QWheelEvent>
#include <tuple>

#include "petunia_theme.h"

namespace {

constexpr double kWidth = 240.0;
constexpr double kHeader = 34.0;
constexpr double kRow = 18.0;
constexpr double kPadding = 10.0;
constexpr double kPinRadius = 3.5;
constexpr double kZoomMin = 0.25;
constexpr double kZoomMax = 4.0;

// Roles on the graphics items: which node, which pin, and whether it is an input.
constexpr int kBoxRole = 0;
constexpr int kNodeRole = 1;
constexpr int kPinRole = 2;
constexpr int kInputRole = 3;

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

}  // namespace

NodeCanvas::NodeCanvas(QWidget* parent) : QGraphicsView(parent) {
    setAccessibleName(QStringLiteral("Caixas e fios do grafo"));
    setAccessibleDescription(
        QStringLiteral("A mesma lógica do contorno, desenhada em caixas. Tab e as setas andam entre as "
                       "caixas, Enter edita, Delete remove, Ctrl com a roda dá zoom. Arrastar uma caixa "
                       "só muda onde ela aparece; a lógica não muda."));
    setToolTip(accessibleDescription());
    setFocusPolicy(Qt::StrongFocus);
    setRenderHint(QPainter::Antialiasing, true);
    setDragMode(QGraphicsView::RubberBandDrag);
    setHorizontalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    setVerticalScrollBarPolicy(Qt::ScrollBarAsNeeded);
    scene_ = new QGraphicsScene(this);
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

void NodeCanvas::setGraph(const QJsonObject& canvas) {
    const int keep = current_;
    canvas_ = canvas;
    scene_->clear();
    boxes_.clear();
    nodes_.clear();
    places_.clear();
    pins_.clear();
    pending_ = Pin();
    pinPlaces_.clear();
    current_ = 0;
    drawNodes(canvas);
    drawEdges(canvas);
    if (!boxes_.isEmpty()) {
        current_ = boxes_.contains(keep) ? keep : boxes_.keys().first();
    }
    paintSelection();
    QRectF bounds = scene_->itemsBoundingRect();
    if (!bounds.isEmpty()) {
        scene_->setSceneRect(bounds.adjusted(-40, -40, 40, 40));
    }
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
        box->setFlag(QGraphicsItem::ItemIsMovable, true);
        box->setFlag(QGraphicsItem::ItemIsSelectable, false);
        // A long sentence must not spill over the next box; the tooltip and the spoken
        // description keep the whole text.
        box->setFlag(QGraphicsItem::ItemClipsChildrenToShape, true);
        box->setData(kBoxRole, true);
        box->setData(kNodeRole, id);
        box->setToolTip(QStringLiteral("%1 — %2").arg(textOf(node, "title"), textOf(node, "summary")));
        boxes_.insert(id, box);
        nodes_.insert(id, node);
        places_.insert(id, place);

        QFont title = font();
        title.setPointSizeF(title.pointSizeF() + 0.5);
        title.setBold(true);
        auto* label = scene_->addSimpleText(fitted(title, textOf(node, "title"), room), title);
        label->setBrush(text);
        label->setParentItem(box);
        label->setPos(kPadding, 5.0);

        QFont small = font();
        small.setPointSizeF(qMax(6.5, font().pointSizeF() - 1.5));
        auto* kind = scene_->addSimpleText(fitted(small, textOf(node, "class"), room), small);
        kind->setBrush(faint);
        kind->setParentItem(box);
        kind->setPos(kPadding, 5.0 + QFontMetricsF(title).height());

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
            auto* dot = scene_->addEllipse(QRectF(x - kPinRadius, y - kPinRadius, kPinRadius * 2.0, kPinRadius * 2.0),
                                           QPen(color, 1.2), QBrush(color));
            dot->setParentItem(box);
            dot->setData(kNodeRole, id);
            dot->setData(kPinRole, name);
            dot->setData(kInputRole, input);
            dot->setToolTip(input ? QStringLiteral("Entrada %1 (%2)").arg(textOf(pin, "label"), textOf(pin, "type"))
                                  : QStringLiteral("Saída %1 (%2)").arg(textOf(pin, "label"), textOf(pin, "type")));
            const QPointF at = place + QPointF(x, y);
            pins_.insert(pinKey(id, name, input), at);
            pinPlaces_.append(Pin{id, name, input, at});

            auto* pinLabel = scene_->addSimpleText(fitted(small, textOf(pin, "label"), room * 0.5), small);
            pinLabel->setBrush(text);
            pinLabel->setParentItem(box);
            pinLabel->setPos(input ? x + 7.0 : x - 7.0 - pinLabel->boundingRect().width(), y - 7.0);
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
            problemY += 16.0;
        }
    }
}

void NodeCanvas::drawEdges(const QJsonObject& canvas) {
    PetuniaTheme& theme = PetuniaTheme::instance();
    const QColor exec = theme.accentPrimary();
    const QColor data = theme.textMuted();
    for (const QJsonValue& value : canvas.value(QStringLiteral("edges")).toArray()) {
        const QJsonObject edge = value.toObject();
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
    }
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

// Pins are small, so the nearest one within reach wins rather than whatever is drawn on top.
NodeCanvas::Pin NodeCanvas::pinAt(const QPointF& position) const {
    Pin best;
    double nearest = 9.0;
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
    // What ran in the newest frame: a light edge. What decided false: a warning colour.
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
    paintSelection();
    if (QGraphicsRectItem* box = boxes_.value(node)) {
        ensureVisible(box);
    }
    describeCurrent();
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

void NodeCanvas::zoomBy(double factor) {
    const double wanted = qBound(kZoomMin, zoom_ * factor, kZoomMax);
    if (qFuzzyCompare(wanted, zoom_)) {
        return;
    }
    zoom_ = wanted;
    setTransform(QTransform::fromScale(zoom_, zoom_));
    emit message(QStringLiteral("Zoom %1 %.").arg(qRound(zoom_ * 100.0)));
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
        zoomBy(1.25);
        event->accept();
        return;
    case Qt::Key_Minus:
        zoomBy(1.0 / 1.25);
        event->accept();
        return;
    case Qt::Key_0:
        zoom_ = 1.0;
        setTransform(QTransform());
        emit message(QStringLiteral("Zoom 100 %."));
        event->accept();
        return;
    default:
        break;
    }
    QGraphicsView::keyPressEvent(event);
}

void NodeCanvas::wheelEvent(QWheelEvent* event) {
    if (event->modifiers().testFlag(Qt::ControlModifier)) {
        zoomBy(event->angleDelta().y() > 0 ? 1.15 : (1.0 / 1.15));
        event->accept();
        return;
    }
    QGraphicsView::wheelEvent(event);
}

void NodeCanvas::mousePressEvent(QMouseEvent* event) {
    if (event->button() != Qt::LeftButton) {
        QGraphicsView::mousePressEvent(event);
        return;
    }
    const QPointF scenePoint = mapToScene(event->position().toPoint());
    const Pin pin = pinAt(scenePoint);
    if (pin.node != 0) {
        if (pending_.node == 0) {
            if (!pin.input) {
                pending_ = pin;
                emit message(QStringLiteral("Saída escolhida: %1. Clique na entrada que vai receber, ou Esc para cancelar.").arg(pin.name));
            } else {
                emit message(QStringLiteral("Para ligar, comece pela saída de um nó."));
            }
            event->accept();
            return;
        }
        if (pin.input && pin.node != pending_.node) {
            emit connectRequested(QJsonObject{{"node", pending_.node}, {"pin", pending_.name}},
                                  QJsonObject{{"node", pin.node}, {"pin", pin.name}});
            pending_ = Pin();
            event->accept();
            return;
        }
        pending_ = Pin();
        event->accept();
        return;
    }
    const int node = nodeAt(scenePoint);
    if (node != 0) {
        current_ = node;
        paintSelection();
        describeCurrent();
        emit nodeSelected(node);
    }
    pending_ = Pin();
    QGraphicsView::mousePressEvent(event);
}

void NodeCanvas::mouseReleaseEvent(QMouseEvent* event) {
    QGraphicsView::mouseReleaseEvent(event);
    QList<std::tuple<int, double, double>> moved;
    for (auto it = boxes_.cbegin(); it != boxes_.cend(); ++it) {
        const QPointF now = it.value()->pos();
        if (!qFuzzyCompare(now.x() + 1.0, places_.value(it.key()).x() + 1.0)
            || !qFuzzyCompare(now.y() + 1.0, places_.value(it.key()).y() + 1.0)) {
            moved.append({it.key(), now.x(), now.y()});
        }
    }
    if (moved.isEmpty()) {
        return;
    }
    // The wires and the places of the pins were computed where the boxes used to be: draw the
    // graph again with the new places, so a wire follows its box and a click finds the right pin.
    QJsonArray nodes = canvas_.value(QStringLiteral("nodes")).toArray();
    for (int index = 0; index < nodes.size(); ++index) {
        QJsonObject node = nodes.at(index).toObject();
        for (const auto& [id, x, y] : std::as_const(moved)) {
            if (node.value(QStringLiteral("node")).toInt() == id) {
                node.insert(QStringLiteral("x"), x);
                node.insert(QStringLiteral("y"), y);
            }
        }
        nodes.replace(index, node);
    }
    canvas_.insert(QStringLiteral("nodes"), nodes);
    const QJsonObject redraw = canvas_;
    setGraph(redraw);
    for (const auto& [id, x, y] : std::as_const(moved)) {
        emit nodeMoved(id, x, y);
    }
}
