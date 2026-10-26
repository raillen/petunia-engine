#include "hierarchy_panel.h"

#include <QHBoxLayout>
#include <QHash>
#include <QHeaderView>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QToolButton>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>

#include "engine_bridge.h"
#include "petunia_icons.h"
#include "petunia_theme.h"
#include "table_icons.h"

namespace {
constexpr int kEntityRole = Qt::UserRole + 1;
constexpr int kIconRole = Qt::UserRole + 2;
}

HierarchyPanel::HierarchyPanel(EngineBridge* bridge, QWidget* parent)
    : QWidget(parent), bridge_(bridge) {
    auto* bar = new QWidget(this);
    bar->setObjectName(QStringLiteral("PetuniaPanelBar"));
    bar->setAttribute(Qt::WA_StyledBackground);
    auto* barLayout = new QHBoxLayout(bar);
    barLayout->setContentsMargins(6, 2, 6, 4);
    barLayout->setSpacing(4);

    createButton_ = new QToolButton(bar);
    createButton_->setPopupMode(QToolButton::InstantPopup);
    createButton_->setProperty("role", QStringLiteral("appTool"));
    createButton_->setToolTip(QStringLiteral("Cria um objeto na frente da câmera (Shift+A abre o menu radial)"));
    createButton_->setAccessibleName(QStringLiteral("Criar objeto"));
    createButton_->setEnabled(false);
    filter_ = new QLineEdit(bar);
    filter_->setPlaceholderText(QStringLiteral("Buscar"));
    filter_->setClearButtonEnabled(true);
    filter_->setAccessibleName(QStringLiteral("Filtrar objetos pelo nome"));
    filter_->addAction(PetuniaIcons::get(QStringLiteral("search"), PetuniaTheme::instance().textMuted().name()), QLineEdit::LeadingPosition);
    barLayout->addWidget(filter_, 1);
    barLayout->addWidget(createButton_);

    tree_ = new QTreeWidget(this);
    tree_->setAccessibleName(QStringLiteral("Objetos da cena"));
    tree_->setSelectionMode(QAbstractItemView::SingleSelection);
    tree_->setUniformRowHeights(true);
    tree_->setColumnCount(2);
    tree_->setHeaderHidden(true);
    tree_->setIndentation(12);
    tree_->setFrameShape(QFrame::NoFrame);
    tree_->setIconSize(QSize(16, 16));
    tree_->header()->setStretchLastSection(false);
    tree_->header()->setSectionResizeMode(0, QHeaderView::Stretch);
    tree_->header()->setSectionResizeMode(1, QHeaderView::ResizeToContents);

    summary_ = new QLabel(this);
    summary_->setWordWrap(true);
    summary_->setProperty("role", QStringLiteral("muted"));
    summary_->setContentsMargins(8, 4, 8, 6);

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addWidget(bar);
    layout->addWidget(tree_, 1);
    layout->addWidget(summary_);

    connect(filter_, &QLineEdit::textChanged, this, &HierarchyPanel::applyFilter);
    connect(tree_, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* current) {
        if (syncing_ || current == nullptr || current == root_) {
            return;
        }
        emit selectionRequested(current->data(0, kEntityRole).toULongLong());
    });
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, &HierarchyPanel::refreshIcons);
    refreshIcons();
}

void HierarchyPanel::setCreateMenu(QMenu* menu) {
    createButton_->setMenu(menu);
}

