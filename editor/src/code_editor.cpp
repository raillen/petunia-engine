#include "code_editor.h"

#include <QAbstractItemView>
#include <QApplication>
#include <QCompleter>
#include <QFontDatabase>
#include <QHelpEvent>
#include <QJsonArray>
#include <QKeyEvent>
#include <QPainter>
#include <QScrollBar>
#include <QStringListModel>
#include <QTextBlock>
#include <QToolTip>

#include "petunia_theme.h"

namespace {

constexpr int kIndentWidth = 4;

bool isWordCharacter(QChar character) {
    return character.isLetterOrNumber() || character == QLatin1Char('_');
}

}  // namespace

// The strip with line numbers and problem dots, left of the text.
class LineNumberArea : public QWidget {
public:
    explicit LineNumberArea(CodeEdit* editor) : QWidget(editor), editor_(editor) {}
    QSize sizeHint() const override { return QSize(editor_->lineNumberAreaWidth(), 0); }

protected:
    void paintEvent(QPaintEvent* event) override { editor_->lineNumberAreaPaintEvent(event); }

private:
    CodeEdit* editor_;
};

// ---- highlighter ----------------------------------------------------------------------------

AipoHighlighter::AipoHighlighter(QTextDocument* document) : QSyntaxHighlighter(document) {
    refreshColors();
}

void AipoHighlighter::refreshColors() {
    const PetuniaTheme& theme = PetuniaTheme::instance();
    keywordFormat_.setForeground(theme.accentPrimary());
    keywordFormat_.setFontWeight(QFont::DemiBold);
    moduleFormat_.setForeground(theme.statusInfo());
    stringFormat_.setForeground(theme.statusSuccess());
    numberFormat_.setForeground(theme.statusWarning());
    commentFormat_.setForeground(theme.textMuted());
    commentFormat_.setFontItalic(true);
    directiveFormat_.setForeground(theme.textSecondary());
    directiveFormat_.setFontWeight(QFont::DemiBold);
    rehighlight();
}

void AipoHighlighter::setVocabulary(const QStringList& keywords, const QStringList& modules) {
    keywords_ = QSet<QString>(keywords.begin(), keywords.end());
    modules_ = QSet<QString>(modules.begin(), modules.end());
    rehighlight();
}

void AipoHighlighter::highlightBlock(const QString& text) {
    const int size = text.size();
    int index = 0;
    while (index < size) {
        const QChar character = text.at(index);
        if (character == QLatin1Char('#')) {
            const bool directive = text.mid(index, 2) == QLatin1String("#!");
            setFormat(index, size - index, directive ? directiveFormat_ : commentFormat_);
            return;
        }
        if (character == QLatin1Char('"')) {
            int end = index + 1;
            while (end < size) {
                if (text.at(end) == QLatin1Char('\\')) {
                    end += 2;
                    continue;
                }
                if (text.at(end++) == QLatin1Char('"')) {
                    break;
                }
            }
            setFormat(index, qMin(end, size) - index, stringFormat_);
            index = end;
            continue;
        }
        if (character.isDigit()) {
            int end = index;
            while (end < size && (isWordCharacter(text.at(end)) || text.at(end) == QLatin1Char('.'))) {
                ++end;
            }
            setFormat(index, end - index, numberFormat_);
            index = end;
            continue;
        }
        if (isWordCharacter(character)) {
            int end = index;
            while (end < size && isWordCharacter(text.at(end))) {
                ++end;
            }
            const QString word = text.mid(index, end - index);
            if (keywords_.contains(word)) {
                setFormat(index, end - index, keywordFormat_);
            } else if (modules_.contains(word) && end < size && text.at(end) == QLatin1Char('.')) {
                setFormat(index, end - index, moduleFormat_);
            }
            index = end;
            continue;
        }
        ++index;
    }
}

// ---- editor ---------------------------------------------------------------------------------

