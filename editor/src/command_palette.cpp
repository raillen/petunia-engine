#include "command_palette.h"

#include <QAction>
#include <QApplication>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QPainter>
#include <QStyledItemDelegate>
#include <QTimer>
#include <QVBoxLayout>

#include <algorithm>
#include <functional>

#include "petunia_icons.h"
#include "petunia_theme.h"

namespace {

constexpr int kActionRole = Qt::UserRole + 1;
constexpr int kMatchRole = Qt::UserRole + 2;
constexpr int kPathRole = Qt::UserRole + 3;
constexpr int kShortcutRole = Qt::UserRole + 4;
constexpr int kMaximumResults = 60;
constexpr int kRecentCount = 5;

// Lower case without accents, so "esfera" and "Esféra" read the same.
QString folded(const QString& text) {
    const QString decomposed = text.normalized(QString::NormalizationForm_D).toLower();
    QString plain;
    plain.reserve(decomposed.size());
    for (const QChar character : decomposed) {
        if (character.category() != QChar::Mark_NonSpacing) {
            plain.append(character);
        }
    }
    return plain;
}

QString cleanLabel(QString text) {
    text.remove(QLatin1Char('&'));
    if (text.endsWith(QStringLiteral("…"))) {
        text.chop(1);
    }
    return text.trimmed();
}

// Letters of the query in order inside the label. Letters right after the previous match and at
// the start of a word score more, so "cx" ranks "Caixa" above "Câmera no início". Returns -1 when
// the letters are not all there; `positions` gets the matched indexes of the label.
int fuzzyScore(const QString& query, const QString& label, QList<int>* positions) {
    const QString haystack = folded(label);
    int score = 0;
    int from = 0;
    int previous = -2;
    for (const QChar wanted : query) {
        if (wanted.isSpace()) {
            continue;
        }
        const int found = haystack.indexOf(wanted, from);
        if (found < 0) {
            return -1;
        }
        score += 1;
        if (found == previous + 1) {
            score += 3;
        }
        if (found == 0 || !haystack.at(found - 1).isLetterOrNumber()) {
            score += 4;
        }
        if (positions != nullptr) {
            positions->append(found);
        }
        previous = found;
        from = found + 1;
    }
    return score * 100 - haystack.size();
}

QStringList& recentLabels() {
    static QStringList recent;
    return recent;
}

// One row: icon, label with the matched letters in bold, the menu it lives in, the shortcut.
class EntryDelegate : public QStyledItemDelegate {
public:
    using QStyledItemDelegate::QStyledItemDelegate;

    QSize sizeHint(const QStyleOptionViewItem& option, const QModelIndex& index) const override {
        Q_UNUSED(index);
        return {option.rect.width(), std::max(28, option.fontMetrics.height() + 12)};
    }

    void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
        const PetuniaTheme& theme = PetuniaTheme::instance();
        painter->save();
        painter->setRenderHint(QPainter::Antialiasing);
        const bool selected = option.state & QStyle::State_Selected;
        const bool enabled = option.state & QStyle::State_Enabled;
        const QRect row = option.rect.adjusted(2, 1, -2, -1);
        if (selected) {
            QColor fill = theme.accentPrimary();
            fill.setAlphaF(0.16);
            painter->setBrush(fill);
            painter->setPen(Qt::NoPen);
            painter->drawRoundedRect(row, theme.radius(), theme.radius());
        }
        int x = row.left() + 8;
        const QIcon icon = index.data(Qt::DecorationRole).value<QIcon>();
        if (!icon.isNull()) {
            icon.paint(painter, QRect(x, row.center().y() - 8, 16, 16), Qt::AlignCenter,
                       enabled ? QIcon::Normal : QIcon::Disabled);
        }
        x += 24;

        const QColor text = enabled ? theme.textPrimary() : theme.textMuted();
        const QString shortcut = index.data(kShortcutRole).toString();
        QFont bold = option.font;
        bold.setWeight(QFont::DemiBold);
        const QFontMetrics plainMetrics(option.font);
        const QFontMetrics boldMetrics(bold);
        int shortcutWidth = 0;
        if (!shortcut.isEmpty()) {
            shortcutWidth = plainMetrics.horizontalAdvance(shortcut) + 12;
            const QRect cap(row.right() - 8 - shortcutWidth, row.center().y() - 9, shortcutWidth, 18);
            painter->setPen(Qt::NoPen);
            painter->setBrush(theme.bgSurfaceActive());
            painter->drawRoundedRect(cap, 3, 3);
            painter->setPen(theme.textSecondary());
            painter->setFont(option.font);
            painter->drawText(cap, Qt::AlignCenter, shortcut);
        }

