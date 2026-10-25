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

// The engine's API as data: every command and query with its summary, status and example.
class ApiPanel : public QTreeWidget {
    Q_OBJECT

public:
    explicit ApiPanel(EngineBridge* bridge, QWidget* parent = nullptr);
    void reload();

private:
    EngineBridge* bridge_;
};
