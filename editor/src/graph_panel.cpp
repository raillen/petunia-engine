#include "graph_panel.h"

#include <algorithm>

#include <QAction>
#include <QCursor>
#include <QDialog>
#include <QDialogButtonBox>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QShortcut>
#include <QSignalBlocker>
#include <QSplitter>
#include <QStackedWidget>
#include <QToolButton>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>
#include <QHash>
#include <QJsonValue>

#include "engine_bridge.h"
#include "node_canvas.h"
#include "petunia_icons.h"
#include "petunia_theme.h"
#include "ui_kit/shell/petunia_segmented.h"

namespace {

constexpr int kRowRole = Qt::UserRole + 1;
constexpr int kKeyRole = Qt::UserRole + 2;

QString textOf(const QJsonObject& object, const char* key) {
    return object.value(QLatin1String(key)).toString();
}

// A palette: type to filter, arrows to move, Enter to choose. Returns the node kind, or empty.
QString pickNode(QWidget* parent, EngineBridge* bridge, const QString& title, const QString& want,
                 const QJsonObject& forPin) {
    QDialog dialog(parent);
    dialog.setWindowTitle(title);
    dialog.resize(520, 420);
    auto* layout = new QVBoxLayout(&dialog);
    auto* hint = new QLabel(QStringLiteral("Digite para filtrar (sem acento, qualquer ordem). Setas escolhem, Enter confirma, Esc cancela."), &dialog);
    hint->setWordWrap(true);
    layout->addWidget(hint);
    auto* query = new QLineEdit(&dialog);
    query->setAccessibleName(QStringLiteral("Buscar nó"));
    layout->addWidget(query);
    auto* list = new QListWidget(&dialog);
    list->setAccessibleName(QStringLiteral("Nós encontrados"));
    layout->addWidget(list, 1);
    auto* summary = new QLabel(&dialog);
    summary->setWordWrap(true);
    summary->setAccessibleName(QStringLiteral("O que o nó faz"));
    layout->addWidget(summary);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dialog);
    layout->addWidget(buttons);
    QObject::connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);

    auto refill = [&] {
        QJsonObject fields{{"query", query->text()}, {"want", want}};
        if (!forPin.isEmpty()) {
            fields.insert(QStringLiteral("for"), forPin);
        }
        const Reply reply = bridge->call(QStringLiteral("graph.search"), fields);
        list->clear();
        for (const QJsonValue& value : reply.result.toObject().value(QStringLiteral("nodes")).toArray()) {
            const QJsonObject node = value.toObject();
            auto* item = new QListWidgetItem(QStringLiteral("%1 — %2").arg(textOf(node, "title"), textOf(node, "category")), list);
            item->setData(Qt::UserRole, textOf(node, "kind"));
            item->setData(Qt::UserRole + 1, textOf(node, "summary"));
            item->setData(Qt::AccessibleDescriptionRole, textOf(node, "summary"));
        }
        if (list->count() > 0) {
            list->setCurrentRow(0);
        } else {
            summary->setText(QStringLiteral("Nenhum nó combina com a busca."));
        }
    };
    QObject::connect(query, &QLineEdit::textChanged, &dialog, refill);
    QObject::connect(list, &QListWidget::currentItemChanged, &dialog, [&](QListWidgetItem* item) {
        summary->setText(item ? item->data(Qt::UserRole + 1).toString() : QString());
    });
    QObject::connect(list, &QListWidget::itemActivated, &dialog, [&](QListWidgetItem*) { dialog.accept(); });
    // Arrow keys in the search box move the list, so the hands never leave the keyboard row.
    auto* down = new QShortcut(QKeySequence(Qt::Key_Down), query);
    QObject::connect(down, &QShortcut::activated, &dialog, [&] { list->setCurrentRow(qMin(list->currentRow() + 1, list->count() - 1)); });
    auto* up = new QShortcut(QKeySequence(Qt::Key_Up), query);
    QObject::connect(up, &QShortcut::activated, &dialog, [&] { list->setCurrentRow(qMax(list->currentRow() - 1, 0)); });
    refill();
    query->setFocus();
    if (dialog.exec() != QDialog::Accepted || list->currentItem() == nullptr) {
        return QString();
    }
    return list->currentItem()->data(Qt::UserRole).toString();
}

// A searchable list of the scene's objects, by id and name, for an input that needs an object.
quint64 pickEntity(QWidget* parent, EngineBridge* bridge, const QString& title, quint64 current) {
    QDialog dialog(parent);
    dialog.setWindowTitle(title);
    dialog.resize(460, 420);
    auto* layout = new QVBoxLayout(&dialog);
    auto* hint = new QLabel(QStringLiteral("Digite parte do nome ou o número. Setas escolhem, Enter confirma, Esc cancela."), &dialog);
    hint->setWordWrap(true);
    layout->addWidget(hint);
    auto* query = new QLineEdit(&dialog);
    query->setAccessibleName(QStringLiteral("Buscar objeto"));
    layout->addWidget(query);
    auto* list = new QListWidget(&dialog);
    list->setAccessibleName(QStringLiteral("Objetos da cena"));
    layout->addWidget(list, 1);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dialog);
    layout->addWidget(buttons);
    QObject::connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
    auto refill = [&] {
        const QJsonArray entities = bridge->snapshot().value(QStringLiteral("entities")).toArray();
        const QString needle = query->text().trimmed();
        list->clear();
        for (const QJsonValue& value : entities) {
            const QJsonObject entity = value.toObject();
            if (entity.contains(QStringLiteral("owner"))) {
                continue;  // a collider is part of an object, not an object
            }
            const quint64 id = static_cast<quint64>(entity.value(QStringLiteral("id")).toDouble());
            const QString name = textOf(entity, "name");
            const QString label = QStringLiteral("#%1  %2").arg(id).arg(name.isEmpty() ? QStringLiteral("Objeto") : name);
            if (!needle.isEmpty() && !label.contains(needle, Qt::CaseInsensitive)) {
                continue;
            }
            auto* item = new QListWidgetItem(label, list);
            item->setData(Qt::UserRole, QVariant::fromValue(id));
            item->setData(Qt::AccessibleDescriptionRole, QStringLiteral("Objeto %1, id %2").arg(name).arg(id));
            if (id == current) {
                list->setCurrentItem(item);
            }
        }
        if (list->currentRow() < 0 && list->count() > 0) {
            list->setCurrentRow(0);
        }
    };
    QObject::connect(query, &QLineEdit::textChanged, &dialog, refill);
    QObject::connect(list, &QListWidget::itemActivated, &dialog, [&](QListWidgetItem*) { dialog.accept(); });
    refill();
    query->setFocus();
    if (dialog.exec() != QDialog::Accepted || list->currentItem() == nullptr) {
        return 0;
    }
    return list->currentItem()->data(Qt::UserRole).toULongLong();
}

}  // namespace

