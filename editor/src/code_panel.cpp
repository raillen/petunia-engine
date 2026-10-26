#include "code_panel.h"

#include <QComboBox>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QShortcut>
#include <QSignalBlocker>
#include <QSpinBox>
#include <QSplitter>
#include <QTabWidget>
#include <QTimer>
#include <QToolButton>
#include <QVBoxLayout>

#include "code_editor.h"
#include "engine_bridge.h"
#include "petunia_icons.h"
#include "petunia_theme.h"
#include "wheel_guard.h"

namespace {

constexpr int kCheckDelayMilliseconds = 450;
constexpr int kDefaultFrames = 60;

QList<CodeMarker> markersFrom(const QJsonArray& diagnostics) {
    QList<CodeMarker> markers;
    for (const QJsonValue& value : diagnostics) {
        const QJsonObject item = value.toObject();
        CodeMarker marker;
        marker.line = item.value(QStringLiteral("line")).toInt(1);
        marker.column = item.value(QStringLiteral("column")).toInt(1);
        marker.endLine = item.value(QStringLiteral("end_line")).toInt(marker.line);
        marker.endColumn = item.value(QStringLiteral("end_column")).toInt(marker.column);
        marker.severity = item.value(QStringLiteral("severity")).toString();
        marker.code = item.value(QStringLiteral("code")).toString();
        marker.message = item.value(QStringLiteral("message")).toString();
        markers.append(marker);
    }
    return markers;
}

QString severityLabel(const QString& severity) {
    if (severity == QLatin1String("error")) {
        return QStringLiteral("Erro");
    }
    return severity == QLatin1String("warning") ? QStringLiteral("Aviso") : QStringLiteral("Info");
}

}  // namespace

