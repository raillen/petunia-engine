#pragma once

#include <QJsonObject>
#include <QList>
#include <QMainWindow>
#include <QHash>
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
class QShortcut;
class QStackedWidget;
class QVBoxLayout;
class PetuniaSegmented;
class CommandPalette;
struct Reply;
enum class ComponentMode;

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
    // One bar on top: the ≡ menu, the project, the workspaces, the command search, undo, redo, save.
    void buildAppBar();
    // The tools float in a column at the top left of the viewport (Blender, Unity "Tools" overlay).
    void buildToolPalette();
    // Workspaces share the scene; each keeps its own arrangement of panels.
    void switchWorkspace(const QString& id);
    void applyDefaultDocks(const QString& id);
    // The menus as words in the bar (on) or behind the ≡ button (off, the default).
    void setFullMenus(bool on);
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
    // Editar > Preferências (Ctrl+,): appearance and the selection outline.
    void showPreferences();
    // Sends the saved outline color and thickness to the engine.
    void applyOutlineStyle();
    void showPrimitivesPie();
    void toggleEditMode();
    void setComponentMode(ComponentMode mode);
    void syncEditMode();
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
    QToolButton* hamburger_ = nullptr;
    QAction* fullMenusAction_ = nullptr;
    QList<QShortcut*> menuShortcuts_;
    CommandPalette* palette_ = nullptr;
    QAction* paletteAction_ = nullptr;
    QLabel* projectLabel_ = nullptr;
    // Central pages, one per workspace: the scene (strip + viewport) and the code.
    QStackedWidget* pages_ = nullptr;
    QVBoxLayout* sceneLayout_ = nullptr;
    PetuniaSegmented* workspaces_ = nullptr;
    QString workspace_ = QStringLiteral("scene");
    QHash<QString, QByteArray> workspaceStates_;
    QAction* sceneAction_ = nullptr;
    QDockWidget* apiDock_ = nullptr;

    QAction* newAction_ = nullptr;
    QAction* openAction_ = nullptr;
    QAction* saveAction_ = nullptr;
    QAction* quitAction_ = nullptr;
    QAction* undoAction_ = nullptr;
    QAction* redoAction_ = nullptr;
    QAction* duplicateAction_ = nullptr;
    QAction* deleteAction_ = nullptr;
    QAction* clearSelectionAction_ = nullptr;
    QAction* editModeAction_ = nullptr;
    QAction* vertexModeAction_ = nullptr;
    QAction* edgeModeAction_ = nullptr;
    QAction* faceModeAction_ = nullptr;
    QAction* shapeActions_[6] = {};
    QAction* toolActions_[8] = {};
    QAction* carveAction_ = nullptr;
    QAction* codeAction_ = nullptr;
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
    // Exibir > Projeção: perspective, orthographic.
    QList<QAction*> projectionActions_;
    // Double tap on a tool shortcut (Petunia3D "Modo Livre").
    QAction* lastToolAction_ = nullptr;
    qint64 lastToolMilliseconds_ = 0;
    QAction* viewActions_[8] = {};
    QAction* focusAction_ = nullptr;
    QAction* resetCameraAction_ = nullptr;
    QAction* largerTextAction_ = nullptr;
    QAction* smallerTextAction_ = nullptr;
    QAction* normalTextAction_ = nullptr;
    QAction* reducedMotionAction_ = nullptr;
    QAction* shortcutsAction_ = nullptr;
    QAction* primitivesMenuAction_ = nullptr;
    QAction* aboutAction_ = nullptr;
    QAction* preferencesAction_ = nullptr;
    // Jogar (F6): runs the scene on a copy; the panels that edit it are locked until it stops.
    void togglePlay();
    void startPlay();
    void stopPlay();
    void setEditingLocked(bool locked);
    QAction* playAction_ = nullptr;
    QToolButton* playButton_ = nullptr;
    // Actions disabled while the game runs, so they can be given back.
    QList<QAction*> lockedActions_;
    // Operações > Colisão: add a named collider of each shape, fit the selected one (DR-27).
    QList<QAction*> colliderActions_;

    QList<QPair<QAction*, QString>> iconActions_;
    QList<QPair<QToolButton*, QString>> iconButtons_;
    QString themeId_;
    int textStep_ = 0;
    int densityIndex_ = 1;
    bool reducedMotion_ = false;
    QString apiProjectRoot_;
    bool quitting_ = false;
};
