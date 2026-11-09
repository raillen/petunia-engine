#pragma once

#include <QJsonArray>
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
    // Opens a graph by name (after another workspace created it).
    void open(const QString& name);

    // While the game runs: what ran and what this graph's conditions decided (N5).
    void showDebug(const QJsonObject& debug);
    void setPlaying(bool playing);

signals:
    // A short sentence for the window's status bar.
    void message(const QString& text);
    // A `.aipo` script was chosen in the sidebar: the window opens it in the code workspace.
    void scriptOpenRequested(const QString& name);

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
    void renameSelected();
    void deleteSelected();
    void asScript();
    void groupSelection();
    void ungroupSelection();
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
    QWidget* canvasBar_ = nullptr;
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
    QToolButton* asScriptButton_;
    QToolButton* attachButton_;
    QToolButton* zoomOutButton_ = nullptr;
    QToolButton* zoomInButton_ = nullptr;
    QToolButton* fitButton_ = nullptr;
    QToolButton* groupButton_ = nullptr;
    QToolButton* ungroupButton_ = nullptr;
    QString openName_;
    QString selectedName_;
    QString selectedKind_;
    QString projectKey_;
    QString selectedKey_;
    QJsonArray codeLines_;
    bool canUndo_ = false;
    bool canRedo_ = false;
    bool rendering_ = false;
    bool canvasFitted_ = false;
};
