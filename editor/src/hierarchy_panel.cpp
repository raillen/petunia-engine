#include "hierarchy_panel.h"

#include <QDropEvent>
#include <QInputDialog>
#include <functional>
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

class HierarchyTree : public QTreeWidget {
public:
    explicit HierarchyTree(QWidget* parent) : QTreeWidget(parent) {}
    std::function<void(quint64 child, quint64 parent)> reparented;

protected:
    void dropEvent(QDropEvent* event) override {
        QTreeWidgetItem* moving = currentItem();
        const quint64 child = moving == nullptr ? 0 : moving->data(0, kEntityRole).toULongLong();
        QTreeWidget::dropEvent(event);
        if (child == 0 || !reparented) {
            return;
        }
        QTreeWidgetItem* parentItem = moving->parent();
        const quint64 parent = parentItem == nullptr ? 0 : parentItem->data(0, kEntityRole).toULongLong();
        reparented(child, parent);
    }
};
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

    auto* hierarchyTree = new HierarchyTree(this);
    tree_ = hierarchyTree;
    tree_->setAccessibleName(QStringLiteral("Objetos da cena"));
    tree_->setSelectionMode(QAbstractItemView::SingleSelection);
    tree_->setUniformRowHeights(true);
    // Name, the eye, the padlock and the id. The name column takes what is left, so a long name is
    // readable instead of being cut; the id keeps its own narrow width on the right.
    tree_->setColumnCount(3);
    tree_->setHeaderHidden(true);
    tree_->setIndentation(8);
    tree_->setFrameShape(QFrame::NoFrame);
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
    tree_->setColumnWidth(1, 22);
    tree_->setColumnWidth(2, 22);
    tree_->setColumnHidden(2, false);
    tree_->setIconSize(QSize(16, 16));
    tree_->setDragEnabled(true);
    tree_->setAcceptDrops(true);
    tree_->setDropIndicatorShown(true);
    tree_->setDragDropMode(QAbstractItemView::InternalMove);
    tree_->setAccessibleDescription(
        QStringLiteral("Arraste um objeto sobre outro para torná-lo filho. Soltar na raiz desfaz o parentesco. O menu do botão direito faz o mesmo sem o mouse."));
    hierarchyTree->reparented = [this](quint64 child, quint64 parent) {
        if (parent == 0 || parent == child) {
            bridge_->call(QStringLiteral("transform.clear_parent"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(child)}});
        } else {
            bridge_->call(QStringLiteral("transform.set_parent"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(child)},
                                      {QStringLiteral("parent"), static_cast<double>(parent)}});
        }
        reload();
    };

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
            root_->setFlags(Qt::ItemIsEnabled | Qt::ItemIsDropEnabled);
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
                // The id lives in the tooltip: a fourth column left the name unreadable at 240 px.
                item->setData(0, kEntityRole, id);
                item->setData(0, kIconRole, TableIcons::forEntity(tables));
                // Visibility and locking (QT-12): an eye and a padlock, drawn as dim when off.
                // The flags come from the engine; an object without them is visible and unlocked.
                const bool hidden = entity.value(QStringLiteral("hidden")).toBool();
                const bool locked = entity.value(QStringLiteral("locked")).toBool();
                item->setIcon(1, PetuniaIcons::get(hidden ? QStringLiteral("eye_off")
                                                          : QStringLiteral("eye")));
                item->setIcon(2, PetuniaIcons::get(locked ? QStringLiteral("lock")
                                                          : QStringLiteral("unlock")));
                item->setToolTip(1, hidden ? QStringLiteral("Oculto (não aparece no viewport)")
                                           : QStringLiteral("Visível"));
                item->setToolTip(2, locked ? QStringLiteral("Travado (não pode ser escolhido)")
                                           : QStringLiteral("Destravado"));
                item->setData(0, Qt::UserRole + 3, hidden);
                item->setData(0, Qt::UserRole + 4, locked);
                if (hidden) {
                    item->setForeground(0, QBrush(QColor(128, 128, 128)));
                }
                // A click on the eye or the padlock acts on that object without selecting it, the
                // way a file manager's checkboxes do.
                item->setData(1, kEntityRole, id);
                item->setData(2, kEntityRole, id);
                const QString ownedTip = tables.contains(QStringLiteral("Script"))
                    ? QStringLiteral("Script #%1 ligado a este objeto").arg(id)
                    : QStringLiteral("Parte do objeto #%1").arg(id);
                item->setToolTip(0, owned ? ownedTip
                                          : QStringLiteral("#%1 · %2").arg(id).arg(tables.join(QStringLiteral(", "))));
                if (owned) {
                    parent->setExpanded(true);
                }
            }
            }
            // A second pass nests objects by Transform.parent. The first pass created every row,
            // so a child that sorts before its parent still finds it.
            for (const QJsonValue& value : entities) {
                const QJsonObject entity = value.toObject();
                if (!entity.contains(QStringLiteral("parent"))) {
                    continue;
                }
                const quint64 id = static_cast<quint64>(entity.value(QStringLiteral("id")).toDouble());
                const quint64 parentId = static_cast<quint64>(entity.value(QStringLiteral("parent")).toDouble());
                QTreeWidgetItem* item = items.value(id);
                QTreeWidgetItem* parentItem = items.value(parentId);
                if (item == nullptr || parentItem == nullptr || item == parentItem) {
                    continue;
                }
                parentItem->addChild(item);
                parentItem->setExpanded(true);
            }
            root_->setExpanded(true);
        }
    
    // The right-click menu of a row: the same three actions the engine exposes (QT-12).
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
        const bool hidden = item->data(0, Qt::UserRole + 3).toBool();
        const bool locked = item->data(0, Qt::UserRole + 4).toBool();
        QMenu menu(this);
        QAction* hide = menu.addAction(hidden ? QStringLiteral("Mostrar") : QStringLiteral("Ocultar"));
        QAction* lock = menu.addAction(locked ? QStringLiteral("Destravar") : QStringLiteral("Travar"));
        menu.addSeparator();
        QAction* solo = menu.addAction(QStringLiteral("Ver só este"));
        QAction* showAll = menu.addAction(QStringLiteral("Mostrar todos"));
        menu.addSeparator();
        QAction* unparent = menu.addAction(QStringLiteral("Soltar da hierarquia"));
        QAction* parentOf = menu.addAction(QStringLiteral("Tornar filho de…"));
        QAction* shot = menu.addAction(QStringLiteral("Câmera fixa neste objeto"));
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
        } else if (chosen == unparent) {
            bridge_->call(QStringLiteral("transform.clear_parent"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)}});
        } else if (chosen == parentOf) {
            QStringList labels;
            QList<quint64> ids;
            for (const QJsonValue& value : bridge_->snapshot().value(QStringLiteral("entities")).toArray()) {
                const QJsonObject entity = value.toObject();
                const quint64 other = static_cast<quint64>(entity.value(QStringLiteral("id")).toDouble());
                if (other == 0 || other == id || entity.contains(QStringLiteral("owner"))) {
                    continue;
                }
                labels << entity.value(QStringLiteral("name")).toString();
                ids << other;
            }
            bool accepted = false;
            const QString picked = QInputDialog::getItem(
                this, QStringLiteral("Tornar filho de"), QStringLiteral("Pai:"), labels, 0, false,
                &accepted);
            const int index = labels.indexOf(picked);
            if (accepted && index >= 0) {
                bridge_->call(QStringLiteral("transform.set_parent"),
                              QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                          {QStringLiteral("parent"), static_cast<double>(ids.at(index))}});
            }
        } else if (chosen == shot) {
            bridge_->call(QStringLiteral("shot.add"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                      {QStringLiteral("priority"), 1}});
        }
        reload();
    });

    // A click on the eye or padlock column toggles that flag.
    connect(tree_, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem* item, int column) {
        if (item == nullptr) {
            return;
        }
        const quint64 id = item->data(column == 1 ? 1 : 2, kEntityRole).toULongLong();
        if (id == 0) {
            return;
        }
        if (column == 1) {
            const bool hidden = item->data(0, Qt::UserRole + 3).toBool();
            bridge_->call(QStringLiteral("object.hide"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                      {QStringLiteral("hidden"), !hidden}});
        } else if (column == 2) {
            const bool locked = item->data(0, Qt::UserRole + 4).toBool();
            bridge_->call(QStringLiteral("object.lock"),
                          QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)},
                                      {QStringLiteral("locked"), !locked}});
        }
        reload();
    });
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

namespace {
bool filterTree(QTreeWidgetItem* item, const QString& needle) {
    const bool self = needle.isEmpty() || item->text(0).contains(needle, Qt::CaseInsensitive);
    bool descendant = false;
    for (int child = 0; child < item->childCount(); ++child) {
        descendant = filterTree(item->child(child), needle) || descendant;
    }
    item->setHidden(!self && !descendant);
    return self || descendant;
}
}

void HierarchyPanel::applyFilter() {
    if (root_ == nullptr) {
        return;
    }
    const QString needle = filter_->text().trimmed();
    // Dragging while rows are hidden would parent into the wrong object.
    tree_->setDragEnabled(needle.isEmpty());
    for (int row = 0; row < root_->childCount(); ++row) {
        filterTree(root_->child(row), needle);
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
