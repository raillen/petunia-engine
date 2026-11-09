#include "assets_panel.h"

#include <QFileDialog>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

#include <cmath>

#include "engine_bridge.h"

namespace {

// Bytes as a human reads them. Sizes come from the engine, already bounded.
QString humanSize(double bytes) {
    static const char* units[] = {"B", "KB", "MB", "GB"};
    double value = bytes;
    int unit = 0;
    while (value >= 1024.0 && unit < 3) {
        value /= 1024.0;
        ++unit;
    }
    return unit == 0 ? QStringLiteral("%1 B").arg(static_cast<qulonglong>(value))
                     : QStringLiteral("%1 %2").arg(value, 0, 'f', 1).arg(units[unit]);
}

QString textOf(const QJsonObject& object, const char* key) {
    return object.value(QLatin1String(key)).toString();
}

}  // namespace

AssetsPanel::AssetsPanel(EngineBridge* bridge, QWidget* parent) : QWidget(parent), bridge_(bridge) {
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(8, 6, 8, 6);
    layout->setSpacing(4);

    auto* header = new QHBoxLayout;
    header->setSpacing(6);
    summary_ = new QLabel(QStringLiteral("Nenhum projeto aberto."), this);
    summary_->setProperty("role", QStringLiteral("muted"));
    summary_->setWordWrap(true);
    header->addWidget(summary_, 1);
    importButton_ = new QPushButton(QStringLiteral("Importar modelo…"), this);
    importButton_->setToolTip(
        QStringLiteral("Copia um arquivo .glb ou .gltf para a pasta assets/ e o põe na cena"));
    importButton_->setAccessibleName(QStringLiteral("Importar modelo 3D para a cena"));
    header->addWidget(importButton_);
    layout->addLayout(header);

    tree_ = new QTreeWidget(this);
    tree_->setAccessibleName(QStringLiteral("Arquivos da pasta assets"));
    tree_->setHeaderHidden(false);
    tree_->setColumnCount(2);
    tree_->setHeaderLabels({QStringLiteral("Arquivo"), QStringLiteral("Tamanho")});
    tree_->setRootIsDecorated(false);
    tree_->setUniformRowHeights(true);
    tree_->setFrameShape(QFrame::NoFrame);
    tree_->header()->setStretchLastSection(false);
    tree_->header()->setSectionResizeMode(0, QHeaderView::Stretch);
    tree_->header()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    layout->addWidget(tree_, 1);

    connect(importButton_, &QPushButton::clicked, this, [this] {
        const QString path = QFileDialog::getOpenFileName(
            this, QStringLiteral("Importar modelo 3D"), QString(),
            QStringLiteral("Modelos glTF (*.glb *.gltf)"));
        if (path.isEmpty()) {
            return;
        }
        const Reply reply = bridge_->call(QStringLiteral("asset.import"),
                                          QJsonObject{{QStringLiteral("path"), path}});
        if (!reply.ok) {
            emit message(reply.explanation());
            return;
        }
        emit message(QStringLiteral("Modelo importado para a cena."));
        reload();
        emit imported();
    });
}

void AssetsPanel::reload() {
    tree_->clear();
    importButton_->setEnabled(bridge_->hasProject());
    if (!bridge_->hasProject()) {
        summary_->setText(QStringLiteral("Nenhum projeto aberto."));
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("asset.list"));
    if (!reply.ok) {
        summary_->setText(reply.explanation());
        return;
    }
    const QJsonObject result = reply.result.toObject();
    const QJsonArray assets = result.value(QStringLiteral("assets")).toArray();
    double total = 0.0;
    for (const QJsonValue& value : assets) {
        const QJsonObject asset = value.toObject();
        const QString path = textOf(asset, "path");
        const double bytes = asset.value(QStringLiteral("bytes")).toDouble();
        total += bytes;
        auto* row = new QTreeWidgetItem(tree_);
        row->setText(0, path);
        row->setText(1, humanSize(bytes));
        row->setToolTip(0, path);
    }
    if (assets.isEmpty()) {
        summary_->setText(
            QStringLiteral("A pasta assets/ está vazia. Use “Importar modelo…” para trazer um .glb."));
    } else {
        summary_->setText(QStringLiteral("%1 arquivo(s), %2 no total.")
                              .arg(assets.size())
                              .arg(humanSize(total)));
    }
}
