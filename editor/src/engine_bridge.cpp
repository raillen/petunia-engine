#include "engine_bridge.h"

#include <QJsonDocument>
#include <QOpenGLContext>

QString Reply::explanation() const {
    return action.isEmpty() ? QStringLiteral("%1 (%2)").arg(message, code)
                            : QStringLiteral("%1 (%2)\n%3").arg(message, code, action);
}

namespace {

QString takeString(char* raw) {
    if (raw == nullptr) {
        return {};
    }
    const QString text = QString::fromUtf8(raw);
    petunia_string_free(raw);
    return text;
}

const void* loadGlFunction(const char* name) {
    QOpenGLContext* context = QOpenGLContext::currentContext();
    return context == nullptr ? nullptr : reinterpret_cast<const void*>(context->getProcAddress(name));
}

}  // namespace

EngineBridge::EngineBridge(QObject* parent) : QObject(parent), handle_(petunia_editor_create()) {}

EngineBridge::~EngineBridge() {
    petunia_editor_destroy(handle_);
}

Reply EngineBridge::call(const QString& operation, const QJsonObject& fields) {
    Reply reply;
    if (handle_ == nullptr) {
        reply.code = QStringLiteral("EDITOR_UNAVAILABLE");
        reply.message = QStringLiteral("o motor do editor não pôde ser iniciado");
        return reply;
    }
    QJsonObject request = fields;
    request.insert(QStringLiteral("op"), operation);
    const QByteArray bytes = QJsonDocument(request).toJson(QJsonDocument::Compact);
    char* raw = petunia_editor_call(handle_, bytes.constData());
    const QByteArray answer = takeString(raw).toUtf8();

    QJsonParseError parseError;
    const QJsonDocument document = QJsonDocument::fromJson(answer, &parseError);
    if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
        reply.code = QStringLiteral("EDITOR_BAD_RESPONSE");
        reply.message = QStringLiteral("resposta ilegível do motor: %1").arg(parseError.errorString());
        return reply;
    }
    const QJsonObject object = document.object();
    reply.ok = object.value(QStringLiteral("ok")).toBool(false);
    if (reply.ok) {
        reply.result = object.value(QStringLiteral("result"));
    } else {
        const QJsonObject error = object.value(QStringLiteral("error")).toObject();
        reply.code = error.value(QStringLiteral("code")).toString();
        reply.message = error.value(QStringLiteral("message")).toString();
        reply.action = error.value(QStringLiteral("action")).toString();
    }
    return reply;
}

QString EngineBridge::initViewport() {
    return takeString(petunia_editor_viewport_init(handle_, &loadGlFunction));
}

QString EngineBridge::render(int width, int height) {
    return takeString(petunia_editor_viewport_render(
        handle_, static_cast<uint32_t>(qMax(width, 0)), static_cast<uint32_t>(qMax(height, 0))));
}

void EngineBridge::releaseViewport() {
    petunia_editor_viewport_release(handle_);
}

void EngineBridge::refresh() {
    const Reply reply = call(QStringLiteral("editor.snapshot"));
    snapshot_ = reply.ok ? reply.result.toObject() : QJsonObject();
    emit snapshotChanged();
}

bool EngineBridge::hasProject() const {
    return snapshot_.value(QStringLiteral("project")).isObject();
}

bool EngineBridge::hasSelection() const {
    return snapshot_.value(QStringLiteral("selection")).isDouble();
}

quint64 EngineBridge::selection() const {
    return static_cast<quint64>(snapshot_.value(QStringLiteral("selection")).toDouble(0));
}
