#include "main_window.h"

#include <cstdio>

#include <QAction>
#include <QDateTime>
#include <QSignalBlocker>
#include <QCursor>
#include <QActionGroup>
#include <QRandomGenerator>
#include <QDoubleSpinBox>
#include <QComboBox>
#include <QCheckBox>
#include <QApplication>
#include <QCloseEvent>
#include <QDockWidget>
#include <QColorDialog>
#include <QDialog>
#include <QEvent>
#include <QKeyEvent>
#include <QFormLayout>
#include <QFrame>
#include <QSlider>
#include <QSet>
#include <QStyle>
#include <QHBoxLayout>
#include <QFileDialog>
#include <QInputDialog>
#include <QJsonObject>
#include <QLabel>
#include <QMenu>
#include <QMenuBar>
#include <QMessageBox>
#include <QPushButton>
#include <QSettings>
#include <QShortcut>
#include <QStackedWidget>
#include <QStatusBar>
#include <QTabWidget>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QVBoxLayout>

#include "appearance.h"
#include "command_palette.h"
#include "code_panel.h"
#include "graph_panel.h"
#include "engine_bridge.h"
#include "hierarchy_panel.h"
#include "info_panels.h"
#include "inspector_panel.h"
#include "petunia_icons.h"
#include "panel_title_bar.h"
#include "petunia_theme.h"
#include "shell/petunia_pie_menu.h"
#include "shell/petunia_segmented.h"
#include "viewport_header.h"
#include "viewport_widget.h"
#include "wheel_guard.h"

// Two taps on G, R or S within this time enter free mode (Petunia3D default: 350 ms).
static constexpr qint64 kDoubleTapMilliseconds = 350;

namespace {

// A spin box steps by its singleStep. The stamp angle uses 15° so a quarter turn is six presses, and
// 1° while Ctrl is held so a bank can be aimed without typing a number (DR-34).
class FineStepFilter : public QObject {
public:
    FineStepFilter(QDoubleSpinBox* box, double fine_step) : QObject(box), box_(box), fine_step_(fine_step) {}

    bool eventFilter(QObject* watched, QEvent* event) override {
        if (watched == box_ && event->type() == QEvent::KeyPress) {
            auto* key = static_cast<QKeyEvent*>(event);
            const bool vertical = key->key() == Qt::Key_Up || key->key() == Qt::Key_Down;
            if (vertical && key->modifiers().testFlag(Qt::ControlModifier)) {
                box_->setValue(box_->value() + (key->key() == Qt::Key_Up ? fine_step_ : -fine_step_));
                box_->selectAll();
                return true;
            }
        }
        return QObject::eventFilter(watched, event);
    }

private:
    QDoubleSpinBox* box_;
    double fine_step_;
};

} // namespace

MainWindow::MainWindow() : bridge_(std::make_unique<EngineBridge>()) {
    setWindowTitle(QStringLiteral("Petunia Editor[*]"));
    resize(1360, 820);

    viewport_ = new ViewportWidget(bridge_.get(), this);
    header_ = new ViewportHeader(this);
    auto* scenePage = new QWidget(this);
    sceneLayout_ = new QVBoxLayout(scenePage);
    sceneLayout_->setContentsMargins(0, 0, 0, 0);
    sceneLayout_->setSpacing(0);
    sceneLayout_->addWidget(header_);
    sceneLayout_->addWidget(viewport_, 1);
    code_ = new CodePanel(bridge_.get(), this);
    pages_ = new QStackedWidget(this);
    pages_->addWidget(scenePage);
    pages_->addWidget(code_);
    graph_ = new GraphPanel(bridge_.get(), this);
    pages_->addWidget(graph_);
    setCentralWidget(pages_);
    // A thin margin of the window color around the panels: they read as separate cards.
    setContentsMargins(PetuniaTheme::instance().scaled(5), 0, PetuniaTheme::instance().scaled(5), 0);

    buildActions();
    buildMenus();
    buildAppBar();
    buildToolPalette();
    buildSculptBar();
    buildNaturalBar();
    buildDocks();

    cameraLabel_ = new QLabel(this);
    countLabel_ = new QLabel(this);
    statusBar()->addPermanentWidget(countLabel_);
    statusBar()->addPermanentWidget(cameraLabel_);

    connect(bridge_.get(), &EngineBridge::snapshotChanged, this, &MainWindow::onSnapshot);
    connect(viewport_, &ViewportWidget::selectionChanged, this, [this] { bridge_->refresh(); });
    connect(viewport_, &ViewportWidget::cameraChanged, this, &MainWindow::updateCameraLabel);
    connect(viewport_, &ViewportWidget::playStopRequested, this, &MainWindow::stopPlay);
    // The canvas highlights what ran and answers "por que esta condição falhou?" while playing.
    // A UI timer rather than the viewport's frame loop: 200 ms is enough to read and cannot
    // compete with drawing.
    debugTimer_ = new QTimer(this);
    debugTimer_->setInterval(200);
    connect(debugTimer_, &QTimer::timeout, this, [this] {
        const Reply reply = bridge_->call(QStringLiteral("play.debug"));
        if (reply.ok) {
            graph_->showDebug(reply.result.toObject());
        }
    });
    connect(viewport_, &ViewportWidget::playOutput, this, [this](const QJsonArray& log, const QJsonArray& faults) {
        for (const QJsonValue& value : log) {
            const QJsonObject line = value.toObject();
            code_->appendConsole(QStringLiteral("[%1] %2").arg(line.value("level").toString(), line.value("text").toString()));
        }
        for (const QJsonValue& value : faults) {
            const QJsonObject fault = value.toObject();
            const QString text = QStringLiteral("%1.%2: %3").arg(fault.value("script").toString(), fault.value("function").toString(), fault.value("message").toString());
            code_->appendConsole(QStringLiteral("[erro] %1").arg(text));
            statusBar()->showMessage(QStringLiteral("Erro no script: %1").arg(text), 8000);
        }
    });
    connect(viewport_, &ViewportWidget::gizmoPreviewed, this, [this] {
        inspector_->reload();
        updateCameraLabel();
    });
    connect(header_, &ViewportHeader::snapRequested, this, [this](const QJsonObject& fields) {
        perform(QStringLiteral("snap.set"), fields, false);
    });
    connect(header_, &ViewportHeader::transformRequested, this, [this](const QJsonObject& fields) {
        perform(QStringLiteral("transform.set"), fields, false);
    });
    connect(header_, &ViewportHeader::floorSelectRequested, this, [this](double entity) {
        perform(QStringLiteral("floor.select"), QJsonObject{{"entity", entity}}, false);
    });
    connect(header_, &ViewportHeader::floorStepRequested, this, [this](int delta) {
        perform(QStringLiteral("floor.step"), QJsonObject{{"delta", delta}}, false);
    });
    connect(header_, &ViewportHeader::viewRequested, this, [this](const QString& name) {
        if (perform(QStringLiteral("camera.view"), QJsonObject{{"name", name}}, false).ok) {
            viewport_->update();
            updateCameraLabel();
        }
    });
    connect(header_, &ViewportHeader::editModeRequested, this, [this](bool on) {
        if (viewport_->isEditMode() != on) {
            toggleEditMode();
        }
    });
    connect(header_, &ViewportHeader::componentModeRequested, this, &MainWindow::setComponentMode);
    connect(viewport_, &ViewportWidget::editModeChanged, header_, &ViewportHeader::showEditMode);
    connect(viewport_, &ViewportWidget::componentModeChanged, header_, &ViewportHeader::showComponentMode);
    connect(viewport_, &ViewportWidget::editModeToggleRequested, this, &MainWindow::toggleEditMode);
    connect(viewport_, &ViewportWidget::renderFailed, this, [this](const QString& message) {
        statusBar()->showMessage(message);
    });
    connect(hierarchy_, &HierarchyPanel::selectionRequested, this, [this](quint64 entity) {
        selectEntity(entity);
    });
    connect(inspector_, &InspectorPanel::edited, this, [this] { bridge_->refresh(); });
    connect(inspector_, &InspectorPanel::previewed, viewport_, qOverload<>(&QWidget::update));
    connect(inspector_, &InspectorPanel::editFailed, this, [this](const QString& text) {
        statusBar()->showMessage(text.section('\n', 0, 0), 8000);
    });

    QSettings settings;
    themeId_ = settings.value("appearance/theme", QStringLiteral("petunia-light")).toString();
    textStep_ = qBound(Appearance::kMinTextStep, settings.value("appearance/textStep", 0).toInt(),
                       Appearance::kMaxTextStep);
    // Compact by default: smaller fields and icons (Petunia3D density "Compacta").
    densityIndex_ = qBound(0, settings.value("appearance/density", 0).toInt(), 2);
    reducedMotion_ = settings.value("appearance/reducedMotion", false).toBool();
    reducedMotionAction_->setChecked(reducedMotion_);
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, [this] {
        PetuniaTheme::instance().applyToApplication();
        refreshIcons();
    });
    applyAppearance();
    restoreGeometry(settings.value("window/geometry").toByteArray());
    workspaceStates_.insert(QStringLiteral("scene"), settings.value("window/workspace/scene").toByteArray());
    workspaceStates_.insert(QStringLiteral("code"), settings.value("window/workspace/code").toByteArray());
    workspaceStates_.insert(QStringLiteral("logic"), settings.value("window/workspace/logic").toByteArray());
    if (workspaceStates_.value(QStringLiteral("scene")).isEmpty() || !restoreState(workspaceStates_.value(QStringLiteral("scene")))) {
        applyDefaultDocks(QStringLiteral("scene"));
    }
    setFullMenus(settings.value("appearance/fullMenus", false).toBool());

    bridge_->refresh();
    applyOutlineStyle();
    statusBar()->showMessage(
        QStringLiteral("Abra um projeto (Ctrl+O) ou crie o projeto de exemplo (Ctrl+N)."));
}

MainWindow::~MainWindow() {
    closing_ = true;
    if (inspector_ != nullptr) {
        inspector_->setEngineAvailable(false);
    }
    if (viewport_ != nullptr) {
        viewport_->cancelActiveEdit();
        viewport_->setEngineAlive(false);
        // The renderer owns GPU objects: free them while the GL context still exists.
        if (viewport_->isValid()) {
            viewport_->makeCurrent();
            bridge_->releaseViewport();
            viewport_->doneCurrent();
        }
    }
}

// ---- construction -------------------------------------------------------------------------

