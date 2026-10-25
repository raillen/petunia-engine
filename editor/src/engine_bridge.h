#pragma once

#include <QJsonObject>
#include <QJsonValue>
#include <QObject>
#include <QString>

#include "petunia_editor_ffi.h"

// Answer of one engine operation.
struct Reply {
    bool ok = false;
    QJsonValue result;
    QString code;
    QString message;
    QString action;

    // "message (CODE)\naction": what to show a person when something failed.
    QString explanation() const;
};

// The only class that talks to the Rust library. Everything else sends operations through
// `call` and reads the cached snapshot.
class EngineBridge : public QObject {
    Q_OBJECT

public:
    explicit EngineBridge(QObject* parent = nullptr);
    ~EngineBridge() override;

    bool valid() const { return handle_ != nullptr; }

    Reply call(const QString& operation, const QJsonObject& fields = {});

    // Viewport (the GL context must be current). Empty string means success.
    QString initViewport();
    QString render(int width, int height);
    void releaseViewport();

    // Pulls a fresh snapshot of the editor state and tells everyone.
    void refresh();
    const QJsonObject& snapshot() const { return snapshot_; }

    // Convenience readers over the snapshot.
    bool hasProject() const;
    bool hasSelection() const;
    quint64 selection() const;

signals:
    void snapshotChanged();

private:
    EditorHandle* handle_ = nullptr;
    QJsonObject snapshot_;
};