CodePanel::CodePanel(EngineBridge* bridge, QWidget* parent) : QWidget(parent), bridge_(bridge) {
    setAccessibleName(QStringLiteral("Editor de código"));

    // A workspace page: the scripts on the left, the text in the middle, the output below it.
    auto* outer = new QHBoxLayout(this);
    outer->setContentsMargins(0, 0, 0, 0);
    outer->setSpacing(0);
    auto* columns = new QSplitter(Qt::Horizontal, this);
    columns->setHandleWidth(PetuniaTheme::instance().scaled(5));
    columns->setChildrenCollapsible(false);
    outer->addWidget(columns);

    auto* sidebar = new QWidget(columns);
    sidebar->setObjectName(QStringLiteral("PetuniaWorkspacePage"));
    sidebar->setAttribute(Qt::WA_StyledBackground);
    auto* sideLayout = new QVBoxLayout(sidebar);
    sideLayout->setContentsMargins(6, 6, 6, 6);
    sideLayout->setSpacing(4);
    auto* sideHeader = new QHBoxLayout;
    auto* sideTitle = new QLabel(QStringLiteral("Scripts"), sidebar);
    sideTitle->setProperty("role", QStringLiteral("panelTitle"));
    sideHeader->addWidget(sideTitle, 1);
    auto* newButton = new QToolButton(sidebar);
    newButton->setProperty("role", QStringLiteral("panelButton"));
    newButton->setProperty("iconName", QStringLiteral("plus"));
    newButton->setIcon(PetuniaIcons::get(QStringLiteral("plus")));
    newButton->setToolTip(QStringLiteral("Novo script a partir do modelo"));
    newButton->setAccessibleName(QStringLiteral("Novo script"));
    sideHeader->addWidget(newButton);
    sideLayout->addLayout(sideHeader);
    scripts_ = new QListWidget(sidebar);
    scripts_->setAccessibleName(QStringLiteral("Scripts do projeto"));
    scripts_->setToolTip(QStringLiteral("Scripts da pasta scripts/ do projeto"));
    scripts_->setFrameShape(QFrame::NoFrame);
    sideLayout->addWidget(scripts_, 1);
    noScripts_ = new QLabel(QStringLiteral("Nenhum script ainda.\nO botão + cria um a partir do modelo; depois, \"Ligar ao objeto\" faz ele rodar no jogo."), sidebar);
    noScripts_->setWordWrap(true);
    noScripts_->setProperty("role", QStringLiteral("muted"));
    noScripts_->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    noScripts_->hide();
    sideLayout->addWidget(noScripts_, 1);
    sidebar->setMinimumWidth(150);

    auto* page = new QWidget(columns);
    page->setObjectName(QStringLiteral("PetuniaWorkspacePage"));
    page->setAttribute(Qt::WA_StyledBackground);
    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(0, 0, 0, 6);
    layout->setSpacing(0);
    columns->addWidget(sidebar);
    columns->addWidget(page);
    columns->setStretchFactor(1, 1);
    columns->setSizes({200, 800});

    // Action bar: save and format, then try it, then attach. Icons with short words.
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
    saveButton_ = makeButton(QStringLiteral("Salvar"), QStringLiteral("save"),
                             QStringLiteral("Grava o script (Ctrl+S dentro do editor). A versão anterior vai para scripts/.backup/"));
    formatButton_ = makeButton(QStringLiteral("Formatar"), QStringLiteral("sort_az"),
                               QStringLiteral("Arruma o texto como o `aipo fmt` (Ctrl+Shift+F). Só funciona se o texto compila na sintaxe"));
    auto* line = new QFrame(barWidget);
    line->setProperty("role", QStringLiteral("vrule"));
    line->setFixedWidth(1);
    bar->addWidget(line);
    runButton_ = makeButton(QStringLiteral("Executar"), QStringLiteral("play"),
                            QStringLiteral("Roda o script numa CÓPIA do mundo (F5): a cena não muda. Mostra o console e o que mudaria"));
    frames_ = new QSpinBox;
    frames_->setRange(1, 3600);
    frames_->setValue(kDefaultFrames);
    frames_->setSuffix(QStringLiteral(" quadros"));
    frames_->setAccessibleName(QStringLiteral("Quadros a simular"));
    frames_->setToolTip(QStringLiteral("Quantos quadros (1/60 s cada) a execução de teste simula"));
    bar->addWidget(frames_);
    bar->addStretch(1);
    attachButton_ = makeButton(QStringLiteral("Ligar ao objeto"), QStringLiteral("link"),
                               QStringLiteral("Liga este script ao objeto selecionado na cena (tabela Script)"));
    attachButton_->setAccessibleName(QStringLiteral("Ligar o script ao objeto selecionado"));
    barWidget->setMinimumWidth(1);
    layout->addWidget(barWidget);
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, [this] {
        for (QToolButton* button : findChildren<QToolButton*>()) {
            const QString icon = button->property("iconName").toString();
            if (!icon.isEmpty()) {
                button->setIcon(PetuniaIcons::get(icon));
            }
        }
    });

    auto* splitter = new QSplitter(Qt::Vertical);
    editor_ = new CodeEdit;
    splitter->addWidget(editor_);
    output_ = new QTabWidget;
    output_->setAccessibleName(QStringLiteral("Saída do editor de código"));
    problems_ = new QListWidget;
    problems_->setAccessibleName(QStringLiteral("Problemas do script"));
    problems_->setToolTip(QStringLiteral("Clique duas vezes para ir à linha"));
    console_ = new QPlainTextEdit;
    console_->setReadOnly(true);
    console_->setAccessibleName(QStringLiteral("Console do script"));
    console_->setMaximumBlockCount(2000);
    output_->addTab(problems_, QStringLiteral("Problemas"));
    output_->addTab(console_, QStringLiteral("Console"));
    splitter->addWidget(output_);
    // The script's state (saved, unsaved, last message) on the free side of the output tabs.
    status_ = new QLabel;
    status_->setAccessibleName(QStringLiteral("Estado do script"));
    status_->setProperty("role", QStringLiteral("caption"));
    status_->setContentsMargins(8, 0, 8, 0);
    output_->setCornerWidget(status_, Qt::TopRightCorner);
    splitter->setStretchFactor(0, 4);
    splitter->setStretchFactor(1, 1);
    // The text gets the room; problems and console stay a strip below it.
    output_->setMaximumHeight(180);
    // A panel that is only tabbed behind the others must not make the bottom area taller: explicit
    // small minimums override the children's size hints (the dock is as tall as its tallest tab).
    editor_->setMinimumHeight(40);
    output_->setMinimumHeight(1);
    problems_->setMinimumHeight(1);
    console_->setMinimumHeight(1);
    layout->addWidget(splitter, 1);

    hint_ = new QLabel;
    hint_->setWordWrap(true);
    hint_->setAccessibleName(QStringLiteral("Ajuda da sugestão"));
    hint_->setMinimumHeight(1);
    hint_->setProperty("role", QStringLiteral("caption"));
    hint_->setContentsMargins(8, 2, 8, 0);
    layout->addWidget(hint_);

    checkTimer_ = new QTimer(this);
    checkTimer_->setSingleShot(true);
    checkTimer_->setInterval(kCheckDelayMilliseconds);
    connect(checkTimer_, &QTimer::timeout, this, &CodePanel::check);

    connect(bridge_, &EngineBridge::snapshotChanged, this, &CodePanel::onSnapshot);
    connect(scripts_, &QListWidget::currentItemChanged, this, [this](QListWidgetItem* item) {
        if (item == nullptr) {
            return;
        }
        const QString name = item->data(Qt::UserRole).toString();
        if (name == current_) {
            return;
        }
        if (!maybeSave()) {
            const QSignalBlocker blocker(scripts_);
            scripts_->setCurrentRow(rowOf(current_));
            return;
        }
        load(name);
    });
    connect(newButton, &QToolButton::clicked, this, &CodePanel::create);
    connect(saveButton_, &QToolButton::clicked, this, &CodePanel::save);
    connect(formatButton_, &QToolButton::clicked, this, &CodePanel::format);
    connect(runButton_, &QToolButton::clicked, this, &CodePanel::run);
    connect(attachButton_, &QToolButton::clicked, this, &CodePanel::attach);
    connect(editor_, &CodeEdit::saveRequested, this, &CodePanel::save);
    connect(editor_, &CodeEdit::hintChanged, hint_, &QLabel::setText);
    connect(editor_, &QPlainTextEdit::textChanged, this, [this] {
        if (loading_ || current_.isEmpty()) {
            return;
        }
        setDirty(true);
        scheduleCheck();
    });
    connect(problems_, &QListWidget::itemActivated, this, [this](QListWidgetItem* item) {
        editor_->goToLine(item->data(Qt::UserRole).toInt(), item->data(Qt::UserRole + 1).toInt());
    });

    auto* runShortcut = new QShortcut(QKeySequence(Qt::Key_F5), this);
    runShortcut->setContext(Qt::WidgetWithChildrenShortcut);
    connect(runShortcut, &QShortcut::activated, this, &CodePanel::run);
    auto* formatShortcut = new QShortcut(QKeySequence(Qt::CTRL | Qt::SHIFT | Qt::Key_F), this);
    formatShortcut->setContext(Qt::WidgetWithChildrenShortcut);
    connect(formatShortcut, &QShortcut::activated, this, &CodePanel::format);

    editor_->setEnabled(false);
    onSnapshot();
}

