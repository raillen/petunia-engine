#include "viewport_overlay.h"

#include <QJsonArray>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>

#include "engine_bridge.h"
#include "petunia_theme.h"

namespace {

constexpr double kNavUnit = 21.0;

QPointF point(const QJsonValue& value) {
    const QJsonArray pair = value.toArray();
    return QPointF(pair.at(0).toDouble(), pair.at(1).toDouble());
}

QVector3D vector(const QJsonValue& value) {
    const QJsonObject object = value.toObject();
    return QVector3D(float(object.value("x").toDouble()), float(object.value("y").toDouble()),
                     float(object.value("z").toDouble()));
}

QColor withAlpha(QColor color, double alpha) {
    color.setAlphaF(float(alpha));
    return color;
}

const char* kAxisNames[3] = {"x", "y", "z"};
const char* kPlaneNames[3] = {"yz", "xz", "xy"};

}  // namespace

// ---- ViewportOverlay ------------------------------------------------------------------------

ViewportOverlay::ViewportOverlay(EngineBridge* bridge, QWidget* parent) : QWidget(parent), bridge_(bridge) {
    setAttribute(Qt::WA_TransparentForMouseEvents);
    setAttribute(Qt::WA_NoSystemBackground);
    setFocusPolicy(Qt::NoFocus);
}

void ViewportOverlay::refresh() {
    const Reply reply = bridge_->call(QStringLiteral("viewport.overlay"),
                                      QJsonObject{{"width", width()}, {"height", height()}});
    model_ = reply.ok ? reply.result.toObject() : QJsonObject();
    update();
}

