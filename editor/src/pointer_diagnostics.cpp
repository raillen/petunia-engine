#include "pointer_diagnostics.h"

#include <cstdio>

#include <QApplication>
#include <QEvent>
#include <QGuiApplication>
#include <QMainWindow>
#include <QMouseEvent>
#include <QPainter>
#include <QScreen>
#include <QTimer>
#include <QWidget>

namespace {

// Where the last click landed, drawn over the whole window. It is mouse-transparent, so the
// crosshair never eats the click it is measuring.
class ClickCrosshair : public QWidget {
public:
    explicit ClickCrosshair(QMainWindow& window) : QWidget(&window) {
        setAttribute(Qt::WA_TransparentForMouseEvents);
        // X11 does not composite a child widget's alpha; not painting the background at all lets
        // the parent's pixels show through, which is what the crosshair needs.
        setAttribute(Qt::WA_NoSystemBackground);
        setFocusPolicy(Qt::NoFocus);
    }

    void mark(const QPoint& point) {
        point_ = point;
        has_point_ = true;
        show();
        update();
    }

protected:
    void paintEvent(QPaintEvent*) override {
        if (!has_point_) {
            return;
        }
        QPainter painter(this);
        painter.setRenderHint(QPainter::Antialiasing);
        QPen pen(QColor(255, 40, 40));
        pen.setWidth(2);
        painter.setPen(pen);
        painter.drawLine(point_ - QPoint(18, 0), point_ + QPoint(18, 0));
        painter.drawLine(point_ - QPoint(0, 18), point_ + QPoint(0, 18));
        painter.drawEllipse(point_, 5, 5);
    }

private:
    QPoint point_;
    bool has_point_ = false;
};

const char* platform_label() {
    const QString name = QGuiApplication::platformName();
    if (name == QLatin1String("wayland")) {
        return "wayland";
    }
    if (name == QLatin1String("xcb")) {
        return "xcb (X11/XWayland)";
    }
    return "outra";
}

void log_scale(QMainWindow& window, const char* moment) {
    QScreen* screen = window.screen();
    if (screen == nullptr) {
        return;
    }
    const QRect geometry = window.geometry();
    const QSize size = window.size();
    std::fprintf(stderr,
                 "[escala] %s plataforma=%s dpr=%.3f tela=%dx%d area_util=%dx%d janela=%dx%d "
                 "origem=%d,%d tela_dpi=%.2f\n",
                 moment, platform_label(), screen->devicePixelRatio(),
                 screen->geometry().width(), screen->geometry().height(),
                 screen->availableGeometry().width(), screen->availableGeometry().height(),
                 size.width(), size.height(), geometry.x(), geometry.y(),
                 screen->logicalDotsPerInch());
    std::fflush(stderr);
}

class PointerDiagnosticsFilter : public QObject {
public:
    PointerDiagnosticsFilter(QMainWindow& window, ClickCrosshair& crosshair)
        : QObject(&window), window_(window), crosshair_(crosshair) {}

protected:
    bool eventFilter(QObject* watched, QEvent* event) override {
        if (event->type() == QEvent::Resize && watched == &window_) {
            crosshair_.setGeometry(window_.rect());
            crosshair_.raise();
        }
        if (event->type() != QEvent::MouseButtonPress) {
            return QObject::eventFilter(watched, event);
        }
        auto* mouse = dynamic_cast<QMouseEvent*>(event);
        if (mouse == nullptr) {
            return QObject::eventFilter(watched, event);
        }
        // Qt delivers the same press to the QWindow and then to the widget; only the widget event
        // carries the local position and the receiver that matters.
        QWidget* receiver = qobject_cast<QWidget*>(watched);
        if (receiver == nullptr) {
            return QObject::eventFilter(watched, event);
        }
        const QPoint global = mouse->globalPosition().toPoint();
        const QPoint local = mouse->position().toPoint();
        const QPoint window_point = window_.mapFromGlobal(global);
        QString receiver_name = receiver->objectName().isEmpty()
                                    ? receiver->metaObject()->className()
                                    : receiver->objectName();
        std::fprintf(stderr,
                     "[ponteiro] botao=%d global=%d,%d janela=%d,%d local=%d,%d dpr=%.3f "
                     "receptor=%s\n",
                     static_cast<int>(mouse->button()), global.x(), global.y(), window_point.x(),
                     window_point.y(), local.x(), local.y(), window_.devicePixelRatio(),
                     receiver_name.toUtf8().constData());
        std::fflush(stderr);
        crosshair_.setGeometry(window_.rect());
        crosshair_.mark(window_point);
        crosshair_.raise();
        return QObject::eventFilter(watched, event);
    }

private:
    QMainWindow& window_;
    ClickCrosshair& crosshair_;
};

}  // namespace

void install_pointer_diagnostics(QMainWindow& window) {
    auto* crosshair = new ClickCrosshair(window);
    crosshair->setGeometry(window.rect());
    crosshair->show();
    crosshair->raise();
    auto* filter = new PointerDiagnosticsFilter(window, *crosshair);
    // One filter on the application sees every widget's click; a second one on the window would
    // log each click twice.
    if (QApplication* application = qobject_cast<QApplication*>(QCoreApplication::instance())) {
        application->installEventFilter(filter);
    }
    // The first report runs after the window manager has placed the window.
    log_scale(window, "inicio");
    QTimer::singleShot(1500, &window, [&window] { log_scale(window, "1.5s"); });
}