void MainWindow::buildActions() {
    auto make = [this](const QString& text, const QString& tip) {
        auto* action = new QAction(text, this);
        action->setStatusTip(tip);
        action->setToolTip(tip);
        return action;
    };
    newAction_ = make(QStringLiteral("Novo projeto de exemplo…"), QStringLiteral("Cria uma pasta com a sala de exemplo e abre"));
    newAction_->setShortcut(QKeySequence::New);
    openAction_ = make(QStringLiteral("Abrir projeto…"), QStringLiteral("Abre uma pasta de projeto Petunia"));
    openAction_->setShortcut(QKeySequence::Open);
    saveAction_ = make(QStringLiteral("Salvar"), QStringLiteral("Grava a cena no projeto"));
    saveAction_->setShortcut(QKeySequence::Save);
    quitAction_ = make(QStringLiteral("Sair"), QStringLiteral("Fecha o editor"));
    quitAction_->setShortcut(QKeySequence::Quit);

    undoAction_ = make(QStringLiteral("Desfazer"), QStringLiteral("Desfaz o último comando"));
    undoAction_->setShortcut(QKeySequence::Undo);
    redoAction_ = make(QStringLiteral("Refazer"), QStringLiteral("Refaz o comando desfeito"));
    redoAction_->setShortcut(QKeySequence::Redo);
    duplicateAction_ = make(QStringLiteral("Duplicar"), QStringLiteral("Copia o objeto selecionado"));
    duplicateAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::Key_D));
    deleteAction_ = make(QStringLiteral("Excluir"), QStringLiteral("Remove o objeto selecionado (dá para desfazer)"));
    deleteAction_->setShortcut(QKeySequence::Delete);
    clearSelectionAction_ = make(QStringLiteral("Limpar seleção"), QStringLiteral("Deixa de selecionar o objeto"));
    clearSelectionAction_->setShortcut(QKeySequence(Qt::Key_Escape));

    editModeAction_ = make(QStringLiteral("Modo de edição"), QStringLiteral("Alterna entre Modo Objeto e Modo Edição de Malha (Tab)"));
    editModeAction_->setCheckable(true);
    editModeAction_->setShortcut(QKeySequence(Qt::Key_Tab));
    vertexModeAction_ = make(QStringLiteral("Vértices"), QStringLiteral("Selecionar vértices da malha (1)"));
    vertexModeAction_->setCheckable(true);
    vertexModeAction_->setShortcut(QKeySequence(Qt::Key_1));
    edgeModeAction_ = make(QStringLiteral("Arestas"), QStringLiteral("Selecionar arestas da malha (2)"));
    edgeModeAction_->setCheckable(true);
    edgeModeAction_->setShortcut(QKeySequence(Qt::Key_2));
    faceModeAction_ = make(QStringLiteral("Faces"), QStringLiteral("Selecionar faces da malha (3)"));
    faceModeAction_->setCheckable(true);
    faceModeAction_->setShortcut(QKeySequence(Qt::Key_3));

    struct ShapeInfo {
        const char* shape;
        const char* label;
        const char* tip;
        const char* icon;
    };
    const ShapeInfo shapes[6] = {
        {"box", "Caixa", "Cria uma caixa na frente da câmera", "cube"},
        {"ramp", "Rampa", "Cria uma rampa na frente da câmera", "wedge"},
        {"cylinder", "Cilindro", "Cria um cilindro na frente da câmera", "cylinder"},
        {"cone", "Cone", "Cria um cone na frente da câmera", "cone"},
        {"sphere", "Esfera", "Cria uma esfera na frente da câmera", "sphere"},
        {"stairs", "Escada", "Cria uma escada na frente da câmera", "add_primitive"},
    };
    for (int index = 0; index < 6; ++index) {
        shapeActions_[index] = make(QString::fromUtf8(shapes[index].label), QString::fromUtf8(shapes[index].tip));
        shapeActions_[index]->setData(QString::fromUtf8(shapes[index].shape));
        shapeActions_[index]->setProperty("iconName", QString::fromUtf8(shapes[index].icon));
        if (index < 3) {
            shapeActions_[index]->setShortcut(QKeySequence(Qt::CTRL | static_cast<Qt::Key>(Qt::Key_1 + index)));
        }
    }

    struct ToolInfo {
        const char* tool;
        const char* label;
        const char* tip;
        const char* icon;
        Qt::Key key;
        Qt::KeyboardModifier modifier = Qt::NoModifier;
    };
    const ToolInfo tools[11] = {
        {"select", "Selecionar e transformar", "Seleciona com um clique; o gizmo universal move, gira e escala (V)", "select", Qt::Key_V},
        {"move", "Mover", "Setas movem num eixo, quadrados num plano, o centro no plano da tela (G)", "move", Qt::Key_G},
        {"rotate", "Girar", "Anéis giram num eixo, o anel externo em volta da vista, o centro livre (R)", "rotate", Qt::Key_R},
        {"scale", "Escalar", "Quadrados escalam num eixo, os de plano em dois, o centro em todos (S)", "scale", Qt::Key_S},
        {"push_pull", "Empurrar / puxar", "Arraste uma face: o brush cresce ou encolhe só daquele lado (P)", "tool_pushpull", Qt::Key_P},
        {"extrude", "Extrudar", "Pressione numa face e arraste: um brush novo cresce dela (E)", "tool_extrude", Qt::Key_E},
        {"sculpt", "Esculpir caverna", "Arraste sobre uma caverna; o modo está na barra de cima, Ctrl faz o oposto. [ e ] mudam o raio (B)", "paint_brush", Qt::Key_B},
        {"sculpt_path", "Túnel por caminho", "Clique pontos na rocha e confirme com Enter: um túnel suave passa por eles. Backspace tira o último ponto (Shift+B)", "paint_path", Qt::Key_B, Qt::ShiftModifier},
        {"terrain", "Pintar terreno", "Arraste para levantar, baixar, suavizar, nivelar ou erodir o terreno; o modo está na barra de cima (T)", "paint_3d", Qt::Key_T},
        {"river_path", "Desenhar rio", "Clique pontos no chão e confirme com Enter: a água segue a curva. Backspace tira o último ponto (Y)", "paint_path", Qt::Key_Y},
        {"road_path", "Desenhar estrada", "Clique pontos no chão e confirme com Enter: a estrada nasce na curva (U)", "paint_line", Qt::Key_U},
    };
    auto* toolGroup = new QActionGroup(this);
    for (int index = 0; index < 11; ++index) {
        toolActions_[index] = make(QString::fromUtf8(tools[index].label), QString::fromUtf8(tools[index].tip));
        toolActions_[index]->setCheckable(true);
        toolActions_[index]->setActionGroup(toolGroup);
        toolActions_[index]->setShortcut(QKeySequence(tools[index].modifier | tools[index].key));
        toolActions_[index]->setData(QString::fromUtf8(tools[index].tool));
        toolActions_[index]->setProperty("iconName", QString::fromUtf8(tools[index].icon));
    }
    toolActions_[0]->setChecked(true);

    const struct {
        const char* name;
        const char* label;
        const char* icon;
        Qt::Key key;
    } views[8] = {{"front", "Vista de frente", "view_front", Qt::Key_1},
                  {"back", "Vista de trás", "view_back", Qt::Key_2},
                  {"left", "Vista da esquerda", "view_left", Qt::Key_3},
                  {"right", "Vista da direita", "view_right", Qt::Key_4},
                  {"top", "Vista de cima", "view_top", Qt::Key_5},
                  {"bottom", "Vista de baixo", "view_bottom", Qt::Key_6},
                  {"isometric", "Vista isométrica verdadeira (35,264°)", "view_front", Qt::Key_7},
                  {"game_2_1", "Vista isométrica de jogo 2:1 (30°)", "view_front", Qt::Key_8}};
    for (int index = 0; index < 8; ++index) {
        viewActions_[index] = make(QString::fromUtf8(views[index].label), QStringLiteral("Olha para a seleção (ou a cena) nesta direção"));
        viewActions_[index]->setShortcut(QKeySequence(Qt::ALT | views[index].key));
        viewActions_[index]->setData(QString::fromUtf8(views[index].name));
        viewActions_[index]->setProperty("iconName", QString::fromUtf8(views[index].icon));
    }

    focusAction_ = make(QStringLiteral("Focar na seleção"), QStringLiteral("Aproxima a câmera do objeto selecionado"));
    focusAction_->setShortcut(QKeySequence(Qt::Key_F));
    resetCameraAction_ = make(QStringLiteral("Câmera no início"), QStringLiteral("Volta a câmera para o início do jogador"));
    resetCameraAction_->setShortcut(QKeySequence(Qt::Key_Home));
    largerTextAction_ = make(QStringLiteral("Texto maior"), QStringLiteral("Aumenta o tamanho do texto da interface"));
    largerTextAction_->setShortcuts({QKeySequence(Qt::CTRL | Qt::Key_Plus), QKeySequence(Qt::CTRL | Qt::Key_Equal)});
    smallerTextAction_ = make(QStringLiteral("Texto menor"), QStringLiteral("Diminui o tamanho do texto da interface"));
    smallerTextAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::Key_Minus));
    normalTextAction_ = make(QStringLiteral("Texto no tamanho normal"), QStringLiteral("Volta o texto ao tamanho padrão"));
    normalTextAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::Key_0));
    reducedMotionAction_ = make(QStringLiteral("Reduzir animações"), QStringLiteral("Desliga transições da interface"));
    reducedMotionAction_->setCheckable(true);
    primitivesMenuAction_ = make(QStringLiteral("Menu de primitivas…"), QStringLiteral("Menu radial para criar formas (Shift+A)"));
    primitivesMenuAction_->setShortcut(QKeySequence(Qt::SHIFT | Qt::Key_A));
    codeAction_ = make(QStringLiteral("Workspace Código"), QStringLiteral("Os scripts do projeto, com o cursor no editor (F4)"));
    codeAction_->setShortcut(QKeySequence(Qt::Key_F4));
    codeAction_->setProperty("iconName", QStringLiteral("script"));
    logicAction_ = make(QStringLiteral("Workspace Lógica"), QStringLiteral("Os grafos de nós do projeto, em linhas que o teclado e o leitor de tela percorrem (F7)"));
    logicAction_->setShortcut(QKeySequence(Qt::Key_F7));
    logicAction_->setProperty("iconName", QStringLiteral("connect"));
    sceneAction_ = make(QStringLiteral("Workspace Cena"), QStringLiteral("A cena 3D com a hierarquia e o inspetor (F3)"));
    sceneAction_->setShortcut(QKeySequence(Qt::Key_F3));
    sceneAction_->setProperty("iconName", QStringLiteral("cube"));
    paletteAction_ = make(QStringLiteral("Buscar comando…"), QStringLiteral("Acha qualquer comando pelo nome e executa (Ctrl+K)"));
    paletteAction_->setShortcuts({QKeySequence(Qt::CTRL | Qt::Key_K), QKeySequence(Qt::CTRL | Qt::SHIFT | Qt::Key_P)});
    paletteAction_->setShortcutContext(Qt::ApplicationShortcut);
    playAction_ = make(QStringLiteral("Jogar"), QStringLiteral("Roda a cena numa cópia, com scripts, física e o personagem do jogador (F6). Parar descarta a cópia"));
    playAction_->setShortcut(QKeySequence(Qt::Key_F6));
    playAction_->setShortcutContext(Qt::ApplicationShortcut);
    playAction_->setProperty("iconName", QStringLiteral("play"));
    connect(playAction_, &QAction::triggered, this, &MainWindow::togglePlay);
    caveAction_ = make(QStringLiteral("Caverna"), QStringLiteral("Um bloco de rocha com um primeiro túnel; esculpa com a ferramenta Esculpir (B)"));
    caveAction_->setProperty("iconName", QStringLiteral("generate_volume"));
    terrainAction_ = make(QStringLiteral("Terreno"), QStringLiteral("Um mapa de altura plano para pintar com a ferramenta Terreno (T)"));
    terrainAction_->setProperty("iconName", QStringLiteral("paint_3d"));
    importModelAction_ = make(QStringLiteral("Importar modelo 3D…"), QStringLiteral("Traz um modelo .glb ou .gltf como objetos editáveis (uma cópia fica em assets/)"));
    importModelAction_->setProperty("iconName", QStringLiteral("cube"));
    connect(importModelAction_, &QAction::triggered, this, &MainWindow::chooseImportModel);
    pointLightAction_ = make(QStringLiteral("Luz pontual"), QStringLiteral("Uma lâmpada na frente da câmera: ilumina até o alcance, em todas as direções"));
    pointLightAction_->setProperty("iconName", QStringLiteral("view_lit"));
    sunLightAction_ = make(QStringLiteral("Sol (luz direcional)"), QStringLiteral("Raios paralelos, como o sol: só a rotação do objeto importa"));
    sunLightAction_->setProperty("iconName", QStringLiteral("view_lit"));
    {
        // Ready-made objects with their behaviour (Porta, Item coletável, Ponto de nascimento).
        const QList<std::tuple<QString, QString, QString, QString>> prefabs = {
            {QStringLiteral("door"), QStringLiteral("Porta"), QStringLiteral("cube"),
             QStringLiteral("Uma porta que sobe quando um personagem chega perto, espera e desce. O comportamento é o grafo `porta` (aba Lógica)")},
            {QStringLiteral("pickup"), QStringLiteral("Item coletável"), QStringLiteral("favorite"),
             QStringLiteral("Uma esfera que some quando um personagem a toca. O comportamento é o grafo `item` (aba Lógica)")},
            {QStringLiteral("spawn"), QStringLiteral("Ponto de nascimento"), QStringLiteral("pin"),
             QStringLiteral("Onde o jogador nasce ao jogar (PlayerStart), com uma marca que não bloqueia")},
        };
        for (const auto& [id, title, icon, tip] : prefabs) {
            QAction* action = make(title, tip);
            action->setProperty("iconName", icon);
            connect(action, &QAction::triggered, this, [this, id, title] {
                const Reply reply = perform(QStringLiteral("prefab.spawn"), QJsonObject{{"id", id}});
                if (reply.ok) {
                    statusBar()->showMessage(QStringLiteral("%1 criado na frente da câmera. Ctrl+Z desfaz.").arg(title), 8000);
                }
            });
            prefabActions_.append(action);
        }
    }
    connect(pointLightAction_, &QAction::triggered, this, [this] {
        perform(QStringLiteral("create.light"), QJsonObject{{"kind", QStringLiteral("point")}});
        statusBar()->showMessage(QStringLiteral("Luz criada. Com uma luz na cena, só as luzes da cena valem (o sol e o ambiente padrão saem)."), 8000);
    });
    connect(sunLightAction_, &QAction::triggered, this, [this] {
        perform(QStringLiteral("create.light"), QJsonObject{{"kind", QStringLiteral("sun")}});
        statusBar()->showMessage(QStringLiteral("Sol criado: gire o objeto para mudar de onde vem a luz."), 8000);
    });
    randomCaveAction_ = make(QStringLiteral("Caverna aleatória"), QStringLiteral("Um bloco de rocha com salas e túneis sinuosos gerados por uma semente; tudo vira traços editáveis"));
    sculptLargerAction_ = make(QStringLiteral("Aumentar o raio de esculpir"), QStringLiteral("Raio do pincel de esculpir maior (])"));
    sculptLargerAction_->setShortcut(QKeySequence(Qt::Key_BracketRight));
    sculptSmallerAction_ = make(QStringLiteral("Diminuir o raio de esculpir"), QStringLiteral("Raio do pincel de esculpir menor ([)"));
    sculptSmallerAction_->setShortcut(QKeySequence(Qt::Key_BracketLeft));
    carveAction_ = make(QStringLiteral("Cavar com a caixa selecionada"), QStringLiteral("Subtrai a caixa selecionada de toda caixa que ela toca: portas, janelas, túneis (Ctrl+Alt+C)"));
    carveAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::ALT | Qt::Key_C));
    hollowAction_ = make(QStringLiteral("Tornar oco"), QStringLiteral("A caixa vira piso, teto e quatro paredes com a espessura do snap: salas, cavernas (Ctrl+Alt+H)"));
    hollowAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::ALT | Qt::Key_H));
    floorUpAction_ = make(QStringLiteral("Subir um andar"), QStringLiteral("A grade, os objetos novos e o snap de superfície sobem um andar; acima do último, cria um andar novo"));
    floorUpAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::Key_PageUp));
    floorDownAction_ = make(QStringLiteral("Descer um andar"), QStringLiteral("A grade e os objetos novos descem um andar"));
    floorDownAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::Key_PageDown));
    editPivotAction_ = make(QStringLiteral("Editar pivô"), QStringLiteral("Arraste o gizmo para mudar onde o objeto gira e escala"));
    editPivotAction_->setCheckable(true);
    editPivotAction_->setShortcut(QKeySequence(Qt::ALT | Qt::Key_P));
    shortcutsAction_ = make(QStringLiteral("Atalhos do teclado"), QStringLiteral("Mostra os atalhos"));
    shortcutsAction_->setShortcut(QKeySequence::HelpContents);
    aboutAction_ = make(QStringLiteral("Sobre o Petunia Editor"), QStringLiteral("Versão e estado do projeto"));
    preferencesAction_ = make(QStringLiteral("Preferências…"), QStringLiteral("Tema, densidade e o contorno da seleção (Ctrl+,)"));
    preferencesAction_->setShortcut(QKeySequence::Preferences);
    if (preferencesAction_->shortcut().isEmpty()) {
        preferencesAction_->setShortcut(QKeySequence(Qt::CTRL | Qt::Key_Comma));
    }
    preferencesAction_->setProperty("iconName", QStringLiteral("settings"));

    connect(newAction_, &QAction::triggered, this, &MainWindow::chooseNewSample);
    connect(openAction_, &QAction::triggered, this, &MainWindow::chooseOpen);
    connect(saveAction_, &QAction::triggered, this, [this] {
        const Reply reply = perform(QStringLiteral("project.save"), {});
        if (reply.ok) {
            statusBar()->showMessage(QStringLiteral("Projeto salvo."), 4000);
        }
    });
    connect(quitAction_, &QAction::triggered, this, &QWidget::close);
    connect(undoAction_, &QAction::triggered, this, [this] { perform(QStringLiteral("edit.undo"), {}); });
    connect(redoAction_, &QAction::triggered, this, [this] { perform(QStringLiteral("edit.redo"), {}); });
    connect(duplicateAction_, &QAction::triggered, this, [this] { perform(QStringLiteral("edit.duplicate"), {}); });
    connect(deleteAction_, &QAction::triggered, this, [this] { perform(QStringLiteral("edit.delete"), {}); });
    connect(clearSelectionAction_, &QAction::triggered, this, [this] {
        if (viewport_->isEditMode()) {
            viewport_->setEditMode(false);
        }
        perform(QStringLiteral("selection.set"), {});
    });
    connect(editModeAction_, &QAction::triggered, this, &MainWindow::toggleEditMode);
    connect(vertexModeAction_, &QAction::triggered, this, [this] { setComponentMode(ComponentMode::Vertex); });
    connect(edgeModeAction_, &QAction::triggered, this, [this] { setComponentMode(ComponentMode::Edge); });
    connect(faceModeAction_, &QAction::triggered, this, [this] { setComponentMode(ComponentMode::Face); });
    for (QAction* action : shapeActions_) {
        connect(action, &QAction::triggered, this, [this, action] {
            perform(QStringLiteral("create.brush"), QJsonObject{{"shape", action->data().toString()}});
        });
    }
    for (QAction* action : toolActions_) {
        connect(action, &QAction::triggered, this, [this, action] {
            // Petunia3D: one tap = the tool with its gizmo; two quick taps (G, R, S) = free mode.
            const qint64 now = QDateTime::currentMSecsSinceEpoch();
            const QString tool = action->data().toString();
            const bool freeTool = tool == QLatin1String("move") || tool == QLatin1String("rotate") || tool == QLatin1String("scale");
            const bool doubleTap = freeTool && action == lastToolAction_ && now - lastToolMilliseconds_ <= kDoubleTapMilliseconds;
            lastToolAction_ = action;
            lastToolMilliseconds_ = now;
            if (doubleTap && bridge_->hasSelection()) {
                lastToolAction_ = nullptr;
                viewport_->startFreeMode(tool);
                return;
            }
            perform(QStringLiteral("tool.set"), QJsonObject{{"tool", tool}}, false);
            viewport_->update();
        });
    }
    for (QAction* action : viewActions_) {
        connect(action, &QAction::triggered, this, [this, action] {
            if (perform(QStringLiteral("camera.view"), QJsonObject{{"name", action->data().toString()}}, false).ok) {
                viewport_->update();
                updateCameraLabel();
            }
        });
    }
    connect(focusAction_, &QAction::triggered, this, [this] {
        if (perform(QStringLiteral("camera.focus"), {}).ok) {
            viewport_->update();
            updateCameraLabel();
        }
    });
    connect(resetCameraAction_, &QAction::triggered, this, [this] {
        if (perform(QStringLiteral("camera.reset"), {}).ok) {
            viewport_->update();
            updateCameraLabel();
        }
    });
    connect(largerTextAction_, &QAction::triggered, this, [this] {
        textStep_ = qMin(textStep_ + 1, Appearance::kMaxTextStep);
        applyAppearance();
    });
    connect(smallerTextAction_, &QAction::triggered, this, [this] {
        textStep_ = qMax(textStep_ - 1, Appearance::kMinTextStep);
        applyAppearance();
    });
    connect(normalTextAction_, &QAction::triggered, this, [this] {
        textStep_ = 0;
        applyAppearance();
    });
    connect(reducedMotionAction_, &QAction::toggled, this, [this](bool on) {
        reducedMotion_ = on;
        applyAppearance();
    });
    connect(shortcutsAction_, &QAction::triggered, this, &MainWindow::showShortcuts);
    connect(primitivesMenuAction_, &QAction::triggered, this, &MainWindow::showPrimitivesPie);
    connect(randomCaveAction_, &QAction::triggered, this, [this] {
        const quint32 seed = QRandomGenerator::global()->generate();
        if (perform(QStringLiteral("create.cave"), QJsonObject{{"seed", static_cast<double>(seed)}}).ok) {
            statusBar()->showMessage(QStringLiteral("Caverna aleatória criada (semente %1). Esculpa com B ou gere mais túneis na barra de cima.").arg(seed), 10000);
        }
    });
    connect(codeAction_, &QAction::triggered, this, [this] {
        switchWorkspace(QStringLiteral("code"));
        code_->focusEditor();
    });
    connect(logicAction_, &QAction::triggered, this, [this] {
        switchWorkspace(QStringLiteral("logic"));
        graph_->reload();
    });
    connect(graph_, &GraphPanel::message, this, [this](const QString& text) { statusBar()->showMessage(text, 8000); });
    connect(sceneAction_, &QAction::triggered, this, [this] {
        switchWorkspace(QStringLiteral("scene"));
        viewport_->setFocus();
    });
    connect(paletteAction_, &QAction::triggered, this, [this] {
        if (palette_ != nullptr) {
            palette_->open();
        }
    });
    connect(terrainAction_, &QAction::triggered, this, [this] {
        const Reply reply = perform(QStringLiteral("create.terrain"), {}, true);
        if (reply.ok) {
            statusBar()->showMessage(QStringLiteral("Terreno criado. Pinte com T; o modo está na barra de cima."), 8000);
        }
    });
    connect(caveAction_, &QAction::triggered, this, [this] {
        perform(QStringLiteral("create.cave"), QJsonObject{});
        statusBar()->showMessage(QStringLiteral("Caverna criada. Escolha Esculpir (B) e arraste sobre ela; Ctrl adiciona rocha."), 8000);
    });
    auto changeRadius = [this](double factor) {
        const Reply reply = perform(QStringLiteral("sculpt.radius"), QJsonObject{{"meters", sculptRadius_ * factor}}, false);
        if (reply.ok) {
            statusBar()->showMessage(QStringLiteral("Raio de esculpir: %1 m")
                                         .arg(QLocale().toString(reply.result.toObject().value(QStringLiteral("radius")).toDouble(), 'f', 2)),
                                     4000);
        }
    };
    connect(sculptLargerAction_, &QAction::triggered, this, [changeRadius] { changeRadius(1.25); });
    connect(sculptSmallerAction_, &QAction::triggered, this, [changeRadius] { changeRadius(0.8); });
    connect(carveAction_, &QAction::triggered, this, [this] {
        if (perform(QStringLiteral("brush.carve"), {}).ok) {
            statusBar()->showMessage(QStringLiteral("Cavado. Ctrl+Z desfaz tudo de uma vez."), 5000);
        }
    });
    connect(hollowAction_, &QAction::triggered, this, [this] {
        if (perform(QStringLiteral("brush.hollow"), {}).ok) {
            statusBar()->showMessage(QStringLiteral("A caixa virou piso, teto e paredes. Ctrl+Z desfaz."), 5000);
        }
    });
    connect(floorUpAction_, &QAction::triggered, this, [this] { perform(QStringLiteral("floor.step"), QJsonObject{{"delta", 1}}, false); });
    connect(floorDownAction_, &QAction::triggered, this, [this] { perform(QStringLiteral("floor.step"), QJsonObject{{"delta", -1}}, false); });
    connect(editPivotAction_, &QAction::toggled, this, [this](bool on) {
        perform(QStringLiteral("transform.set"), QJsonObject{{"edit_pivot", on}}, false);
    });
    connect(aboutAction_, &QAction::triggered, this, &MainWindow::showAbout);
    connect(preferencesAction_, &QAction::triggered, this, &MainWindow::showPreferences);
}