void ViewportOverlay::paintEvent(QPaintEvent*) {
    if (model_.isEmpty()) {
        return;
    }
    const PetuniaTheme& theme = PetuniaTheme::instance();
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    // Gold reads against all three axis colors and the purple accent.
    const QColor highlight = theme.statusWarning();

    // Guides: long lines through the pivot along the axes being used.
    for (const QJsonValue& value : model_.value("guides").toArray()) {
        const QJsonObject guide = value.toObject();
        QPen pen(withAlpha(theme.axisColor(guide.value("axis").toInt()), 0.65), 1.5, Qt::DashLine);
        painter.setPen(pen);
        painter.drawLine(point(guide.value("from")), point(guide.value("to")));
    }

    const QJsonObject gizmo = model_.value("gizmo").toObject();
    if (!gizmo.isEmpty()) {
        const QString lit = gizmo.value("highlight").toString();
        const QString tool = gizmo.value("tool").toString();
        const QPointF origin = point(gizmo.value("origin"));
        const QJsonArray axes = gizmo.value("axes").toArray();
        const QJsonArray planes = gizmo.value("planes").toArray();
        const double thickness = 2.4;

        // Plane squares (move and scale).
        for (int normal = 0; normal < 3 && normal < planes.size(); ++normal) {
            const QJsonArray quad = planes.at(normal).toArray();
            if (quad.size() != 4) {
                continue;
            }
            const bool hot = lit.endsWith(QLatin1String(kPlaneNames[normal])) && lit.contains('_');
            QPolygonF polygon;
            for (const QJsonValue& corner : quad) {
                polygon << point(corner);
            }
            const QColor color = hot ? highlight : theme.axisColor(normal);
            painter.setPen(QPen(withAlpha(color, 0.9), 1.0));
            painter.setBrush(withAlpha(color, hot ? 0.55 : 0.22));
            painter.drawPolygon(polygon);
        }
        painter.setBrush(Qt::NoBrush);

        for (int index = 0; index < 3 && index < axes.size(); ++index) {
            const QJsonObject axis = axes.at(index).toObject();
            const QString name = QLatin1String(kAxisNames[index]);
            const QColor base = theme.axisColor(index);

            // Rings: the arc facing the camera bright, the back arc dim (Petunia3D `ring_front`).
            const QJsonArray ring = axis.value("ring").toArray();
            const QJsonArray front = axis.value("ring_front").toArray();
            if (ring.size() > 1) {
                const bool hot = lit == QStringLiteral("rotate_") + name;
                const QColor color = hot ? highlight : base;
                for (int i = 0; i + 1 < ring.size(); ++i) {
                    const bool facing = front.at(i).toBool() && front.at(i + 1).toBool();
                    painter.setPen(QPen(withAlpha(color, facing ? 1.0 : 0.3), facing ? thickness + 0.6 : thickness - 0.6,
                                        Qt::SolidLine, Qt::RoundCap));
                    painter.drawLine(point(ring.at(i)), point(ring.at(i + 1)));
                }
            }

            const bool rodHot = lit == QStringLiteral("move_") + name ||
                                (tool == QLatin1String("scale") && lit == QStringLiteral("scale_") + name);
            const QColor rodColor = rodHot ? highlight : base;
            if (axis.value("rod").toBool()) {
                painter.setPen(QPen(rodColor, thickness, Qt::SolidLine, Qt::RoundCap));
                painter.drawLine(origin, point(axis.value("end")));
            }
            const QJsonArray arrow = axis.value("arrow").toArray();
            if (arrow.size() == 3) {
                QPolygonF triangle{point(arrow.at(0)), point(arrow.at(1)), point(arrow.at(2))};
                painter.setPen(QPen(rodColor, 1.0));
                painter.setBrush(rodColor);
                painter.drawPolygon(triangle);
                painter.setBrush(Qt::NoBrush);
            }
            const QJsonObject box = axis.value("scale_box").toObject();
            if (!box.isEmpty()) {
                const QPointF center = point(box.value("center"));
                const double half = box.value("half").toDouble();
                const bool hot = lit == QStringLiteral("scale_") + name;
                const QColor color = hot ? highlight : base;
                painter.setPen(QPen(color.darker(140), 1.0));
                painter.setBrush(color);
                painter.drawRect(QRectF(center.x() - half, center.y() - half, half * 2, half * 2));
                painter.setBrush(Qt::NoBrush);
            }
            // The axis is never told by color alone: its letter sits at the end.
            if (axis.value("rod").toBool() || ring.size() > 1) {
                QFont font = painter.font();
                font.setBold(true);
                font.setPixelSize(11);
                painter.setFont(font);
                painter.setPen(rodHot || lit == QStringLiteral("rotate_") + name ? highlight : base);
                const QPointF label = point(axis.value("label"));
                painter.drawText(QRectF(label.x() - 8, label.y() - 8, 16, 16), Qt::AlignCenter, PetuniaTheme::axisLetter(index));
            }
        }

        // Rotate: the outer view ring turns around the line of sight.
        if (gizmo.value("view_ring_radius").isDouble()) {
            const double radius = gizmo.value("view_ring_radius").toDouble();
            const bool hot = lit == QLatin1String("rotate_view");
            painter.setPen(QPen(hot ? highlight : withAlpha(theme.textSecondary(), 0.8), hot ? 2.6 : 1.6));
            painter.drawEllipse(origin, radius, radius);
        }
        // Center: move on the screen plane, uniform scale or free rotation; the universal gizmo
        // keeps a small hollow ring as a dead zone (a click there selects).
        const QString center = gizmo.value("center").toString();
        const double centerRadius = 6.0;
        if (center.isEmpty()) {
            painter.setPen(QPen(withAlpha(theme.textPrimary(), 0.7), 1.2));
            painter.drawEllipse(origin, 3.0, 3.0);
        } else {
            const bool hot = lit == center;
            painter.setPen(QPen(withAlpha(QColor(Qt::black), 0.6), 1.0));
            painter.setBrush(hot ? highlight : withAlpha(theme.textPrimary(), 0.85));
            painter.drawEllipse(origin, centerRadius, centerRadius);
            painter.setBrush(Qt::NoBrush);
        }
    }

    // Pivot: a small diamond (the point things turn and scale around).
    if (model_.value("pivot").isArray()) {
        // Always visible for the selection; bigger and labeled while it is being edited.
        const QPointF pivot = point(model_.value("pivot"));
        const bool editing = model_.value("edit_pivot").toBool();
        const double r = editing ? 9.0 : 5.0;
        QPolygonF diamond{pivot + QPointF(0, -r), pivot + QPointF(r, 0), pivot + QPointF(0, r), pivot + QPointF(-r, 0)};
        painter.setPen(QPen(Qt::white, editing ? 2.0 : 1.2));
        painter.setBrush(editing ? theme.statusWarning() : theme.accentSelection());
        painter.drawPolygon(diamond);
        painter.setBrush(Qt::NoBrush);
        if (editing) {
            QFont font = painter.font();
            font.setPixelSize(11);
            font.setBold(true);
            painter.setFont(font);
            painter.setPen(theme.statusWarning());
            painter.drawText(pivot + QPointF(12, -10), QStringLiteral("Pivô"));
        }
    }
    // 3D cursor: dashed ring with a cross, red and white so it reads on any background.
    if (model_.value("cursor").isArray()) {
        const QPointF cursor = point(model_.value("cursor"));
        painter.setPen(QPen(Qt::white, 2.0));
        painter.drawEllipse(cursor, 10.0, 10.0);
        painter.setPen(QPen(QColor(230, 60, 60), 2.0, Qt::DashLine));
        painter.drawEllipse(cursor, 10.0, 10.0);
        painter.setPen(QPen(Qt::black, 1.0));
        painter.drawLine(cursor + QPointF(-16, 0), cursor + QPointF(-6, 0));
        painter.drawLine(cursor + QPointF(6, 0), cursor + QPointF(16, 0));
        painter.drawLine(cursor + QPointF(0, -16), cursor + QPointF(0, -6));
        painter.drawLine(cursor + QPointF(0, 6), cursor + QPointF(0, 16));
    }

    // Heads-up card for the gesture in progress (top-left of the view).
    const QJsonObject hud = model_.value("hud").toObject();
    if (!hud.isEmpty()) {
        const QString title = hud.value("title").toString();
        const QString detail = hud.value("detail").toString();
        const QString snap = hud.value("snap").toString();
        QFont titleFont = painter.font();
        titleFont.setPixelSize(12);
        titleFont.setBold(true);
        QFont detailFont = titleFont;
        detailFont.setPixelSize(18);
        QFont noteFont = titleFont;
        noteFont.setBold(false);
        noteFont.setPixelSize(11);
        const QRectF card(12, 12, 330, 74);
        painter.setPen(QPen(theme.borderStrong(), 1.0));
        painter.setBrush(theme.overlaySurface());
        painter.drawRoundedRect(card, 6, 6);
        painter.setPen(theme.textSecondary());
        painter.setFont(titleFont);
        painter.drawText(card.adjusted(10, 6, -10, 0), Qt::AlignLeft | Qt::AlignTop, title);
        painter.setPen(theme.textPrimary());
        painter.setFont(detailFont);
        painter.drawText(card.adjusted(10, 22, -10, 0), Qt::AlignLeft | Qt::AlignTop, detail);
        painter.setPen(theme.textMuted());
        painter.setFont(noteFont);
        painter.drawText(card.adjusted(10, 50, -10, 0), Qt::AlignLeft | Qt::AlignTop, snap);
    }
}

