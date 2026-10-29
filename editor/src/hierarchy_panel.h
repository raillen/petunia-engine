#pragma once

#include <QJsonArray>
#include <QWidget>

class EngineBridge;
class QLineEdit;
class QMenu;
class QTreeWidget;
class QTreeWidgetItem;
class QLabel;
class QToolButton;

// Tree of every object in the scene under the project's name, with a "Criar" button and a
// filter box on top. Selecting here selects in the engine.
class HierarchyPanel : public QWidget {
    Q_OBJECT

public:
    explicit HierarchyPanel(EngineBridge* bridge, QWidget* parent = nullptr);

    // Follows the engine's snapshot.
    void reload();
    // The menu behind the "Criar" button (the same actions as the Criar menu of the window).
    void setCreateMenu(QMenu* menu);

signals:
    void selectionRequested(quint64 entity);

protected:
    // Down or Enter inside the filter picks the first object still visible, so the search has a
    // keyboard path (typing a name is not a reason to reach for the mouse).
    bool eventFilter(QObject* watched, QEvent* event) override;

private:
    void applyFilter();
    bool selectFirstVisibleObject();
    void syncSelection();
    void refreshIcons();

    EngineBridge* bridge_;
    QToolButton* createButton_;
    QLineEdit* filter_;
    QTreeWidget* tree_;
    QTreeWidgetItem* root_ = nullptr;
    QLabel* summary_;
    QJsonArray shown_;
    QString shownProject_;
    bool syncing_ = false;
};