GraphPanel::GraphPanel(EngineBridge* bridge, QWidget* parent) : QWidget(parent), bridge_(bridge) {
    setAccessibleName(QStringLiteral("Lógica do jogo (grafos de nós)"));
    auto* outer = new QHBoxLayout(this);
    outer->setContentsMargins(0, 0, 0, 0);
    outer->setSpacing(0);
    auto* columns = new QSplitter(Qt::Horizontal, this);
    columns->setHandleWidth(PetuniaTheme::instance().scaled(5));
    columns->setChildrenCollapsible(false);
    outer->addWidget(columns);

    // Left: the project's graphs.
    auto* sidebar = new QWidget(columns);
    sidebar->setObjectName(QStringLiteral("PetuniaWorkspacePage"));
    sidebar->setAttribute(Qt::WA_StyledBackground);
    auto* sideLayout = new QVBoxLayout(sidebar);
    sideLayout->setContentsMargins(6, 6, 6, 6);
    sideLayout->setSpacing(4);
    auto* sideHeader = new QHBoxLayout;
    auto* sideTitle = new QLabel(QStringLiteral("Scripts e grafos"), sidebar);
    sideTitle->setProperty("role", QStringLiteral("panelTitle"));
    sideHeader->addWidget(sideTitle, 1);
    newButton_ = new QToolButton(sidebar);
    newButton_->setProperty("role", QStringLiteral("panelButton"));
    newButton_->setProperty("iconName", QStringLiteral("plus"));
    newButton_->setToolTip(QStringLiteral("Novo grafo"));
    newButton_->setAccessibleName(QStringLiteral("Novo grafo"));
    sideHeader->addWidget(newButton_);
    sideLayout->addLayout(sideHeader);
    graphs_ = new QListWidget(sidebar);
    graphs_->setAccessibleName(QStringLiteral("Scripts e grafos do projeto"));
    graphs_->setToolTip(QStringLiteral("Scripts .aipo e grafos .graph.json; um ponto marca os que rodam no objeto selecionado"));
    graphs_->setFrameShape(QFrame::NoFrame);
    graphs_->setIconSize(QSize(16, 16));
    sideLayout->addWidget(graphs_, 1);
    noGraphs_ = new QLabel(QStringLiteral("Nenhum script ou grafo ainda.\nO botão + cria um grafo; use o workspace Código para um script .aipo. Depois, \"Ligar ao objeto\" faz ele rodar no jogo."), sidebar);
    noGraphs_->setWordWrap(true);
    noGraphs_->setProperty("role", QStringLiteral("muted"));
    noGraphs_->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    noGraphs_->hide();
    sideLayout->addWidget(noGraphs_, 1);
    sidebar->setMinimumWidth(150);

    // Right: the bar, the outline (or the generated code), the findings.
    auto* page = new QWidget(columns);
    page->setObjectName(QStringLiteral("PetuniaWorkspacePage"));
    page->setAttribute(Qt::WA_StyledBackground);
    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(0, 0, 0, 6);
    layout->setSpacing(0);
    columns->addWidget(sidebar);
    columns->addWidget(page);
    columns->setStretchFactor(1, 1);
    columns->setSizes({200, 900});

    auto* barWidget = new QWidget(page);
    barWidget->setObjectName(QStringLiteral("PetuniaViewportHeader"));
    barWidget->setAttribute(Qt::WA_StyledBackground);
    auto* bar = new QHBoxLayout(barWidget);
    bar->setContentsMargins(6, 3, 6, 3);
    bar->setSpacing(2);
    auto makeButton = [bar](const QString& text, const QString& icon, const QString& tip) {
        auto* button = new QToolButton;
        button->setText(text);
        button->setToolTip(tip);
        button->setAccessibleName(text);
        button->setProperty("role", QStringLiteral("header"));
        button->setProperty("iconName", icon);
        button->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
        button->setIcon(PetuniaIcons::get(icon));
        bar->addWidget(button);
        return button;
    };
    addEventButton_ = makeButton(QStringLiteral("Evento"), QStringLiteral("bolt"),
                                 QStringLiteral("Começa um caminho: quando algo acontece (começar, a cada quadro, tocar...)"));
    addButton_ = makeButton(QStringLiteral("Adicionar depois"), QStringLiteral("plus"),
                            QStringLiteral("Põe um nó depois da linha escolhida, ou dentro do ramo escolhido (Insert)"));
    editButton_ = makeButton(QStringLiteral("Editar"), QStringLiteral("mode_edit"),
                             QStringLiteral("Muda o valor ou a opção da linha escolhida (Enter)"));
    removeButton_ = makeButton(QStringLiteral("Remover"), QStringLiteral("delete"),
                               QStringLiteral("Remove o nó da linha escolhida, ou esvazia a entrada (Delete)"));
    undoButton_ = makeButton(QStringLiteral("Desfazer"), QStringLiteral("undo"), QStringLiteral("Desfaz a última mudança deste grafo (Ctrl+Z)"));
    redoButton_ = makeButton(QStringLiteral("Refazer"), QStringLiteral("redo"), QStringLiteral("Refaz o que foi desfeito (Ctrl+Shift+Z)"));
    switch_ = new PetuniaSegmented(barWidget);
    switch_->setGroupLabel(QStringLiteral("Como ver o grafo"));
    switch_->setItems({
        {QStringLiteral("outline"), QStringLiteral("Contorno"), QStringLiteral("As linhas, editáveis pelo teclado"), QStringLiteral("list_view")},
        {QStringLiteral("canvas"), QStringLiteral("Caixas e fios"), QStringLiteral("O mesmo grafo desenhado; arrastar muda só a posição"), QStringLiteral("connect")},
    });
    switch_->setCurrent(QStringLiteral("outline"));
    bar->addWidget(switch_);
    bar->addStretch(1);
    codeButton_ = makeButton(QStringLiteral("Ver como código"), QStringLiteral("terminal"),
                             QStringLiteral("Mostra o texto Aipo que este grafo gera. Só leitura: editar o grafo é pelas linhas"));
    codeButton_->setCheckable(true);
    attachButton_ = makeButton(QStringLiteral("Ligar ao objeto"), QStringLiteral("link"),
                               QStringLiteral("Liga este grafo ao objeto selecionado na cena (tabela Script)"));
    attachButton_->setAccessibleName(QStringLiteral("Ligar o grafo ao objeto selecionado"));
    barWidget->setMinimumWidth(1);
    layout->addWidget(barWidget);

    // Canvas-only controls live on a second row: the main bar keeps its usual positions.
    canvasBar_ = new QWidget(page);
    canvasBar_->setObjectName(QStringLiteral("PetuniaViewportHeader"));
    canvasBar_->setAttribute(Qt::WA_StyledBackground);
    auto* canvasBarLayout = new QHBoxLayout(canvasBar_);
    canvasBarLayout->setContentsMargins(6, 1, 6, 3);
    canvasBarLayout->setSpacing(2);
    auto makeCanvasButton = [canvasBarLayout](const QString& text, const QString& icon, const QString& tip) {
        auto* button = new QToolButton;
        button->setText(text);
        button->setToolTip(tip);
        button->setAccessibleName(text);
        button->setProperty("role", QStringLiteral("header"));
        button->setProperty("iconName", icon);
        button->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
        button->setIcon(PetuniaIcons::get(icon));
        canvasBarLayout->addWidget(button);
        return button;
    };
    auto* allButton = makeCanvasButton(QStringLiteral("Todos"), QStringLiteral("select_all"),
                                       QStringLiteral("Selecionar todas as caixas (Ctrl+A); depois use Agrupar"));
    zoomOutButton_ = makeCanvasButton(QStringLiteral("−"), QStringLiteral("minus"), QStringLiteral("Diminuir o zoom do canvas"));
    zoomInButton_ = makeCanvasButton(QStringLiteral("+"), QStringLiteral("plus"), QStringLiteral("Aumentar o zoom do canvas"));
    fitButton_ = makeCanvasButton(QStringLiteral("Centralizar"), QStringLiteral("frame_all"),
                                  QStringLiteral("Enquadrar todos os nós na tela (Home)"));
    groupButton_ = makeCanvasButton(QStringLiteral("Agrupar"), QStringLiteral("collection"),
                                    QStringLiteral("Agrupar as caixas escolhidas (Ctrl+clique para escolher várias)"));
    ungroupButton_ = makeCanvasButton(QStringLiteral("Desagrupar"), QStringLiteral("close"),
                                      QStringLiteral("Desfazer o grupo da caixa escolhida"));
    canvasBarLayout->addStretch(1);
    canvasBar_->hide();
    layout->addWidget(canvasBar_);

    auto* splitter = new QSplitter(Qt::Vertical);
    views_ = new QStackedWidget;
    tree_ = new QTreeWidget;
    tree_->setAccessibleName(QStringLiteral("Lógica do grafo"));
    tree_->setAccessibleDescription(QStringLiteral("Insert adiciona um nó depois, Enter edita, Delete remove, Ctrl+Z desfaz"));
    tree_->setHeaderHidden(true);
    tree_->setColumnCount(1);
    tree_->setIndentation(18);
    tree_->setUniformRowHeights(true);
    tree_->setSelectionMode(QAbstractItemView::SingleSelection);
    tree_->setContextMenuPolicy(Qt::CustomContextMenu);
    tree_->setIconSize(QSize(16, 16));
    tree_->setFrameShape(QFrame::NoFrame);
    views_->addWidget(tree_);
    auto* codePage = new QWidget(views_);
    auto* codeLayout = new QVBoxLayout(codePage);
    codeLayout->setContentsMargins(8, 6, 8, 0);
    codeLayout->setSpacing(4);
    auto* codeHint = new QLabel(
        QStringLiteral("Código gerado pelo grafo. Mudar o grafo muda este texto; "
                       "edite e use \"Aplicar código\" para voltar ao grafo."),
        codePage);
    codeHint->setWordWrap(true);
    codeHint->setProperty("role", QStringLiteral("muted"));
    auto* codeHeader = new QHBoxLayout;
    codeHeader->setSpacing(6);
    codeHeader->addWidget(codeHint, 1);
    asScriptButton_ = new QToolButton(codePage);
    asScriptButton_->setText(QStringLiteral("Transformar em script"));
    asScriptButton_->setToolTip(QStringLiteral("Copia o código gerado para um arquivo .aipo independente; o grafo não o atualiza depois"));
    asScriptButton_->setAccessibleName(asScriptButton_->text());
    asScriptButton_->setProperty("role", QStringLiteral("header"));
    asScriptButton_->setProperty("iconName", QStringLiteral("script"));
    asScriptButton_->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    asScriptButton_->setIcon(PetuniaIcons::get(QStringLiteral("script")));
    codeHeader->addWidget(asScriptButton_);
    codeLayout->addLayout(codeHeader);
    code_ = new QPlainTextEdit;
    code_->setReadOnly(true);
    code_->setAccessibleName(QStringLiteral("Código gerado pelo grafo"));
    code_->setLineWrapMode(QPlainTextEdit::NoWrap);
    codeLayout->addWidget(code_, 1);
    views_->addWidget(codePage);
    canvas_ = new NodeCanvas;
    views_->addWidget(canvas_);
    splitter->addWidget(views_);
    diagnostics_ = new QListWidget;
    diagnostics_->setAccessibleName(QStringLiteral("Problemas do grafo"));
    diagnostics_->setToolTip(QStringLiteral("Enter vai até o nó do problema"));
    diagnostics_->setIconSize(QSize(16, 16));
    diagnostics_->setMaximumHeight(150);
    diagnostics_->setMinimumHeight(1);
    splitter->addWidget(diagnostics_);
    debugRows_ = new QListWidget;
    debugRows_->setAccessibleName(QStringLiteral("Por que esta condição falhou"));
    debugRows_->setToolTip(QStringLiteral("Enter vai até o nó da condição"));
    debugRows_->setIconSize(QSize(16, 16));
    debugRows_->setMaximumHeight(120);
    debugRows_->setMinimumHeight(1);
    debugRows_->hide();
    splitter->addWidget(debugRows_);
    splitter->setStretchFactor(0, 4);
    splitter->setStretchFactor(1, 1);
    layout->addWidget(splitter, 1);
    status_ = new QLabel;
    status_->setAccessibleName(QStringLiteral("Estado do grafo"));
    status_->setProperty("role", QStringLiteral("caption"));
    status_->setContentsMargins(8, 2, 8, 0);
    layout->addWidget(status_);

    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, [this] {
        for (QToolButton* button : findChildren<QToolButton*>()) {
            const QString icon = button->property("iconName").toString();
            if (!icon.isEmpty()) {
                button->setIcon(PetuniaIcons::get(icon));
            }
        }
        refreshView();
    });
    newButton_->setIcon(PetuniaIcons::get(QStringLiteral("plus")));

    connect(bridge_, &EngineBridge::snapshotChanged, this, &GraphPanel::onSnapshot);
    connect(graphs_, &QListWidget::currentItemChanged, this, [this](QListWidgetItem* item) {
        selectedName_ = item != nullptr ? item->data(Qt::UserRole).toString() : QString();
        selectedKind_ = item != nullptr ? item->data(Qt::UserRole + 1).toString() : QString();
        updateButtons();
        if (item == nullptr) {
            return;
        }
        if (selectedKind_ == QLatin1String("graph")) {
            if (selectedName_ != openName_) {
                openGraph(selectedName_);
            }
        } else if (selectedKind_ == QLatin1String("script")) {
            emit scriptOpenRequested(selectedName_);
        } else if (selectedKind_ == QLatin1String("missing")) {
            emit message(QStringLiteral("O objeto pede o script `%1`, mas o arquivo ainda não existe. Crie `%1.aipo` no workspace Código ou um grafo com esse nome.").arg(selectedName_));
        }
    });
    connect(graphs_, &QListWidget::itemDoubleClicked, this, [this](QListWidgetItem* item) {
        if (item == nullptr || item->data(Qt::UserRole + 1).toString() != QLatin1String("missing")) {
            return;
        }
        const QString name = item->data(Qt::UserRole).toString();
        const Reply reply = bridge_->call(QStringLiteral("script.new"), QJsonObject{{"name", name}});
        if (!reply.ok) {
            emit message(reply.message);
            return;
        }
        reloadList(openName_);
        emit scriptOpenRequested(name);
        emit message(QStringLiteral("Script %1 criado a partir do nome pedido pelo objeto.").arg(name));
    });
    graphs_->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(graphs_, &QListWidget::customContextMenuRequested, this, [this](const QPoint& position) {
        QListWidgetItem* item = graphs_->itemAt(position);
        if (item == nullptr || item->data(Qt::UserRole + 1).toString() == QLatin1String("missing")) {
            return;
        }
        graphs_->setCurrentItem(item);
        QMenu menu(this);
        QAction* rename = menu.addAction(PetuniaIcons::get(QStringLiteral("mode_edit")), QStringLiteral("Renomear…"));
        QAction* remove = menu.addAction(PetuniaIcons::get(QStringLiteral("delete")), QStringLiteral("Excluir…"));
        QAction* chosen = menu.exec(graphs_->viewport()->mapToGlobal(position));
        if (chosen == rename) {
            renameSelected();
        } else if (chosen == remove) {
            deleteSelected();
        }
    });
    connect(newButton_, &QToolButton::clicked, this, &GraphPanel::createGraph);
    connect(addEventButton_, &QToolButton::clicked, this, &GraphPanel::addEvent);
    connect(addButton_, &QToolButton::clicked, this, &GraphPanel::addAfter);
    connect(editButton_, &QToolButton::clicked, this, &GraphPanel::editCurrent);
    connect(removeButton_, &QToolButton::clicked, this, &GraphPanel::removeCurrent);
    connect(undoButton_, &QToolButton::clicked, this, &GraphPanel::undo);
    connect(redoButton_, &QToolButton::clicked, this, &GraphPanel::redo);
    connect(attachButton_, &QToolButton::clicked, this, &GraphPanel::attach);
    connect(asScriptButton_, &QToolButton::clicked, this, &GraphPanel::asScript);
    connect(applyCodeButton_, &QToolButton::clicked, this, &GraphPanel::applyCode);
    connect(code_, &QPlainTextEdit::cursorPositionChanged, this, [this] {
        if (codeLines_.isEmpty()) {
            return;
        }
        const int line = code_->textCursor().blockNumber();
        if (line >= 0 && line < codeLines_.size()) {
            const int node = codeLines_.at(line).toInt();
            if (node != 0) {
                selectNode(node);
                if (canvas_ != nullptr) {
                    canvas_->selectNode(node);
                }
            }
        }
    });
    connect(allButton, &QToolButton::clicked, this, [this] { if (canvas_ != nullptr) canvas_->selectAll(); });
    connect(zoomOutButton_, &QToolButton::clicked, this, [this] { if (canvas_ != nullptr) canvas_->zoomOut(); });
    connect(zoomInButton_, &QToolButton::clicked, this, [this] { if (canvas_ != nullptr) canvas_->zoomIn(); });
    connect(fitButton_, &QToolButton::clicked, this, [this] { if (canvas_ != nullptr) canvas_->fitGraph(); });
    connect(groupButton_, &QToolButton::clicked, this, &GraphPanel::groupSelection);
    connect(ungroupButton_, &QToolButton::clicked, this, &GraphPanel::ungroupSelection);
    connect(codeButton_, &QToolButton::toggled, this, [this](bool on) {
        showMode(on ? QStringLiteral("code") : switch_->current());
        if (on) {
            showCode();
        }
    });
    connect(switch_, &PetuniaSegmented::activated, this, [this](const QString& mode) {
        switch_->setCurrent(mode);
        if (codeButton_->isChecked()) {
            codeButton_->setChecked(false);
        }
        showMode(mode);
    });
    connect(debugRows_, &QListWidget::itemActivated, this, [this](QListWidgetItem* item) {
        if (item == nullptr) {
            return;
        }
        const int node = item->data(Qt::UserRole).toInt();
        selectNode(node);
        if (canvas_ != nullptr) {
            canvas_->selectNode(node);
        }
    });
    connect(tree_, &QTreeWidget::itemActivated, this, [this] { editCurrent(); });
    connect(tree_, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* item) {
        if (!rendering_) {
            selectedKey_ = item != nullptr ? item->data(0, kKeyRole).toString() : QString();
            if (canvas_ != nullptr && item != nullptr) {
                const QJsonObject row = item->data(0, kRowRole).toJsonObject();
                const QString type = textOf(row, "type");
                if (type == QLatin1String("step") || type == QLatin1String("event")) {
                    canvas_->selectNode(row.value(QStringLiteral("node")).toInt());
                }
            }
        }
        updateButtons();
    });
    connect(tree_, &QTreeWidget::customContextMenuRequested, this, &GraphPanel::contextMenu);
    connect(canvas_, &NodeCanvas::nodeSelected, this, [this](int node) {
        selectNode(node);
        updateButtons();
    });
    connect(canvas_, &NodeCanvas::nodeActivated, this, [this](int node) {
        selectNode(node);
        editCurrent();
    });
    connect(canvas_, &NodeCanvas::nodeRemoveRequested, this, [this](int node) {
        selectNode(node);
        removeCurrent();
    });
    connect(canvas_, &NodeCanvas::nodesMoved, this, [this](const QJsonArray& places) {
        const Reply reply = bridge_->call(
            QStringLiteral("graph.set_layouts"),
            QJsonObject{{"nodes", places}});
        if (!reply.ok) {
            emit message(reply.message);
            refreshView();
        }
    });
    connect(canvas_, &NodeCanvas::selectionChanged, this, &GraphPanel::updateButtons);
    connect(canvas_, &NodeCanvas::zoomChanged, this, [this](int percent) {
        emit message(QStringLiteral("Zoom %1 %.").arg(percent));
    });
    connect(canvas_, &NodeCanvas::connectRequested, this, [this](const QJsonObject& from, const QJsonObject& to) {
        if (run(QStringLiteral("graph.connect"), QJsonObject{{"from", from}, {"to", to}})) {
            refreshView();
            emit message(QStringLiteral("Ligado. Ctrl+Z desfaz."));
        }
    });
    connect(canvas_, &NodeCanvas::message, this, &GraphPanel::message);
    connect(diagnostics_, &QListWidget::itemActivated, this, [this](QListWidgetItem* item) {
        goToDiagnostic(item->data(Qt::UserRole).toJsonObject());
    });

    auto shortcut = [this](const QKeySequence& keys, void (GraphPanel::*slot)()) {
        auto* made = new QShortcut(keys, this);
        made->setContext(Qt::WidgetWithChildrenShortcut);
        connect(made, &QShortcut::activated, this, slot);
    };
    shortcut(QKeySequence(Qt::Key_Insert), &GraphPanel::addAfter);
    shortcut(QKeySequence(Qt::Key_Delete), &GraphPanel::removeCurrent);
    shortcut(QKeySequence(Qt::CTRL | Qt::Key_Z), &GraphPanel::undo);
    shortcut(QKeySequence(Qt::CTRL | Qt::SHIFT | Qt::Key_Z), &GraphPanel::redo);
    shortcut(QKeySequence(Qt::CTRL | Qt::Key_Y), &GraphPanel::redo);

    onSnapshot();
}