void MainWindow::buildMenus() {
    menus_ = new QMenuBar(this);
    menus_->setAccessibleName(QStringLiteral("Menus"));
    QMenu* file = menus_->addMenu(QStringLiteral("&Arquivo"));
    file->addAction(newAction_);
    file->addAction(openAction_);
    file->addAction(saveAction_);
    file->addSeparator();
    file->addAction(importModelAction_);
    file->addSeparator();
    file->addAction(quitAction_);

    QMenu* edit = menus_->addMenu(QStringLiteral("&Editar"));
    edit->addAction(undoAction_);
    edit->addAction(redoAction_);
    edit->addSeparator();
    edit->addAction(duplicateAction_);
    edit->addAction(deleteAction_);
    edit->addAction(clearSelectionAction_);
    edit->addSeparator();
    edit->addAction(preferencesAction_);
    edit->addSeparator();
    edit->addAction(editModeAction_);
    edit->addAction(vertexModeAction_);
    edit->addAction(edgeModeAction_);
    edit->addAction(faceModeAction_);

    QMenu* create = menus_->addMenu(QStringLiteral("&Criar"));
    create->addAction(primitivesMenuAction_);
    create->addSeparator();
    for (QAction* action : shapeActions_) {
        create->addAction(action);
    }
    create->addSeparator();
    for (QAction* action : std::as_const(prefabActions_)) {
        create->addAction(action);
    }
    create->addSeparator();
    create->addAction(terrainAction_);
    create->addAction(caveAction_);
    create->addAction(randomCaveAction_);
    create->addSeparator();
    create->addAction(importModelAction_);
    create->addAction(pointLightAction_);
    create->addAction(sunLightAction_);

    QMenu* operations = menus_->addMenu(QStringLiteral("&Operações"));
    QMenu* collision = operations->addMenu(QStringLiteral("Colisão"));
    collision->setIcon(PetuniaIcons::get(QStringLiteral("bounding_box")));
    const QList<QPair<QString, QString>> colliderShapes = {
        {QStringLiteral("box"), QStringLiteral("Adicionar colisão: caixa")},
        {QStringLiteral("sphere"), QStringLiteral("Adicionar colisão: esfera")},
        {QStringLiteral("capsule"), QStringLiteral("Adicionar colisão: cápsula")},
        {QStringLiteral("convex"), QStringLiteral("Adicionar colisão: convexa (pela malha)")},
    };
    for (const auto& [shape, label] : colliderShapes) {
        QAction* action = collision->addAction(label);
        action->setToolTip(QStringLiteral("Uma colisão com nome no objeto selecionado, ajustada a ele; um objeto pode ter várias"));
        connect(action, &QAction::triggered, this, [this, shape = shape] {
            perform(QStringLiteral("collider.add"), QJsonObject{{"shape", shape}});
        });
        colliderActions_.append(action);
    }
    QAction* fitCollider = collision->addAction(QStringLiteral("Ajustar a colisão ao objeto"));
    fitCollider->setToolTip(QStringLiteral("A colisão selecionada volta a envolver o objeto"));
    connect(fitCollider, &QAction::triggered, this, [this] { perform(QStringLiteral("collider.fit"), {}); });
    colliderActions_.append(fitCollider);
    operations->addSeparator();
    operations->addAction(toolActions_[4]);
    operations->addAction(toolActions_[5]);
    operations->addAction(toolActions_[6]);
    operations->addAction(toolActions_[7]);
    operations->addAction(toolActions_[8]);
    operations->addAction(toolActions_[9]);
    operations->addAction(toolActions_[10]);
    operations->addAction(sculptLargerAction_);
    operations->addAction(sculptSmallerAction_);
    operations->addSeparator();
    operations->addAction(carveAction_);
    operations->addAction(hollowAction_);
    operations->addSeparator();
    operations->addAction(editPivotAction_);
    operations->addSeparator();
    operations->addAction(floorUpAction_);
    operations->addAction(floorDownAction_);

    QMenu* tools = menus_->addMenu(QStringLiteral("&Ferramentas"));
    for (QAction* action : toolActions_) {
        tools->addAction(action);
    }

    QMenu* view = menus_->addMenu(QStringLiteral("E&xibir"));
    viewMenu_ = view;
    view->addAction(paletteAction_);
    view->addSeparator();
    view->addAction(playAction_);
    view->addAction(sceneAction_);
    view->addAction(codeAction_);
    view->addAction(logicAction_);
    view->addSeparator();
    view->addAction(focusAction_);
    view->addAction(resetCameraAction_);
    QMenu* viewsMenu = view->addMenu(QStringLiteral("Vistas"));
    for (QAction* action : viewActions_) {
        viewsMenu->addAction(action);
    }
    QMenu* projectionMenu = view->addMenu(QStringLiteral("Projeção"));
    auto* projectionGroup = new QActionGroup(projectionMenu);
    const QList<QPair<QString, QString>> projectionModes = {
        {QStringLiteral("perspective"), QStringLiteral("Perspectiva")},
        {QStringLiteral("orthographic"), QStringLiteral("Ortográfica")},
    };
    for (const auto& [mode, label] : projectionModes) {
        QAction* action = projectionMenu->addAction(label);
        action->setCheckable(true);
        action->setData(mode);
        projectionGroup->addAction(action);
        projectionActions_.append(action);
        connect(action, &QAction::triggered, this, [this, mode = mode] {
            if (perform(QStringLiteral("camera.projection"), QJsonObject{{"mode", mode}}, false).ok) {
                viewport_->update();
            }
            updateCameraLabel();
        });
    }
    QMenu* aboveMenu = view->addMenu(QStringLiteral("Objetos acima do andar"));
    auto* aboveGroup = new QActionGroup(this);
    const QList<QPair<QString, QString>> aboveModes = {
        {QStringLiteral("ghost"), QStringLiteral("Fantasma (translúcidos, não selecionáveis)")},
        {QStringLiteral("hide"), QStringLiteral("Ocultar")},
        {QStringLiteral("show"), QStringLiteral("Mostrar")},
    };
    for (const auto& [mode, label] : aboveModes) {
        QAction* action = aboveMenu->addAction(label);
        action->setCheckable(true);
        action->setData(mode);
        aboveGroup->addAction(action);
        aboveActions_.append(action);
        connect(action, &QAction::triggered, this, [this, mode = mode] {
            perform(QStringLiteral("floor.above"), QJsonObject{{"mode", mode}}, false);
        });
    }
    view->addSeparator();
    view->addAction(largerTextAction_);
    view->addAction(smallerTextAction_);
    view->addAction(normalTextAction_);
    view->addSeparator();

    QMenu* themeMenu = view->addMenu(QStringLiteral("Tema"));
    auto* themeGroup = new QActionGroup(this);
    for (const Appearance::ThemeInfo& info : Appearance::themes()) {
        QAction* action = themeMenu->addAction(info.name);
        action->setCheckable(true);
        action->setActionGroup(themeGroup);
        action->setData(info.id);
        action->setChecked(info.id == QSettings().value("appearance/theme", QStringLiteral("petunia-light")).toString());
        connect(action, &QAction::triggered, this, [this, id = info.id] {
            themeId_ = id;
            applyAppearance();
        });
    }
    QMenu* densityMenu = view->addMenu(QStringLiteral("Densidade"));
    auto* densityGroup = new QActionGroup(this);
    const QStringList densityNames = {QStringLiteral("Compacta"), QStringLiteral("Padrão"), QStringLiteral("Confortável")};
    for (int index = 0; index < densityNames.size(); ++index) {
        QAction* action = densityMenu->addAction(densityNames.at(index));
        action->setCheckable(true);
        action->setActionGroup(densityGroup);
        action->setChecked(index == QSettings().value("appearance/density", 0).toInt());
        connect(action, &QAction::triggered, this, [this, index] {
            densityIndex_ = index;
            applyAppearance();
        });
    }
    view->addAction(reducedMotionAction_);
    fullMenusAction_ = view->addAction(QStringLiteral("Menus por extenso na barra"));
    fullMenusAction_->setCheckable(true);
    fullMenusAction_->setToolTip(QStringLiteral("Mostra Arquivo, Editar, Criar… como palavras em vez do botão ≡"));
    connect(fullMenusAction_, &QAction::toggled, this, [this](bool on) {
        setFullMenus(on);
        QSettings().setValue("appearance/fullMenus", on);
    });
    view->addSeparator();
    // Dock toggles are added once the docks exist (buildDocks).

    QMenu* help = menus_->addMenu(QStringLiteral("A&juda"));
    help->addAction(shortcutsAction_);
    help->addAction(aboutAction_);
}

