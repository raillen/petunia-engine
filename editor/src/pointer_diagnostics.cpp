#include "pointer_diagnostics.h"

#include <QApplication>
#include <QDateTime>
#include <QGuiApplication>
#include <QMainWindow>
#include <QMouseEvent>
#include <QPainter>
#include <QScreen>
#include <QTextStream>
#include <QTimer>
#include <QWidget>
#include <QWindow>

#include <algorithm>

namespace {

QString describeWidget(const QWidget* widget) {
    if (widget == nullptr) {
        return QStringLiteral("(nenhum)");
    }
    return QStringLiteral("%1/%2")
        .arg(QString::fromUtf8(widget->metaObject()->className()),
             widget->objectName().isEmpty() ? QStringLiteral("(sem nome)") : widget->objectName());
}

QString describeScreen(const QScreen* screen) {
    if (screen == nullptr) {
        return QStringLiteral("(fora de qualquer tela)");
    }
    const QRect geometry = screen->geometry();
    const QRect available = screen->availableGeometry();
    return QStringLiteral("%1 geom=%2x%3+%4+%5 útil=%6x%7 dpr=%8")
        .arg(screen->name())
        .arg(geometry.width())
        .arg(geometry.height())
        .arg(geometry.x())
        .arg(geometry.y())
        .arg(available.width())
        .arg(available.height())
        .arg(screen->devicePixelRatio());
}

QString describeWindow(const QWidget* window) {
    if (window == nullptr) {
        return QStringLiteral("(sem janela ativa)");
    }
    const QRect geometry = window->geometry();
    const QRect frame = window->frameGeometry();
    const QWindow* handle = window->windowHandle();
    return QStringLiteral("%1 geom=%2x%3+%4+%5 moldura=%6x%7+%8+%9 dpr=%10")
        .arg(describeWidget(window))
        .arg(geometry.width())
        .arg(geometry.height())
        .arg(geometry.x())
        .arg(geometry.y())
        .arg(frame.width())
        .arg(frame.height())
        .arg(frame.x())
        .arg(frame.y())
        .arg(handle != nullptr ? handle->devicePixelRatio() : window->devicePixelRatio());
}

// Uma linha por amostra: plataforma, telas e a geometria com que a janela ficou. O compositor
// ajusta a janela depois de mapeá-la (o niri ladrilha), por isso a segunda amostra é a que vale.
void printScaleState(const char* when) {
    QTextStream out(stderr);
    out << "[escala] " << when << ": plataforma=" << QGuiApplication::platformName();
    for (const QScreen* screen : QGuiApplication::screens()) {
        out << " | tela " << describeScreen(screen);
    }
    for (const QWindow* window : QGuiApplication::topLevelWindows()) {
        if (!window->isVisible() || window->transientParent() != nullptr) {
            continue;
        }
        const QRect geometry = window->geometry();
        const qreal ratio = window->devicePixelRatio();
        out << " | janela " << geometry.width() << "x" << geometry.height() << "+" << geometry.x()
            << "+" << geometry.y()
            << " buffer=" << qRound(geometry.width() * ratio) << "x"
            << qRound(geometry.height() * ratio) << " dpr=" << ratio;
        if (window->screen() != nullptr) {
            out << " tela_da_janela=" << window->screen()->name();
        }
    }
    out << "\n";
    out.flush();
}

// Camada transparente ao mouse onde a cruz do clique aparece por alguns segundos. Não recebe
// evento nenhum: o clique continua indo para o controle de baixo, como se a camada não existisse.
class PointerOverlay final : public QWidget {
public:
    explicit PointerOverlay(QWidget* parent) : QWidget(parent) {
        setAttribute(Qt::WA_TransparentForMouseEvents);
        setAttribute(Qt::WA_NoSystemBackground);
        setFocusPolicy(Qt::NoFocus);
        timer_.setInterval(250);
        connect(&timer_, &QTimer::timeout, this, [this] {
            const qint64 now = QDateTime::currentMSecsSinceEpoch();
            const auto before = marks_.size();
            marks_.erase(std::remove_if(marks_.begin(), marks_.end(),
                                        [now](const Mark& mark) { return now - mark.when > 4000; }),
                         marks_.end());
            if (marks_.size() != before) {
                update();
            }
        });
    }

    void mark(const QPoint& point) {
        marks_.push_back({point, QDateTime::currentMSecsSinceEpoch()});
        if (marks_.size() > 8) {
            marks_.removeFirst();
        }
        if (!timer_.isActive()) {
            timer_.start();
        }
        raise();
        update();
    }

protected:
    void paintEvent(QPaintEvent*) override {
        QPainter painter(this);
        painter.setRenderHint(QPainter::Antialiasing);
        painter.setBrush(Qt::NoBrush);
        painter.setPen(QPen(QColor(230, 30, 30), 2));
        for (const Mark& mark : marks_) {
            painter.drawLine(mark.point - QPoint(14, 0), mark.point + QPoint(14, 0));
            painter.drawLine(mark.point - QPoint(0, 14), mark.point + QPoint(0, 14));
            painter.drawEllipse(mark.point, 9, 9);
        }
    }

private:
    struct Mark {
        QPoint point;
        qint64 when;
    };
    QList<Mark> marks_;
    QTimer timer_;
};

class PointerDiagnostics final : public QObject {
public:
    explicit PointerDiagnostics(QMainWindow& window)
        : QObject(&window), window_(window), overlay_(new PointerOverlay(&window)) {
        overlay_->setGeometry(window_.rect());
        overlay_->show();
        overlay_->raise();
        printScaleState("abriu");
        // O compositor ajusta a janela depois de mapeá-la; a segunda amostra mostra o tamanho real.
        QTimer::singleShot(1500, this, [] { printScaleState("1,5 s depois"); });
    }

    void start() {
        // Instalado na aplicação: a linha aparece para qualquer widget, antes de o controle tratar
        // o clique, e continua valendo para menus e diálogos.
        qApp->installEventFilter(this);
    }

protected:
    bool eventFilter(QObject* watched, QEvent* event) override {
        if (event->type() == QEvent::Resize && watched == &window_) {
            overlay_->setGeometry(window_.rect());
            overlay_->raise();
            printScaleState("redimensionou");
            return false;
        }
        if (event->type() != QEvent::MouseButtonPress) {
            return false;
        }
        auto* mouse = static_cast<QMouseEvent*>(event);
        if (mouse->button() != Qt::LeftButton) {
            return false;
        }
        const QPoint global = mouse->globalPosition().toPoint();
        QTextStream out(stderr);
        out << "[ponteiro] clique"
            << " global=" << global.x() << "," << global.y()
            << " local=" << mouse->position().x() << "," << mouse->position().y()
            << " recebeu=" << describeWidget(qobject_cast<QWidget*>(watched))
            << " sob_o_cursor=" << describeWidget(QApplication::widgetAt(global))
            << " janela=" << describeWindow(QApplication::activeWindow())
            << " tela=" << describeScreen(QGuiApplication::screenAt(global))
            << "\n";
        out.flush();
        overlay_->mark(overlay_->mapFromGlobal(global));
        return false;
    }

private:
    QMainWindow& window_;
    PointerOverlay* overlay_ = nullptr;
};

}  // namespace

void installPointerDiagnostics(QMainWindow& window) {
    auto* diagnostics = new PointerDiagnostics(window);
    diagnostics->start();
}
