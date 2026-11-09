#pragma once

#include <QJsonArray>
#include <QWidget>

class EngineBridge;
class QLabel;
class QListWidget;
class QPushButton;

// The models in the project's `assets/` folder: list them, import a new file and put one in the
// scene. The list comes from the engine (`asset.list`), so it shows exactly what the project keeps;
// the panel never reads the disk itself.
class AssetPanel : public QWidget {
    Q_OBJECT

public:
    explicit AssetPanel(EngineBridge* bridge, QWidget* parent = nullptr);
    void reload();

signals:
    // Something happened that belongs in the status bar, not in a dialog.
    void statusMessage(const QString& text);

private:
    void insertSelected();
    void importFromDisk();
    void fill(const QJsonArray& assets, const QString& folder);

    EngineBridge* bridge_;
    QListWidget* list_;
    QPushButton* insertButton_;
    QPushButton* importButton_;
    QLabel* hint_;
};