void GraphPanel::onSnapshot() {
    const QJsonValue project = bridge_->snapshot().value(QStringLiteral("project"));
    const QString key = project.isObject() ? project.toObject().value(QStringLiteral("root")).toString() : QString();
    for (QWidget* widget : std::initializer_list<QWidget*>{graphs_, newButton_, attachButton_}) {
        widget->setEnabled(!key.isEmpty());
    }
    if (key != projectKey_) {
        projectKey_ = key;
        openName_.clear();
        selectedName_.clear();
        selectedKind_.clear();
        selectedKey_.clear();
        tree_->clear();
        diagnostics_->clear();
        code_->clear();
        status_->clear();
        if (!key.isEmpty()) {
            reloadList();
        } else {
            graphs_->clear();
            noGraphs_->hide();
        }
        updateButtons();
    }
}

void GraphPanel::reload() {
    if (projectKey_.isEmpty()) {
        return;
    }
    reloadList(openName_);
    if (!openName_.isEmpty()) {
        refreshView();
    }
}

void GraphPanel::open(const QString& name) {
    reloadList(name);
    openGraph(name);
}

bool GraphPanel::run(const QString& operation, const QJsonObject& fields, QJsonObject* result) {
    const Reply reply = bridge_->call(operation, fields);
    if (!reply.ok) {
        emit message(reply.message);
        QMessageBox::warning(this, QStringLiteral("Não foi possível"), reply.explanation());
        return false;
    }
    if (result != nullptr) {
        *result = reply.result.toObject();
    }
    return true;
}