void CodePanel::onSnapshot() {
    const QJsonValue project = bridge_->snapshot().value(QStringLiteral("project"));
    const QString key = project.isObject() ? project.toObject().value(QStringLiteral("root")).toString() : QString();
    for (QWidget* widget : std::initializer_list<QWidget*>{scripts_, saveButton_, formatButton_, runButton_, attachButton_}) {
        widget->setEnabled(!key.isEmpty());
    }
    if (key == projectKey_) {
        return;
    }
    projectKey_ = key;
    current_.clear();
    {
        const QSignalBlocker blocker(editor_);
        loading_ = true;
        editor_->clear();
        loading_ = false;
    }
    editor_->setMarkers({});
    problems_->clear();
    setDirty(false);
    editor_->setEnabled(false);
    if (key.isEmpty()) {
        scripts_->clear();
        updateTitle();
        return;
    }
    const Reply api = bridge_->call(QStringLiteral("script.api"));
    if (api.ok) {
        editor_->setApi(api.result.toObject());
    }
    reloadList();
}

void CodePanel::reloadList(const QString& select) {
    const Reply reply = bridge_->call(QStringLiteral("script.list"));
    const QSignalBlocker blocker(scripts_);
    scripts_->clear();
    if (!reply.ok) {
        say(reply.message);
        return;
    }
    for (const QJsonValue& value : reply.result.toObject().value(QStringLiteral("scripts")).toArray()) {
        const QJsonObject item = value.toObject();
        const QString name = item.value(QStringLiteral("name")).toString();
        const int errors = item.value(QStringLiteral("errors")).toInt();
        const int attached = item.value(QStringLiteral("attached")).toInt();
        auto* row = new QListWidgetItem(PetuniaIcons::get(errors > 0 ? QStringLiteral("warning") : QStringLiteral("script")), name, scripts_);
        row->setData(Qt::UserRole, name);
        QStringList facts;
        if (errors > 0) {
            facts << QStringLiteral("%1 erro%2").arg(errors).arg(errors == 1 ? "" : "s");
        }
        facts << (attached > 0 ? QStringLiteral("ligado a %1 objeto%2").arg(attached).arg(attached == 1 ? "" : "s")
                               : QStringLiteral("não ligado a nenhum objeto"));
        row->setToolTip(QStringLiteral("%1.aipo — %2").arg(name, facts.join(QStringLiteral(", "))));
        row->setData(Qt::AccessibleTextRole, QStringLiteral("%1, %2").arg(name, facts.join(QStringLiteral(", "))));
    }
    scripts_->setVisible(scripts_->count() > 0);
    noScripts_->setVisible(scripts_->count() == 0);
    const QString want = select.isEmpty() ? current_ : select;
    const int index = rowOf(want);
    if (index >= 0) {
        scripts_->setCurrentRow(index);
    } else if (scripts_->count() > 0 && current_.isEmpty()) {
        scripts_->setCurrentRow(0);
        const QString first = scripts_->item(0)->data(Qt::UserRole).toString();
        QTimer::singleShot(0, this, [this, first] { load(first); });
    }
}