void MainWindow::buildAppBar() {
    auto* bar = new QWidget(this);
    bar->setObjectName(QStringLiteral("PetuniaAppBar"));
    bar->setAttribute(Qt::WA_StyledBackground);
    bar->setAccessibleName(QStringLiteral("Barra principal"));
    auto* layout = new QHBoxLayout(bar);
    layout->setContentsMargins(4, 4, 4, 4);
    layout->setSpacing(2);

    auto button = [this, bar, layout](QAction* action, const QString& iconName) {
        auto* tool = new QToolButton(bar);
        tool->setDefaultAction(action);
        tool->setProperty("role", QStringLiteral("appTool"));
        tool->setToolButtonStyle(Qt::ToolButtonIconOnly);
        tool->setIconSize(QSize(16, 16));
        tool->setAutoRaise(true);
        setActionIcon(action, iconName);
        layout->addWidget(tool);
        return tool;
    };

    // The menus behind one button (JetBrains "new UI"); "Menus por extenso" shows them as words.
    hamburger_ = new QToolButton(bar);
    hamburger_->setProperty("role", QStringLiteral("appTool"));
    hamburger_->setPopupMode(QToolButton::InstantPopup);
    hamburger_->setIconSize(QSize(16, 16));
    hamburger_->setToolTip(QStringLiteral("Menus (F10 ou Alt+\\; Alt+letra abre um menu direto)"));
    hamburger_->setAccessibleName(QStringLiteral("Menus"));
    auto* all = new QMenu(hamburger_);
    for (QAction* action : menus_->actions()) {
        if (QMenu* menu = action->menu()) {
            all->addMenu(menu);
        }
    }
    hamburger_->setMenu(all);
    iconButtons_.append({hamburger_, QStringLiteral("list_view")});
    hamburger_->setIcon(PetuniaIcons::get(QStringLiteral("list_view")));
    layout->addWidget(hamburger_);
    layout->addWidget(menus_);
    menus_->hide();
    for (const QKeySequence& keys : {QKeySequence(Qt::Key_F10), QKeySequence(Qt::ALT | Qt::Key_Backslash)}) {
        auto* open = new QShortcut(keys, this);
        connect(open, &QShortcut::activated, this, [this] {
            if (hamburger_->isVisible()) {
                hamburger_->showMenu();
            } else if (!menus_->actions().isEmpty()) {
                menus_->setActiveAction(menus_->actions().constFirst());
            }
        });
    }
    // Alt+letter opens that menu, as a menu bar would.
    for (QAction* action : menus_->actions()) {
        QMenu* menu = action->menu();
        const QKeySequence mnemonic = QKeySequence::mnemonic(action->text());
        if (menu == nullptr || mnemonic.isEmpty()) {
            continue;
        }
        auto* shortcut = new QShortcut(mnemonic, this);
        connect(shortcut, &QShortcut::activated, this, [this, menu] {
            menu->popup(hamburger_->mapToGlobal(QPoint(0, hamburger_->height() + 2)));
        });
        menuShortcuts_.append(shortcut);
    }

    projectLabel_ = new QLabel(bar);
    projectLabel_->setProperty("role", QStringLiteral("caption"));
    projectLabel_->setAccessibleName(QStringLiteral("Projeto aberto"));
    projectLabel_->setContentsMargins(6, 0, 6, 0);
    layout->addWidget(projectLabel_);
    layout->addStretch(1);

    workspaces_ = new PetuniaSegmented(bar);
    workspaces_->setItems({{QStringLiteral("scene"), QStringLiteral("Cena"), QStringLiteral("Workspace Cena: a cena 3D, a hierarquia e o inspetor (F3)"), QStringLiteral("cube")},
                           {QStringLiteral("code"), QStringLiteral("Código"), QStringLiteral("Workspace Código: os scripts do projeto (F4)"), QStringLiteral("script")},
                           {QStringLiteral("logic"), QStringLiteral("Lógica"), QStringLiteral("Workspace Lógica: os grafos de nós do projeto (F7)"), QStringLiteral("connect")}});
    workspaces_->setGroupLabel(QStringLiteral("Workspaces"));
    workspaces_->setCurrent(QStringLiteral("scene"));
    for (QPushButton* segment : workspaces_->buttons()) {
        segment->setProperty("role", QStringLiteral("workspace"));
    }
    connect(workspaces_, &PetuniaSegmented::activated, this, [this](const QString& id) {
        (id == QLatin1String("code") ? codeAction_ : (id == QLatin1String("logic") ? logicAction_ : sceneAction_))->trigger();
    });
    layout->addWidget(workspaces_);
    layout->addSpacing(8);
    playButton_ = new QToolButton(bar);
    playButton_->setProperty("role", QStringLiteral("play"));
    playButton_->setDefaultAction(playAction_);
    playButton_->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    playButton_->setIconSize(QSize(14, 14));
    playButton_->setAutoRaise(false);
    layout->addWidget(playButton_);
    layout->addStretch(1);

    // The command search, visible so it is not a secret shortcut.
    auto* search = new QPushButton(bar);
    search->setProperty("role", QStringLiteral("search"));
    search->setToolTip(paletteAction_->toolTip());
    search->setAccessibleName(QStringLiteral("Buscar comando"));
    search->setMinimumWidth(PetuniaTheme::instance().scaled(200));
    auto* searchLayout = new QHBoxLayout(search);
    searchLayout->setContentsMargins(8, 0, 4, 0);
    searchLayout->setSpacing(6);
    auto* searchIcon = new QLabel(search);
    searchIcon->setAttribute(Qt::WA_TransparentForMouseEvents);
    auto* searchText = new QLabel(QStringLiteral("Buscar comando"), search);
    searchText->setAttribute(Qt::WA_TransparentForMouseEvents);
    searchText->setProperty("role", QStringLiteral("muted"));
    auto* keycap = new QLabel(QStringLiteral("Ctrl K"), search);
    keycap->setAttribute(Qt::WA_TransparentForMouseEvents);
    keycap->setProperty("role", QStringLiteral("keycap"));
    searchLayout->addWidget(searchIcon);
    searchLayout->addWidget(searchText, 1);
    searchLayout->addWidget(keycap);
    auto paintSearchIcon = [searchIcon] {
        searchIcon->setPixmap(PetuniaIcons::get(QStringLiteral("search"), PetuniaTheme::instance().textMuted().name()).pixmap(14, 14));
    };
    paintSearchIcon();
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, searchIcon, paintSearchIcon);
    connect(search, &QPushButton::clicked, paletteAction_, &QAction::trigger);
    layout->addWidget(search);
    layout->addSpacing(6);

    button(undoAction_, QStringLiteral("undo"));
    button(redoAction_, QStringLiteral("redo"));
    button(saveAction_, QStringLiteral("save"));

    // Icons used by the menus and the palette.
    setActionIcon(duplicateAction_, QStringLiteral("duplicate"));
    setActionIcon(deleteAction_, QStringLiteral("trash"));
    setActionIcon(focusAction_, QStringLiteral("frame_selection"));
    setActionIcon(codeAction_, QStringLiteral("script"));
    setActionIcon(logicAction_, QStringLiteral("connect"));
    setActionIcon(sceneAction_, QStringLiteral("cube"));
    setActionIcon(paletteAction_, QStringLiteral("search"));
    setActionIcon(preferencesAction_, QStringLiteral("settings"));
    playAction_->setIcon(PetuniaIcons::get(QStringLiteral("play"), PetuniaTheme::instance().textActive().name()));
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, [this] {
        playAction_->setIcon(PetuniaIcons::get(viewport_->isPlaying() ? QStringLiteral("stop") : QStringLiteral("play"),
                                               PetuniaTheme::instance().textActive().name()));
    });
    setActionIcon(newAction_, QStringLiteral("new_file"));
    setActionIcon(openAction_, QStringLiteral("open_project"));
    setActionIcon(caveAction_, QStringLiteral("generate_volume"));
    setActionIcon(terrainAction_, QStringLiteral("paint_3d"));
    setActionIcon(importModelAction_, QStringLiteral("cube"));
    for (QAction* action : std::as_const(prefabActions_)) {
        setActionIcon(action, action->property("iconName").toString());
    }
    setActionIcon(pointLightAction_, QStringLiteral("view_lit"));
    setActionIcon(sunLightAction_, QStringLiteral("view_lit"));
    for (QAction* action : viewActions_) {
        setActionIcon(action, action->property("iconName").toString());
    }
    setMenuWidget(bar);

    QList<QMenu*> roots;
    for (QAction* action : menus_->actions()) {
        if (QMenu* menu = action->menu()) {
            roots.append(menu);
        }
    }
    palette_ = new CommandPalette(this, roots);
}

void MainWindow::setFullMenus(bool on) {
    menus_->setVisible(on);
    hamburger_->setVisible(!on);
    // With the words visible, the menu bar answers Alt+letter itself.
    for (QShortcut* shortcut : std::as_const(menuShortcuts_)) {
        shortcut->setEnabled(!on);
    }
    if (fullMenusAction_ != nullptr && fullMenusAction_->isChecked() != on) {
        const QSignalBlocker blocker(fullMenusAction_);
        fullMenusAction_->setChecked(on);
    }
}

void MainWindow::buildToolPalette() {
    auto* palette = new QFrame(viewport_);
    palette->setObjectName(QStringLiteral("PetuniaToolPalette"));
    palette->setAccessibleName(QStringLiteral("Ferramentas"));
    auto* layout = new QVBoxLayout(palette);
    layout->setContentsMargins(3, 3, 3, 3);
    layout->setSpacing(2);
    auto rule = [palette, layout] {
        auto* line = new QFrame(palette);
        line->setProperty("role", QStringLiteral("hrule"));
        line->setFixedHeight(1);
        layout->addWidget(line);
    };
    // Transform · faces · sculpting, then the primitives.
    for (int index = 0; index < 11; ++index) {
        if (index == 4 || index == 6 || index == 8) {
            rule();
        }
        auto* tool = new QToolButton(palette);
        tool->setDefaultAction(toolActions_[index]);
        tool->setProperty("role", QStringLiteral("appTool"));
        tool->setIconSize(QSize(16, 16));
        tool->setFixedSize(28, 28);
        setActionIcon(toolActions_[index], toolActions_[index]->property("iconName").toString());
        layout->addWidget(tool);
    }
    rule();
    auto* primitives = new QToolButton(palette);
    primitives->setProperty("role", QStringLiteral("appTool"));
    primitives->setIconSize(QSize(16, 16));
    primitives->setFixedSize(28, 28);
    primitives->setToolTip(QStringLiteral("Criar primitiva na frente da câmera (Shift+A)"));
    primitives->setAccessibleName(QStringLiteral("Criar primitiva"));
    auto* menu = new QMenu(primitives);
    for (QAction* action : shapeActions_) {
        setActionIcon(action, action->property("iconName").toString());
        menu->addAction(action);
    }
    // The column is narrow: the list opens to its right, not over the tools.
    connect(primitives, &QToolButton::clicked, this, [primitives, menu] {
        menu->popup(primitives->mapToGlobal(QPoint(primitives->width() + 6, 0)));
    });
    iconButtons_.append({primitives, QStringLiteral("add_primitive")});
    primitives->setIcon(PetuniaIcons::get(QStringLiteral("add_primitive")));
    layout->addWidget(primitives);
    palette->adjustSize();
    palette->move(8, 8);
    palette->show();
}

void MainWindow::switchWorkspace(const QString& id) {
    if (id == workspace_) {
        workspaces_->setCurrent(id);
        return;
    }
    workspaceStates_.insert(workspace_, saveState());
    workspace_ = id;
    pages_->setCurrentIndex(id == QLatin1String("code") ? 1 : (id == QLatin1String("logic") ? 2 : 0));
    const QByteArray state = workspaceStates_.value(id);
    if (state.isEmpty() || !restoreState(state)) {
        applyDefaultDocks(id);
    }
    workspaces_->setCurrent(id);
}

