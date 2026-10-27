#include "graph_panel.h"

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

#include "engine_bridge.h"
#include "petunia_icons.h"
#include "petunia_theme.h"

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
    auto* sideTitle = new QLabel(QStringLiteral("Grafos"), sidebar);
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
    graphs_->setAccessibleName(QStringLiteral("Grafos do projeto"));
    graphs_->setToolTip(QStringLiteral("Grafos da pasta scripts/ do projeto (arquivos .graph.json)"));
    graphs_->setFrameShape(QFrame::NoFrame);
    graphs_->setIconSize(QSize(16, 16));
    sideLayout->addWidget(graphs_, 1);
    noGraphs_ = new QLabel(QStringLiteral("Nenhum grafo ainda.\nO botão + cria um. Depois, \"Ligar ao objeto\" faz ele rodar no jogo."), sidebar);
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
    bar->addStretch(1);
    codeButton_ = makeButton(QStringLiteral("Ver como código"), QStringLiteral("terminal"),
                             QStringLiteral("Mostra o texto Aipo que este grafo gera. Só leitura: editar o grafo é pelas linhas"));
    codeButton_->setCheckable(true);
    attachButton_ = makeButton(QStringLiteral("Ligar ao objeto"), QStringLiteral("link"),
                               QStringLiteral("Liga este grafo ao objeto selecionado na cena (tabela Script)"));
    attachButton_->setAccessibleName(QStringLiteral("Ligar o grafo ao objeto selecionado"));
    barWidget->setMinimumWidth(1);
    layout->addWidget(barWidget);

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
    code_ = new QPlainTextEdit;
    code_->setReadOnly(true);
    code_->setAccessibleName(QStringLiteral("Código gerado pelo grafo"));
    code_->setLineWrapMode(QPlainTextEdit::NoWrap);
    views_->addWidget(code_);
    splitter->addWidget(views_);
    diagnostics_ = new QListWidget;
    diagnostics_->setAccessibleName(QStringLiteral("Problemas do grafo"));
    diagnostics_->setToolTip(QStringLiteral("Enter vai até o nó do problema"));
    diagnostics_->setIconSize(QSize(16, 16));
    diagnostics_->setMaximumHeight(150);
    diagnostics_->setMinimumHeight(1);
    splitter->addWidget(diagnostics_);
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
        if (item != nullptr && item->data(Qt::UserRole).toString() != openName_) {
            openGraph(item->data(Qt::UserRole).toString());
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
    connect(codeButton_, &QToolButton::toggled, this, [this](bool on) {
        views_->setCurrentIndex(on ? 1 : 0);
        if (on) {
            showCode();
        }
        updateButtons();
    });
    connect(tree_, &QTreeWidget::itemActivated, this, [this] { editCurrent(); });
    connect(tree_, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* item) {
        if (!rendering_) {
            selectedKey_ = item != nullptr ? item->data(0, kKeyRole).toString() : QString();
        }
        updateButtons();
    });
    connect(tree_, &QTreeWidget::customContextMenuRequested, this, &GraphPanel::contextMenu);
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
    QJsonObject listing;
    const Reply reply = bridge_->call(QStringLiteral("graph.list"));
    if (!reply.ok) {
        return;
    }
    listing = reply.result.toObject();
    const QSignalBlocker blocker(graphs_);
    graphs_->clear();
    for (const QJsonValue& value : listing.value(QStringLiteral("graphs")).toArray()) {
        auto* item = new QListWidgetItem(PetuniaIcons::get(QStringLiteral("connect")), value.toString(), graphs_);
        item->setData(Qt::UserRole, value.toString());
        if (value.toString() == select) {
            graphs_->setCurrentItem(item);
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
    renderView(view);
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
    }
    rendering_ = false;

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
    status_->setText(errors == 0 ? QStringLiteral("%1 — %2 nó(s), nenhum erro. Tudo é salvo na hora.").arg(openName_).arg(outline.value(QStringLiteral("nodes")).toInt())
                                 : QStringLiteral("%1 — %2 erro(s): este grafo ainda não roda.").arg(openName_).arg(errors));
    if (codeButton_->isChecked()) {
        showCode();
    }
    updateButtons();
}

void GraphPanel::showCode() {
    const Reply reply = bridge_->call(QStringLiteral("graph.code"));
    const QJsonObject result = reply.result.toObject();
    if (result.value(QStringLiteral("ok")).toBool()) {
        code_->setPlainText(textOf(result, "text"));
        return;
    }
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
            return;
        }
    }
}

void GraphPanel::updateButtons() {
    const bool open = !openName_.isEmpty();
    const QJsonObject row = currentRow();
    const QString type = textOf(row, "type");
    const bool inTree = views_->currentIndex() == 0;
    addEventButton_->setEnabled(open && inTree);
    addButton_->setEnabled(open && inTree && !anchorOf(tree_->currentItem()).isEmpty());
    editButton_->setEnabled(open && inTree && (type == QLatin1String("input") || type == QLatin1String("setting")));
    removeButton_->setEnabled(open && inTree && (type == QLatin1String("step") || type == QLatin1String("event") || type == QLatin1String("input")));
    undoButton_->setEnabled(open && canUndo_);
    redoButton_->setEnabled(open && canRedo_);
    codeButton_->setEnabled(open);
    attachButton_->setEnabled(open && !projectKey_.isEmpty());
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
    QAction* fixed = nullptr;
    if (valueType != QLatin1String("Dicionário") && valueType != QLatin1String("Lista")) {
        fixed = menu.addAction(QStringLiteral("Valor fixo…"));
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
    if (chosen == fixed) {
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
        selectedKey_.clear();
        reloadList(openName_);
        renderView(view);
        emit message(QStringLiteral("Grafo criado. Use o botão Evento para começar."));
    }
}

void GraphPanel::attach() {
    if (openName_.isEmpty()) {
        return;
    }
    QJsonObject result;
    if (run(QStringLiteral("script.attach"), QJsonObject{{"name", openName_}}, &result)) {
        bridge_->refresh();
        emit message(QStringLiteral("O grafo %1 foi ligado ao objeto selecionado.").arg(openName_));
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