CodeEdit::CodeEdit(QWidget* parent) : QPlainTextEdit(parent) {
    setAccessibleName(QStringLiteral("Editor de código"));
    setAccessibleDescription(QStringLiteral(
        "Texto do script. Tab recua; Esc e depois Tab sai do editor. Ctrl+Espaço abre as sugestões, Ctrl+S salva."));
    setLineWrapMode(QPlainTextEdit::NoWrap);
    setTabStopDistance(0);
    setUndoRedoEnabled(true);
    gutter_ = new LineNumberArea(this);
    highlighter_ = new AipoHighlighter(document());

    completionModel_ = new QStringListModel(this);
    completer_ = new QCompleter(completionModel_, this);
    completer_->setWidget(this);
    completer_->setCompletionMode(QCompleter::PopupCompletion);
    completer_->setCaseSensitivity(Qt::CaseInsensitive);
    connect(completer_, QOverload<const QString&>::of(&QCompleter::activated), this, &CodeEdit::insertCompletion);
    connect(completer_, QOverload<const QString&>::of(&QCompleter::highlighted), this, [this](const QString& word) {
        const QString key = completionModule_.isEmpty() ? word : completionModule_ + QLatin1Char('.') + word;
        emit hintChanged(descriptions_.value(key));
    });

    connect(this, &QPlainTextEdit::blockCountChanged, this, [this] { updateLineNumberAreaWidth(); });
    connect(this, &QPlainTextEdit::updateRequest, this, &CodeEdit::updateLineNumberArea);
    connect(this, &QPlainTextEdit::cursorPositionChanged, this, &CodeEdit::applySelections);
    refreshAppearance();
    updateLineNumberAreaWidth();
    applySelections();
}

void CodeEdit::refreshAppearance() {
    QFont mono = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    mono.setPointSizeF(QApplication::font().pointSizeF());
    setFont(mono);
    setTabStopDistance(fontMetrics().horizontalAdvance(QLatin1Char(' ')) * kIndentWidth);
    highlighter_->refreshColors();
    updateLineNumberAreaWidth();
    applySelections();
}

void CodeEdit::changeEvent(QEvent* event) {
    QPlainTextEdit::changeEvent(event);
    if (event->type() == QEvent::ApplicationFontChange || event->type() == QEvent::FontChange) {
        setTabStopDistance(fontMetrics().horizontalAdvance(QLatin1Char(' ')) * kIndentWidth);
        updateLineNumberAreaWidth();
    }
}

void CodeEdit::setApi(const QJsonObject& api) {
    QStringList keywords;
    for (const QJsonValue& value : api.value(QStringLiteral("keywords")).toArray()) {
        keywords.append(value.toString());
    }
    QStringList modules;
    moduleMembers_.clear();
    descriptions_.clear();
    globalWords_ = keywords;
    for (const QJsonValue& value : api.value(QStringLiteral("modules")).toArray()) {
        const QJsonObject module = value.toObject();
        const QString name = module.value(QStringLiteral("module")).toString();
        QStringList members;
        for (const QJsonValue& member : module.value(QStringLiteral("members")).toArray()) {
            members.append(member.toString());
        }
        moduleMembers_.insert(name, members);
        modules.append(name);
    }
    // The engine's own modules (world, game, log) come from the function table.
    for (const QJsonValue& value : api.value(QStringLiteral("functions")).toArray()) {
        const QJsonObject function = value.toObject();
        const QString module = function.value(QStringLiteral("module")).toString();
        const QString name = function.value(QStringLiteral("name")).toString();
        if (!modules.contains(module)) {
            modules.append(module);
        }
        QStringList& members = moduleMembers_[module];
        if (!members.contains(name)) {
            members.append(name);
        }
        descriptions_.insert(module + QLatin1Char('.') + name,
                             QStringLiteral("%1 — %2\nEx.: %3")
                                 .arg(function.value(QStringLiteral("signature")).toString(),
                                      function.value(QStringLiteral("summary")).toString(),
                                      function.value(QStringLiteral("example")).toString()));
    }
    for (const QJsonValue& value : api.value(QStringLiteral("handlers")).toArray()) {
        const QJsonObject handler = value.toObject();
        const QString name = handler.value(QStringLiteral("name")).toString();
        globalWords_.append(name);
        descriptions_.insert(name, handler.value(QStringLiteral("help")).toString());
    }
    for (const QJsonValue& value : api.value(QStringLiteral("globals")).toArray()) {
        globalWords_.append(value.toString());
    }
    globalWords_.append(modules);
    globalWords_.removeDuplicates();
    globalWords_.sort(Qt::CaseInsensitive);
    highlighter_->setVocabulary(keywords, modules);
}

void CodeEdit::setMarkers(const QList<CodeMarker>& markers) {
    markers_ = markers;
    applySelections();
    gutter_->update();
}