void GraphPanel::reloadList(const QString& select) {
    const Reply reply = bridge_->call(QStringLiteral("programming.list"));
    if (!reply.ok) {
        return;
    }
    QList<QJsonObject> items;
    for (const QJsonValue& value : reply.result.toObject().value(QStringLiteral("items")).toArray()) {
        items.append(value.toObject());
    }
    std::sort(items.begin(), items.end(), [](const QJsonObject& left, const QJsonObject& right) {
        const bool leftAttached = !left.value(QStringLiteral("attached")).toArray().isEmpty();
        const bool rightAttached = !right.value(QStringLiteral("attached")).toArray().isEmpty();
        if (leftAttached != rightAttached) {
            return leftAttached;
        }
        const QString leftKind = left.value(QStringLiteral("kind")).toString();
        const QString rightKind = right.value(QStringLiteral("kind")).toString();
        if (leftKind != rightKind) {
            return leftKind == QLatin1String("graph");
        }
        return left.value(QStringLiteral("name")).toString() < right.value(QStringLiteral("name")).toString();
    });
    const QSignalBlocker blocker(graphs_);
    graphs_->clear();
    selectedName_.clear();
    selectedKind_.clear();
    for (const QJsonObject& entry : std::as_const(items)) {
        const QString name = entry.value(QStringLiteral("name")).toString();
        const QString kind = entry.value(QStringLiteral("kind")).toString();
        const QJsonArray attached = entry.value(QStringLiteral("attached")).toArray();
        QString label = name;
        if (!attached.isEmpty()) {
            QStringList objects;
            for (const QJsonValue& object : attached) {
                const QJsonObject entity = object.toObject();
                objects << QStringLiteral("#%1 %2").arg(entity.value(QStringLiteral("id")).toInt()).arg(entity.value(QStringLiteral("name")).toString());
            }
            label = QStringLiteral("• %1  —  %2").arg(name, objects.join(QStringLiteral(", ")));
        }
        if (kind == QLatin1String("missing")) {
            label = QStringLiteral("! %1  —  arquivo ausente").arg(name);
        }
        const QString icon = kind == QLatin1String("graph") ? QStringLiteral("connect")
            : (kind == QLatin1String("script") ? QStringLiteral("script") : QStringLiteral("warning"));
        auto* item = new QListWidgetItem(PetuniaIcons::get(icon), label, graphs_);
        item->setData(Qt::UserRole, name);
        item->setData(Qt::UserRole + 1, kind);
        item->setData(Qt::AccessibleDescriptionRole,
                      kind == QLatin1String("missing")
                          ? QStringLiteral("Script %1 pedido por um objeto, mas o arquivo não existe").arg(name)
                          : QStringLiteral("%1 %2, %3 objeto(s) usam").arg(kind, name).arg(attached.size()));
        if (kind == QLatin1String("graph") && name == select) {
            graphs_->setCurrentItem(item);
            selectedName_ = name;
            selectedKind_ = kind;
        }
    }
    noGraphs_->setVisible(graphs_->count() == 0);
    graphs_->setVisible(graphs_->count() > 0);
}

void GraphPanel::openGraph(const QString& name) {
    QJsonObject view;
    if (!run(QStringLiteral("graph.open"), QJsonObject{{"name", name}}, &view)) {
        // The graph that was open stays; the list goes back to it.
        reloadList(openName_);
        return;
    }
    openName_ = name;
    selectedKey_.clear();
    selectedName_ = name;
    selectedKind_ = QStringLiteral("graph");
    renderView(view);
    canvasFitted_ = false;
    if (canvas_ != nullptr && views_->currentIndex() == 2) {
        canvas_->fitGraph();
        canvasFitted_ = true;
    }
}

void GraphPanel::refreshView() {
    if (openName_.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("graph.view"));
    if (reply.ok) {
        renderView(reply.result.toObject());
    }
}

