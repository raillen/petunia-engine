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
#include <QFrame>
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
#include <QStatusBar>
#include <QTabWidget>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QVBoxLayout>

#include "appearance.h"
#include "code_panel.h"
#include "engine_bridge.h"
#include "hierarchy_panel.h"
#include "info_panels.h"
#include "inspector_panel.h"
#include "petunia_icons.h"
#include "panel_title_bar.h"
#include "petunia_theme.h"
#include "shell/petunia_pie_menu.h"
#include "viewport_header.h"
#include "viewport_widget.h"
#include "wheel_guard.h"

// Two taps on G, R or S within this time enter free mode (Petunia3D default: 350 ms).
static constexpr qint64 kDoubleTapMilliseconds = 350;

MainWindow::MainWindow() : bridge_(std::make_unique<EngineBridge>()) {
    setWindowTitle(QStringLiteral("Petunia Editor[*]"));
    resize(1360, 820);

    viewport_ = new ViewportWidget(bridge_.get(), this);
    header_ = new ViewportHeader(this);
    auto* center = new QWidget(this);
    auto* centerLayout = new QVBoxLayout(center);
    centerLayout->setContentsMargins(0, 0, 0, 0);
    centerLayout->setSpacing(0);
    centerLayout->addWidget(header_);
    centerLayout->addWidget(viewport_, 1);
    setCentralWidget(center);

    buildActions();
    buildMenus();
    buildAppBar();
    buildSculptBar();
    buildDocks();

    cameraLabel_ = new QLabel(this);
    countLabel_ = new QLabel(this);
    statusBar()->addPermanentWidget(countLabel_);
    statusBar()->addPermanentWidget(cameraLabel_);

    connect(bridge_.get(), &EngineBridge::snapshotChanged, this, &MainWindow::onSnapshot);
    connect(viewport_, &ViewportWidget::selectionChanged, this, [this] { bridge_->refresh(); });
    connect(viewport_, &ViewportWidget::cameraChanged, this, &MainWindow::updateCameraLabel);
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
    restoreState(settings.value("window/state").toByteArray());

    bridge_->refresh();
    statusBar()->showMessage(
        QStringLiteral("Abra um projeto (Ctrl+O) ou crie o projeto de exemplo (Ctrl+N)."));
}

