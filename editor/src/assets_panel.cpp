#include "assets_panel.h"

#include <QFileDialog>
#include <QHBoxLayout>
#include <QJsonObject>
#include <QLabel>
#include <QListWidget>
#include <QPushButton>
#include <QVBoxLayout>

#include "engine_bridge.h"
#include "petunia_icons.h"

namespace {

// "1,2 MB" for a person reading the list; the exact byte count stays in the tooltip.
QString readableSize(qint64 bytes) {
    if (bytes < 1024) {
        return QStringLiteral("%1 B").arg(bytes);
    }
    if (bytes < 1024 * 1024) {
        return QStringLiteral("%1 kB").arg(QString::number(bytes / 1024.0, 'f', 1));
    }
    return QStringLiteral("%1 MB").arg(QString::number(bytes / (1024.0 * 1024.0), 'f', 1));
}

}  // namespace

AssetPanel::AssetPanel(EngineBridge* bridge, QWidget* parent) : QWidget(parent), bridge_(bridge) {
    list_ = new QListWidget(this);
    list_->setAccessibleName(QStringLiteral("Modelos na pasta assets do projeto"));
    list_->setUniformItemSizes(true);
    list_->setAlternatingRowColors(true);

    hint_ = new QLabel(this);
    hint_->setWordWrap(true);
    hint_->setObjectName(QStringLiteral("PetuniaPanelHint"));

    importButton_ = new QPushButton(QStringLiteral("Importar…"), this);
    importButton_->setToolTip(QStringLiteral("Copia um .glb ou .gltf para a pasta assets/ e o põe na cena"));
    importButton_->setIcon(PetuniaIcons::get(QStringLiteral("import")));
    importButton_->setAccessibleName(QStringLiteral("Importar um modelo 3D"));
    insertButton_ = new QPushButton(QStringLiteral("Inserir na cena"), this);
    insertButton_->setToolTip(QStringLiteral("Põe outra cópia do modelo escolhido na frente da câmera"));
    insertButton_->setIcon(PetuniaIcons::get(QStringLiteral("plus")));
    insertButton_->setAccessibleName(QStringLiteral("Inserir o modelo escolhido na cena"));
    insertButton_->setEnabled(false);
    auto* refresh = new QPushButton(QStringLiteral("Atualizar"), this);
    refresh->setToolTip(QStringLiteral("Relê a pasta assets/ (use depois de copiar um arquivo para lá)"));
    refresh->setIcon(PetuniaIcons::get(QStringLiteral("refresh")));
    refresh->setAccessibleName(QStringLiteral("Atualizar a lista de modelos"));

    auto* buttons = new QHBoxLayout();
    buttons->setContentsMargins(0, 0, 0, 0);
    buttons->setSpacing(6);
    buttons->addWidget(importButton_);
    buttons->addWidget(insertButton_);
    buttons->addWidget(refresh);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(8, 6, 8, 6);
    layout->setSpacing(6);
    layout->addWidget(list_, 1);
    layout->addWidget(hint_);
    layout->addLayout(buttons);

    connect(importButton_, &QPushButton::clicked, this, &AssetPanel::importFromDisk);
    connect(refresh, &QPushButton::clicked, this, &AssetPanel::reload);
    connect(insertButton_, &QPushButton::clicked, this, &AssetPanel::insertSelected);
    connect(list_, &QListWidget::itemDoubleClicked, this, [this](QListWidgetItem*) { insertSelected(); });
    connect(list_, &QListWidget::currentItemChanged, this, [this](QListWidgetItem* current) {
        insertButton_->setEnabled(current != nullptr);
    });
}

void AssetPanel::reload() {
    list_->clear();
    if (!bridge_->hasProject()) {
        hint_->setText(QStringLiteral("Abra um projeto para ver os modelos dele."));
        insertButton_->setEnabled(false);
        importButton_->setEnabled(false);
        return;
    }
    importButton_->setEnabled(true);
    const Reply reply = bridge_->call(QStringLiteral("asset.list"));
    if (!reply.ok) {
        hint_->setText(reply.explanation());
        return;
    }
    const QJsonObject result = reply.result.toObject();
    fill(result.value(QStringLiteral("assets")).toArray(), result.value(QStringLiteral("folder")).toString());
}

void AssetPanel::fill(const QJsonArray& assets, const QString& folder) {
    if (assets.isEmpty()) {
        hint_->setText(QStringLiteral("Nenhum modelo em %1/. Use Importar… ou copie um .glb para lá.")
                           .arg(folder.isEmpty() ? QStringLiteral("assets") : folder));
        insertButton_->setEnabled(false);
        return;
    }
    for (const QJsonValue& value : assets) {
        const QJsonObject asset = value.toObject();
        const QString path = asset.value(QStringLiteral("path")).toString();
        const QString name = asset.value(QStringLiteral("name")).toString();
        auto* item = new QListWidgetItem(QStringLiteral("%1  —  %2")
                                             .arg(name.isEmpty() ? path : name,
                                                  readableSize(static_cast<qint64>(asset.value(QStringLiteral("bytes")).toDouble()))));
        item->setData(Qt::UserRole, path);
        item->setToolTip(path);
        item->setData(Qt::AccessibleTextRole, QStringLiteral("Modelo %1, %2").arg(name, path));
        list_->addItem(item);
    }
    hint_->setText(QStringLiteral("%1 modelo(s). Clique duas vezes para pôr um na cena.").arg(assets.size()));
}

void AssetPanel::insertSelected() {
    QListWidgetItem* item = list_->currentItem();
    if (item == nullptr) {
        return;
    }
    const QString path = item->data(Qt::UserRole).toString();
    const Reply reply = bridge_->call(QStringLiteral("asset.import"), QJsonObject{{QStringLiteral("path"), path}});
    if (!reply.ok) {
        emit statusMessage(reply.explanation());
        return;
    }
    const QJsonObject result = reply.result.toObject();
    const int imported = result.value(QStringLiteral("imported")).toInt();
    emit statusMessage(QStringLiteral("%1 objeto(s) do modelo %2 entraram na cena — Ctrl+Z desfaz.")
                           .arg(imported)
                           .arg(item->text().section(QStringLiteral("  —  "), 0, 0)));
    bridge_->refresh();
}

void AssetPanel::importFromDisk() {
    if (!bridge_->hasProject()) {
        return;
    }
    const QString path = QFileDialog::getOpenFileName(this, QStringLiteral("Importar modelo 3D"), QString(),
                                                      QStringLiteral("Modelos glTF (*.glb *.gltf)"));
    if (path.isEmpty()) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("asset.import"), QJsonObject{{QStringLiteral("path"), path}});
    if (!reply.ok) {
        emit statusMessage(reply.explanation());
        return;
    }
    const QJsonObject result = reply.result.toObject();
    const QJsonArray warnings = result.value(QStringLiteral("warnings")).toArray();
    QString message = QStringLiteral("Modelo importado: %1 objeto(s). Cópia em %2.")
                          .arg(result.value(QStringLiteral("imported")).toInt())
                          .arg(result.value(QStringLiteral("asset")).toString());
    if (!warnings.isEmpty()) {
        message += QStringLiteral(" Aviso: %1").arg(warnings.first().toString());
    }
    emit statusMessage(message);
    bridge_->refresh();
    reload();
}