namespace {

QString keyOf(const QString& type, int node, const QString& pin) {
    return QStringLiteral("%1:%2:%3").arg(type).arg(node).arg(pin);
}

QTreeWidgetItem* makeItem(QTreeWidgetItem* parent, QTreeWidget* tree, const QString& text, const QString& icon,
                          const QJsonObject& row, const QString& key, const QString& description) {
    auto* item = parent != nullptr ? new QTreeWidgetItem(parent) : new QTreeWidgetItem(tree);
    item->setText(0, text);
    item->setIcon(0, PetuniaIcons::get(icon));
    item->setData(0, kRowRole, row);
    item->setData(0, kKeyRole, key);
    item->setData(0, Qt::AccessibleTextRole, text);
    item->setData(0, Qt::AccessibleDescriptionRole, description);
    item->setToolTip(0, description.isEmpty() ? text : QStringLiteral("%1\n%2").arg(text, description));
    return item;
}

QString problemsText(const QJsonArray& problems) {
    QStringList lines;
    for (const QJsonValue& value : problems) {
        lines << textOf(value.toObject(), "message");
    }
    return lines.join(QStringLiteral(" "));
}

void addStep(QTreeWidgetItem* parent, QTreeWidget* tree, const QJsonObject& step);

void addInputs(QTreeWidgetItem* owner, QTreeWidget* tree, const QJsonObject& step) {
    const int node = step.value(QStringLiteral("node")).toInt();
    for (const QJsonValue& value : step.value(QStringLiteral("inputs")).toArray()) {
        const QJsonObject input = value.toObject();
        const QString source = textOf(input, "source");
        const QJsonObject nested = input.value(QStringLiteral("node")).toObject();
        QString text = QStringLiteral("%1: %2").arg(textOf(input, "label"), textOf(input, "text"));
        QString hint = QStringLiteral("Entrada do tipo %1. Enter para editar.").arg(textOf(input, "type"));
        QString icon = QStringLiteral("circle");
        if (source == QLatin1String("missing")) {
            text = QStringLiteral("%1: FALTA preencher").arg(textOf(input, "label"));
            icon = QStringLiteral("warning");
            hint += QStringLiteral(" Esta entrada ainda não tem valor.");
        } else if (source == QLatin1String("default")) {
            icon = QStringLiteral("circle");
        } else if (source == QLatin1String("wire")) {
            icon = QStringLiteral("connect");
        }
        QJsonObject row{{"type", QStringLiteral("input")},
                        {"node", node},
                        {"pin", textOf(input, "pin")},
                        {"label", textOf(input, "label")},
                        {"valueType", textOf(input, "type")},
                        {"source", source},
                        {"constant", input.value(QStringLiteral("constant"))},
                        {"nested", nested.value(QStringLiteral("node")).toInt(0)}};
        auto* item = makeItem(owner, tree, text, icon, row, keyOf(QStringLiteral("input"), node, textOf(input, "pin")), hint);
        if (!nested.isEmpty()) {
            // The value node wired here, with its own inputs underneath: edited in place.
            addInputs(item, tree, nested);
            for (const QJsonValue& setting : nested.value(QStringLiteral("settings")).toArray()) {
                const QJsonObject choice = setting.toObject();
                QJsonObject settingRow{{"type", QStringLiteral("setting")},
                                       {"node", nested.value(QStringLiteral("node")).toInt()},
                                       {"name", textOf(choice, "name")},
                                       {"label", textOf(choice, "label")},
                                       {"value", choice.value(QStringLiteral("value"))},
                                       {"choices", choice.value(QStringLiteral("choices"))}};
                makeItem(item, tree, QStringLiteral("%1: %2").arg(textOf(choice, "label"), textOf(choice, "value_label")),
                         QStringLiteral("tune"), settingRow,
                         keyOf(QStringLiteral("setting"), nested.value(QStringLiteral("node")).toInt(), textOf(choice, "name")),
                         QStringLiteral("Opção do nó. Enter para mudar."));
            }
            item->setExpanded(true);
        }
    }
}

void addStep(QTreeWidgetItem* parent, QTreeWidget* tree, const QJsonObject& step) {
    const int node = step.value(QStringLiteral("node")).toInt();
    const QString kind = textOf(step, "class");
    const QJsonArray problems = step.value(QStringLiteral("problems")).toArray();
    QString text = textOf(step, "text");
    QString icon = kind == QLatin1String("flow") ? QStringLiteral("loop") : (kind == QLatin1String("event") ? QStringLiteral("bolt") : QStringLiteral("play"));
    if (kind == QLatin1String("ghost")) {
        icon = QStringLiteral("warning");
    }
    QString description = textOf(step, "summary");
    if (!problems.isEmpty()) {
        description += QStringLiteral(" PROBLEMA: ") + problemsText(problems);
        icon = QStringLiteral("warning");
    }
    QJsonObject row{{"type", kind == QLatin1String("event") ? QStringLiteral("event") : QStringLiteral("step")},
                    {"node", node},
                    {"after", step.value(QStringLiteral("after"))},
                    {"kind", textOf(step, "kind")}};
    auto* item = makeItem(parent, tree, text, icon, row, keyOf(textOf(row, "type"), node, QString()), description.trimmed());
    if (kind != QLatin1String("event")) {
        addInputs(item, tree, step);
        for (const QJsonValue& setting : step.value(QStringLiteral("settings")).toArray()) {
            const QJsonObject choice = setting.toObject();
            QJsonObject settingRow{{"type", QStringLiteral("setting")},
                                   {"node", node},
                                   {"name", textOf(choice, "name")},
                                   {"label", textOf(choice, "label")},
                                   {"value", choice.value(QStringLiteral("value"))},
                                   {"choices", choice.value(QStringLiteral("choices"))}};
            makeItem(item, tree, QStringLiteral("%1: %2").arg(textOf(choice, "label"), textOf(choice, "value_label")),
                     QStringLiteral("tune"), settingRow, keyOf(QStringLiteral("setting"), node, textOf(choice, "name")),
                     QStringLiteral("Opção do nó. Enter para mudar."));
        }
    }
    for (const QJsonValue& value : step.value(QStringLiteral("branches")).toArray()) {
        const QJsonObject branch = value.toObject();
        QJsonObject branchRow{{"type", QStringLiteral("branch")},
                              {"node", node},
                              {"after", branch.value(QStringLiteral("after"))}};
        const QJsonArray inner = branch.value(QStringLiteral("steps")).toArray();
        auto* branchItem = makeItem(item, tree, textOf(branch, "label").toUpper(), QStringLiteral("chevron_right"), branchRow,
                                    keyOf(QStringLiteral("branch"), node, textOf(branch, "pin")),
                                    inner.isEmpty() ? QStringLiteral("Caminho vazio. Insert para adicionar o primeiro nó.")
                                                    : QStringLiteral("Caminho com %1 nó(s).").arg(inner.size()));
        for (const QJsonValue& innerStep : inner) {
            addStep(branchItem, tree, innerStep.toObject());
        }
        branchItem->setExpanded(true);
    }
    for (const QJsonValue& value : step.value(QStringLiteral("steps")).toArray()) {
        addStep(item, tree, value.toObject());
    }
    item->setExpanded(true);
}

}  // namespace

void GraphPanel::renderView(const QJsonObject& view) {
    rendering_ = true;
    const QJsonObject outline = view.value(QStringLiteral("outline")).toObject();
    canUndo_ = view.value(QStringLiteral("can_undo")).toBool();
    canRedo_ = view.value(QStringLiteral("can_redo")).toBool();
    tree_->clear();
    const QJsonArray events = outline.value(QStringLiteral("events")).toArray();
    for (const QJsonValue& value : events) {
        addStep(nullptr, tree_, value.toObject());
    }
    const QJsonArray loose = outline.value(QStringLiteral("loose")).toArray();
    const QJsonArray looseValues = outline.value(QStringLiteral("loose_values")).toArray();
    if (!loose.isEmpty() || !looseValues.isEmpty()) {
        auto* section = makeItem(nullptr, tree_, QStringLiteral("SOLTOS (não rodam)"), QStringLiteral("warning"),
                                 QJsonObject{{"type", QStringLiteral("section")}}, QStringLiteral("section"),
                                 QStringLiteral("Nós que não estão ligados a nenhum evento. Apague ou ligue a um caminho."));
        for (const QJsonValue& chain : loose) {
            for (const QJsonValue& step : chain.toObject().value(QStringLiteral("steps")).toArray()) {
                addStep(section, tree_, step.toObject());
            }
        }
        for (const QJsonValue& step : looseValues) {
            addStep(section, tree_, step.toObject());
        }
        section->setExpanded(true);
    }
    if (events.isEmpty() && loose.isEmpty() && looseValues.isEmpty()) {
        auto* empty = makeItem(nullptr, tree_, QStringLiteral("Grafo vazio. Use o botão Evento para começar (por exemplo, A cada quadro)."),
                               QStringLiteral("circle_dot"), QJsonObject{{"type", QStringLiteral("empty")}}, QStringLiteral("empty"), QString());
        empty->setFlags(Qt::ItemIsEnabled);
    }
    // Back to the row that was selected, by what it is (not by its place).
    QTreeWidgetItem* wanted = nullptr;
    for (QTreeWidgetItemIterator it(tree_); *it; ++it) {
        if ((*it)->data(0, kKeyRole).toString() == selectedKey_) {
            wanted = *it;
            break;
        }
    }
    if (wanted == nullptr && tree_->topLevelItemCount() > 0) {
        wanted = tree_->topLevelItem(0);
    }
    if (wanted != nullptr) {
        tree_->setCurrentItem(wanted);
        const QJsonObject row = wanted->data(0, kRowRole).toJsonObject();
        const QString type = textOf(row, "type");
        if (canvas_ != nullptr && (type == QLatin1String("step") || type == QLatin1String("event"))) {
            canvas_->selectNode(row.value(QStringLiteral("node")).toInt());
        }
    }
    rendering_ = false;
    if (canvas_ != nullptr) {
        canvas_->setGraph(view.value(QStringLiteral("canvas")).toObject());
    }

    diagnostics_->clear();
    int errors = 0;
    for (const QJsonValue& value : outline.value(QStringLiteral("diagnostics")).toArray()) {
        const QJsonObject finding = value.toObject();
        const bool isError = textOf(finding, "severity") == QLatin1String("error");
        errors += isError ? 1 : 0;
        auto* item = new QListWidgetItem(QStringLiteral("%1: %2").arg(isError ? QStringLiteral("Erro") : QStringLiteral("Aviso"), textOf(finding, "message")), diagnostics_);
        item->setIcon(PetuniaIcons::get(isError ? QStringLiteral("warning") : QStringLiteral("circle_dot")));
        item->setData(Qt::UserRole, finding);
    }
    QString state = errors == 0 ? QStringLiteral("%1 — %2 nó(s), nenhum erro. Tudo é salvo na hora.").arg(openName_).arg(outline.value(QStringLiteral("nodes")).toInt())
                                : QStringLiteral("%1 — %2 erro(s): este grafo ainda não roda.").arg(openName_).arg(errors);
    const QString layoutProblem = textOf(view, "layout_problem");
    if (!layoutProblem.isEmpty()) {
        state += QStringLiteral(" ") + layoutProblem;
    }
    status_->setText(state);
    if (codeButton_->isChecked()) {
        showCode();
    }
    updateButtons();
}