MainWindow::~MainWindow() {
    closing_ = true;
    // The renderer owns GPU objects: free them while the GL context still exists.
    if (viewport_ != nullptr && viewport_->isValid()) {
        viewport_->makeCurrent();
        bridge_->releaseViewport();
        viewport_->doneCurrent();
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
    const ToolInfo tools[8] = {
        {"select", "Selecionar e transformar", "Seleciona com um clique; o gizmo universal move, gira e escala (V)", "select", Qt::Key_V},
        {"move", "Mover", "Setas movem num eixo, quadrados num plano, o centro no plano da tela (G)", "move", Qt::Key_G},
        {"rotate", "Girar", "Anéis giram num eixo, o anel externo em volta da vista, o centro livre (R)", "rotate", Qt::Key_R},
        {"scale", "Escalar", "Quadrados escalam num eixo, os de plano em dois, o centro em todos (S)", "scale", Qt::Key_S},
        {"push_pull", "Empurrar / puxar", "Arraste uma face: o brush cresce ou encolhe só daquele lado (P)", "tool_pushpull", Qt::Key_P},
        {"extrude", "Extrudar", "Pressione numa face e arraste: um brush novo cresce dela (E)", "tool_extrude", Qt::Key_E},
        {"sculpt", "Esculpir caverna", "Arraste sobre uma caverna; o modo está na barra de cima, Ctrl faz o oposto. [ e ] mudam o raio (B)", "paint_brush", Qt::Key_B},
        {"sculpt_path", "Túnel por caminho", "Clique pontos na rocha e confirme com Enter: um túnel suave passa por eles. Backspace tira o último ponto (Shift+B)", "paint_path", Qt::Key_B, Qt::ShiftModifier},
    };
    auto* toolGroup = new QActionGroup(this);
    for (int index = 0; index < 8; ++index) {
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
    } views[6] = {{"front", "Vista de frente", "view_front", Qt::Key_1},
                  {"back", "Vista de trás", "view_back", Qt::Key_2},
                  {"left", "Vista da esquerda", "view_left", Qt::Key_3},
                  {"right", "Vista da direita", "view_right", Qt::Key_4},
                  {"top", "Vista de cima", "view_top", Qt::Key_5},
                  {"bottom", "Vista de baixo", "view_bottom", Qt::Key_6}};
    for (int index = 0; index < 6; ++index) {
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
    codeAction_ = make(QStringLiteral("Editor de código"), QStringLiteral("Abre o editor de scripts e põe o cursor nele (F4)"));
    codeAction_->setShortcut(QKeySequence(Qt::Key_F4));
    caveAction_ = make(QStringLiteral("Caverna"), QStringLiteral("Um bloco de rocha com um primeiro túnel; esculpa com a ferramenta Esculpir (B)"));
    caveAction_->setProperty("iconName", QStringLiteral("generate_volume"));
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
    connect(clearSelectionAction_, &QAction::triggered, this, [this] { perform(QStringLiteral("selection.set"), {}); });
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
        codeDock_->show();
        codeDock_->raise();
        // Room to write: the bottom panels start short, enough for a table, not for code.
        resizeDocks({codeDock_}, {qMax(codeDock_->height(), 380)}, Qt::Vertical);
        code_->focusEditor();
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
}

void MainWindow::buildMenus() {
    menus_ = new QMenuBar(this);
    menus_->setAccessibleName(QStringLiteral("Menus"));
    QMenu* file = menus_->addMenu(QStringLiteral("&Arquivo"));
    file->addAction(newAction_);
    file->addAction(openAction_);
    file->addAction(saveAction_);
    file->addSeparator();
    file->addAction(quitAction_);

    QMenu* edit = menus_->addMenu(QStringLiteral("&Editar"));
    edit->addAction(undoAction_);
    edit->addAction(redoAction_);
    edit->addSeparator();
    edit->addAction(duplicateAction_);
    edit->addAction(deleteAction_);
    edit->addAction(clearSelectionAction_);

    QMenu* create = menus_->addMenu(QStringLiteral("&Criar"));
    create->addAction(primitivesMenuAction_);
    create->addSeparator();
    for (QAction* action : shapeActions_) {
        create->addAction(action);
    }
    create->addSeparator();
    create->addAction(caveAction_);
    create->addAction(randomCaveAction_);

    QMenu* operations = menus_->addMenu(QStringLiteral("&Operações"));
    operations->addAction(toolActions_[4]);
    operations->addAction(toolActions_[5]);
    operations->addAction(toolActions_[6]);
    operations->addAction(toolActions_[7]);
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
    tools->addAction(codeAction_);
    tools->addSeparator();
    for (QAction* action : toolActions_) {
        tools->addAction(action);
    }

    QMenu* view = menus_->addMenu(QStringLiteral("E&xibir"));
    viewMenu_ = view;
    view->addAction(focusAction_);
    view->addAction(resetCameraAction_);
    QMenu* viewsMenu = view->addMenu(QStringLiteral("Vistas"));
    for (QAction* action : viewActions_) {
        viewsMenu->addAction(action);
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
    layout->setContentsMargins(4, 3, 8, 3);
    layout->setSpacing(2);

    auto rule = [bar, layout] {
        auto* line = new QFrame(bar);
        line->setProperty("role", QStringLiteral("vrule"));
        line->setFixedWidth(1);
        layout->addWidget(line);
    };
    auto button = [this, bar, layout](QAction* action, const QString& iconName) {
        auto* tool = new QToolButton(bar);
        tool->setDefaultAction(action);
        tool->setProperty("role", QStringLiteral("appTool"));
        tool->setToolButtonStyle(Qt::ToolButtonIconOnly);
        tool->setIconSize(QSize(18, 18));
        tool->setAutoRaise(true);
        setActionIcon(action, iconName);
        layout->addWidget(tool);
        return tool;
    };

    menus_->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Preferred);
    layout->addWidget(menus_);
    rule();

    // The tools, grouped: transform, faces, sculpting. Then the primitives list.
    for (int index = 0; index < 8; ++index) {
        if (index == 4 || index == 6) {
            layout->addSpacing(6);
        }
        button(toolActions_[index], toolActions_[index]->property("iconName").toString());
    }
    layout->addSpacing(6);
    auto* primitives = new QToolButton(bar);
    primitives->setProperty("role", QStringLiteral("appTool"));
    primitives->setPopupMode(QToolButton::InstantPopup);
    primitives->setIconSize(QSize(18, 18));
    primitives->setToolTip(QStringLiteral("Primitivas: cria um objeto na frente da câmera (Shift+A)"));
    primitives->setAccessibleName(QStringLiteral("Primitivas"));
    auto* menu = new QMenu(primitives);
    for (QAction* action : shapeActions_) {
        setActionIcon(action, action->property("iconName").toString());
        menu->addAction(action);
    }
    primitives->setMenu(menu);
    iconButtons_.append({primitives, QStringLiteral("add_primitive")});
    primitives->setIcon(PetuniaIcons::get(QStringLiteral("add_primitive")));
    layout->addWidget(primitives);
    rule();

    layout->addSpacing(2);
    layout->addWidget(header_->transformControls());
    layout->addStretch(1);

    button(undoAction_, QStringLiteral("undo"));
    button(redoAction_, QStringLiteral("redo"));
    rule();
    button(saveAction_, QStringLiteral("save"));

    // Icons used by the menus only.
    setActionIcon(duplicateAction_, QStringLiteral("duplicate"));
    setActionIcon(deleteAction_, QStringLiteral("trash"));
    setActionIcon(focusAction_, QStringLiteral("frame_selection"));
    for (QAction* action : viewActions_) {
        setActionIcon(action, action->property("iconName").toString());
    }
    setMenuWidget(bar);
}

void MainWindow::buildSculptBar() {
    // The tool's own options sit under the viewport strip, not across the whole window.
    sculptBar_ = new QToolBar(QStringLiteral("Esculpir"), this);
    sculptBar_->setObjectName(QStringLiteral("PetuniaToolOptions"));
    sculptBar_->setMovable(false);
    sculptBar_->setFloatable(false);
    sculptBar_->setAccessibleName(QStringLiteral("Opções de esculpir"));
    if (auto* centerLayout = qobject_cast<QVBoxLayout*>(centralWidget()->layout())) {
        centerLayout->insertWidget(1, sculptBar_);
    }
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

void MainWindow::buildDocks() {
    auto addDock = [this](const QString& title, QWidget* widget, Qt::DockWidgetArea area) {
        auto* dock = new QDockWidget(title, this);
        dock->setObjectName(title);
        dock->setWidget(widget);
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
    code_ = new CodePanel(bridge_.get(), this);

    QDockWidget* hierarchyDock = addDock(QStringLiteral("Hierarquia"), hierarchy_, Qt::LeftDockWidgetArea);
    QDockWidget* inspectorDock = addDock(QStringLiteral("Inspetor"), inspector_, Qt::RightDockWidgetArea);
    QDockWidget* diagnosticsDock = addDock(QStringLiteral("Diagnósticos"), diagnostics_, Qt::BottomDockWidgetArea);
    QDockWidget* historyDock = addDock(QStringLiteral("Histórico"), history_, Qt::BottomDockWidgetArea);
    QDockWidget* apiDock = addDock(QStringLiteral("API"), api_, Qt::BottomDockWidgetArea);
    codeDock_ = addDock(QStringLiteral("Código"), code_, Qt::BottomDockWidgetArea);
    tabifyDockWidget(diagnosticsDock, historyDock);
    tabifyDockWidget(historyDock, apiDock);
    tabifyDockWidget(apiDock, codeDock_);
    diagnosticsDock->raise();

    auto* createMenu = new QMenu(hierarchy_);
    for (QAction* action : shapeActions_) {
        createMenu->addAction(action);
    }
    createMenu->addSeparator();
    createMenu->addAction(caveAction_);
    createMenu->addAction(randomCaveAction_);
    hierarchy_->setCreateMenu(createMenu);

    hierarchyDock->setMinimumWidth(220);
    inspectorDock->setMinimumWidth(320);
    resizeDocks({hierarchyDock, inspectorDock}, {250, 380}, Qt::Horizontal);
    resizeDocks({diagnosticsDock}, {200}, Qt::Vertical);

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

    saveAction_->setEnabled(project);
    undoAction_->setEnabled(snapshot.value("can_undo").toBool());
    redoAction_->setEnabled(snapshot.value("can_redo").toBool());
    for (QAction* action : shapeActions_) {
        action->setEnabled(project);
    }
    for (QAction* action : {caveAction_, randomCaveAction_, sculptLargerAction_, sculptSmallerAction_}) {
        action->setEnabled(project);
    }
    sculptRadius_ = snapshot.value("sculpt").toObject().value(QStringLiteral("radius")).toDouble(sculptRadius_);
    syncSculptBar(snapshot);
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
    countLabel_->setText(project ? QStringLiteral("Objetos: %1   Erros: %2   Avisos: %3")
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
    cameraLabel_->setText(QStringLiteral("   Câmera: %1, %2, %3")
                              .arg(camera.value("x").toDouble(), 0, 'f', 1)
                              .arg(camera.value("y").toDouble(), 0, 'f', 1)
                              .arg(camera.value("z").toDouble(), 0, 'f', 1));
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

void MainWindow::showShortcuts() {
    QMessageBox::information(
        this, QStringLiteral("Atalhos do teclado"),
        QStringLiteral(
            "<b>Viewport 3D</b><br>"
            "Setas: mover a câmera · PageUp/PageDown: subir e descer · Shift: mais rápido<br>"
            "Segure o botão direito: girar a câmera e voar com W A S D (Q desce, E sobe)<br>"
            "Roda do mouse: aproximar e afastar · Clique esquerdo: selecionar<br>"
            "Cubo de navegação (canto superior direito): face, aresta ou canto mudam a vista<br>"
            "Alt+1 a Alt+6: vistas frente, trás, esquerda, direita, topo, baixo<br><br>"
            "<b>Ferramentas</b><br>"
            "V selecionar e transformar (gizmo universal) · G mover · R girar · S escalar<br>"
            "Toque duas vezes em G, R ou S: modo livre (o mouse muda o valor; X Y Z restringem; digite um número)<br>"
            "P empurrar/puxar uma face · E extrudar · Ctrl+Alt+C cavar · Ctrl+Alt+H tornar oco · Alt+P editar pivô<br>"
            "Ctrl+PageUp / Ctrl+PageDown: subir e descer um andar · Shift+A menu de primitivas<br>"
            "B esculpir caverna (arraste cava; com Ctrl adiciona rocha) · [ e ] raio do pincel<br>"
            "No gizmo: seta = eixo, quadrado = plano, centro = tela; Ctrl inverte o snap; Esc cancela<br><br>"
            "<b>Edição</b><br>"
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
    if (!quitting_ && (!code_->maybeSave() || !confirmDiscardChanges())) {
        event->ignore();
        return;
    }
    QSettings settings;
    settings.setValue("window/geometry", saveGeometry());
    settings.setValue("window/state", saveState());
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
