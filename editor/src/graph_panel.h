#pragma once

#include <QJsonObject>
#include <QString>
#include <QWidget>

class EngineBridge;
class QLabel;
class QListWidget;
class QPlainTextEdit;
class QStackedWidget;
class QToolButton;
class QTreeWidget;
class QTreeWidgetItem;
class NodeCanvas;
class PetuniaSegmented;

// The logic workspace (DR-30): node graphs as an outline. Each row reads like a sentence
// ("QUANDO A cada quadro", "SE Comparar é menor que ..."), the whole graph is editable from the
// keyboard, and every row has a spoken description. A canvas of boxes and wires will draw the same
// graph later; this view stays as the accessible way to edit it.
//
//   Insert  add a node after the selected row (or inside a branch)
//   Enter   edit the selected input or setting
//   Delete  remove the selected node, or clear the selected input
//   Ctrl+Z  undo in this graph (Ctrl+Shift+Z redoes)
class GraphPanel : public QWidget {
    Q_OBJECT

public:
    explicit GraphPanel(EngineBridge* bridge, QWidget* parent = nullptr);

    // Re-reads the list of graphs and the open one (when the workspace is shown).
    void reload();

    // While the game runs: what ran and what this graph's conditions decided (N5).
    void showDebug(const QJsonObject& debug);
    void setPlaying(bool playing);

signals:
    // A short sentence for the window's status bar.
    void message(const QString& text);

private:
    void onSnapshot();
    void reloadList(const QString& select = QString());
    void openGraph(const QString& name);
    void refreshView();
    void renderView(const QJsonObject& view);
    void showCode();

    void addEvent();
    void addAfter();
    void removeCurrent();
    void editCurrent();
    void editInput(const QJsonObject& row, const QPoint& where);
    void editSetting(const QJsonObject& row, const QPoint& where);
    void undo();
    void redo();
    void createGraph();
    void attach();
    void goToDiagnostic(const QJsonObject& diagnostic);
    void contextMenu(const QPoint& position);
    void showMode(const QString& mode);

    // Calls an operation; on failure tells the person why and returns an empty object.
    bool run(const QString& operation, const QJsonObject& fields, QJsonObject* result = nullptr);
    QJsonObject currentRow() const;
    QJsonObject anchorOf(const QTreeWidgetItem* item) const;
    void selectNode(int node);
    void updateButtons();

    EngineBridge* bridge_;
    QListWidget* graphs_;
    QLabel* noGraphs_;
    QTreeWidget* tree_;
    QStackedWidget* views_;
    QPlainTextEdit* code_;
    NodeCanvas* canvas_ = nullptr;
    PetuniaSegmented* switch_ = nullptr;
    QListWidget* diagnostics_;
    QListWidget* debugRows_ = nullptr;
    QLabel* status_;
    QToolButton* newButton_;
    QToolButton* addEventButton_;
    QToolButton* addButton_;
    QToolButton* editButton_;
    QToolButton* removeButton_;
    QToolButton* undoButton_;
    QToolButton* redoButton_;
    QToolButton* codeButton_;
    QToolButton* attachButton_;
    QString openName_;
    QString projectKey_;
    QString selectedKey_;
    bool canUndo_ = false;
    bool canRedo_ = false;
    bool rendering_ = false;
};