// ---- NavCubeWidget --------------------------------------------------------------------------

NavCubeWidget::NavCubeWidget(EngineBridge* bridge, QWidget* parent) : QWidget(parent), bridge_(bridge) {
    setMouseTracking(true);
    setAttribute(Qt::WA_NoSystemBackground);
    setToolTip(QStringLiteral("Clique numa face, aresta ou canto para olhar dali; nas letras para girar; na casa para voltar"));
    setAccessibleName(QStringLiteral("Cubo de navegação"));
    layoutCube();
    const QRectF bounds = cube_.bounds();
    setFixedSize(int(bounds.width()) + 4, int(bounds.height()) + 4);
    layoutCube();
}

void NavCubeWidget::layoutCube() {
    cube_.setGeometry(QPointF(width() / 2.0, height() / 2.0), kNavUnit);
}

void NavCubeWidget::setCamera(const QJsonObject& camera) {
    if (camera.isEmpty()) {
        return;
    }
    cube_.setCamera(vector(camera.value("right")), vector(camera.value("up")), vector(camera.value("forward")));
    update();
}

void NavCubeWidget::paintEvent(QPaintEvent*) {
    layoutCube();
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    // The cube was drawn for Petunia3D's dark canvas; a translucent disc keeps its labels
    // readable over a bright sky or floor.
    const PetuniaTheme& theme = PetuniaTheme::instance();
    painter.setPen(Qt::NoPen);
    painter.setBrush(withAlpha(theme.bgCanvas(), underMouse() ? 0.62 : 0.45));
    const QRectF bounds = cube_.bounds();
    painter.drawEllipse(bounds.center(), bounds.width() / 2.0 - 4.0, bounds.height() / 2.0 - 4.0);
    painter.setBrush(Qt::NoBrush);
    cube_.paint(painter, hover_, underMouse());
}

void NavCubeWidget::mouseMoveEvent(QMouseEvent* event) {
    const PetuniaNavCube::Hit hit = cube_.hitTest(event->position());
    if (hit.id != hover_.id || hit.type != hover_.type) {
        hover_ = hit;
        setCursor(hit.type == PetuniaNavCube::Hit::Type::None ? Qt::ArrowCursor : Qt::PointingHandCursor);
        update();
    }
}

void NavCubeWidget::leaveEvent(QEvent* event) {
    hover_ = {};
    update();
    QWidget::leaveEvent(event);
}

void NavCubeWidget::mousePressEvent(QMouseEvent* event) {
    if (event->button() != Qt::LeftButton) {
        return;
    }
    const PetuniaNavCube::Hit hit = cube_.hitTest(event->position());
    using Type = PetuniaNavCube::Hit::Type;
    static const char* presets[7] = {"", "front", "back", "right", "left", "top", "bottom"};
    Reply reply;
    switch (hit.type) {
        case Type::Preset:
            if (hit.preset >= 1 && hit.preset <= 6) {
                reply = bridge_->call(QStringLiteral("camera.view"), QJsonObject{{"name", QLatin1String(presets[hit.preset])}});
            }
            break;
        case Type::Direction:
            reply = bridge_->call(QStringLiteral("camera.view_direction"),
                                  QJsonObject{{"x", hit.direction.x()}, {"y", hit.direction.y()}, {"z", hit.direction.z()}});
            break;
        case Type::Yaw:
            reply = bridge_->call(QStringLiteral("camera.yaw"), QJsonObject{{"degrees", hit.yawDegrees}});
            break;
        case Type::Home:
            reply = bridge_->call(QStringLiteral("camera.reset"));
            break;
        case Type::Menu:
        case Type::None:
            return;
    }
    if (reply.ok) {
        emit viewChanged();
    }
}