// First visit of a workspace: the scene shows the hierarchy, the inspector and the bottom tabs; the
// code shows only the API reference next to the editor.
void MainWindow::applyDefaultDocks(const QString& id) {
    const bool code = id == QLatin1String("code");
    const bool logic = id == QLatin1String("logic");
    for (QDockWidget* dock : std::as_const(docks_)) {
        // The logic workspace has everything it needs on its own page.
        dock->setVisible(logic ? false : (code ? dock == apiDock_ : dock != apiDock_));
    }
}

void MainWindow::buildSculptBar() {
    // The tool's own options sit under the viewport strip, not across the whole window.
    sculptBar_ = new QToolBar(QStringLiteral("Esculpir"), this);
    sculptBar_->setObjectName(QStringLiteral("PetuniaToolOptions"));
    sculptBar_->setMovable(false);
    sculptBar_->setFloatable(false);
    sculptBar_->setAccessibleName(QStringLiteral("Opções de esculpir"));
    sceneLayout_->insertWidget(1, sculptBar_);
    sculptBar_->hide();

    sculptBar_->addWidget(new QLabel(QStringLiteral(" Modo ")));
    sculptMode_ = new QComboBox;
    sculptMode_->setAccessibleName(QStringLiteral("Modo do pincel de esculpir"));
    sculptMode_->setToolTip(QStringLiteral("O que o arrasto faz. Com Ctrl: cavar vira adicionar, inflar vira esvaziar"));
    const QList<QPair<QString, QString>> modes = {
        {QStringLiteral("Cavar"), QStringLiteral("carve")},
        {QStringLiteral("Adicionar rocha"), QStringLiteral("add")},
        {QStringLiteral("Inflar"), QStringLiteral("inflate")},
        {QStringLiteral("Nivelar"), QStringLiteral("flatten")},
        {QStringLiteral("Suavizar"), QStringLiteral("smooth")},
    };
    for (const auto& [label, mode] : modes) {
        sculptMode_->addItem(label, mode);
    }
    WheelGuard::protect(sculptMode_);
    sculptBar_->addWidget(sculptMode_);
    connect(sculptMode_, &QComboBox::activated, this, [this](int index) {
        perform(QStringLiteral("sculpt.mode"), QJsonObject{{"mode", sculptMode_->itemData(index).toString()}}, false);
    });

    sculptBar_->addWidget(new QLabel(QStringLiteral("  Raio ")));
    sculptRadiusBox_ = new QDoubleSpinBox;
    sculptRadiusBox_->setRange(0.25, 16.0);
    sculptRadiusBox_->setSingleStep(0.25);
    sculptRadiusBox_->setSuffix(QStringLiteral(" m"));
    sculptRadiusBox_->setAccessibleName(QStringLiteral("Raio do pincel"));
    sculptRadiusBox_->setToolTip(QStringLiteral("Tamanho do pincel; [ e ] também mudam"));
    sculptBar_->addWidget(sculptRadiusBox_);
    connect(sculptRadiusBox_, &QDoubleSpinBox::editingFinished, this, [this] {
        perform(QStringLiteral("sculpt.radius"), QJsonObject{{"meters", sculptRadiusBox_->value()}}, false);
    });

    sculptBar_->addWidget(new QLabel(QStringLiteral("  Força ")));
    sculptForceBox_ = new QDoubleSpinBox;
    sculptForceBox_->setRange(0.0, 100.0);
    sculptForceBox_->setDecimals(0);
    sculptForceBox_->setSingleStep(5.0);
    sculptForceBox_->setSuffix(QStringLiteral(" %"));
    sculptForceBox_->setAccessibleName(QStringLiteral("Força do pincel"));
    sculptForceBox_->setToolTip(QStringLiteral("Quanto inflar, nivelar ou suavizar a cada passada"));
    sculptBar_->addWidget(sculptForceBox_);
    connect(sculptForceBox_, &QDoubleSpinBox::editingFinished, this, [this] {
        perform(QStringLiteral("sculpt.mode"), QJsonObject{{"force", sculptForceBox_->value() / 100.0}}, false);
    });
    sculptBar_->addSeparator();

    facetedBox_ = new QCheckBox(QStringLiteral("Facetada"));
    facetedBox_->setToolTip(QStringLiteral("Low poly: triângulos chapados em vez de superfície lisa, na caverna selecionada"));
    facetedBox_->setAccessibleName(QStringLiteral("Caverna facetada"));
    sculptBar_->addWidget(facetedBox_);
    connect(facetedBox_, &QCheckBox::clicked, this, [this](bool on) {
        perform(QStringLiteral("sculpt.faceted"), QJsonObject{{"value", on}}, false);
    });

    generateButton_ = new QPushButton(QStringLiteral("Gerar túneis"));
    generateButton_->setToolTip(QStringLiteral("Soma salas e túneis sinuosos à caverna selecionada, a partir de uma semente nova"));
    sculptBar_->addWidget(generateButton_);
    connect(generateButton_, &QPushButton::clicked, this, [this] {
        const quint32 seed = QRandomGenerator::global()->generate();
        const Reply reply = perform(QStringLiteral("sculpt.generate"), QJsonObject{{"seed", static_cast<double>(seed)}}, false);
        if (reply.ok) {
            statusBar()->showMessage(QStringLiteral("%1 traços gerados (semente %2). Ctrl+Z desfaz.")
                                         .arg(reply.result.toObject().value(QStringLiteral("strokes")).toInt())
                                         .arg(seed),
                                     8000);
        }
    });
    consolidateButton_ = new QPushButton(QStringLiteral("Fundir traços"));
    consolidateButton_->setToolTip(QStringLiteral("Junta os traços curtos de arrastos longos em poucos traços longos, sem mudar a forma"));
    sculptBar_->addWidget(consolidateButton_);
    connect(consolidateButton_, &QPushButton::clicked, this, [this] {
        const Reply reply = perform(QStringLiteral("sculpt.consolidate"), {}, false);
        if (reply.ok) {
            const QJsonObject result = reply.result.toObject();
            statusBar()->showMessage(result.value(QStringLiteral("changed")).toBool()
                                         ? QStringLiteral("Traços fundidos: %1 → %2. Ctrl+Z desfaz.")
                                               .arg(result.value(QStringLiteral("before")).toInt())
                                               .arg(result.value(QStringLiteral("after")).toInt())
                                         : QStringLiteral("Nada para fundir: os traços já estão enxutos."),
                                     8000);
        }
    });
    strokesLabel_ = new QLabel;
    strokesLabel_->setAccessibleName(QStringLiteral("Traços da caverna"));
    strokesLabel_->setProperty("role", QStringLiteral("caption"));
    sculptBar_->addWidget(strokesLabel_);

    // When the viewport is narrow, the last options move into Qt's overflow button: give it an icon.
    if (auto* overflow = sculptBar_->findChild<QToolButton*>(QStringLiteral("qt_toolbar_ext_button"))) {
        overflow->setToolTip(QStringLiteral("Mais opções de esculpir"));
        overflow->setAccessibleName(overflow->toolTip());
        iconButtons_.append({overflow, QStringLiteral("more_horiz")});
        overflow->setIcon(PetuniaIcons::get(QStringLiteral("more_horiz")));
    }
}

// Shows the options bar only while a sculpting tool is active, and follows the engine's values.
void MainWindow::syncSculptBar(const QJsonObject& snapshot) {
    if (!sculptBar_) {
        return;
    }
    const QString tool = snapshot.value(QStringLiteral("tool")).toString();
    const bool sculpting = snapshot.value(QStringLiteral("project")).isObject() &&
                           (tool == QLatin1String("sculpt") || tool == QLatin1String("sculpt_path"));
    sculptBar_->setVisible(sculpting);
    const QJsonObject sculpt = snapshot.value(QStringLiteral("sculpt")).toObject();
    const QSignalBlocker a(sculptMode_);
    const QSignalBlocker b(sculptRadiusBox_);
    const QSignalBlocker c(sculptForceBox_);
    const QSignalBlocker d(facetedBox_);
    sculptMode_->setCurrentIndex(qMax(0, sculptMode_->findData(sculpt.value(QStringLiteral("mode")).toString())));
    if (!sculptRadiusBox_->hasFocus()) {
        sculptRadiusBox_->setValue(sculpt.value(QStringLiteral("radius")).toDouble(1.5));
    }
    if (!sculptForceBox_->hasFocus()) {
        sculptForceBox_->setValue(sculpt.value(QStringLiteral("force")).toDouble(0.5) * 100.0);
    }
    const bool hasCave = !sculpt.value(QStringLiteral("cave")).isNull();
    facetedBox_->setEnabled(hasCave);
    facetedBox_->setChecked(sculpt.value(QStringLiteral("faceted")).toBool());
    generateButton_->setEnabled(hasCave);
    consolidateButton_->setEnabled(hasCave);
    strokesLabel_->setText(hasCave ? QStringLiteral("  Traços: %1 / %2")
                                         .arg(sculpt.value(QStringLiteral("strokes")).toInt())
                                         .arg(sculpt.value(QStringLiteral("max_strokes")).toInt())
                                   : QStringLiteral("  Selecione uma caverna"));
    // The path tool has no mode: carving is its only job (Ctrl adds rock).
    sculptMode_->setEnabled(tool == QLatin1String("sculpt"));
    sculptForceBox_->setEnabled(tool == QLatin1String("sculpt"));
}


