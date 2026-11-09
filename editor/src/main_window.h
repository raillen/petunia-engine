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
class GraphPanel;
class ApiPanel;
class AssetPanel;
class QAction;
class QLabel;
class QToolButton;
class QToolBar;
class QComboBox;
class QDoubleSpinBox;
class QSpinBox;
class QCheckBox;
class QPushButton;
class QDockWidget;
class QMenu;
class QMenuBar;
class QShortcut;
class QStackedWidget;
class QTimer;
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
    void chooseAnimation();
    // Mundo: climate and a vegetation rule. The commands already exist; this is the window.
    void setClimate(const QString& preset);
    void chooseHour();
    void chooseFlora();
    // Starts the editor clock that steps clips and falling precipitation.
    void startWorldClock();
    bool createSampleAt(const QString& path, const QString& name, bool interactive,
                        const QString& method = QStringLiteral("aipo"));
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
    void buildMeshActions();
    void buildMeshBar();
    void applyMesh(const QString& operation);
    void toggleKnife();
    void showBooleanDialog();
    void showCutPlaneDialog();
    void showArrangeLineDialog();
    void showArrangeCircleDialog();
    void showMirrorDialog();
    void convertSelectionToMesh();
    void syncMeshPreviewValues();
    void buildNaturalBar();
    void syncNaturalBar(const QJsonObject& snapshot);
    void buildDocks();
    // Several scenes per project, the automatic save and its recovery.
    void meshBoolean();
    void meshKnife();
    // The knife by clicking: two clicks on the mesh, each naming a point on an edge.
    void startKnifeByClick();
    bool handleKnifeClick(const QPoint& position);
    void cancelKnife();
    // A mesh operation shown live while the key is held: press shows it, moving the mouse changes
    // the value, releasing the key (or clicking/Enter) applies it as ONE undo step, Esc cancels.
    void startMeshPreview(const QString& operation);
    void updateMeshPreview();
    void endMeshPreview(bool commit);
    void showMeshInfo();
    // Several scenes per project, the automatic save and its recovery.
    void buildSceneActions();
    void chooseOpenScene();
    void chooseNewScene();
    void autosaveNow();
    void checkRecovery(bool interactive);
    void openRecoveryDialog();

    Reply perform(const QString& operation, const QJsonObject& fields, bool interactive = true);
    void showFailure(const Reply& reply, bool interactive);
    bool eventFilter(QObject* watched, QEvent* event) override;
    void onSnapshot();
    void updateCameraLabel();
    bool confirmDiscardChanges();
    void chooseNewSample();
    void chooseOpen();
    void chooseImportModel();
    void applyAppearance();
    void refreshIcons();
    void setActionIcon(QAction* action, const QString& iconName);
    void showShortcuts();
    // Editar > Preferências (Ctrl+,): appearance and the selection outline.
    void showPreferences();
    // Sends the saved outline color and thickness to the engine.
    void applyOutlineStyle();
    void showPrimitivesPie();
    // Rebuilds Criar > Prefab do projeto from the prefabs saved in the project folder.
    void refreshProjectPrefabs();
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
    AssetPanel* assets_ = nullptr;
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
    QAction* openSceneAction_ = nullptr;
    QAction* newSceneAction_ = nullptr;
    QAction* recoverAction_ = nullptr;
    QTimer* autosaveTimer_ = nullptr;
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
    QAction* toolActions_[11] = {};
    QAction* carveAction_ = nullptr;
    QAction* codeAction_ = nullptr;
    CodePanel* code_ = nullptr;
    QAction* logicAction_ = nullptr;
    GraphPanel* graph_ = nullptr;
    QAction* caveAction_ = nullptr;
    QAction* terrainAction_ = nullptr;
    // Criar > Luz: a bulb and a sun (DR-29).
    QAction* importModelAction_ = nullptr;
    QAction* pointLightAction_ = nullptr;
    QAction* sunLightAction_ = nullptr;
    QList<QAction*> prefabActions_;
    // Criar > Prefab do projeto: saved molds (prefab.save / prefab.spawn), rebuilt when opened.
    QAction* savePrefabAction_ = nullptr;
    QMenu* projectPrefabsMenu_ = nullptr;
    QAction* randomCaveAction_ = nullptr;
    QAction* sculptLargerAction_ = nullptr;
    QAction* sculptSmallerAction_ = nullptr;
    // Brush radius of the sculpting tool, as the engine last reported it.
    double sculptRadius_ = 1.5;
    // Set when the window is going away: a focused field emits editingFinished while it is
    // destroyed, and that must not reach an engine that is already gone.
    bool closing_ = false;
    // Options bar of the mesh edit tools; only visible in edit mode.
    QToolBar* meshBar_ = nullptr;
    // Second row: the live-preview combo and the numbers. One row would be wider than the viewport
    // column at the default window size, and a toolbar in a layout clips instead of scrolling.
    QToolBar* meshValuesBar_ = nullptr;
    QList<QAction*> meshActions_;
    QAction* booleanAction_ = nullptr;
    QAction* convertMeshAction_ = nullptr;
    QComboBox* meshPreviewBox_ = nullptr;
    // Push/pull and extrude belong to edit mode. Hidden while the viewport is in object mode.
    QList<QWidget*> editOnlyTools_;
    QDoubleSpinBox* meshDistance_ = nullptr;
    QSpinBox* meshCuts_ = nullptr;
    QSpinBox* meshSegments_ = nullptr;
    QDoubleSpinBox* meshSlide_ = nullptr;
    QAction* knifeAction_ = nullptr;
    QAction* knifeNumbersAction_ = nullptr;
    bool meshPreviewing_ = false;
    QTimer* meshPreviewCommitTimer_ = nullptr;
    QString meshPreviewOperation_;
    double meshPreviewBase_ = 0.2;
    double meshPreviewValue_ = 0.2;
    QPoint meshPreviewOrigin_;
    bool knifeClick_ = false;
    bool knifeHasStart_ = false;
    QJsonObject knifeStart_;
    QAction* meshInfoAction_ = nullptr;
    // Options bar of the sculpting tools; only visible while one of them is active.
    QToolBar* sculptBar_ = nullptr;
    QToolBar* naturalBar_ = nullptr;
    QComboBox* naturalMode_ = nullptr;
    // Which shape a "Carimbo" stroke stamps and how far it is turned. Only shown while that mode
    // is selected.
    QComboBox* naturalStampBox_ = nullptr;
    QDoubleSpinBox* naturalRotationBox_ = nullptr;
    QDoubleSpinBox* naturalRadiusBox_ = nullptr;
    QDoubleSpinBox* naturalStrengthBox_ = nullptr;
    QDoubleSpinBox* naturalWidthBox_ = nullptr;
    QDoubleSpinBox* naturalDepthBox_ = nullptr;
    QCheckBox* naturalGuardrailBox_ = nullptr;
    QPushButton* naturalStructuresButton_ = nullptr;
    // Items of the options bar, grouped by the tool that uses them. A widget inside a QToolBar
    // follows the visibility of its action, so the actions are what gets toggled.
    QList<QAction*> naturalTerrainItems_;
    QList<QAction*> naturalStampItems_;
    QList<QAction*> naturalPathItems_;
    QList<QAction*> naturalRiverItems_;
    QList<QAction*> naturalRoadItems_;
    QComboBox* sculptMode_ = nullptr;
    QDoubleSpinBox* sculptRadiusBox_ = nullptr;
    QDoubleSpinBox* sculptForceBox_ = nullptr;
    QCheckBox* facetedBox_ = nullptr;
    QLabel* strokesLabel_ = nullptr;
    QPushButton* generateButton_ = nullptr;
    QPushButton* consolidateButton_ = nullptr;
    QAction* hollowAction_ = nullptr;
    QAction* cutPlaneAction_ = nullptr;
    QAction* arrangeLineAction_ = nullptr;
    QAction* arrangeCircleAction_ = nullptr;
    QAction* mirrorAction_ = nullptr;
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
    // While the game runs, asks what the graphs reported (which node ran, why a condition failed).
    QTimer* debugTimer_ = nullptr;
    QTimer* animationTimer_ = nullptr;
    QAction* animationAction_ = nullptr;
    QAction* stopAnimationAction_ = nullptr;
    // Mundo: climate presets, hour and a vegetation rule. Enabled only with a project open.
    QList<QAction*> worldActions_;
    QAction* comfortAction_ = nullptr;
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
