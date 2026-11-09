#include <QApplication>
#include <QCommandLineParser>
#include <QMessageBox>
#include <QOffscreenSurface>
#include <QOpenGLContext>
#include <QOpenGLFunctions>
#include <QSurfaceFormat>
#include <QTextStream>
#include <QTimer>

#include <optional>

#include <unistd.h>

#include "main_window.h"
#include "pointer_diagnostics.h"

namespace {

// The renderer needs OpenGL 3.3 (core features; WebGL2-compatible subset). Drivers differ in
// which window formats they accept (for example NVIDIA on Wayland refuses some multisampled
// configurations with EGL_BAD_MATCH), so the editor tries formats from best to simplest and
// uses the first one the driver creates.
struct Candidate {
    const char* label;
    int samples;
    int depthBits;
    bool coreProfile;
    bool requestVersion;
};

constexpr Candidate kCandidates[] = {
    {"3.3 core, antisserrilhado 4x, profundidade 24", 4, 24, true, true},
    {"3.3 core, sem antisserrilhado, profundidade 24", 0, 24, true, true},
    {"3.3 core, sem antisserrilhado, sem profundidade na janela", 0, 0, true, true},
    {"3.3 sem perfil (compatibilidade)", 0, 0, false, true},
    {"formato padrão do driver", 0, 0, false, false},
};

QSurfaceFormat formatFor(const Candidate& candidate) {
    QSurfaceFormat format;
    if (candidate.requestVersion) {
        format.setVersion(3, 3);
    }
    format.setProfile(candidate.coreProfile ? QSurfaceFormat::CoreProfile : QSurfaceFormat::NoProfile);
    format.setDepthBufferSize(candidate.depthBits);
    format.setSamples(candidate.samples);
    return format;
}

// Tries to create a context (no window is shown). Returns what the driver reports on success.
std::optional<QString> tryFormat(const QSurfaceFormat& wanted) {
    QOpenGLContext context;
    context.setFormat(wanted);
    if (!context.create()) {
        return std::nullopt;
    }
    QOffscreenSurface surface;
    surface.setFormat(context.format());
    surface.create();
    if (!context.makeCurrent(&surface)) {
        return std::nullopt;
    }
    QOpenGLFunctions* gl = context.functions();
    const auto text = [gl](GLenum name) {
        return QString::fromUtf8(reinterpret_cast<const char*>(gl->glGetString(name)));
    };
    const QString description = QStringLiteral("%1 · %2 · OpenGL %3")
                                    .arg(text(GL_VENDOR), text(GL_RENDERER), text(GL_VERSION));
    const QSurfaceFormat actual = context.format();
    context.doneCurrent();
    if (actual.majorVersion() * 10 + actual.minorVersion() < 33) {
        return std::nullopt;
    }
    return description;
}

}  // namespace

int main(int argc, char** argv) {
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("Petunia Editor"));
    QApplication::setOrganizationName(QStringLiteral("Petunia"));
    QApplication::setStyle(QStringLiteral("Fusion"));

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("Editor da Petunia Engine"));
    parser.addHelpOption();
    parser.addOption({{"p", "project"}, QStringLiteral("Abre a pasta de projeto."), QStringLiteral("pasta")});
    parser.addOption({"sample", QStringLiteral("Cria o projeto de exemplo nesta pasta e abre."), QStringLiteral("pasta")});
    parser.addOption({"method", QStringLiteral("Método de programação do projeto de exemplo: aipo (padrão), luau ou nodes."), QStringLiteral("metodo"), QStringLiteral("aipo")});
    parser.addOption({"select", QStringLiteral("Seleciona a entidade com este id."), QStringLiteral("id")});
    parser.addOption({"screenshot", QStringLiteral("Salva uma imagem da janela e sai (para testes)."), QStringLiteral("arquivo.png")});
    parser.addOption({"screenshot-delay", QStringLiteral("Espera antes da imagem, em milissegundos (padrão 1500)."), QStringLiteral("ms"), QStringLiteral("1500")});
    parser.addOption({"gl-info", QStringLiteral("Mostra quais formatos de OpenGL o driver aceita e sai (não abre janela).")});
    parser.addOption({"pointer-info", QStringLiteral("Registra escala e cada clique (global, janela e local) para diagnosticar ponteiro em escala fracionária.")});
    parser.process(app);

    QTextStream out(stderr);
    const bool infoOnly = parser.isSet("gl-info");
    out << "Plataforma gráfica do Qt: " << QGuiApplication::platformName() << "\n";

    std::optional<QSurfaceFormat> chosen;
    for (const Candidate& candidate : kCandidates) {
        const QSurfaceFormat format = formatFor(candidate);
        const std::optional<QString> result = tryFormat(format);
        if (infoOnly || !chosen) {
            out << (result ? "  ok     " : "  recusou ") << candidate.label;
            if (result) {
                out << "  →  " << *result;
            }
            out << "\n";
        }
        if (result && !chosen) {
            chosen = format;
            if (!infoOnly) {
                break;
            }
        }
    }
    if (!chosen && QGuiApplication::platformName() == QStringLiteral("wayland") &&
        qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM") && !qEnvironmentVariableIsEmpty("DISPLAY")) {
        // Qt's native Wayland backend can fail to create OpenGL contexts on some drivers (seen with
        // NVIDIA + Qt 6.12). XWayland works there, so restart through it. The variable we set also
        // stops this from looping.
        out << "O Wayland nativo recusou o OpenGL; reiniciando via X11 (XWayland).\n";
        out.flush();
        qputenv("QT_QPA_PLATFORM", "xcb");
        execv("/proc/self/exe", argv);  // returns only on failure
        out << "Não foi possível reiniciar o editor via X11.\n";
    }
    if (infoOnly) {
        return chosen ? 0 : 2;
    }
    if (!chosen) {
        const QString message = QStringLiteral(
            "O driver gráfico não conseguiu criar um contexto OpenGL 3.3.\n\n"
            "Rode  petunia-editor --gl-info  para ver o que ele aceita.\n"
            "Se você está no Wayland, tente também: QT_QPA_PLATFORM=xcb scripts/editor.sh");
        out << message << "\n";
        QMessageBox::critical(nullptr, QStringLiteral("OpenGL indisponível"), message);
        return 2;
    }
    QSurfaceFormat::setDefaultFormat(*chosen);

    MainWindow window;
    window.show();
    if (parser.isSet("pointer-info")) {
        install_pointer_diagnostics(window);
    }

    if (parser.isSet("sample")) {
        window.createSampleAt(parser.value("sample"), QStringLiteral("Sala de exemplo"), false,
                              parser.value("method"));
    } else if (parser.isSet("project")) {
        window.openProjectAt(parser.value("project"), false);
    }
    if (parser.isSet("select")) {
        window.selectEntity(parser.value("select").toULongLong());
    }
    if (parser.isSet("screenshot")) {
        window.captureAndQuit(parser.value("screenshot"), parser.value("screenshot-delay").toInt());
    }
    return app.exec();
}