void MainWindow::buildNaturalBar() {
    naturalBar_ = new QToolBar(QStringLiteral("Terreno e caminhos"), this);
    naturalBar_->setObjectName(QStringLiteral("PetuniaToolOptions"));
    naturalBar_->setMovable(false);
    naturalBar_->setFloatable(false);
    naturalBar_->setAccessibleName(QStringLiteral("Opções de terreno, rio e estrada"));
    sceneLayout_->insertWidget(1, naturalBar_);
    naturalBar_->hide();

    naturalTerrainItems_.append(naturalBar_->addWidget(new QLabel(QStringLiteral("Modo"))));
    naturalMode_ = new QComboBox;
    naturalMode_->setAccessibleName(QStringLiteral("Modo do pincel de terreno"));
    naturalMode_->setToolTip(QStringLiteral("O que o arrasto faz no terreno"));
    naturalMode_->addItem(QStringLiteral("Subir"), QStringLiteral("raise"));
    naturalMode_->addItem(QStringLiteral("Descer"), QStringLiteral("lower"));
    naturalMode_->addItem(QStringLiteral("Suavizar"), QStringLiteral("smooth"));
    naturalMode_->addItem(QStringLiteral("Nivelar"), QStringLiteral("flatten"));
    naturalMode_->addItem(QStringLiteral("Ruído"), QStringLiteral("noise"));
    naturalMode_->addItem(QStringLiteral("Erosão"), QStringLiteral("erode"));
    naturalMode_->addItem(QStringLiteral("Carimbo"), QStringLiteral("stamp"));
    WheelGuard::protect(naturalMode_);
    naturalTerrainItems_.append(naturalBar_->addWidget(naturalMode_));
    connect(naturalMode_, &QComboBox::activated, this, [this](int index) {
        perform(QStringLiteral("terrain.mode"), QJsonObject{{"mode", naturalMode_->itemData(index).toString()}}, false);
    });

    naturalTerrainItems_.append(naturalBar_->addWidget(new QLabel(QStringLiteral("Raio"))));
    naturalRadiusBox_ = new QDoubleSpinBox;
    naturalRadiusBox_->setRange(0.25, 128.0);
    naturalRadiusBox_->setSingleStep(0.5);
    naturalRadiusBox_->setSuffix(QStringLiteral(" m"));
    naturalRadiusBox_->setAccessibleName(QStringLiteral("Raio do pincel de terreno"));
    naturalTerrainItems_.append(naturalBar_->addWidget(naturalRadiusBox_));
    connect(naturalRadiusBox_, &QDoubleSpinBox::editingFinished, this, [this] {
        perform(QStringLiteral("terrain.mode"), QJsonObject{{"radius", naturalRadiusBox_->value()}}, false);
    });

    naturalTerrainItems_.append(naturalBar_->addWidget(new QLabel(QStringLiteral("Força"))));
    naturalStrengthBox_ = new QDoubleSpinBox;
    naturalStrengthBox_->setRange(0.0, 32.0);
    naturalStrengthBox_->setSingleStep(0.25);
    naturalStrengthBox_->setSuffix(QStringLiteral(" m"));
    naturalStrengthBox_->setAccessibleName(QStringLiteral("Força do pincel de terreno"));
    naturalTerrainItems_.append(naturalBar_->addWidget(naturalStrengthBox_));
    connect(naturalStrengthBox_, &QDoubleSpinBox::editingFinished, this, [this] {
        perform(QStringLiteral("terrain.mode"), QJsonObject{{"strength", naturalStrengthBox_->value()}}, false);
    });

    // The shape of a "Carimbo". It only shows while that mode is chosen, so the bar keeps the same
    // width for the everyday brushes.
    naturalStampItems_.append(naturalBar_->addWidget(new QLabel(QStringLiteral("Forma"))));
    naturalTerrainItems_.append(naturalStampItems_.last());
    naturalStampBox_ = new QComboBox;
    naturalStampBox_->setAccessibleName(QStringLiteral("Forma do carimbo de terreno"));
    naturalStampBox_->setToolTip(QStringLiteral("A silhueta que o carimbo grava no terreno"));
    naturalStampBox_->addItem(QStringLiteral("Cratera"), QStringLiteral("crater"));
    naturalStampBox_->addItem(QStringLiteral("Monte"), QStringLiteral("mound"));
    naturalStampBox_->addItem(QStringLiteral("Platô"), QStringLiteral("plateau"));
    naturalStampBox_->addItem(QStringLiteral("Crista"), QStringLiteral("ridge"));
    WheelGuard::protect(naturalStampBox_);
    naturalStampItems_.append(naturalBar_->addWidget(naturalStampBox_));
    naturalTerrainItems_.append(naturalStampItems_.last());
    connect(naturalStampBox_, &QComboBox::activated, this, [this](int index) {
        perform(QStringLiteral("terrain.mode"), QJsonObject{{"stamp", naturalStampBox_->itemData(index).toString()}}, false);
    });

    // The turn belongs to the shape, so it only shows next to it. 15° per step and 1° with Ctrl
    // (`FineStepFilter`); keyboardTracking is off so a typed number is sent once, on Enter.
    naturalStampItems_.append(naturalBar_->addWidget(new QLabel(QStringLiteral("Girar"))));
    naturalTerrainItems_.append(naturalStampItems_.last());
    naturalRotationBox_ = new QDoubleSpinBox;
    naturalRotationBox_->setRange(0.0, 360.0);
    naturalRotationBox_->setSingleStep(15.0);
    naturalRotationBox_->setKeyboardTracking(false);
    naturalRotationBox_->setSuffix(QStringLiteral(" °"));
    naturalRotationBox_->setAccessibleName(QStringLiteral("Giro do carimbo de terreno"));
    naturalRotationBox_->setToolTip(QStringLiteral("Ângulo da forma: 15° por passo, Ctrl para 1°"));
    naturalRotationBox_->installEventFilter(new FineStepFilter(naturalRotationBox_, 1.0));
    naturalStampItems_.append(naturalBar_->addWidget(naturalRotationBox_));
    naturalTerrainItems_.append(naturalStampItems_.last());
    connect(naturalRotationBox_, &QDoubleSpinBox::valueChanged, this, [this](double degrees) {
        perform(QStringLiteral("terrain.mode"), QJsonObject{{"rotation", degrees}}, false);
    });

    // The step brush (Space) says what it did. The mode label comes from this combo, so it is the
    // same word the creator sees in the bar, and the height confirms the result without looking.
    connect(viewport_, &ViewportWidget::terrainStepped, this, [this](const QJsonObject& result) {
        QString mode = QStringLiteral("pincel");
        const int index = naturalMode_->findData(result.value(QStringLiteral("mode")).toString());
        if (index >= 0) {
            mode = naturalMode_->itemText(index);
        }
        const int steps = result.value(QStringLiteral("steps")).toInt();
        QString text = steps > 1 ? QStringLiteral("Passo: %1 ×%2").arg(mode).arg(steps)
                                 : QStringLiteral("Passo: %1").arg(mode);
        if (result.value(QStringLiteral("stamp")).isString()) {
            const int shape = naturalStampBox_->findData(result.value(QStringLiteral("stamp")).toString());
            if (shape >= 0) {
                const QString angle =
                    QLocale().toString(result.value(QStringLiteral("rotation")).toDouble(), 'f', 0);
                text += QStringLiteral(" (%1, %2°)").arg(naturalStampBox_->itemText(shape)).arg(angle);
            }
        }
        text += QStringLiteral(", raio %1 m")
                    .arg(QLocale().toString(result.value(QStringLiteral("radius")).toDouble(), 'f', 2));
        if (result.value(QStringLiteral("height")).isDouble()) {
            text += QStringLiteral(". Altura agora %1 m")
                        .arg(QLocale().toString(result.value(QStringLiteral("height")).toDouble(), 'f', 2));
        }
        text += QStringLiteral(". Ctrl+Z desfaz.");
        statusBar()->showMessage(text, 6000);
    });

    naturalPathItems_.append(naturalBar_->addWidget(new QLabel(QStringLiteral("Largura"))));
    naturalWidthBox_ = new QDoubleSpinBox;
    naturalWidthBox_->setRange(0.1, 256.0);
    naturalWidthBox_->setSingleStep(0.5);
    naturalWidthBox_->setSuffix(QStringLiteral(" m"));
    naturalWidthBox_->setAccessibleName(QStringLiteral("Largura do rio ou da estrada"));
    naturalPathItems_.append(naturalBar_->addWidget(naturalWidthBox_));
    connect(naturalWidthBox_, &QDoubleSpinBox::editingFinished, this, [this] {
        perform(QStringLiteral("natural.width"), QJsonObject{{"width", naturalWidthBox_->value()}}, false);
    });

    naturalRiverItems_.append(naturalBar_->addWidget(new QLabel(QStringLiteral("Profundidade"))));
    naturalDepthBox_ = new QDoubleSpinBox;
    naturalDepthBox_->setRange(0.05, 64.0);
    naturalDepthBox_->setSingleStep(0.25);
    naturalDepthBox_->setSuffix(QStringLiteral(" m"));
    naturalDepthBox_->setAccessibleName(QStringLiteral("Profundidade do rio"));
    naturalRiverItems_.append(naturalBar_->addWidget(naturalDepthBox_));
    connect(naturalDepthBox_, &QDoubleSpinBox::editingFinished, this, [this] {
        perform(QStringLiteral("natural.width"), QJsonObject{{"depth", naturalDepthBox_->value()}}, false);
    });

    naturalRoadItems_.append(naturalBar_->addSeparator());
    naturalGuardrailBox_ = new QCheckBox(QStringLiteral("Defensas"));
    naturalGuardrailBox_->setAccessibleName(QStringLiteral("Defensas da estrada"));
    naturalGuardrailBox_->setToolTip(QStringLiteral("Desenha duas defensas ao longo da estrada selecionada"));
    naturalRoadItems_.append(naturalBar_->addWidget(naturalGuardrailBox_));
    connect(naturalGuardrailBox_, &QCheckBox::toggled, this, [this](bool on) {
        const QJsonValue selection = bridge_->snapshot().value(QStringLiteral("selection"));
        if (!selection.isDouble()) {
            return;
        }
        perform(QStringLiteral("command.execute"),
                QJsonObject{{"command", QStringLiteral("path.set")},
                            {"payload", QJsonObject{{"entity", selection.toInt()}, {"guardrail", on}}}},
                false);
    });

    naturalStructuresButton_ = new QPushButton(QStringLiteral("Pontes e bueiros"));
    naturalStructuresButton_->setAccessibleName(QStringLiteral("Detectar pontes e bueiros"));
    naturalStructuresButton_->setToolTip(QStringLiteral("Compara a estrada selecionada com o terreno e cria as pontes e os bueiros"));
    naturalRoadItems_.append(naturalBar_->addWidget(naturalStructuresButton_));
    connect(naturalStructuresButton_, &QPushButton::clicked, this, [this] {
        const QJsonValue selection = bridge_->snapshot().value(QStringLiteral("selection"));
        if (!selection.isDouble()) {
            return;
        }
        const Reply reply = perform(QStringLiteral("command.execute"),
                                    QJsonObject{{"command", QStringLiteral("road.detect_structures")},
                                                {"payload", QJsonObject{{"road", selection.toInt()}}}},
                                    true);
        if (reply.ok) {
            const QJsonObject result = reply.result.toObject();
            statusBar()->showMessage(QStringLiteral("%1 pontes e %2 bueiros encontrados.")
                                         .arg(result.value(QStringLiteral("bridges")).toInt())
                                         .arg(result.value(QStringLiteral("culverts")).toInt()), 8000);
        }
    });
}

void MainWindow::syncNaturalBar(const QJsonObject& snapshot) {
    if (!naturalBar_) {
        return;
    }
    const QString tool = snapshot.value(QStringLiteral("tool")).toString();
    const bool natural = snapshot.value(QStringLiteral("project")).isObject() &&
                         (tool == QLatin1String("terrain") || tool == QLatin1String("river_path") ||
                          tool == QLatin1String("road_path"));
    naturalBar_->setVisible(natural);
    const QJsonObject state = snapshot.value(QStringLiteral("natural")).toObject();
    const QSignalBlocker a(naturalMode_);
    const QSignalBlocker b(naturalRadiusBox_);
    const QSignalBlocker c(naturalStrengthBox_);
    const QSignalBlocker d(naturalWidthBox_);
    const QSignalBlocker e(naturalDepthBox_);
    const QSignalBlocker g(naturalStampBox_);
    const QSignalBlocker h(naturalRotationBox_);
    naturalMode_->setCurrentIndex(qMax(0, naturalMode_->findData(state.value(QStringLiteral("mode")).toString())));
    naturalStampBox_->setCurrentIndex(
        qMax(0, naturalStampBox_->findData(state.value(QStringLiteral("stamp")).toString())));
    if (!naturalRotationBox_->hasFocus()) {
        naturalRotationBox_->setValue(state.value(QStringLiteral("rotation")).toDouble(0.0));
    }
    if (!naturalRadiusBox_->hasFocus()) {
        naturalRadiusBox_->setValue(state.value(QStringLiteral("radius")).toDouble(4.0));
    }
    if (!naturalStrengthBox_->hasFocus()) {
        naturalStrengthBox_->setValue(state.value(QStringLiteral("strength")).toDouble(0.5));
    }
    if (!naturalWidthBox_->hasFocus()) {
        naturalWidthBox_->setValue(state.value(QStringLiteral("path_width")).toDouble(6.0));
    }
    if (!naturalDepthBox_->hasFocus()) {
        naturalDepthBox_->setValue(state.value(QStringLiteral("path_depth")).toDouble(1.0));
    }
    // Each tool shows only the controls it uses: the whole set at once does not fit in the bar, and
    // the bridge button used to end up out of reach behind the toolbar's overflow corner.
    const bool terrainTool = tool == QLatin1String("terrain");
    const bool riverTool = tool == QLatin1String("river_path");
    const bool roadTool = tool == QLatin1String("road_path");
    const bool pathTool = riverTool || roadTool;
    const bool stampMode = state.value(QStringLiteral("mode")).toString() == QLatin1String("stamp");
    const auto showItems = [](const QList<QAction*>& items, bool visible) {
        for (QAction* item : items) {
            item->setVisible(visible);
        }
    };
    showItems(naturalTerrainItems_, terrainTool);
    // The shape belongs to the stamp mode only; the rest of the bar keeps its place either way.
    showItems(naturalStampItems_, terrainTool && stampMode);
    showItems(naturalPathItems_, pathTool);
    showItems(naturalRiverItems_, riverTool);
    showItems(naturalRoadItems_, roadTool);
    // `selected_road` is the id of the road or null, never a boolean: testing it against a boolean
    // kept the bridge button and the guardrail box disabled for every selection.
    const bool hasRoad = state.value(QStringLiteral("selected_road")).isDouble();
    naturalGuardrailBox_->setEnabled(hasRoad);
    naturalStructuresButton_->setEnabled(hasRoad);
    const QSignalBlocker f(naturalGuardrailBox_);
    naturalGuardrailBox_->setChecked(state.value(QStringLiteral("guardrail")).toBool());
}

void MainWindow::buildDocks() {
    auto addDock = [this](const QString& title, QWidget* widget, Qt::DockWidgetArea area) {
        auto* dock = new QDockWidget(title, this);
        dock->setObjectName(title);
        // A body with rounded lower corners; the title bar has the upper ones: one card.
        auto* body = new QWidget(dock);
        body->setObjectName(QStringLiteral("PetuniaDockBody"));
        body->setAttribute(Qt::WA_StyledBackground);
        auto* bodyLayout = new QVBoxLayout(body);
        bodyLayout->setContentsMargins(0, 0, 0, 4);
        bodyLayout->addWidget(widget);
        dock->setWidget(body);
        dock->setAccessibleName(title);
        if (area == Qt::BottomDockWidgetArea) {
            // The tabs on top already name the bottom panels: no second title.
            dock->setTitleBarWidget(new QWidget(dock));
        } else {
            dock->setTitleBarWidget(new PanelTitleBar(dock));
        }
        addDockWidget(area, dock);
        docks_.append(dock);
        return dock;
    };

    // The side panels run the full height; the bottom tabs sit under the viewport only.
    setCorner(Qt::BottomLeftCorner, Qt::LeftDockWidgetArea);
    setCorner(Qt::BottomRightCorner, Qt::RightDockWidgetArea);
    setTabPosition(Qt::BottomDockWidgetArea, QTabWidget::North);

    hierarchy_ = new HierarchyPanel(bridge_.get(), this);
    inspector_ = new InspectorPanel(bridge_.get(), this);
    diagnostics_ = new DiagnosticsPanel(bridge_.get(), this);
    history_ = new HistoryPanel(bridge_.get(), this);
    api_ = new ApiPanel(bridge_.get(), this);

    QDockWidget* hierarchyDock = addDock(QStringLiteral("Hierarquia"), hierarchy_, Qt::LeftDockWidgetArea);
    QDockWidget* inspectorDock = addDock(QStringLiteral("Inspetor"), inspector_, Qt::RightDockWidgetArea);
    apiDock_ = addDock(QStringLiteral("Referência da API"), api_, Qt::RightDockWidgetArea);
    QDockWidget* diagnosticsDock = addDock(QStringLiteral("Diagnósticos"), diagnostics_, Qt::BottomDockWidgetArea);
    QDockWidget* historyDock = addDock(QStringLiteral("Histórico"), history_, Qt::BottomDockWidgetArea);
    tabifyDockWidget(diagnosticsDock, historyDock);
    diagnosticsDock->raise();

    auto* createMenu = new QMenu(hierarchy_);
    for (QAction* action : shapeActions_) {
        createMenu->addAction(action);
    }
    createMenu->addSeparator();
    for (QAction* action : std::as_const(prefabActions_)) {
        createMenu->addAction(action);
    }
    createMenu->addSeparator();
    createMenu->addAction(terrainAction_);
    createMenu->addAction(caveAction_);
    createMenu->addAction(randomCaveAction_);
    createMenu->addSeparator();
    createMenu->addAction(importModelAction_);
    createMenu->addAction(pointLightAction_);
    createMenu->addAction(sunLightAction_);
    hierarchy_->setCreateMenu(createMenu);

    hierarchyDock->setMinimumWidth(200);
    inspectorDock->setMinimumWidth(280);
    resizeDocks({hierarchyDock, inspectorDock, apiDock_}, {240, 340, 340}, Qt::Horizontal);
    resizeDocks({diagnosticsDock}, {160}, Qt::Vertical);

    for (QDockWidget* dock : std::as_const(docks_)) {
        viewMenu_->addAction(dock->toggleViewAction());
    }
}

// ---- operations ---------------------------------------------------------------------------