void HierarchyPanel::reload() {
    const QJsonObject snapshot = bridge_->snapshot();
    const QJsonArray entities = snapshot.value(QStringLiteral("entities")).toArray();
    const QString project = snapshot.value(QStringLiteral("project")).toObject().value(QStringLiteral("name")).toString();
    createButton_->setEnabled(bridge_->hasProject());
    if (entities != shown_ || project != shownProject_) {
        shown_ = entities;
        shownProject_ = project;
        syncing_ = true;
        tree_->clear();
        root_ = nullptr;
        if (bridge_->hasProject()) {
            root_ = new QTreeWidgetItem(tree_);
            root_->setText(0, project);
            root_->setData(0, kIconRole, QStringLiteral("hierarchy"));
            root_->setFlags(Qt::ItemIsEnabled);
            QFont bold = root_->font(0);
            bold.setWeight(QFont::DemiBold);
            root_->setFont(0, bold);
            // Objects first; then each collider under the object it belongs to.
            QHash<quint64, QTreeWidgetItem*> items;
            for (const bool parts : {false, true}) {
            for (const QJsonValue& value : entities) {
                const QJsonObject entity = value.toObject();
                const bool owned = entity.contains(QStringLiteral("owner"));
                if (owned != parts) {
                    continue;
                }
                const quint64 id = static_cast<quint64>(entity.value(QStringLiteral("id")).toDouble());
                QStringList tables;
                for (const QJsonValue& table : entity.value(QStringLiteral("tables")).toArray()) {
                    tables << table.toString();
                }
                QTreeWidgetItem* parent = root_;
                if (owned) {
                    parent = items.value(static_cast<quint64>(entity.value(QStringLiteral("owner")).toDouble()), root_);
                }
                auto* item = new QTreeWidgetItem(parent);
                items.insert(id, item);
                item->setText(0, entity.value(QStringLiteral("name")).toString());
                item->setText(1, QStringLiteral("#%1").arg(id));
                item->setTextAlignment(1, Qt::AlignRight | Qt::AlignVCenter);
                item->setData(0, kEntityRole, id);
                item->setData(0, kIconRole, TableIcons::forEntity(tables));
                item->setToolTip(0, owned ? QStringLiteral("Colisão #%1 (scripts a acham pelo nome)").arg(id)
                                          : QStringLiteral("#%1 · %2").arg(id).arg(tables.join(QStringLiteral(", "))));
                if (owned) {
                    parent->setExpanded(true);
                }
            }
            }
            root_->setExpanded(true);
        }
        refreshIcons();
        syncing_ = false;
        applyFilter();
    }
    syncSelection();

    if (!bridge_->hasProject()) {
        summary_->setText(QStringLiteral("Nenhum projeto aberto."));
        summary_->show();
    } else if (snapshot.value(QStringLiteral("truncated")).toBool()) {
        summary_->setText(QStringLiteral("A lista mostra só os primeiros %1 objetos.").arg(entities.size()));
        summary_->show();
    } else {
        summary_->hide();
    }
}

// Icons follow the theme's text color; the id column is muted so the name reads first.
void HierarchyPanel::refreshIcons() {
    createButton_->setIcon(PetuniaIcons::get(QStringLiteral("plus")));
    const QColor muted = PetuniaTheme::instance().textMuted();
    for (QTreeWidgetItemIterator it(tree_); *it; ++it) {
        QTreeWidgetItem* item = *it;
        item->setIcon(0, PetuniaIcons::get(item->data(0, kIconRole).toString()));
        item->setForeground(1, muted);
    }
}

void HierarchyPanel::applyFilter() {
    if (root_ == nullptr) {
        return;
    }
    const QString needle = filter_->text().trimmed();
    // An object stays visible when it or one of its colliders matches.
    for (int row = 0; row < root_->childCount(); ++row) {
        QTreeWidgetItem* item = root_->child(row);
        bool any = needle.isEmpty() || item->text(0).contains(needle, Qt::CaseInsensitive);
        for (int child = 0; child < item->childCount(); ++child) {
            const bool match = needle.isEmpty() || item->child(child)->text(0).contains(needle, Qt::CaseInsensitive);
            item->child(child)->setHidden(!match && !any);
            any = any || match;
        }
        item->setHidden(!any);
    }
}

void HierarchyPanel::syncSelection() {
    syncing_ = true;
    if (!bridge_->hasSelection() || root_ == nullptr) {
        tree_->clearSelection();
        tree_->setCurrentItem(nullptr);
    } else {
        const quint64 wanted = bridge_->selection();
        for (QTreeWidgetItemIterator it(tree_); *it; ++it) {
            QTreeWidgetItem* item = *it;
            if (item != root_ && item->data(0, kEntityRole).toULongLong() == wanted) {
                tree_->setCurrentItem(item);
                tree_->scrollToItem(item);
                break;
            }
        }
    }
    syncing_ = false;
}