void CodeEdit::goToLine(int line, int column) {
    QTextCursor cursor(document()->findBlockByNumber(qMax(0, line - 1)));
    cursor.movePosition(QTextCursor::Right, QTextCursor::MoveAnchor, qMax(0, column - 1));
    setTextCursor(cursor);
    centerCursor();
    setFocus();
}

// ---- gutter ---------------------------------------------------------------------------------

int CodeEdit::lineNumberAreaWidth() const {
    int digits = 1;
    for (int count = qMax(1, blockCount()); count >= 10; count /= 10) {
        ++digits;
    }
    // Room for the problem dot as well as the digits.
    return fontMetrics().height() + 8 + fontMetrics().horizontalAdvance(QLatin1Char('9')) * digits;
}

void CodeEdit::updateLineNumberAreaWidth() {
    setViewportMargins(lineNumberAreaWidth(), 0, 0, 0);
}

void CodeEdit::updateLineNumberArea(const QRect& rect, int dy) {
    if (dy != 0) {
        gutter_->scroll(0, dy);
    } else {
        gutter_->update(0, rect.y(), gutter_->width(), rect.height());
    }
    if (rect.contains(viewport()->rect())) {
        updateLineNumberAreaWidth();
    }
}

void CodeEdit::resizeEvent(QResizeEvent* event) {
    QPlainTextEdit::resizeEvent(event);
    const QRect area = contentsRect();
    gutter_->setGeometry(QRect(area.left(), area.top(), lineNumberAreaWidth(), area.height()));
}

void CodeEdit::lineNumberAreaPaintEvent(QPaintEvent* event) {
    const PetuniaTheme& theme = PetuniaTheme::instance();
    QPainter painter(gutter_);
    painter.fillRect(event->rect(), theme.bgPanelHeader());
    QTextBlock block = firstVisibleBlock();
    int number = block.blockNumber();
    int top = qRound(blockBoundingGeometry(block).translated(contentOffset()).top());
    int bottom = top + qRound(blockBoundingRect(block).height());
    const int dot = fontMetrics().height() / 2;
    while (block.isValid() && top <= event->rect().bottom()) {
        if (block.isVisible() && bottom >= event->rect().top()) {
            const bool current = number == textCursor().blockNumber();
            painter.setPen(current ? theme.textPrimary() : theme.textMuted());
            painter.drawText(0, top, gutter_->width() - 4, fontMetrics().height(), Qt::AlignRight,
                             QString::number(number + 1));
            // The worst problem of the line: a filled dot (shape, not only color, marks a problem).
            const CodeMarker* worst = nullptr;
            for (const CodeMarker& marker : markers_) {
                if (marker.line == number + 1 && (!worst || marker.severity == QLatin1String("error"))) {
                    worst = &marker;
                }
            }
            if (worst) {
                painter.setPen(Qt::NoPen);
                painter.setBrush(worst->severity == QLatin1String("error") ? theme.statusError() : theme.statusWarning());
                painter.drawEllipse(QRect(3, top + (fontMetrics().height() - dot) / 2, dot, dot));
            }
        }
        block = block.next();
        top = bottom;
        bottom = top + qRound(blockBoundingRect(block).height());
        ++number;
    }
}

// ---- selections and tooltips ----------------------------------------------------------------

