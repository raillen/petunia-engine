#pragma once

#include <QTreeWidget>
#include <QWidget>

class EngineBridge;
class QListWidget;

// Project Doctor: everything the engine found wrong (or worth knowing) about the project.
class DiagnosticsPanel : public QTreeWidget {
    Q_OBJECT

public:
    explicit DiagnosticsPanel(EngineBridge* bridge, QWidget* parent = nullptr);
    void reload();

private:
    EngineBridge* bridge_;
};

// What the editor did: commands, undo/redo, open and save, newest at the bottom.
class HistoryPanel : public QWidget {
    Q_OBJECT

public:
    explicit HistoryPanel(EngineBridge* bridge, QWidget* parent = nullptr);
    void reload();

private:
    EngineBridge* bridge_;
    QListWidget* list_;
    int shown_ = -1;
};

// The project's `assets/` folder: the models a scene can be built from. Double-click one to
// place it in the scene; the engine copies imported files here.
class AssetPanel : public QWidget {
    Q_OBJECT

public:
    explicit AssetPanel(EngineBridge* bridge, QWidget* parent = nullptr);
    // Re-reads the folder. Cheap when nothing changed: the list is rebuilt only on a difference.
    void reload();

signals:
    // A file of the folder was chosen to be placed in the scene (path relative to the project).
    void importRequested(const QString& path);
    // The person asked for a file from disk: the window opens the picker.
    void importModelRequested();

protected:
    void showEvent(QShowEvent* event) override;

private:
    EngineBridge* bridge_;
    QListWidget* list_;
    QString shown_;
};

// The engine's API as data: every command and query with its summary, status and example.
class ApiPanel : public QTreeWidget {
    Q_OBJECT

public:
    explicit ApiPanel(EngineBridge* bridge, QWidget* parent = nullptr);
    void reload();

private:
    EngineBridge* bridge_;
};