        const QString label = index.data(Qt::DisplayRole).toString();
        const QList<int> matched = index.data(kMatchRole).value<QList<int>>();
        const int baseline = row.center().y() + plainMetrics.ascent() / 2 - 1;
        const int limit = row.right() - 16 - shortcutWidth;
        for (int position = 0; position < label.size() && x < limit; ++position) {
            const bool hit = matched.contains(position);
            painter->setFont(hit ? bold : option.font);
            painter->setPen(hit && enabled ? theme.accentPrimary() : text);
            const QString letter = label.mid(position, 1);
            painter->drawText(x, baseline, letter);
            x += (hit ? boldMetrics : plainMetrics).horizontalAdvance(letter);
        }
        const QString path = index.data(kPathRole).toString();
        if (!path.isEmpty() && x + 24 < limit) {
            painter->setFont(option.font);
            painter->setPen(theme.textMuted());
            painter->drawText(QRect(x + 10, row.top(), limit - x - 10, row.height()), Qt::AlignVCenter | Qt::AlignLeft,
                              plainMetrics.elidedText(path, Qt::ElideRight, limit - x - 10));
        }
        painter->restore();
    }
};

}  // namespace

CommandPalette::CommandPalette(QWidget* window, QList<QMenu*> menus)
    : QFrame(window, Qt::Popup | Qt::FramelessWindowHint), window_(window), menus_(std::move(menus)) {
    setObjectName(QStringLiteral("PetuniaCommandPalette"));
    setProperty("role", QStringLiteral("card"));
    setAccessibleName(QStringLiteral("Paleta de comandos"));

    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(8, 8, 8, 8);
    layout->setSpacing(6);
    query_ = new QLineEdit(this);
    query_->setPlaceholderText(QStringLiteral("Buscar comando (ex.: caixa, desfazer, caverna)"));
    query_->setAccessibleName(QStringLiteral("Buscar comando"));
    query_->setClearButtonEnabled(true);
    query_->addAction(PetuniaIcons::get(QStringLiteral("search"), PetuniaTheme::instance().textMuted().name()),
                      QLineEdit::LeadingPosition);
    layout->addWidget(query_);

    results_ = new QListWidget(this);
    results_->setAccessibleName(QStringLiteral("Comandos encontrados"));
    results_->setItemDelegate(new EntryDelegate(results_));
    results_->setFocusPolicy(Qt::NoFocus);
    results_->setUniformItemSizes(true);
    results_->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    layout->addWidget(results_, 1);

    empty_ = new QLabel(QStringLiteral("Nenhum comando com esse nome."), this);
    empty_->setProperty("role", QStringLiteral("muted"));
    empty_->setAlignment(Qt::AlignCenter);
    empty_->hide();
    layout->addWidget(empty_);

    auto* hint = new QLabel(QStringLiteral("↑ ↓ escolhem · Enter executa · Esc fecha"), this);
    hint->setProperty("role", QStringLiteral("muted"));
    layout->addWidget(hint);

    query_->installEventFilter(this);
    connect(query_, &QLineEdit::textChanged, this, &CommandPalette::refresh);
    connect(results_, &QListWidget::itemClicked, this, [this](QListWidgetItem* item) {
        results_->setCurrentItem(item);
        runCurrent();
    });
}

void CommandPalette::open() {
    collect();
    const int width = std::min(600, window_->width() - 40);
    resize(width, std::min(420, window_->height() - 120));
    const QPoint topCenter = window_->mapToGlobal(QPoint(window_->width() / 2, 0));
    move(topCenter.x() - width / 2, topCenter.y() + 56);
    query_->clear();
    refresh();
    show();
    raise();
    activateWindow();
    query_->setFocus();
}

void CommandPalette::collect() {
    entries_.clear();
    std::function<void(QMenu*, const QString&)> walk = [&](QMenu* menu, const QString& path) {
        for (QAction* action : menu->actions()) {
            if (action->isSeparator() || !action->isVisible()) {
                continue;
            }
            if (QMenu* child = action->menu()) {
                walk(child, path.isEmpty() ? cleanLabel(child->title()) : path + QStringLiteral(" › ") + cleanLabel(child->title()));
                continue;
            }
            const QString label = cleanLabel(action->text());
            if (label.isEmpty()) {
                continue;
            }
            bool duplicate = false;
            for (const Entry& entry : std::as_const(entries_)) {
                duplicate = duplicate || entry.action == action;
            }
            if (!duplicate) {
                entries_.append({action, label, path, folded(label + QLatin1Char(' ') + path + QLatin1Char(' ') + action->toolTip())});
            }
        }
    };
    for (QMenu* menu : std::as_const(menus_)) {
        walk(menu, cleanLabel(menu->title()));
    }
}