void GraphPanel::showMode(const QString& mode) {
    int index = 0;
    if (mode == QLatin1String("code")) {
        index = 1;
    } else if (mode == QLatin1String("canvas")) {
        index = 2;
        if (canvas_ != nullptr) {
            if (!canvasFitted_) {
                canvas_->fitGraph();
                canvasFitted_ = true;
            }
            canvas_->setFocus();
        }
    }
    if (canvasBar_ != nullptr) {
        canvasBar_->setVisible(index == 2);
    }
    views_->setCurrentIndex(index);
    updateButtons();
}

void GraphPanel::setPlaying(bool playing) {
    if (canvas_ != nullptr) {
        canvas_->setPlaying(playing);
    }
    if (!playing && debugRows_ != nullptr) {
        debugRows_->clear();
        debugRows_->hide();
    }
}

// The answer to "por que esta condição falhou?": the condition that decided false, with the values
// it was made of (the generated code records them while playing, N5).
void GraphPanel::showDebug(const QJsonObject& debug) {
    if (canvas_ == nullptr || debugRows_ == nullptr) {
        return;
    }
    canvas_->setMarks(debug);
    debugRows_->clear();
    if (!debug.value(QStringLiteral("playing")).toBool()) {
        debugRows_->hide();
        return;
    }
    QHash<int, QStringList> values;
    QList<int> failed;
    for (const QJsonValue& value : debug.value(QStringLiteral("marks")).toArray()) {
        const QJsonObject mark = value.toObject();
        const int node = mark.value(QStringLiteral("node")).toInt();
        const QString kind = textOf(mark, "kind");
        if (kind == QLatin1String("value")) {
            values[node].append(textOf(mark, "text"));
        } else if (kind == QLatin1String("check") && !mark.value(QStringLiteral("value")).toBool()
                   && !failed.contains(node)) {
            failed.append(node);
        }
    }
    if (failed.isEmpty()) {
        auto* item = new QListWidgetItem(PetuniaIcons::get(QStringLiteral("circle_dot")),
                                         QStringLiteral("Nenhuma condição deu falso neste quadro."), debugRows_);
        item->setFlags(Qt::ItemIsEnabled);
        debugRows_->show();
        return;
    }
    for (int node : std::as_const(failed)) {
        const QStringList seen = values.value(node);
        const QString text = seen.isEmpty()
            ? QStringLiteral("%1 (nó %2): a condição deu FALSO; não há valores ligados direto nela.")
                  .arg(canvas_->titleOf(node)).arg(node)
            : QStringLiteral("%1 (nó %2): a condição deu FALSO. Valores lidos: %3.")
                  .arg(canvas_->titleOf(node)).arg(node).arg(seen.join(QStringLiteral(", ")));
        auto* item = new QListWidgetItem(PetuniaIcons::get(QStringLiteral("warning")), text, debugRows_);
        item->setData(Qt::UserRole, node);
        item->setData(Qt::AccessibleDescriptionRole, text);
    }
    debugRows_->show();
}

