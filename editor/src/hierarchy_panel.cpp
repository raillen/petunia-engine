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
/// Engine-reported visibility and locking, kept on the name column of every row (QT-12).
constexpr int kHiddenRole = Qt::UserRole + 3;
constexpr int kLockedRole = Qt::UserRole + 4;
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
    tree_->setColumnCount(3);
    tree_->setHeaderHidden(true);
    tree_->setIndentation(8);
    tree_->setFrameShape(QFrame::NoFrame);
    tree_->setIconSize(QSize(16, 16));
    tree_->header()->setStretchLastSection(false);
    tree_->header()->setSectionResizeMode(0, QHeaderView::Stretch);
    // Visibility and locking (QT-12): an eye and a padlock at a fixed narrow width, so the name
    // keeps the rest of the row.
    tree_->header()->setSectionResizeMode(1, QHeaderView::Fixed);
    tree_->header()->setSectionResizeMode(2, QHeaderView::Fixed);
    tree_->setColumnWidth(1, 14);
    tree_->setColumnWidth(2, 14);
    // Re-asserted after the fixed widths: setting them first left column 0 at its initial size and
    // the names were cut with empty space at the right.
    tree_->header()->setSectionResizeMode(0, QHeaderView::Stretch);
    // A click on the eye or the padlock acts on that row without selecting it, the way a file
    // manager's check boxes do.
    connect(tree_, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem* item, int column) {
        if (item == nullptr || (column != 1 && column != 2)) {
            return;
        }
        const quint64 id = item->data(0, kEntityRole).toULongLong();
        if (id == 0) {
            return;
        }
        if (column == 1) {
            bridge_->call(QStringLiteral("object.hide"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                      {QStringLiteral("hidden"),
                                       !item->data(0, kHiddenRole).toBool()}});
        } else {
            bridge_->call(QStringLiteral("object.lock"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                      {QStringLiteral("locked"),
                                       !item->data(0, kLockedRole).toBool()}});
        }
        reload();
    });
    tree_->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(tree_, &QTreeWidget::customContextMenuRequested, this, [this](const QPoint& point) {
        QTreeWidgetItem* item = tree_->itemAt(point);
        if (item == nullptr) {
            return;
        }
        const quint64 id = item->data(0, kEntityRole).toULongLong();
        if (id == 0) {
            return;
        }
        const bool hidden = item->data(0, kHiddenRole).toBool();
        const bool locked = item->data(0, kLockedRole).toBool();
        QMenu menu(this);
        QAction* hide = menu.addAction(hidden ? QStringLiteral("Mostrar") : QStringLiteral("Ocultar"));
        QAction* lock = menu.addAction(locked ? QStringLiteral("Destravar") : QStringLiteral("Travar"));
        menu.addSeparator();
        QAction* solo = menu.addAction(QStringLiteral("Ver só este"));
        QAction* showAll = menu.addAction(QStringLiteral("Mostrar todos"));
        QAction* chosen = menu.exec(tree_->viewport()->mapToGlobal(point));
        if (chosen == hide) {
            bridge_->call(QStringLiteral("object.hide"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                      {QStringLiteral("hidden"), !hidden}});
        } else if (chosen == lock) {
            bridge_->call(QStringLiteral("object.lock"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                      {QStringLiteral("locked"), !locked}});
        } else if (chosen == solo) {
            bridge_->call(QStringLiteral("object.solo"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)}});
        } else if (chosen == showAll) {
            bridge_->call(QStringLiteral("object.hide"),
                          QJsonObject{{QStringLiteral("all"), true},
                                      {QStringLiteral("hidden"), false}});
        }
        reload();
    });

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
                item->setData(0, kEntityRole, id);
                item->setData(0, kIconRole, TableIcons::forEntity(tables));
                const QString ownedTip = tables.contains(QStringLiteral("Script"))
                    ? QStringLiteral("Script #%1 ligado a este objeto").arg(id)
                    : QStringLiteral("Parte do objeto #%1").arg(id);
                // The id moves to the tooltip: a third column left the name unreadable at 240 px.
                const bool hidden = entity.value(QStringLiteral("hidden")).toBool();
                const bool locked = entity.value(QStringLiteral("locked")).toBool();
                item->setData(0, kHiddenRole, hidden);
                item->setData(0, kLockedRole, locked);
                item->setIcon(1, PetuniaIcons::get(hidden ? QStringLiteral("eye_hidden")
                                                          : QStringLiteral("eye")));
                item->setIcon(2, PetuniaIcons::get(locked ? QStringLiteral("lock")
                                                          : QStringLiteral("unlock")));
                item->setToolTip(1, hidden ? QStringLiteral("Oculto (não aparece no viewport)")
                                           : QStringLiteral("Visível"));
                item->setToolTip(2, locked ? QStringLiteral("Travado (não pode ser escolhido)")
                                           : QStringLiteral("Destravado"));
                if (hidden) {
                    item->setForeground(0, QBrush(PetuniaTheme::instance().textMuted()));
                }
                item->setToolTip(0, owned ? ownedTip
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