void CodeEdit::applySelections() {
    const PetuniaTheme& theme = PetuniaTheme::instance();
    QList<QTextEdit::ExtraSelection> selections;
    if (!isReadOnly()) {
        QTextEdit::ExtraSelection line;
        line.format.setBackground(theme.bgSurfaceHover());
        line.format.setProperty(QTextFormat::FullWidthSelection, true);
        line.cursor = textCursor();
        line.cursor.clearSelection();
        selections.append(line);
    }
    for (const CodeMarker& marker : std::as_const(markers_)) {
        QTextBlock first = document()->findBlockByNumber(marker.line - 1);
        QTextBlock last = document()->findBlockByNumber(marker.endLine - 1);
        if (!first.isValid()) {
            continue;
        }
        QTextCursor cursor(first);
        cursor.setPosition(first.position() + qMin(qMax(0, marker.column - 1), first.length() - 1));
        int endPosition = cursor.position() + 1;
        if (last.isValid() && (marker.endLine > marker.line || marker.endColumn > marker.column)) {
            endPosition = last.position() + qMin(qMax(0, marker.endColumn - 1), last.length() - 1);
        }
        endPosition = qMin(endPosition, document()->characterCount() - 1);
        if (endPosition <= cursor.position()) {
            // A zero-width problem still gets a visible mark: the rest of the word, or one character.
            cursor.movePosition(QTextCursor::EndOfWord, QTextCursor::KeepAnchor);
            if (!cursor.hasSelection()) {
                cursor.movePosition(QTextCursor::NextCharacter, QTextCursor::KeepAnchor);
            }
        } else {
            cursor.setPosition(endPosition, QTextCursor::KeepAnchor);
        }
        QTextEdit::ExtraSelection mark;
        mark.cursor = cursor;
        mark.format.setUnderlineStyle(QTextCharFormat::WaveUnderline);
        mark.format.setUnderlineColor(marker.severity == QLatin1String("error") ? theme.statusError() : theme.statusWarning());
        selections.append(mark);
    }
    setExtraSelections(selections);
    gutter_->update();
}

const CodeMarker* CodeEdit::markerAt(const QPoint& position) const {
    const QTextCursor cursor = cursorForPosition(position);
    const int line = cursor.blockNumber() + 1;
    const int column = cursor.positionInBlock() + 1;
    for (const CodeMarker& marker : markers_) {
        const bool afterStart = line > marker.line || (line == marker.line && column >= marker.column);
        const bool beforeEnd = line < marker.endLine || (line == marker.endLine && column <= qMax(marker.endColumn, marker.column + 1));
        if (afterStart && beforeEnd) {
            return &marker;
        }
    }
    return nullptr;
}

bool CodeEdit::event(QEvent* event) {
    if (event->type() == QEvent::ToolTip) {
        auto* help = static_cast<QHelpEvent*>(event);
        if (const CodeMarker* marker = markerAt(help->pos())) {
            QToolTip::showText(help->globalPos(), QStringLiteral("%1 (%2)").arg(marker->message, marker->code), this);
        } else {
            QToolTip::hideText();
        }
        return true;
    }
    if (event->type() == QEvent::ShortcutOverride) {
        // Ctrl+S saves the script while the cursor is in it, instead of the scene.
        auto* key = static_cast<QKeyEvent*>(event);
        if (key->matches(QKeySequence::Save)) {
            event->accept();
            return true;
        }
    }
    return QPlainTextEdit::event(event);
}

// ---- keys, indentation, completion ----------------------------------------------------------

QString CodeEdit::wordBeforeCursor(QString* module) const {
    const QTextBlock block = textCursor().block();
    const QString text = block.text();
    int end = textCursor().positionInBlock();
    int start = end;
    while (start > 0 && isWordCharacter(text.at(start - 1))) {
        --start;
    }
    if (module) {
        module->clear();
        if (start > 0 && text.at(start - 1) == QLatin1Char('.')) {
            int moduleEnd = start - 1;
            int moduleStart = moduleEnd;
            while (moduleStart > 0 && isWordCharacter(text.at(moduleStart - 1))) {
                --moduleStart;
            }
            *module = text.mid(moduleStart, moduleEnd - moduleStart);
        }
    }
    return text.mid(start, end - start);
}

void CodeEdit::showCompletion(bool forced) {
    QString module;
    const QString prefix = wordBeforeCursor(&module);
    const bool members = !module.isEmpty() && moduleMembers_.contains(module);
    if (!members && (prefix.size() < 2 && !forced)) {
        completer_->popup()->hide();
        return;
    }
    completionModule_ = members ? module : QString();
    completionModel_->setStringList(members ? moduleMembers_.value(module) : globalWords_);
    completer_->setCompletionPrefix(prefix);
    if (completer_->completionCount() == 0 || (!forced && !members && completer_->completionCount() == 1 &&
                                                completer_->currentCompletion() == prefix)) {
        completer_->popup()->hide();
        return;
    }
    completer_->popup()->setCurrentIndex(completer_->completionModel()->index(0, 0));
    QRect box = cursorRect();
    box.setWidth(completer_->popup()->sizeHintForColumn(0) + completer_->popup()->verticalScrollBar()->sizeHint().width());
    completer_->complete(box);
}