Reply MainWindow::perform(const QString& operation, const QJsonObject& fields, bool interactive) {
    if (closing_ || !bridge_) {
        return Reply{};
    }
    const Reply reply = bridge_->call(operation, fields);
    if (!reply.ok) {
        showFailure(reply, interactive);
    }
    bridge_->refresh();
    return reply;
}

void MainWindow::showFailure(const Reply& reply, bool interactive) {
    statusBar()->showMessage(reply.message, 10000);
    if (interactive) {
        QMessageBox::warning(this, QStringLiteral("Não foi possível"), reply.explanation());
    }
}

bool MainWindow::confirmDiscardChanges() {
    if (!isWindowModified()) {
        return true;
    }
    QMessageBox box(QMessageBox::Question, QStringLiteral("Alterações não salvas"),
                    QStringLiteral("A cena tem alterações que ainda não foram salvas."), QMessageBox::NoButton, this);
    QPushButton* save = box.addButton(QStringLiteral("Salvar"), QMessageBox::AcceptRole);
    QPushButton* discard = box.addButton(QStringLiteral("Descartar"), QMessageBox::DestructiveRole);
    box.addButton(QStringLiteral("Cancelar"), QMessageBox::RejectRole);
    box.exec();
    if (box.clickedButton() == save) {
        return bridge_->call(QStringLiteral("project.save")).ok;
    }
    return box.clickedButton() == discard;
}

bool MainWindow::openProjectAt(const QString& path, bool interactive) {
    if (viewport_ != nullptr) {
        viewport_->cancelActiveEdit();
    }
    if (interactive && code_ != nullptr && !code_->maybeSave()) {
        return false;
    }
    if (interactive && !confirmDiscardChanges()) {
        return false;
    }
    const Reply reply = perform(QStringLiteral("project.open"), QJsonObject{{"path", path}}, interactive);
    if (reply.ok) {
        statusBar()->showMessage(QStringLiteral("Projeto aberto."), 4000);
    }
    return reply.ok;
}

bool MainWindow::createSampleAt(const QString& path, const QString& name, bool interactive) {
    if (viewport_ != nullptr) {
        viewport_->cancelActiveEdit();
    }
    if (interactive && code_ != nullptr && !code_->maybeSave()) {
        return false;
    }
    if (interactive && !confirmDiscardChanges()) {
        return false;
    }
    const Reply reply = perform(QStringLiteral("project.new_sample"), QJsonObject{{"path", path}, {"name", name}}, interactive);
    if (reply.ok) {
        statusBar()->showMessage(QStringLiteral("Projeto de exemplo criado em %1.").arg(path), 6000);
    }
    return reply.ok;
}

void MainWindow::selectEntity(quint64 entity) {
    if (viewport_->isEditMode() && bridge_->selection() != entity) {
        viewport_->setEditMode(false);
    }
    perform(QStringLiteral("selection.set"), QJsonObject{{"entity", static_cast<double>(entity)}}, false);
}

void MainWindow::chooseNewSample() {
    const QString parent = QFileDialog::getExistingDirectory(this, QStringLiteral("Em qual pasta criar o projeto?"));
    if (parent.isEmpty()) {
        return;
    }
    bool accepted = false;
    const QString name = QInputDialog::getText(this, QStringLiteral("Novo projeto de exemplo"),
                                               QStringLiteral("Nome do projeto (também é o nome da pasta):"),
                                               QLineEdit::Normal, QStringLiteral("minha_sala"), &accepted)
                             .trimmed();
    if (!accepted || name.isEmpty()) {
        return;
    }
    createSampleAt(parent + QLatin1Char('/') + name, name, true);
}

void MainWindow::chooseImportModel() {
    const QString path = QFileDialog::getOpenFileName(this, QStringLiteral("Importar modelo 3D"), QString(),
                                                      QStringLiteral("Modelos glTF (*.glb *.gltf)"));
    if (path.isEmpty()) {
        return;
    }
    const Reply reply = perform(QStringLiteral("asset.import"), QJsonObject{{"path", path}}, true);
    if (!reply.ok) {
        return;
    }
    const QJsonObject result = reply.result.toObject();
    QString message = QStringLiteral("Modelo importado: %1 objeto(s). Cópia em %2.")
                          .arg(result.value(QStringLiteral("imported")).toInt())
                          .arg(result.value(QStringLiteral("asset")).toString());
    const QJsonArray warnings = result.value(QStringLiteral("warnings")).toArray();
    if (!warnings.isEmpty()) {
        message += QStringLiteral(" Avisos: %1").arg(warnings.first().toString());
    }
    statusBar()->showMessage(message, 10000);
}

void MainWindow::chooseOpen() {
    const QString path = QFileDialog::getExistingDirectory(this, QStringLiteral("Abrir pasta de projeto Petunia"));
    if (!path.isEmpty()) {
        openProjectAt(path, true);
    }
}

// ---- following the engine -----------------------------------------------------------------

void MainWindow::onSnapshot() {
    const QJsonObject snapshot = bridge_->snapshot();
    const bool project = bridge_->hasProject();
    const bool selected = bridge_->hasSelection();
    const QJsonObject info = snapshot.value(QStringLiteral("project")).toObject();

    setWindowTitle(project ? QStringLiteral("%1[*] — Petunia Editor").arg(info.value("name").toString())
                           : QStringLiteral("Petunia Editor"));
    setWindowModified(project && info.value("dirty").toBool());
    projectLabel_->setText(project ? info.value("name").toString() + (info.value("dirty").toBool() ? QStringLiteral("  •") : QString())
                                   : QStringLiteral("Nenhum projeto"));
    projectLabel_->setToolTip(project && info.value("dirty").toBool() ? QStringLiteral("Alterações não salvas (Ctrl+S salva)") : QString());

    saveAction_->setEnabled(project);
    undoAction_->setEnabled(snapshot.value("can_undo").toBool());
    redoAction_->setEnabled(snapshot.value("can_redo").toBool());
    for (QAction* action : shapeActions_) {
        action->setEnabled(project);
    }
    for (QAction* action : {terrainAction_, caveAction_, randomCaveAction_, importModelAction_, pointLightAction_, sunLightAction_, sculptLargerAction_, sculptSmallerAction_}) {
        action->setEnabled(project);
    }
    for (QAction* action : std::as_const(prefabActions_)) {
        action->setEnabled(project);
    }
    sculptRadius_ = snapshot.value("sculpt").toObject().value(QStringLiteral("radius")).toDouble(sculptRadius_);
    syncSculptBar(snapshot);
    syncNaturalBar(snapshot);
    resetCameraAction_->setEnabled(project);
    for (QAction* action : toolActions_) {
        action->setEnabled(project);
        if (action->data().toString() == snapshot.value("tool").toString()) {
            action->setChecked(true);
        }
    }
    for (QAction* action : viewActions_) {
        action->setEnabled(project);
    }
    header_->setControlsEnabled(project);
    header_->showSnap(snapshot.value("snap").toObject());
    header_->showTransform(snapshot.value("transform").toObject());
    primitivesMenuAction_->setEnabled(project);
    header_->showFloor(snapshot.value("floor").toObject());
    const QString above = snapshot.value("floor").toObject().value(QStringLiteral("above")).toString();
    for (QAction* action : aboveActions_) {
        action->setEnabled(project);
        action->setChecked(action->data().toString() == above);
    }
    for (QAction* action : std::as_const(colliderActions_)) {
        action->setEnabled(selected);
    }
    for (QAction* action : {carveAction_, hollowAction_, editPivotAction_}) {
        action->setEnabled(selected);
    }
    for (QAction* action : {floorUpAction_, floorDownAction_}) {
        action->setEnabled(project);
    }
    {
        const QSignalBlocker blocker(editPivotAction_);
        editPivotAction_->setChecked(snapshot.value("transform").toObject().value("edit_pivot").toBool());
    }
    for (QAction* action : {duplicateAction_, deleteAction_, focusAction_, clearSelectionAction_}) {
        action->setEnabled(selected);
    }
    if (!selected && viewport_->isEditMode()) {
        viewport_->setEditMode(false);
    }
    syncEditMode();

    hierarchy_->reload();
    inspector_->reload();
    diagnostics_->reload();
    history_->reload();
    const QString root = info.value("root").toString();
    if (root != apiProjectRoot_) {
        apiProjectRoot_ = root;
        api_->reload();
    }

    const QJsonObject diagnostics = snapshot.value("diagnostics").toObject();
    countLabel_->setText(project ? QStringLiteral("%1 objetos · %2 erros · %3 avisos")
                                       .arg(snapshot.value("object_count").toInt())
                                       .arg(diagnostics.value("errors").toInt())
                                       .arg(diagnostics.value("warnings").toInt())
                                 : QString());
    updateCameraLabel();
    viewport_->update();
}

void MainWindow::updateCameraLabel() {
    const Reply reply = bridge_->call(QStringLiteral("camera.get"));
    if (!reply.ok) {
        return;
    }
    const QJsonObject camera = reply.result.toObject();
    cameraLabel_->setText(QStringLiteral("   câmera %1, %2, %3")
                              .arg(camera.value("x").toDouble(), 0, 'f', 1)
                              .arg(camera.value("y").toDouble(), 0, 'f', 1)
                              .arg(camera.value("z").toDouble(), 0, 'f', 1));
    const QString projectionMode = camera.value(QStringLiteral("projection_mode")).toString();
    if (!projectionMode.isEmpty()) {
        for (QAction* action : std::as_const(projectionActions_)) {
            if (action->data().toString() == projectionMode) {
                action->setChecked(true);
            }
        }
    }
}

// ---- appearance, help, closing ------------------------------------------------------------

void MainWindow::applyAppearance() {
    PetuniaTheme& tokens = PetuniaTheme::instance();
    tokens.setUiScale(Appearance::scaleForStep(textStep_));
    tokens.setDensity(static_cast<PetuniaTheme::Density>(densityIndex_));
    tokens.setReducedMotion(reducedMotion_);
    if (!Appearance::applyTheme(themeId_)) {
        themeId_ = QStringLiteral("petunia-light");
        Appearance::applyTheme(themeId_);
    }
    refreshIcons();
    QSettings settings;
    settings.setValue("appearance/theme", themeId_);
    settings.setValue("appearance/textStep", textStep_);
    settings.setValue("appearance/density", densityIndex_);
    settings.setValue("appearance/reducedMotion", reducedMotion_);
}

void MainWindow::setActionIcon(QAction* action, const QString& iconName) {
    iconActions_.append({action, iconName});
    action->setIcon(PetuniaIcons::get(iconName));
}

void MainWindow::refreshIcons() {
    for (const auto& pair : std::as_const(iconActions_)) {
        pair.first->setIcon(PetuniaIcons::get(pair.second));
    }
    for (const auto& pair : std::as_const(iconButtons_)) {
        pair.first->setIcon(PetuniaIcons::get(pair.second));
    }
}

void MainWindow::togglePlay() {
    if (viewport_->isPlaying()) {
        stopPlay();
    } else {
        startPlay();
    }
}

void MainWindow::startPlay() {
    if (!bridge_->hasProject() || viewport_->isPlaying()) {
        return;
    }
    const Reply reply = perform(QStringLiteral("play.start"), {}, true);
    if (!reply.ok) {
        return;
    }
    const QJsonObject result = reply.result.toObject();
    code_->appendConsole(QStringLiteral("— jogo iniciado —"));
    for (const QJsonValue& problem : result.value("problems").toArray()) {
        code_->appendConsole(QStringLiteral("[aviso] %1").arg(problem.toString()));
    }
    viewport_->setPlaying(true);
    graph_->setPlaying(true);
    debugTimer_->start();
    setEditingLocked(true);
    playAction_->setText(QStringLiteral("Parar"));
    playAction_->setIcon(PetuniaIcons::get(QStringLiteral("stop"), PetuniaTheme::instance().textActive().name()));
    playButton_->setProperty("running", true);
    playButton_->style()->unpolish(playButton_);
    playButton_->style()->polish(playButton_);
    const bool player = result.value("has_player").toBool();
    statusBar()->showMessage(player ? QStringLiteral("Jogando: W A S D andam, Espaço pula, botão direito gira a câmera, Esc para.")
                                    : QStringLiteral("Jogando (sem personagem do jogador: dê a um objeto a tabela Personagem com \"Jogador\" ligado). Esc para."));
}

void MainWindow::stopPlay() {
    if (!viewport_->isPlaying()) {
        return;
    }
    viewport_->setPlaying(false);
    debugTimer_->stop();
    graph_->setPlaying(false);
    perform(QStringLiteral("play.stop"), {}, false);
    setEditingLocked(false);
    code_->appendConsole(QStringLiteral("— jogo parado —"));
    playAction_->setText(QStringLiteral("Jogar"));
    playAction_->setIcon(PetuniaIcons::get(QStringLiteral("play"), PetuniaTheme::instance().textActive().name()));
    playButton_->setProperty("running", false);
    playButton_->style()->unpolish(playButton_);
    playButton_->style()->polish(playButton_);
    statusBar()->showMessage(QStringLiteral("Jogo parado: a cena está como estava."), 5000);
    viewport_->update();
}

// While the game runs the scene is not editable (the engine refuses too); the controls that
// would only produce a refusal are greyed out, and given back exactly as they were.
void MainWindow::setEditingLocked(bool locked) {
    header_->setEnabled(!locked);
    hierarchy_->setEnabled(!locked);
    inspector_->setEnabled(!locked);
    if (auto* palette = viewport_->findChild<QFrame*>(QStringLiteral("PetuniaToolPalette"))) {
        palette->setEnabled(!locked);
    }
    if (locked) {
        const QSet<QAction*> keep = {playAction_, paletteAction_, sceneAction_, codeAction_, logicAction_, preferencesAction_,
                                     shortcutsAction_, aboutAction_, largerTextAction_, smallerTextAction_,
                                     normalTextAction_, reducedMotionAction_, fullMenusAction_};
        lockedActions_.clear();
        for (QAction* action : findChildren<QAction*>()) {
            if (!action->menu() && action->isEnabled() && !keep.contains(action) && !action->text().isEmpty() &&
                !action->property("alwaysOn").toBool() && action->parent() != nullptr &&
                qobject_cast<QDockWidget*>(action->parent()) == nullptr) {
                lockedActions_.append(action);
                action->setEnabled(false);
            }
        }
    } else {
        for (QAction* action : std::as_const(lockedActions_)) {
            action->setEnabled(true);
        }
        lockedActions_.clear();
        bridge_->refresh();
    }
}