int CodePanel::rowOf(const QString& name) const {
    for (int row = 0; row < scripts_->count(); ++row) {
        if (scripts_->item(row)->data(Qt::UserRole).toString() == name) {
            return row;
        }
    }
    return -1;
}

void CodePanel::load(const QString& name) {
    const Reply reply = bridge_->call(QStringLiteral("script.read"), QJsonObject{{"name", name}});
    if (!reply.ok) {
        say(reply.message);
        return;
    }
    current_ = name;
    loading_ = true;
    editor_->setPlainText(reply.result.toObject().value(QStringLiteral("text")).toString());
    loading_ = false;
    editor_->document()->clearUndoRedoStacks();
    editor_->setEnabled(true);
    setDirty(false);
    {
        const QSignalBlocker blocker(scripts_);
        scripts_->setCurrentRow(rowOf(name));
    }
    check();
}

void CodePanel::open(const QString& name) {
    if (name == current_ || !maybeSave()) {
        return;
    }
    reloadList(name);
    load(name);
}

void CodePanel::focusEditor() {
    editor_->setFocus();
}

void CodePanel::setDirty(bool dirty) {
    dirty_ = dirty;
    updateTitle();
}

void CodePanel::updateTitle() {
    if (current_.isEmpty()) {
        status_->setText(projectKey_.isEmpty() ? QStringLiteral("Abra um projeto") : QStringLiteral("Crie ou escolha um script"));
        return;
    }
    status_->setText(dirty_ ? QStringLiteral("%1 — alterações não salvas").arg(current_) : QStringLiteral("%1 — salvo").arg(current_));
    status_->setToolTip(status_->text());
}