void GraphPanel::applyCode() {
    if (openName_.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(
        QStringLiteral("graph.from_code"),
        QJsonObject{{QStringLiteral("text"), code_->toPlainText()}});
    if (!reply.ok) {
        emit message(QStringLiteral("Não foi possível aplicar o código: %1").arg(reply.explanation()));
        return;
    }
    refreshView();
    emit message(QStringLiteral("Código aplicado ao grafo."));
}

void GraphPanel::showCode() {
    const Reply reply = bridge_->call(QStringLiteral("graph.code"));
    const QJsonObject result = reply.result.toObject();
    if (result.value(QStringLiteral("ok")).toBool()) {
        codeLines_ = result.value(QStringLiteral("line_nodes")).toArray();
        code_->setPlainText(textOf(result, "text"));
        return;
    }
    codeLines_ = QJsonArray();
    QStringList lines{QStringLiteral("Ainda não dá para gerar o código. Falta resolver:"), QString()};
    for (const QJsonValue& value : result.value(QStringLiteral("problems")).toArray()) {
        lines << QStringLiteral("- %1").arg(textOf(value.toObject(), "message"));
    }
    code_->setPlainText(lines.join(QLatin1Char('\n')));
}

QJsonObject GraphPanel::currentRow() const {
    const QTreeWidgetItem* item = tree_->currentItem();
    return item != nullptr ? item->data(0, kRowRole).toJsonObject() : QJsonObject();
}

// Where "add after" puts a node for this row: the row's own place in its chain, or the place of the
// closest chain row above it (an input or setting row belongs to its node).
QJsonObject GraphPanel::anchorOf(const QTreeWidgetItem* item) const {
    for (; item != nullptr; item = item->parent()) {
        const QJsonObject row = item->data(0, kRowRole).toJsonObject();
        const QString type = textOf(row, "type");
        if ((type == QLatin1String("step") || type == QLatin1String("event") || type == QLatin1String("branch"))
            && row.value(QStringLiteral("after")).isObject()) {
            return row.value(QStringLiteral("after")).toObject();
        }
    }
    return QJsonObject();
}

void GraphPanel::selectNode(int node) {
    for (QTreeWidgetItemIterator it(tree_); *it; ++it) {
        const QJsonObject row = (*it)->data(0, kRowRole).toJsonObject();
        const QString type = textOf(row, "type");
        if ((type == QLatin1String("step") || type == QLatin1String("event")) && row.value(QStringLiteral("node")).toInt() == node) {
            tree_->setCurrentItem(*it);
            tree_->scrollToItem(*it);
            if (canvas_ != nullptr) {
                canvas_->selectNode(node);
            }
            return;
        }
    }
}

void GraphPanel::updateButtons() {
    const bool open = !openName_.isEmpty();
    const QJsonObject row = currentRow();
    const QString type = textOf(row, "type");
    const int mode = views_->currentIndex();
    const bool inTree = mode != 1;
    addEventButton_->setEnabled(open && inTree);
    addButton_->setEnabled(open && inTree && !anchorOf(tree_->currentItem()).isEmpty());
    editButton_->setEnabled(open && inTree && (type == QLatin1String("input") || type == QLatin1String("setting")));
    removeButton_->setEnabled(open && inTree && (type == QLatin1String("step") || type == QLatin1String("event") || type == QLatin1String("input")));
    undoButton_->setEnabled(open && canUndo_);
    redoButton_->setEnabled(open && canRedo_);
    codeButton_->setEnabled(open);
    asScriptButton_->setEnabled(open);
    attachButton_->setEnabled(!projectKey_.isEmpty() && !selectedName_.isEmpty() && selectedKind_ != QLatin1String("missing"));
    if (zoomInButton_ != nullptr) {
        zoomInButton_->setEnabled(open);
    }
    if (zoomOutButton_ != nullptr) {
        zoomOutButton_->setEnabled(open);
    }
    if (fitButton_ != nullptr) {
        fitButton_->setEnabled(open);
    }
    if (groupButton_ != nullptr) {
        groupButton_->setEnabled(open && canvas_ != nullptr && canvas_->selectedNodes().size() >= 2);
    }
    if (ungroupButton_ != nullptr) {
        ungroupButton_->setEnabled(open && canvas_ != nullptr && canvas_->groupOf(canvas_->currentNode()) != 0);
    }
}

void GraphPanel::addEvent() {
    if (openName_.isEmpty()) {
        return;
    }
    const QString kind = pickNode(this, bridge_, QStringLiteral("Evento: quando isto acontecer…"), QStringLiteral("event"), {});
    if (kind.isEmpty()) {
        return;
    }
    QJsonObject result;
    if (run(QStringLiteral("graph.add"), QJsonObject{{"kind", kind}}, &result)) {
        selectedKey_ = keyOf(QStringLiteral("event"), result.value(QStringLiteral("node")).toInt(), QString());
        refreshView();
        emit message(QStringLiteral("Evento adicionado. Insert adiciona o primeiro passo."));
    }
}

void GraphPanel::addAfter() {
    const QJsonObject anchor = anchorOf(tree_->currentItem());
    if (openName_.isEmpty() || anchor.isEmpty()) {
        return;
    }
    const QString kind = pickNode(this, bridge_, QStringLiteral("Adicionar: fazer isto…"), QStringLiteral("chain"), {});
    if (kind.isEmpty()) {
        return;
    }
    QJsonObject result;
    if (run(QStringLiteral("graph.add"), QJsonObject{{"kind", kind}, {"after", anchor}}, &result)) {
        selectedKey_ = keyOf(QStringLiteral("step"), result.value(QStringLiteral("node")).toInt(), QString());
        refreshView();
        emit message(QStringLiteral("Nó adicionado. Enter nas entradas para preencher."));
    }
}

void GraphPanel::removeCurrent() {
    const QJsonObject row = currentRow();
    const QString type = textOf(row, "type");
    bool done = false;
    if (type == QLatin1String("step") || type == QLatin1String("event")) {
        done = run(QStringLiteral("graph.remove"), QJsonObject{{"node", row.value(QStringLiteral("node"))}});
        selectedKey_.clear();
    } else if (type == QLatin1String("input")) {
        const int nested = row.value(QStringLiteral("nested")).toInt();
        if (nested > 0) {
            done = run(QStringLiteral("graph.remove"), QJsonObject{{"node", nested}});
        } else {
            done = run(QStringLiteral("graph.set"), QJsonObject{{"node", row.value(QStringLiteral("node"))}, {"name", row.value(QStringLiteral("pin"))}, {"value", QJsonValue::Null}});
        }
    }
    if (done) {
        refreshView();
        emit message(QStringLiteral("Removido. Ctrl+Z desfaz."));
    }
}

void GraphPanel::editCurrent() {
    const QJsonObject row = currentRow();
    const QString type = textOf(row, "type");
    const QTreeWidgetItem* item = tree_->currentItem();
    const QPoint where = item != nullptr ? tree_->viewport()->mapToGlobal(tree_->visualItemRect(item).bottomLeft()) : QCursor::pos();
    if (type == QLatin1String("input")) {
        editInput(row, where);
    } else if (type == QLatin1String("setting")) {
        editSetting(row, where);
    }
}

void GraphPanel::editInput(const QJsonObject& row, const QPoint& where) {
    const int node = row.value(QStringLiteral("node")).toInt();
    const QString pin = textOf(row, "pin");
    const QString valueType = textOf(row, "valueType");
    const QJsonObject target{{"node", node}, {"pin", pin}};
    QMenu menu(this);
    menu.setAccessibleName(QStringLiteral("Como preencher %1").arg(textOf(row, "label")));
    const bool entityPin = valueType == QLatin1String("Inteiro")
        && (pin == QLatin1String("entity") || pin == QLatin1String("other") || pin == QLatin1String("target"));
    QAction* thisObject = nullptr;
    QAction* chooseObject = nullptr;
    if (entityPin) {
        thisObject = menu.addAction(QStringLiteral("Este objeto (o dono do evento)"));
        chooseObject = menu.addAction(QStringLiteral("Escolher objeto…"));
        menu.addSeparator();
    }
    QAction* fixed = nullptr;
    if (valueType != QLatin1String("Dicionário") && valueType != QLatin1String("Lista")) {
        fixed = menu.addAction(entityPin ? QStringLiteral("Número do objeto…") : QStringLiteral("Valor fixo…"));
    }
    QMenu* sourcesMenu = menu.addMenu(QStringLiteral("Usar o valor de…"));
    const Reply sourcesReply = bridge_->call(QStringLiteral("graph.sources"), QJsonObject{{"for", target}});
    const QJsonArray sources = sourcesReply.result.toObject().value(QStringLiteral("sources")).toArray();
    for (const QJsonValue& value : sources) {
        const QJsonObject source = value.toObject();
        QAction* action = sourcesMenu->addAction(QStringLiteral("%1 (%2)").arg(textOf(source, "label"), textOf(source, "type")));
        action->setData(source.value(QStringLiteral("from")).toObject());
    }
    sourcesMenu->setEnabled(!sources.isEmpty());
    if (sources.isEmpty()) {
        sourcesMenu->setTitle(QStringLiteral("Usar o valor de… (nada disponível aqui)"));
    }
    QAction* newNode = menu.addAction(QStringLiteral("Novo nó que calcula o valor…"));
    menu.addSeparator();
    QAction* clear = menu.addAction(QStringLiteral("Esvaziar"));
    clear->setEnabled(textOf(row, "source") != QLatin1String("missing") && textOf(row, "source") != QLatin1String("default"));
    QAction* chosen = menu.exec(where);
    if (chosen == nullptr) {
        return;
    }
    bool changed = false;
    if (chosen == thisObject) {
        changed = run(QStringLiteral("graph.set"), QJsonObject{{"node", node}, {"name", pin}, {"value", QJsonValue::Null}});
    } else if (chosen == chooseObject) {
        const quint64 current = row.value(QStringLiteral("constant")).isDouble()
            ? static_cast<quint64>(row.value(QStringLiteral("constant")).toDouble())
            : 0;
        const quint64 id = pickEntity(this, bridge_, QStringLiteral("Escolher objeto para %1").arg(textOf(row, "label")), current);
        if (id != 0) {
            changed = run(QStringLiteral("graph.set"), QJsonObject{{"node", node}, {"name", pin}, {"value", static_cast<double>(id)}});
        }
    } else if (chosen == fixed) {
        QVariant value;
        bool ok = false;
        const QString title = QStringLiteral("Valor de %1").arg(textOf(row, "label"));
        if (valueType == QLatin1String("Inteiro")) {
            const int number = QInputDialog::getInt(this, title, QStringLiteral("Número inteiro:"), 0, -2147483647, 2147483647, 1, &ok);
            value = number;
        } else if (valueType == QLatin1String("Decimal")) {
            const double number = QInputDialog::getDouble(this, title, QStringLiteral("Número:"), 0.0, -1.0e9, 1.0e9, 6, &ok);
            value = number;
        } else if (valueType == QLatin1String("Sim/Não")) {
            const QString answer = QInputDialog::getItem(this, title, QStringLiteral("Valor:"), {QStringLiteral("Sim"), QStringLiteral("Não")}, 0, false, &ok);
            value = answer == QLatin1String("Sim");
        } else {
            const QString text = QInputDialog::getText(this, title, QStringLiteral("Texto (ou número, true/false, none):"), QLineEdit::Normal, QString(), &ok);
            bool numeric = false;
            const qint64 whole = text.toLongLong(&numeric);
            if (valueType == QLatin1String("Qualquer") && numeric) {
                value = whole;
            } else if (valueType == QLatin1String("Qualquer") && (text == QLatin1String("true") || text == QLatin1String("false"))) {
                value = text == QLatin1String("true");
            } else {
                value = text;
            }
        }
        if (ok) {
            changed = run(QStringLiteral("graph.set"), QJsonObject{{"node", node}, {"name", pin}, {"value", QJsonValue::fromVariant(value)}});
        }
    } else if (chosen == newNode) {
        const QString kind = pickNode(this, bridge_, QStringLiteral("Novo nó para %1").arg(textOf(row, "label")), QStringLiteral("value"), target);
        if (!kind.isEmpty()) {
            changed = run(QStringLiteral("graph.add"), QJsonObject{{"kind", kind}, {"into", target}});
        }
    } else if (chosen == clear) {
        if (row.value(QStringLiteral("nested")).toInt() > 0) {
            changed = run(QStringLiteral("graph.remove"), QJsonObject{{"node", row.value(QStringLiteral("nested"))}});
        } else if (textOf(row, "source") == QLatin1String("wire")) {
            changed = run(QStringLiteral("graph.disconnect"), QJsonObject{{"pin", target}});
        } else {
            changed = run(QStringLiteral("graph.set"), QJsonObject{{"node", node}, {"name", pin}, {"value", QJsonValue::Null}});
        }
    } else if (chosen->data().isValid() && chosen->data().canConvert<QVariantMap>()) {
        changed = run(QStringLiteral("graph.connect"), QJsonObject{{"from", QJsonObject::fromVariantMap(chosen->data().toMap())}, {"to", target}});
    }
    if (changed) {
        refreshView();
    }
}

void GraphPanel::editSetting(const QJsonObject& row, const QPoint& where) {
    const int node = row.value(QStringLiteral("node")).toInt();
    const QString name = textOf(row, "name");
    const QJsonArray choices = row.value(QStringLiteral("choices")).toArray();
    bool changed = false;
    if (!choices.isEmpty()) {
        QMenu menu(this);
        menu.setAccessibleName(textOf(row, "label"));
        for (const QJsonValue& value : choices) {
            const QJsonObject choice = value.toObject();
            QAction* action = menu.addAction(textOf(choice, "label"));
            action->setData(textOf(choice, "value"));
            action->setCheckable(true);
            action->setChecked(textOf(choice, "value") == row.value(QStringLiteral("value")).toString());
        }
        QAction* chosen = menu.exec(where);
        if (chosen != nullptr) {
            changed = run(QStringLiteral("graph.set"), QJsonObject{{"node", node}, {"name", name}, {"value", chosen->data().toString()}});
        }
    } else {
        bool ok = false;
        const QString text = QInputDialog::getText(this, textOf(row, "label"), QStringLiteral("%1:").arg(textOf(row, "label")), QLineEdit::Normal,
                                                   row.value(QStringLiteral("value")).toString(), &ok);
        if (ok) {
            changed = run(QStringLiteral("graph.set"), QJsonObject{{"node", node}, {"name", name}, {"value", text}});
        }
    }
    if (changed) {
        refreshView();
    }
}

void GraphPanel::undo() {
    if (!openName_.isEmpty() && canUndo_ && run(QStringLiteral("graph.undo"), {})) {
        refreshView();
    }
}

void GraphPanel::redo() {
    if (!openName_.isEmpty() && canRedo_ && run(QStringLiteral("graph.redo"), {})) {
        refreshView();
    }
}

void GraphPanel::createGraph() {
    bool ok = false;
    const QString name = QInputDialog::getText(this, QStringLiteral("Novo grafo"),
                                               QStringLiteral("Nome (letras, números, _ e -, começando por uma letra):"), QLineEdit::Normal, QString(), &ok);
    if (!ok || name.trimmed().isEmpty()) {
        return;
    }
    QJsonObject view;
    if (run(QStringLiteral("graph.new"), QJsonObject{{"name", name.trimmed()}}, &view)) {
        openName_ = name.trimmed();
        selectedName_ = name.trimmed();
        selectedKind_ = QStringLiteral("graph");
        selectedKey_.clear();
        reloadList(openName_);
        renderView(view);
        canvasFitted_ = false;
        if (canvas_ != nullptr && views_->currentIndex() == 2) {
            canvas_->fitGraph();
            canvasFitted_ = true;
        }
        emit message(QStringLiteral("Grafo criado. Use o botão Evento para começar."));
    }
}

void GraphPanel::attach() {
    const QString name = !selectedName_.isEmpty() ? selectedName_ : openName_;
    if (name.isEmpty() || selectedKind_ == QLatin1String("missing")) {
        return;
    }
    QJsonObject result;
    if (run(QStringLiteral("script.attach"), QJsonObject{{"name", name}}, &result)) {
        bridge_->refresh();
        reloadList(openName_);
        emit message(QStringLiteral("%1 foi ligado ao objeto selecionado.").arg(name));
    }
}

void GraphPanel::asScript() {
    if (openName_.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("graph.code"));
    const QJsonObject result = reply.result.toObject();
    if (!result.value(QStringLiteral("ok")).toBool()) {
        emit message(QStringLiteral("O grafo ainda tem problemas; resolva-os antes de transformar em script."));
        return;
    }
    bool ok = false;
    const QString suggestion = openName_ + QStringLiteral("_script");
    const QString name = QInputDialog::getText(this, QStringLiteral("Transformar em script"),
                                               QStringLiteral("Nome do novo script .aipo:"), QLineEdit::Normal,
                                               suggestion, &ok);
    if (!ok || name.trimmed().isEmpty()) {
        return;
    }
    const Reply saved = bridge_->call(
        QStringLiteral("script.write"),
        QJsonObject{{"name", name.trimmed()}, {"text", result.value(QStringLiteral("text"))}});
    if (!saved.ok) {
        emit message(QStringLiteral("Não foi possível criar o script %1: %2").arg(name.trimmed(), saved.message));
        return;
    }
    emit message(QStringLiteral("Script %1 criado a partir do grafo; ele agora é independente.").arg(name.trimmed()));
    emit scriptOpenRequested(name.trimmed());
}

void GraphPanel::renameSelected() {
    if (selectedName_.isEmpty()) {
        return;
    }
    bool ok = false;
    const QString name = QInputDialog::getText(this, QStringLiteral("Renomear"),
                                               QStringLiteral("Novo nome (letras, números, _ e -):"),
                                               QLineEdit::Normal, selectedName_, &ok)
                             .trimmed();
    if (!ok || name.isEmpty() || name == selectedName_) {
        return;
    }
    const QString old = selectedName_;
    const QString kind = selectedKind_;
    const Reply reply = bridge_->call(
        QStringLiteral("script.rename"),
        QJsonObject{{"name", old}, {"new", name}, {"kind", kind}});
    if (!reply.ok) {
        QMessageBox::warning(this, QStringLiteral("Não foi possível"), reply.explanation());
        return;
    }
    if (kind == QLatin1String("graph") && openName_ == old) {
        openName_ = name;
        selectedName_ = name;
        refreshView();
    }
    reloadList(openName_);
    emit message(QStringLiteral("%1 renomeado para %2.").arg(old, name));
}

void GraphPanel::deleteSelected() {
    if (selectedName_.isEmpty()) {
        return;
    }
    const QString name = selectedName_;
    const QString kind = selectedKind_;
    const QString question = QStringLiteral("Excluir \"%1\"?\n\nUma cópia vai para scripts/.backup/ e os objetos que usam %1 ficarão sem ele.")
                                 .arg(name);
    if (QMessageBox::question(this, QStringLiteral("Excluir"), question) != QMessageBox::Yes) {
        return;
    }
    const Reply reply = bridge_->call(
        QStringLiteral("script.delete"),
        QJsonObject{{"name", name}, {"kind", kind}, {"force", true}});
    if (!reply.ok) {
        QMessageBox::warning(this, QStringLiteral("Não foi possível"), reply.explanation());
        return;
    }
    if (kind == QLatin1String("graph") && openName_ == name) {
        openName_.clear();
        selectedName_.clear();
        selectedKind_.clear();
        tree_->clear();
        diagnostics_->clear();
        code_->clear();
        status_->clear();
        if (canvas_ != nullptr) {
            canvas_->setGraph(QJsonObject{});
        }
    }
    reloadList(openName_);
    emit message(QStringLiteral("%1 excluído; cópia em scripts/.backup/.").arg(name));
}

void GraphPanel::groupSelection() {
    if (openName_.isEmpty() || canvas_ == nullptr) {
        return;
    }
    const QList<int> chosen = canvas_->selectedNodes();
    if (chosen.size() < 2) {
        emit message(QStringLiteral("Escolha duas ou mais caixas com Ctrl+clique para agrupar."));
        return;
    }
    bool ok = false;
    const QString label = QInputDialog::getText(this, QStringLiteral("Agrupar caixas"),
                                                QStringLiteral("Nome do grupo:"), QLineEdit::Normal,
                                                QStringLiteral("Grupo"), &ok);
    if (!ok || label.trimmed().isEmpty()) {
        return;
    }
    QJsonArray nodes;
    for (int node : chosen) {
        nodes.append(node);
    }
    if (run(QStringLiteral("graph.group"),
            QJsonObject{{"label", label.trimmed()}, {"nodes", nodes}})) {
        refreshView();
        emit message(QStringLiteral("Grupo %1 criado. Arraste o quadro para mover as caixas juntas.").arg(label.trimmed()));
    }
}

void GraphPanel::ungroupSelection() {
    if (openName_.isEmpty() || canvas_ == nullptr) {
        return;
    }
    const int node = canvas_->currentNode();
    const int group = canvas_->groupOf(node);
    if (group == 0) {
        emit message(QStringLiteral("A caixa escolhida não está em um grupo."));
        return;
    }
    if (run(QStringLiteral("graph.ungroup"), QJsonObject{{"group", group}})) {
        refreshView();
        emit message(QStringLiteral("Grupo desfeito."));
    }
}

void GraphPanel::goToDiagnostic(const QJsonObject& diagnostic) {
    const QJsonValue node = diagnostic.value(QStringLiteral("node"));
    if (node.isDouble()) {
        views_->setCurrentIndex(0);
        codeButton_->setChecked(false);
        selectNode(node.toInt());
        tree_->setFocus();
    }
}

void GraphPanel::contextMenu(const QPoint& position) {
    QTreeWidgetItem* item = tree_->itemAt(position);
    if (item != nullptr) {
        tree_->setCurrentItem(item);
    }
    QMenu menu(this);
    QAction* edit = menu.addAction(QStringLiteral("Editar (Enter)"));
    edit->setEnabled(editButton_->isEnabled());
    QAction* add = menu.addAction(QStringLiteral("Adicionar depois (Insert)"));
    add->setEnabled(addButton_->isEnabled());
    QAction* remove = menu.addAction(QStringLiteral("Remover (Delete)"));
    remove->setEnabled(removeButton_->isEnabled());
    QAction* chosen = menu.exec(tree_->viewport()->mapToGlobal(position));
    if (chosen == edit) {
        editCurrent();
    } else if (chosen == add) {
        addAfter();
    } else if (chosen == remove) {
        removeCurrent();
    }
}
