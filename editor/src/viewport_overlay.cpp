#include "viewport_overlay.h"

#include <cmath>

#include <QJsonArray>
#include <QKeyEvent>
#include <QMenu>
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

void ViewportOverlay::setKnifeLine(bool active, const QPointF& start, const QPointF& current) {
    knifeActive_ = active;
    knifeStart_ = start;
    knifeCurrent_ = current;
    update();
}

void ViewportOverlay::paintEvent(QPaintEvent*) {
    // The knife line is independent of the overlay model: it exists only while the tool is armed
    // and one point has been marked.
    if (knifeActive_) {
        QPainter knifePainter(this);
        knifePainter.setRenderHint(QPainter::Antialiasing);
        QPen pen(QColor(255, 90, 90), 1.6, Qt::DashLine);
        pen.setDashPattern({6.0, 4.0});
        knifePainter.setPen(pen);
        knifePainter.drawLine(knifeStart_, knifeCurrent_);
    }
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
    setFocusPolicy(Qt::StrongFocus);
    setAttribute(Qt::WA_NoSystemBackground);
    setToolTip(QStringLiteral("Clique numa face, aresta ou canto para olhar dali; nas letras para girar; na casa para voltar; arraste ou use as setas para orbitar; Espaço/Enter abre a projeção"));
    setAccessibleName(QStringLiteral("Cubo de navegação"));
    setAccessibleDescription(QStringLiteral("Arraste com o botão esquerdo para orbitar. Use as setas do teclado para orbitar e Espaço ou Enter para abrir o menu de projeção."));
    setFixedSize(140, 144);
    layout_cube();
}

void NavCubeWidget::layout_cube() {
    cube_.setGeometry(QPointF(width() / 2.0, (height() - 6.0) / 2.0), kNavUnit);
}

void NavCubeWidget::setCamera(const QJsonObject& camera) {
    if (camera.isEmpty()) {
        return;
    }
    cube_.setCamera(vector(camera.value("right")), vector(camera.value("up")), vector(camera.value("forward")));
    const QString projection_mode = camera.value(QStringLiteral("projection_mode")).toString();
    if (!projection_mode.isEmpty()) {
        cube_.setOrtho(projection_mode == QLatin1String("orthographic"));
    }
    update();
}

void NavCubeWidget::paintEvent(QPaintEvent*) {
    layout_cube();
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    // The cube was drawn for Petunia3D's dark canvas; a translucent disc keeps its labels
    // readable over a bright sky or floor.
    const PetuniaTheme& theme = PetuniaTheme::instance();
    const bool active = underMouse() || hasFocus();
    painter.setPen(hasFocus() ? QPen(theme.accentPrimary(), 1.5) : Qt::NoPen);
    painter.setBrush(withAlpha(theme.bgCanvas(), active ? 0.62 : 0.45));
    const double disc_radius = cube_.discRadius();
    painter.drawEllipse(cube_.center(), disc_radius, disc_radius);
    painter.setPen(Qt::NoPen);
    cube_.paint(painter, hover_, active);
}

void NavCubeWidget::mouseMoveEvent(QMouseEvent* event) {
    constexpr double drag_threshold = 4.0;
    if (has_left_press_ && (event->buttons() & Qt::LeftButton)) {
        const double delta_x = event->position().x() - press_position_.x();
        const double delta_y = event->position().y() - press_position_.y();
        const double distance = std::hypot(delta_x, delta_y);
        if (!is_dragging_ && distance > drag_threshold) {
            is_dragging_ = true;
            hover_ = {};
            setCursor(Qt::ClosedHandCursor);
            update();
        }
        if (is_dragging_) {
            const double move_delta_x = event->position().x() - last_drag_position_.x();
            const double move_delta_y = event->position().y() - last_drag_position_.y();
            last_drag_position_ = event->position();
            if (move_delta_x != 0.0 || move_delta_y != 0.0) {
                invoke_command(QStringLiteral("camera.orbit"),
                               QJsonObject{{"dx", move_delta_x}, {"dy", move_delta_y}});
            }
            event->accept();
            return;
        }
    }

    const PetuniaNavCube::Hit hit = cube_.hitTest(event->position());
    if (hit.id != hover_.id || hit.type != hover_.type) {
        hover_ = hit;
        setCursor(hit.type == PetuniaNavCube::Hit::Type::None ? Qt::ArrowCursor : Qt::PointingHandCursor);
        update();
    }
    event->accept();
}

void NavCubeWidget::mousePressEvent(QMouseEvent* event) {
    if (event->button() != Qt::LeftButton) {
        QWidget::mousePressEvent(event);
        return;
    }
    has_left_press_ = true;
    press_position_ = event->position();
    last_drag_position_ = event->position();
    pressed_hit_ = cube_.hitTest(event->position());
    is_dragging_ = false;
    event->accept();
}

void NavCubeWidget::mouseReleaseEvent(QMouseEvent* event) {
    if (event->button() != Qt::LeftButton || !has_left_press_) {
        QWidget::mouseReleaseEvent(event);
        return;
    }
    has_left_press_ = false;

    constexpr double drag_threshold = 4.0;
    const double release_delta_x = event->position().x() - press_position_.x();
    const double release_delta_y = event->position().y() - press_position_.y();
    const double release_distance = std::hypot(release_delta_x, release_delta_y);

    const bool had_drag = is_dragging_ || (release_distance > drag_threshold);
    is_dragging_ = false;

    const PetuniaNavCube::Hit release_hit = cube_.hitTest(event->position());
    hover_ = release_hit;
    setCursor(release_hit.type == PetuniaNavCube::Hit::Type::None ? Qt::ArrowCursor : Qt::PointingHandCursor);
    update();

    if (had_drag) {
        event->accept();
        return;
    }

    const bool same_target = (release_hit.type == pressed_hit_.type &&
                              release_hit.id == pressed_hit_.id &&
                              release_hit.preset == pressed_hit_.preset &&
                              release_hit.type != PetuniaNavCube::Hit::Type::None);
    if (same_target) {
        handle_hit(release_hit, event->globalPosition().toPoint());
    }
    event->accept();
}