void CommandPalette::refresh() {
    const QString query = folded(query_->text().trimmed());
    struct Ranked {
        const Entry* entry;
        int score;
        QList<int> positions;
    };
    QList<Ranked> ranked;
    for (const Entry& entry : std::as_const(entries_)) {
        if (entry.action.isNull()) {
            continue;
        }
        QList<int> positions;
        int score = 0;
        if (!query.isEmpty()) {
            score = fuzzyScore(query, entry.label, &positions);
            if (score < 0) {
                // Also find it by the menu or the explanation, without highlighting.
                positions.clear();
                score = entry.searchable.contains(query) ? 1 : -1;
            }
            if (score < 0) {
                continue;
            }
        }
        const int recent = recentLabels().indexOf(entry.label);
        if (recent >= 0) {
            score += 100000 - recent;
        }
        ranked.append({&entry, score, positions});
    }
    std::stable_sort(ranked.begin(), ranked.end(), [](const Ranked& a, const Ranked& b) { return a.score > b.score; });

    results_->clear();
    for (const Ranked& item : std::as_const(ranked)) {
        if (results_->count() >= kMaximumResults) {
            break;
        }
        QAction* action = item.entry->action;
        auto* row = new QListWidgetItem(action->icon(), item.entry->label, results_);
        const QString shortcut = action->shortcut().toString(QKeySequence::NativeText);
        row->setData(kActionRole, QVariant::fromValue(static_cast<QObject*>(action)));
        row->setData(kMatchRole, QVariant::fromValue(item.positions));
        row->setData(kPathRole, item.entry->path);
        row->setData(kShortcutRole, shortcut);
        QString spoken = QStringLiteral("%1, %2").arg(item.entry->label, item.entry->path);
        if (!shortcut.isEmpty()) {
            spoken += QStringLiteral(", atalho %1").arg(shortcut);
        }
        if (!action->isEnabled()) {
            // Shown, never hidden: the person learns the command exists and why it is off.
            row->setFlags(row->flags() & ~Qt::ItemIsEnabled);
            row->setToolTip(QStringLiteral("Indisponível agora (por exemplo, sem projeto aberto ou sem seleção)"));
            spoken += QStringLiteral(", indisponível agora");
        } else {
            row->setToolTip(action->toolTip());
        }
        row->setData(Qt::AccessibleTextRole, spoken);
    }
    empty_->setVisible(results_->count() == 0);
    for (int index = 0; index < results_->count(); ++index) {
        if (results_->item(index)->flags() & Qt::ItemIsEnabled) {
            results_->setCurrentRow(index);
            break;
        }
    }
}

void CommandPalette::runCurrent() {
    QListWidgetItem* item = results_->currentItem();
    if (item == nullptr || !(item->flags() & Qt::ItemIsEnabled)) {
        return;
    }
    QPointer<QAction> action = qobject_cast<QAction*>(item->data(kActionRole).value<QObject*>());
    if (action.isNull()) {
        return;
    }
    QStringList& recent = recentLabels();
    recent.removeAll(item->text());
    recent.prepend(item->text());
    while (recent.size() > kRecentCount) {
        recent.removeLast();
    }
    hide();
    // After the popup is gone, so the command sees the focus where it was before.
    QTimer::singleShot(0, window_, [action] {
        if (!action.isNull() && action->isEnabled()) {
            action->trigger();
        }
    });
}

bool CommandPalette::eventFilter(QObject* watched, QEvent* event) {
    if (watched == query_ && event->type() == QEvent::KeyPress) {
        auto* key = static_cast<QKeyEvent*>(event);
        const int count = results_->count();
        auto step = [this, count](int delta) {
            if (count == 0) {
                return;
            }
            int row = results_->currentRow();
            for (int tries = 0; tries < count; ++tries) {
                row = (row + delta + count) % count;
                if (results_->item(row)->flags() & Qt::ItemIsEnabled) {
                    break;
                }
            }
            results_->setCurrentRow(row);
        };
        switch (key->key()) {
        case Qt::Key_Down: step(1); return true;
        case Qt::Key_Up: step(-1); return true;
        case Qt::Key_PageDown: for (int n = 0; n < 8; ++n) step(1); return true;
        case Qt::Key_PageUp: for (int n = 0; n < 8; ++n) step(-1); return true;
        case Qt::Key_Return:
        case Qt::Key_Enter: runCurrent(); return true;
        case Qt::Key_Escape: hide(); return true;
        default: break;
        }
    }
    return QFrame::eventFilter(watched, event);
}