void CodePanel::say(const QString& message) {
    status_->setText(message);
    status_->setToolTip(message);
}

void CodePanel::console(const QString& line) {
    console_->appendPlainText(line);
}

void CodePanel::scheduleCheck() {
    checkTimer_->start();
}

void CodePanel::check() {
    if (current_.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("script.check"),
                                      QJsonObject{{"name", current_}, {"text", editor_->toPlainText()}});
    if (reply.ok) {
        showProblems(reply.result.toObject().value(QStringLiteral("diagnostics")).toArray());
    }
}

void CodePanel::showProblems(const QJsonArray& diagnostics) {
    editor_->setMarkers(markersFrom(diagnostics));
    problems_->clear();
    int errors = 0;
    for (const QJsonValue& value : diagnostics) {
        const QJsonObject item = value.toObject();
        const int line = item.value(QStringLiteral("line")).toInt(1);
        const int column = item.value(QStringLiteral("column")).toInt(1);
        auto* row = new QListWidgetItem(QStringLiteral("%1 · linha %2, coluna %3 · %4 (%5)")
                                            .arg(severityLabel(item.value(QStringLiteral("severity")).toString()))
                                            .arg(line)
                                            .arg(column)
                                            .arg(item.value(QStringLiteral("message")).toString(),
                                                 item.value(QStringLiteral("code")).toString()));
        row->setData(Qt::UserRole, line);
        row->setData(Qt::UserRole + 1, column);
        problems_->addItem(row);
        errors += item.value(QStringLiteral("severity")).toString() == QLatin1String("error") ? 1 : 0;
    }
    output_->setTabText(0, diagnostics.isEmpty() ? QStringLiteral("Problemas")
                                                 : QStringLiteral("Problemas (%1)").arg(diagnostics.size()));
    if (diagnostics.isEmpty()) {
        problems_->addItem(QStringLiteral("Nenhum problema encontrado."));
    }
    Q_UNUSED(errors);
}

bool CodePanel::maybeSave() {
    if (!dirty_ || current_.isEmpty()) {
        return true;
    }
    QMessageBox box(QMessageBox::Question, QStringLiteral("Script com alterações"),
                    QStringLiteral("O script \"%1\" tem alterações que ainda não foram salvas.").arg(current_),
                    QMessageBox::NoButton, this);
    QPushButton* saveChoice = box.addButton(QStringLiteral("Salvar"), QMessageBox::AcceptRole);
    QPushButton* discard = box.addButton(QStringLiteral("Descartar"), QMessageBox::DestructiveRole);
    box.addButton(QStringLiteral("Cancelar"), QMessageBox::RejectRole);
    box.exec();
    if (box.clickedButton() == saveChoice) {
        save();
        return !dirty_;
    }
    if (box.clickedButton() == discard) {
        setDirty(false);
        return true;
    }
    return false;
}

void CodePanel::save() {
    if (current_.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("script.write"),
                                      QJsonObject{{"name", current_}, {"text", editor_->toPlainText()}});
    if (!reply.ok) {
        say(reply.message);
        console(QStringLiteral("Não salvou: ") + reply.explanation());
        return;
    }
    setDirty(false);
    showProblems(reply.result.toObject().value(QStringLiteral("diagnostics")).toArray());
    bridge_->refresh();  // the Project Doctor's counts follow the file
    reloadList();
}

void CodePanel::format() {
    if (current_.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("script.format"),
                                      QJsonObject{{"name", current_}, {"text", editor_->toPlainText()}});
    if (!reply.ok) {
        say(reply.message);
        output_->setCurrentIndex(0);
        return;
    }
    const QString text = reply.result.toObject().value(QStringLiteral("text")).toString();
    if (text != editor_->toPlainText()) {
        // One undo step: Ctrl+Z gives the old text back.
        QTextCursor cursor = editor_->textCursor();
        cursor.beginEditBlock();
        cursor.select(QTextCursor::Document);
        cursor.insertText(text);
        cursor.endEditBlock();
    }
    say(QStringLiteral("Formatado."));
}