void MainWindow::applyOutlineStyle() {
    QSettings settings;
    const QColor color(settings.value("viewport/outlineColor", QStringLiteral("#ff9e1a")).toString());
    const double width = qBound(1.0, settings.value("viewport/outlineWidth", 2.0).toDouble(), 8.0);
    const QColor valid = color.isValid() ? color : QColor(QStringLiteral("#ff9e1a"));
    perform(QStringLiteral("viewport.outline_style"),
            QJsonObject{{"color", QJsonObject{{"r", valid.redF()}, {"g", valid.greenF()}, {"b", valid.blueF()}}},
                        {"width", width}},
            false);
    viewport_->update();
}

void MainWindow::showPreferences() {
    QDialog dialog(this);
    dialog.setWindowTitle(QStringLiteral("Preferências"));
    dialog.setMinimumWidth(PetuniaTheme::instance().scaled(380));
    auto* layout = new QVBoxLayout(&dialog);
    layout->setContentsMargins(16, 14, 16, 14);
    layout->setSpacing(10);
    auto section = [&](const QString& title) {
        auto* label = new QLabel(title, &dialog);
        label->setProperty("role", QStringLiteral("title"));
        layout->addWidget(label);
        auto* form = new QFormLayout;
        form->setHorizontalSpacing(12);
        form->setVerticalSpacing(8);
        layout->addLayout(form);
        return form;
    };

    QFormLayout* appearance = section(QStringLiteral("Aparência"));
    auto* themeBox = new QComboBox(&dialog);
    for (const Appearance::ThemeInfo& info : Appearance::themes()) {
        themeBox->addItem(info.name, info.id);
    }
    themeBox->setCurrentIndex(qMax(0, themeBox->findData(themeId_)));
    appearance->addRow(QStringLiteral("Tema"), themeBox);
    auto* densityBox = new QComboBox(&dialog);
    densityBox->addItems({QStringLiteral("Compacta"), QStringLiteral("Padrão"), QStringLiteral("Confortável")});
    densityBox->setCurrentIndex(densityIndex_);
    densityBox->setToolTip(QStringLiteral("Altura dos controles e tamanho do texto: 24, 28 ou 32 px"));
    appearance->addRow(QStringLiteral("Densidade"), densityBox);
    connect(themeBox, &QComboBox::activated, this, [this, themeBox](int index) {
        themeId_ = themeBox->itemData(index).toString();
        applyAppearance();
    });
    connect(densityBox, &QComboBox::activated, this, [this](int index) {
        densityIndex_ = index;
        applyAppearance();
    });

    QFormLayout* outline = section(QStringLiteral("Contorno da seleção"));
    QSettings settings;
    auto* colorButton = new QPushButton(&dialog);
    colorButton->setAccessibleName(QStringLiteral("Cor do contorno da seleção"));
    auto paint = [colorButton](const QColor& color) {
        const QColor text = color.lightnessF() > 0.55 ? Qt::black : Qt::white;
        colorButton->setText(color.name(QColor::HexRgb).toUpper());
        colorButton->setStyleSheet(QStringLiteral("background-color: %1; color: %2;").arg(color.name(), text.name()));
    };
    paint(QColor(settings.value("viewport/outlineColor", QStringLiteral("#ff9e1a")).toString()));
    outline->addRow(QStringLiteral("Cor"), colorButton);
    auto* widthRow = new QHBoxLayout;
    auto* widthSlider = new QSlider(Qt::Horizontal, &dialog);
    widthSlider->setRange(1, 8);
    widthSlider->setValue(qBound(1, qRound(settings.value("viewport/outlineWidth", 2.0).toDouble()), 8));
    widthSlider->setAccessibleName(QStringLiteral("Espessura do contorno da seleção, em pixels"));
    auto* widthValue = new QLabel(&dialog);
    widthValue->setMinimumWidth(40);
    widthValue->setText(QStringLiteral("%1 px").arg(widthSlider->value()));
    widthRow->addWidget(widthSlider, 1);
    widthRow->addWidget(widthValue);
    outline->addRow(QStringLiteral("Espessura"), widthRow);
    auto* reset = new QPushButton(QStringLiteral("Restaurar padrão"), &dialog);
    reset->setProperty("role", QStringLiteral("quiet"));
    outline->addRow(QString(), reset);

    connect(colorButton, &QPushButton::clicked, this, [this, paint, &dialog] {
        const QColor current(QSettings().value("viewport/outlineColor", QStringLiteral("#ff9e1a")).toString());
        const QColor chosen = QColorDialog::getColor(current, &dialog, QStringLiteral("Cor do contorno da seleção"));
        if (chosen.isValid()) {
            QSettings().setValue("viewport/outlineColor", chosen.name(QColor::HexRgb));
            paint(chosen);
            applyOutlineStyle();
        }
    });
    connect(widthSlider, &QSlider::valueChanged, this, [this, widthValue](int value) {
        widthValue->setText(QStringLiteral("%1 px").arg(value));
        QSettings().setValue("viewport/outlineWidth", value);
        applyOutlineStyle();
    });
    connect(reset, &QPushButton::clicked, this, [this, paint, widthSlider] {
        QSettings().setValue("viewport/outlineColor", QStringLiteral("#ff9e1a"));
        paint(QColor(QStringLiteral("#ff9e1a")));
        widthSlider->setValue(2);
        applyOutlineStyle();
    });

    layout->addStretch(1);
    auto* close = new QPushButton(QStringLiteral("Fechar"), &dialog);
    close->setProperty("role", QStringLiteral("primary"));
    close->setDefault(true);
    connect(close, &QPushButton::clicked, &dialog, &QDialog::accept);
    auto* buttons = new QHBoxLayout;
    buttons->addStretch(1);
    buttons->addWidget(close);
    layout->addLayout(buttons);
    dialog.exec();
}

void MainWindow::showShortcuts() {
    QMessageBox::information(
        this, QStringLiteral("Atalhos do teclado"),
        QStringLiteral(
            "<b>Viewport 3D</b><br>"
            "Setas: mover a câmera · PageUp/PageDown: subir e descer · Shift: mais rápido<br>"
            "Segure o botão direito: girar a câmera e voar com W A S D (Q desce, E sobe)<br>"
            "Roda do mouse: aproximar e afastar · Clique esquerdo: selecionar<br>"
            "Cubo de navegação (canto superior direito): arrastar orbita; face, aresta ou canto mudam a vista; seta/rótulo abrem projeção<br>"
            "Alt+1 a Alt+8: vistas frente, trás, esquerda, direita, topo, baixo, isométrica, isométrica 2:1<br><br>"
            "<b>Ferramentas</b><br>"
            "V selecionar e transformar (gizmo universal) · G mover · R girar · S escalar<br>"
            "Toque duas vezes em G, R ou S: modo livre (o mouse muda o valor; X Y Z restringem; digite um número)<br>"
            "P empurrar/puxar uma face · E extrudar · Ctrl+Alt+C cavar · Ctrl+Alt+H tornar oco · Alt+P editar pivô<br>"
            "Ctrl+PageUp / Ctrl+PageDown: subir e descer um andar · Shift+A menu de primitivas<br>"
            "B esculpir caverna (arraste cava; com Ctrl adiciona rocha) · [ e ] raio do pincel<br>"
            "T terreno · Y rio · U estrada · com o terreno, a barra de espaços aplica um carimbo sem arrastar<br>"
            "No gizmo: seta = eixo, quadrado = plano, centro = tela; Ctrl inverte o snap; Esc cancela<br><br>"
            "<b>Edição</b><br>"
            "Tab modo edição de malha · 1 vértice · 2 aresta · 3 face<br>"
            "Ctrl+Z desfazer · Ctrl+Shift+Z refazer<br>"
            "Ctrl+D duplicar · Delete excluir · Esc limpar seleção<br>"
            "Ctrl+1 caixa · Ctrl+2 rampa · Ctrl+3 cilindro<br>"
            "F focar na seleção · Home câmera no início<br><br>"
            "<b>Projeto</b><br>"
            "Ctrl+N novo exemplo · Ctrl+O abrir · Ctrl+S salvar<br><br>"
            "<b>Aparência</b><br>"
            "Ctrl+= texto maior · Ctrl+- menor · Ctrl+0 normal"));
}

// Shift+A: the radial menu of primitives (Petunia3D's PetuniaPieMenu, copied unchanged). Number
// keys, arrows and Enter work too; "Mostrar como lista" turns it into a plain menu.
void MainWindow::showPrimitivesPie() {
    if (!bridge_->hasProject()) {
        return;
    }
    auto* pie = new PetuniaPieMenu(QStringLiteral("Criar primitiva"), this);
    pie->setAttribute(Qt::WA_DeleteOnClose);
    QVector<PetuniaPieItem> items;
    for (QAction* action : shapeActions_) {
        items.append({action->data().toString(), action->text(), action->property("iconName").toString(),
                      [action] { action->trigger(); }, action->isEnabled()});
    }
    pie->setItems(items);
    connect(pie, &PetuniaPieMenu::closed, pie, &QWidget::close);
    const QPoint cursor = QCursor::pos();
    const QRect viewportArea(viewport_->mapToGlobal(QPoint(0, 0)), viewport_->size());
    pie->popup(viewportArea.contains(cursor) ? cursor : viewportArea.center());
}

void MainWindow::toggleEditMode() {
    if (!bridge_->hasProject()) {
        return;
    }
    if (viewport_->isEditMode()) {
        if (bridge_->hasSelection()) {
            bridge_->call(QStringLiteral("mesh.select"), QJsonObject{
                {QStringLiteral("entity"), static_cast<double>(bridge_->selection())},
                {QStringLiteral("mode"), QStringLiteral("clear")}
            });
        }
        viewport_->setEditMode(false);
        syncEditMode();
        statusBar()->showMessage(QStringLiteral("Modo Objeto"), 2000);
        viewport_->update();
        return;
    }

    if (!bridge_->hasSelection()) {
        statusBar()->showMessage(QStringLiteral("Selecione um objeto para editar malha (Tab)"), 3000);
        return;
    }

    const quint64 entity = bridge_->selection();
    const Reply reply = bridge_->call(QStringLiteral("mesh.convert"), QJsonObject{
        {QStringLiteral("entity"), static_cast<double>(entity)}
    });
    if (reply.ok) {
        bridge_->refresh();
    } else if (reply.code != QLatin1String("EDITOR_ALREADY_A_MESH")) {
        showFailure(reply, true);
        return;
    }

    viewport_->setEditMode(true);
    syncEditMode();
    statusBar()->showMessage(QStringLiteral("Modo Edição: 1 Vértice, 2 Aresta, 3 Face"), 3000);
    viewport_->update();
}

void MainWindow::setComponentMode(ComponentMode mode) {
    if (!viewport_->isEditMode()) {
        return;
    }
    viewport_->setComponentMode(mode);
    syncEditMode();
    const QString name = (mode == ComponentMode::Vertex) ? QStringLiteral("Vértices (1)")
                       : (mode == ComponentMode::Edge)   ? QStringLiteral("Arestas (2)")
                                                         : QStringLiteral("Faces (3)");
    statusBar()->showMessage(QStringLiteral("Seleção de %1").arg(name), 2000);
}

void MainWindow::syncEditMode() {
    const bool inEdit = viewport_->isEditMode();
    header_->showEditMode(inEdit);
    header_->showComponentMode(viewport_->componentMode());
    if (editModeAction_) {
        editModeAction_->setChecked(inEdit);
        editModeAction_->setEnabled(bridge_->hasSelection());
    }
    if (vertexModeAction_) {
        vertexModeAction_->setEnabled(inEdit);
        vertexModeAction_->setChecked(viewport_->componentMode() == ComponentMode::Vertex);
    }
    if (edgeModeAction_) {
        edgeModeAction_->setEnabled(inEdit);
        edgeModeAction_->setChecked(viewport_->componentMode() == ComponentMode::Edge);
    }
    if (faceModeAction_) {
        faceModeAction_->setEnabled(inEdit);
        faceModeAction_->setChecked(viewport_->componentMode() == ComponentMode::Face);
    }
}

void MainWindow::showAbout() {
    QMessageBox::about(
        this, QStringLiteral("Sobre o Petunia Editor"),
        QStringLiteral(
            "<b>Petunia Editor</b> — primeira versão (fase 1)<br><br>"
            "Interface em Qt sobre o motor em Rust. O editor só envia comandos "
            "validados ao motor; tudo o que muda a cena dá para desfazer.<br><br>"
            "Estado do que existe e do que foi verificado: docs/05-roadmap/03-estado-da-implementacao.md"));
}

void MainWindow::closeEvent(QCloseEvent* event) {
    if (viewport_ != nullptr) {
        viewport_->cancelActiveEdit();
    }
    if (!quitting_ && (!code_->maybeSave() || !confirmDiscardChanges())) {
        event->ignore();
        return;
    }
    QSettings settings;
    settings.setValue("window/geometry", saveGeometry());
    workspaceStates_.insert(workspace_, saveState());
    settings.setValue("window/workspace/scene", workspaceStates_.value(QStringLiteral("scene")));
    settings.setValue("window/workspace/code", workspaceStates_.value(QStringLiteral("code")));
    settings.setValue("window/workspace/logic", workspaceStates_.value(QStringLiteral("logic")));
    event->accept();
}

void MainWindow::captureAndQuit(const QString& path, int delayMilliseconds) {
    quitting_ = true;
    QTimer::singleShot(delayMilliseconds, this, [this, path] {
        viewport_->update();
        QApplication::processEvents();
        const bool window = grab().save(path);
        const bool view = viewport_->grabFramebuffer().save(path + QStringLiteral(".viewport.png"));
        fprintf(stderr, "screenshot: janela=%s viewport=%s\n", window ? "ok" : "falhou", view ? "ok" : "falhou");
        QApplication::exit(window && view ? 0 : 3);
    });
}
