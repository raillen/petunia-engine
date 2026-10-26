#include "info_panels.h"

#include <QHash>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonObject>
#include <QListWidget>
#include <QVBoxLayout>

#include "engine_bridge.h"

namespace {

QString severityLabel(const QString& severity) {
    static const QHash<QString, QString> names = {
        {"error", "Erro"}, {"warning", "Aviso"}, {"info", "Informação"}};
    return names.value(severity, severity);
}

QString statusLabel(const QString& status) {
    static const QHash<QString, QString> names = {{"verified", "verificado"},
                                                  {"provisional", "provisório"},
                                                  {"approximation", "aproximação"},
                                                  {"planned", "planejado"}};
    return names.value(status, status);
}

QString reversibilityLabel(const QString& value) {
    static const QHash<QString, QString> names = {{"undoable", "desfazível"}};
    return names.value(value, value);
}

}  // namespace

// ---- DiagnosticsPanel ---------------------------------------------------------------------

DiagnosticsPanel::DiagnosticsPanel(EngineBridge* bridge, QWidget* parent)
    : QTreeWidget(parent), bridge_(bridge) {
    setAccessibleName(QStringLiteral("Diagnósticos do projeto"));
    setColumnCount(5);
    setHeaderLabels({"Gravidade", "Código", "Alvo", "Problema", "O que fazer"});
    setRootIsDecorated(false);
    setAlternatingRowColors(true);
    setUniformRowHeights(true);
    header()->setSectionResizeMode(QHeaderView::Interactive);
    header()->resizeSection(0, 90);
    header()->resizeSection(1, 220);
    header()->resizeSection(2, 160);
    header()->resizeSection(3, 360);
}

void DiagnosticsPanel::reload() {
    clear();
    const Reply reply = bridge_->call(QStringLiteral("diagnostics.get"));
    const QJsonArray found = reply.result.toObject().value("diagnostics").toArray();
    if (found.isEmpty()) {
        auto* item = new QTreeWidgetItem({QStringLiteral("OK"), QString(), QString(),
                                          bridge_->hasProject()
                                              ? QStringLiteral("Nenhum problema encontrado.")
                                              : QStringLiteral("Abra um projeto para ver os diagnósticos."),
                                          QString()});
        addTopLevelItem(item);
        return;
    }
    for (const QJsonValue& value : found) {
        const QJsonObject entry = value.toObject();
        auto* item = new QTreeWidgetItem({severityLabel(entry.value("severity").toString()),
                                          entry.value("code").toString(),
                                          entry.value("target").toString(),
                                          entry.value("message").toString(),
                                          entry.value("action").toString()});
        for (int column = 0; column < 5; ++column) {
            item->setToolTip(column, entry.value("message").toString() + "\n" + entry.value("action").toString());
        }
        addTopLevelItem(item);
    }
}

// ---- HistoryPanel -------------------------------------------------------------------------

HistoryPanel::HistoryPanel(EngineBridge* bridge, QWidget* parent) : QWidget(parent), bridge_(bridge) {
    list_ = new QListWidget(this);
    list_->setAccessibleName(QStringLiteral("Histórico de ações do editor"));
    list_->setUniformItemSizes(true);
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->addWidget(list_);
}

void HistoryPanel::reload() {
    const Reply reply = bridge_->call(QStringLiteral("history.get"));
    const QJsonArray entries = reply.result.toObject().value("entries").toArray();
    if (entries.size() == shown_) {
        return;
    }
    shown_ = static_cast<int>(entries.size());
    list_->clear();
    for (const QJsonValue& value : entries) {
        const QJsonObject entry = value.toObject();
        const bool ok = entry.value("ok").toBool();
        QString text = QStringLiteral("%1  %2 %3")
                           .arg(entry.value("number").toInt(), 3)
                           .arg(ok ? QStringLiteral("✓") : QStringLiteral("✗"))
                           .arg(entry.value("text").toString());
        const QString detail = entry.value("detail").toString();
        if (!ok && !detail.isEmpty()) {
            text += QStringLiteral("  —  ") + detail;
        }
        auto* item = new QListWidgetItem(text);
        item->setToolTip(detail);
        list_->addItem(item);
    }
    list_->scrollToBottom();
}

// ---- ApiPanel -----------------------------------------------------------------------------

