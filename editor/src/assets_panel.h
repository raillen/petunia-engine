#pragma once

#include <QTreeWidget>
#include <QWidget>

class EngineBridge;
class QLabel;
class QPushButton;

// The project's `assets/` folder (phase 2 pendency): what is there, how big, and a way to bring a
// new model in. The engine owns the list (`asset.list`) and the import (`asset.import`); this panel
// only shows and asks, like every other panel in the window.
class AssetsPanel : public QWidget {
    Q_OBJECT

public:
    explicit AssetsPanel(EngineBridge* bridge, QWidget* parent = nullptr);
    void reload();

signals:
    void message(const QString& text);
    /// Asked after an import, so the scene view can refresh.
    void imported();

private:
    EngineBridge* bridge_;
    QTreeWidget* tree_;
    QLabel* summary_;
    QPushButton* importButton_;
};