void CodeEdit::insertCompletion(const QString& completion) {
    QString module;
    const QString prefix = wordBeforeCursor(&module);
    QTextCursor cursor = textCursor();
    cursor.movePosition(QTextCursor::Left, QTextCursor::KeepAnchor, prefix.size());
    cursor.insertText(completion);
    setTextCursor(cursor);
    emit hintChanged(QString());
}

void CodeEdit::indentSelection(bool outdent) {
    QTextCursor cursor = textCursor();
    const QString unit(kIndentWidth, QLatin1Char(' '));
    cursor.beginEditBlock();
    const int first = document()->findBlock(cursor.selectionStart()).blockNumber();
    const int last = document()->findBlock(cursor.selectionEnd()).blockNumber();
    for (int number = first; number <= last; ++number) {
        QTextCursor line(document()->findBlockByNumber(number));
        if (outdent) {
            int removable = 0;
            const QString text = line.block().text();
            while (removable < kIndentWidth && removable < text.size() && text.at(removable) == QLatin1Char(' ')) {
                ++removable;
            }
            line.movePosition(QTextCursor::Right, QTextCursor::KeepAnchor, removable);
            line.removeSelectedText();
        } else {
            line.insertText(unit);
        }
    }
    cursor.endEditBlock();
}

void CodeEdit::keyPressEvent(QKeyEvent* event) {
    const int key = event->key();
    if (completer_->popup()->isVisible()) {
        switch (key) {
        case Qt::Key_Enter:
        case Qt::Key_Return:
        case Qt::Key_Tab:
        case Qt::Key_Backtab:
        case Qt::Key_Escape:
            event->ignore();  // the completer takes these
            return;
        default:
            break;
        }
    }
    if (event->matches(QKeySequence::Save)) {
        emit saveRequested();
        return;
    }
    if (key == Qt::Key_Escape) {
        escapeLatch_ = true;
        QPlainTextEdit::keyPressEvent(event);
        return;
    }
    if ((key == Qt::Key_Tab || key == Qt::Key_Backtab) && escapeLatch_) {
        escapeLatch_ = false;
        focusNextPrevChild(key == Qt::Key_Tab);  // Esc then Tab: leave the editor
        return;
    }
    escapeLatch_ = false;
    if (key == Qt::Key_Space && event->modifiers().testFlag(Qt::ControlModifier)) {
        showCompletion(true);
        return;
    }
    if (key == Qt::Key_Tab && !event->modifiers().testFlag(Qt::ControlModifier)) {
        if (textCursor().hasSelection()) {
            indentSelection(false);
        } else {
            const int column = textCursor().positionInBlock();
            textCursor().insertText(QString(kIndentWidth - column % kIndentWidth, QLatin1Char(' ')));
        }
        return;
    }
    if (key == Qt::Key_Backtab) {
        indentSelection(true);
        return;
    }
    if (key == Qt::Key_Return || key == Qt::Key_Enter) {
        // Keep the indentation of the line; one level deeper after an opening brace.
        const QString before = textCursor().block().text().left(textCursor().positionInBlock());
        QString indent;
        for (const QChar character : before) {
            if (character != QLatin1Char(' ') && character != QLatin1Char('\t')) {
                break;
            }
            indent += character;
        }
        const bool opens = before.trimmed().endsWith(QLatin1Char('{'));
        textCursor().insertText(QLatin1Char('\n') + indent + (opens ? QString(kIndentWidth, QLatin1Char(' ')) : QString()));
        ensureCursorVisible();
        return;
    }
    QPlainTextEdit::keyPressEvent(event);
    if (key == Qt::Key_BraceRight) {
        // A closing brace alone on its line steps back one level.
        QTextCursor cursor = textCursor();
        const QString text = cursor.block().text();
        if (text.trimmed() == QLatin1String("}") && text.startsWith(QString(kIndentWidth, QLatin1Char(' ')))) {
            cursor.movePosition(QTextCursor::StartOfBlock);
            cursor.movePosition(QTextCursor::Right, QTextCursor::KeepAnchor, kIndentWidth);
            cursor.removeSelectedText();
        }
    }
    const QString printable = event->text();
    if (!printable.isEmpty() && (isWordCharacter(printable.at(0)) || printable.at(0) == QLatin1Char('.'))) {
        showCompletion(false);
    } else if (key == Qt::Key_Backspace) {
        completer_->popup()->hide();
    }
}