void NavCubeWidget::invoke_command(const QString& operation, const QJsonObject& fields) {
    if (bridge_ == nullptr) {
        return;
    }
    const Reply reply = bridge_->call(operation, fields);
    if (reply.ok) {
        emit viewChanged();
    } else {
        emit operationFailed(reply.explanation());
    }
}

void NavCubeWidget::handle_hit(const PetuniaNavCube::Hit& hit, const QPoint& global_position) {
    using Type = PetuniaNavCube::Hit::Type;
    static const char* presets[7] = {"", "front", "back", "right", "left", "top", "bottom"};
    switch (hit.type) {
        case Type::Preset:
            if (hit.preset >= 1 && hit.preset <= 6) {
                invoke_command(QStringLiteral("camera.view"),
                               QJsonObject{{"name", QLatin1String(presets[hit.preset])}});
            }
            break;
        case Type::Direction:
            invoke_command(QStringLiteral("camera.view_direction"),
                           QJsonObject{{"x", hit.direction.x()}, {"y", hit.direction.y()}, {"z", hit.direction.z()}});
            break;
        case Type::Yaw:
            invoke_command(QStringLiteral("camera.yaw"),
                           QJsonObject{{"degrees", hit.yawDegrees}});
            break;
        case Type::Home:
            invoke_command(QStringLiteral("camera.reset"));
            break;
        case Type::Menu:
            show_projection_menu(global_position);
            break;
        case Type::None:
            break;
    }
}

void NavCubeWidget::show_projection_menu(const QPoint& global_position) {
    QMenu menu(this);
    menu.setAccessibleName(QStringLiteral("Menu de projeção"));

    auto* perspective_action = menu.addAction(QStringLiteral("Perspectiva"));
    perspective_action->setCheckable(true);
    perspective_action->setChecked(!cube_.isOrtho());

    auto* orthographic_action = menu.addAction(QStringLiteral("Ortográfica"));
    orthographic_action->setCheckable(true);
    orthographic_action->setChecked(cube_.isOrtho());

    menu.addSeparator();

    auto* isometric_action = menu.addAction(QStringLiteral("Isométrica verdadeira (35,264°)"));
    auto* game_isometric_action = menu.addAction(QStringLiteral("Isométrica de jogo 2:1 (30°)"));

    QAction* selected_action = menu.exec(global_position);
    if (selected_action == nullptr || bridge_ == nullptr) {
        return;
    }

    if (selected_action == perspective_action || selected_action == orthographic_action) {
        const bool orthographic = selected_action == orthographic_action;
        const Reply reply = bridge_->call(QStringLiteral("camera.projection"),
                                          QJsonObject{{"mode", orthographic ? QStringLiteral("orthographic")
                                                                           : QStringLiteral("perspective")}});
        if (reply.ok) {
            cube_.setOrtho(orthographic);
            update();
            emit viewChanged();
        } else {
            emit operationFailed(reply.explanation());
        }
    } else if (selected_action == isometric_action) {
        invoke_command(QStringLiteral("camera.view"),
                       QJsonObject{{"name", QStringLiteral("isometric")}});
    } else if (selected_action == game_isometric_action) {
        invoke_command(QStringLiteral("camera.view"),
                       QJsonObject{{"name", QStringLiteral("game_2_1")}});
    }
}

void NavCubeWidget::keyPressEvent(QKeyEvent* event) {
    constexpr double key_orbit_step = 15.0;
    switch (event->key()) {
        case Qt::Key_Left:
            invoke_command(QStringLiteral("camera.orbit"),
                           QJsonObject{{"dx", -key_orbit_step}, {"dy", 0.0}});
            break;
        case Qt::Key_Right:
            invoke_command(QStringLiteral("camera.orbit"),
                           QJsonObject{{"dx", key_orbit_step}, {"dy", 0.0}});
            break;
        case Qt::Key_Up:
            invoke_command(QStringLiteral("camera.orbit"),
                           QJsonObject{{"dx", 0.0}, {"dy", -key_orbit_step}});
            break;
        case Qt::Key_Down:
            invoke_command(QStringLiteral("camera.orbit"),
                           QJsonObject{{"dx", 0.0}, {"dy", key_orbit_step}});
            break;
        case Qt::Key_Space:
        case Qt::Key_Return:
        case Qt::Key_Enter:
            event->accept();
            show_projection_menu(mapToGlobal(QPoint(width() / 2, height())));
            return;
        default:
            QWidget::keyPressEvent(event);
            return;
    }
    event->accept();
}

void NavCubeWidget::keyReleaseEvent(QKeyEvent* event) {
    switch (event->key()) {
        case Qt::Key_Left:
        case Qt::Key_Right:
        case Qt::Key_Up:
        case Qt::Key_Down:
        case Qt::Key_Space:
        case Qt::Key_Return:
        case Qt::Key_Enter:
            event->accept();
            return;
        default:
            QWidget::keyReleaseEvent(event);
            return;
    }
}

void NavCubeWidget::focusInEvent(QFocusEvent* event) {
    update();
    QWidget::focusInEvent(event);
}

void NavCubeWidget::focusOutEvent(QFocusEvent* event) {
    update();
    QWidget::focusOutEvent(event);
}

void NavCubeWidget::leaveEvent(QEvent* event) {
    if (!is_dragging_) {
        hover_ = {};
        update();
    }
    QWidget::leaveEvent(event);
}