void CodePanel::run() {
    if (current_.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("script.run"), QJsonObject{{"name", current_},
                                                                               {"text", editor_->toPlainText()},
                                                                               {"ticks", frames_->value()}});
    output_->setCurrentIndex(1);
    if (!reply.ok) {
        say(reply.message);
        console(reply.explanation());
        return;
    }
    const QJsonObject result = reply.result.toObject();
    const QJsonArray diagnostics = result.value(QStringLiteral("diagnostics")).toArray();
    console(QStringLiteral("— Execução de teste de \"%1\" (numa cópia do mundo) —").arg(current_));
    if (!diagnostics.isEmpty()) {
        showProblems(diagnostics);
        console(QStringLiteral("Não deu para executar: o script tem erros (veja Problemas)."));
        output_->setCurrentIndex(0);
        return;
    }
    for (const QJsonValue& value : result.value(QStringLiteral("log")).toArray()) {
        const QJsonObject line = value.toObject();
        console(QStringLiteral("[%1] quadro %2 · %3")
                    .arg(line.value(QStringLiteral("level")).toString())
                    .arg(line.value(QStringLiteral("tick")).toInt())
                    .arg(line.value(QStringLiteral("text")).toString()));
    }
    for (const QJsonValue& value : result.value(QStringLiteral("faults")).toArray()) {
        const QJsonObject fault = value.toObject();
        console(QStringLiteral("FALHA em %1.%2 (objeto %3): %4 [%5]")
                    .arg(fault.value(QStringLiteral("script")).toString(),
                         fault.value(QStringLiteral("function")).toString())
                    .arg(fault.value(QStringLiteral("entity")).toInt())
                    .arg(fault.value(QStringLiteral("message")).toString(),
                         fault.value(QStringLiteral("code")).toString()));
    }
    const QString summary = QStringLiteral("%1 quadros · %2 comandos · %3 objetos alterados, %4 criados, %5 removidos. A cena NÃO foi mudada.")
                                .arg(result.value(QStringLiteral("ticks")).toInt())
                                .arg(result.value(QStringLiteral("commands")).toInt())
                                .arg(result.value(QStringLiteral("changed")).toInt())
                                .arg(result.value(QStringLiteral("created")).toInt())
                                .arg(result.value(QStringLiteral("removed")).toInt());
    console(summary);
    say(result.value(QStringLiteral("ok")).toBool() ? QStringLiteral("Execução de teste terminou sem falhas.")
                                                    : QStringLiteral("Execução de teste teve falhas (veja o Console)."));
    check();
}

void CodePanel::create() {
    if (!maybeSave()) {
        return;
    }
    bool accepted = false;
    const QString name = QInputDialog::getText(this, QStringLiteral("Novo script"),
                                               QStringLiteral("Nome do script (letras, números, _ e -):"),
                                               QLineEdit::Normal, QString(), &accepted)
                             .trimmed();
    if (!accepted || name.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("script.new"), QJsonObject{{"name", name}});
    if (!reply.ok) {
        QMessageBox::warning(this, QStringLiteral("Não foi possível"), reply.explanation());
        return;
    }
    reloadList(name);
    load(name);
    bridge_->refresh();
    editor_->setFocus();
}

void CodePanel::attach() {
    if (current_.isEmpty()) {
        return;
    }
    if (!maybeSave()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("script.attach"), QJsonObject{{"name", current_}});
    if (!reply.ok) {
        say(reply.message);
        QMessageBox::warning(this, QStringLiteral("Não foi possível"), reply.explanation());
        return;
    }
    say(QStringLiteral("\"%1\" ligado ao objeto selecionado. Ctrl+Z desfaz.").arg(current_));
    bridge_->refresh();
    reloadList();
}
