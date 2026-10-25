#pragma once

#include <QJsonObject>
#include <QList>
#include <QMainWindow>
#include <QPair>

#include <memory>

class EngineBridge;
class ViewportWidget;
class ViewportHeader;
class HierarchyPanel;
class InspectorPanel;
class DiagnosticsPanel;
class HistoryPanel;
class CodePanel;
class ApiPanel;
class QAction;
class QLabel;
class QToolButton;
class QToolBar;
class QComboBox;
class QDoubleSpinBox;
class QCheckBox;
class QPushButton;
class QDockWidget;
class QMenu;
class QMenuBar;
struct Reply;

class MainWindow : public QMainWindow {
    Q_OBJECT

public:
    MainWindow();
    ~MainWindow() override;

    // Used by the command line (`--project`, `--sample`) and by the menus.
    bool openProjectAt(const QString& path, bool interactive);
    bool createSampleAt(const QString& path, const QString& name, bool interactive);
    void selectEntity(quint64 entity);
    // Saves a picture of the window (and of the 3D view alone) after a short delay, then quits.
    void captureAndQuit(const QString& path, int delayMilliseconds);

protected:
    void closeEvent(QCloseEvent* event) override;

private:
    void buildActions();
    void buildMenus();
    // One bar on top: menus, the tools, the gizmo controls, then undo, redo and save.
    void buildAppBar();
    void buildSculptBar();
    void syncSculptBar(const QJsonObject& snapshot);
    void buildDocks();

    Reply perform(const QString& operation, const QJsonObject& fields, bool interactive = true);
    void showFailure(const Reply& reply, bool interactive);
    void onSnapshot();
    void updateCameraLabel();
    bool confirmDiscardChanges();
    void chooseNewSample();
    void chooseOpen();
    void applyAppearance();
    void refreshIcons();
    void setActionIcon(QAction* action, const QString& iconName);
    void showShortcuts();
    void showPrimitivesPie();
    void showAbout();

    std::unique_ptr<EngineBridge> bridge_;
    ViewportWidget* viewport_ = nullptr;
    ViewportHeader* header_ = nullptr;
    HierarchyPanel* hierarchy_ = nullptr;
    InspectorPanel* inspector_ = nullptr;
    DiagnosticsPanel* diagnostics_ = nullptr;
    HistoryPanel* history_ = nullptr;
    ApiPanel* api_ = nullptr;
    QLabel* cameraLabel_ = nullptr;
    QLabel* countLabel_ = nullptr;
    QList<QDockWidget*> docks_;
    // The menus live inside the top bar, not in QMainWindow's own menu bar slot.
    QMenuBar* menus_ = nullptr;
    QMenu* viewMenu_ = nullptr;

    QAction* newAction_ = nullptr;
    QAction* openAction_ = nullptr;
    QAction* saveAction_ = nullptr;
    QAction* quitAction_ = nullptr;
    QAction* undoAction_ = nullptr;
    QAction* redoAction_ = nullptr;
    QAction* duplicateAction_ = nullptr;
    QAction* deleteAction_ = nullptr;
    QAction* clearSelectionAction_ = nullptr;
    QAction* shapeActions_[6] = {};
    QAction* toolActions_[8] = {};
    QAction* carveAction_ = nullptr;
    QAction* codeAction_ = nullptr;
    QDockWidget* codeDock_ = nullptr;
    CodePanel* code_ = nullptr;
    QAction* caveAction_ = nullptr;
    QAction* randomCaveAction_ = nullptr;
    QAction* sculptLargerAction_ = nullptr;
    QAction* sculptSmallerAction_ = nullptr;
    // Brush radius of the sculpting tool, as the engine last reported it.
    double sculptRadius_ = 1.5;
    // Set when the window is going away: a focused field emits editingFinished while it is
    // destroyed, and that must not reach an engine that is already gone.
    bool closing_ = false;
    // Options bar of the sculpting tools; only visible while one of them is active.
    QToolBar* sculptBar_ = nullptr;
    QComboBox* sculptMode_ = nullptr;
    QDoubleSpinBox* sculptRadiusBox_ = nullptr;
    QDoubleSpinBox* sculptForceBox_ = nullptr;
    QCheckBox* facetedBox_ = nullptr;
    QLabel* strokesLabel_ = nullptr;
    QPushButton* generateButton_ = nullptr;
    QPushButton* consolidateButton_ = nullptr;
    QAction* hollowAction_ = nullptr;
    QAction* floorUpAction_ = nullptr;
    QAction* floorDownAction_ = nullptr;
    QAction* editPivotAction_ = nullptr;
    // Exibir > Objetos acima do andar: ghost, hide, show.
    QList<QAction*> aboveActions_;
    // Double tap on a tool shortcut (Petunia3D "Modo Livre").
    QAction* lastToolAction_ = nullptr;
    qint64 lastToolMilliseconds_ = 0;
    QAction* viewActions_[6] = {};
    QAction* focusAction_ = nullptr;
    QAction* resetCameraAction_ = nullptr;
    QAction* largerTextAction_ = nullptr;
    QAction* smallerTextAction_ = nullptr;
    QAction* normalTextAction_ = nullptr;
    QAction* reducedMotionAction_ = nullptr;
    QAction* shortcutsAction_ = nullptr;
    QAction* primitivesMenuAction_ = nullptr;
    QAction* aboutAction_ = nullptr;

    QList<QPair<QAction*, QString>> iconActions_;
    QList<QPair<QToolButton*, QString>> iconButtons_;
    QString themeId_;
    int textStep_ = 0;
    int densityIndex_ = 1;
    bool reducedMotion_ = false;
    QString apiProjectRoot_;
    bool quitting_ = false;
};