ApiPanel::ApiPanel(EngineBridge* bridge, QWidget* parent) : QTreeWidget(parent), bridge_(bridge) {
    setAccessibleName(QStringLiteral("Referência da API"));
    // Two columns fit a side panel; state, undo and parameters are in each row's tooltip.
    setColumnCount(2);
    setHeaderLabels({"Nome", "Resumo"});
    setIndentation(12);
    header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
    header()->setStretchLastSection(true);
}

void ApiPanel::reload() {
    clear();
    // What a script calls comes first: this panel sits next to the code editor.
    const Reply scriptReply = bridge_->call(QStringLiteral("script.api"));
    const QJsonObject scriptApi = scriptReply.result.toObject();
    QHash<QString, QTreeWidgetItem*> modules;
    for (const QJsonValue& value : scriptApi.value("functions").toArray()) {
        const QJsonObject function = value.toObject();
        const QString module = function.value("module").toString();
        QTreeWidgetItem*& group = modules[module];
        if (group == nullptr) {
            group = new QTreeWidgetItem({QStringLiteral("%1 (script)").arg(module)});
            group->setFirstColumnSpanned(true);
            addTopLevelItem(group);
            group->setExpanded(true);
        }
        auto* item = new QTreeWidgetItem({QStringLiteral("%1.%2").arg(module, function.value("name").toString()),
                                          function.value("summary").toString()});
        item->setToolTip(0, QStringLiteral("%1\nPermissão: %2\nExemplo: %3")
                                .arg(function.value("signature").toString(), function.value("permission").toString(),
                                     function.value("example").toString()));
        item->setToolTip(1, item->toolTip(0));
        group->addChild(item);
    }
    const QJsonArray handlers = scriptApi.value("handlers").toArray();
    if (!handlers.isEmpty()) {
        auto* group = new QTreeWidgetItem({QStringLiteral("Funções que o motor chama")});
        group->setFirstColumnSpanned(true);
        addTopLevelItem(group);
        for (const QJsonValue& value : handlers) {
            const QJsonObject handler = value.toObject();
            auto* item = new QTreeWidgetItem({handler.value("name").toString(), handler.value("help").toString()});
            item->setToolTip(1, handler.value("help").toString());
            group->addChild(item);
        }
        group->setExpanded(true);
    }

    const Reply reply = bridge_->call(QStringLiteral("schema.get"));
    const QJsonObject schema = reply.result.toObject();

    auto addGroup = [this](const QString& title, const QJsonArray& entries, bool command) {
        auto* group = new QTreeWidgetItem({title});
        group->setFirstColumnSpanned(true);
        addTopLevelItem(group);
        for (const QJsonValue& value : entries) {
            const QJsonObject entry = value.toObject();
            auto* item = new QTreeWidgetItem({entry.value("id").toString(), entry.value("summary").toString()});
            const QString facts = QStringLiteral("Estado: %1 · Desfazer: %2")
                                      .arg(statusLabel(entry.value("status").toString()),
                                           command ? reversibilityLabel(entry.value("reversibility").toString()) : QStringLiteral("—"));
            QStringList parameters;
            for (const QJsonValue& param : entry.value("params").toArray()) {
                const QJsonObject parameter = param.toObject();
                parameters << QStringLiteral("%1%2: %3")
                                  .arg(parameter.value("name").toString(),
                                       parameter.value("required").toBool() ? QStringLiteral("*") : QString(),
                                       parameter.value("kind").toString());
            }
            item->setToolTip(0, QStringLiteral("%1\nParâmetros: %2\nExemplo: %3")
                                    .arg(facts, parameters.join(QStringLiteral(", ")), entry.value("example").toString()));
            item->setToolTip(1, item->toolTip(0));
            group->addChild(item);
        }
        // The engine tables are for reference; the script functions above stay open.
        group->setExpanded(false);
    };
    addGroup(QStringLiteral("Comandos do motor (mudam o mundo)"), schema.value("commands").toArray(), true);
    addGroup(QStringLiteral("Consultas do motor (só leem)"), schema.value("queries").toArray(), false);
    const QJsonArray events = schema.value("events").toArray();
    if (!events.isEmpty()) {
        addGroup(QStringLiteral("Eventos"), events, false);
    }
}
