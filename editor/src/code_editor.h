#pragma once

#include <QHash>
#include <QJsonObject>
#include <QList>
#include <QPlainTextEdit>
#include <QSet>
#include <QStringList>
#include <QSyntaxHighlighter>
#include <QTextCharFormat>

class QCompleter;
class QStringListModel;

// One problem the engine found in the text, drawn as an underline and a dot in the gutter.
struct CodeMarker {
    int line = 1;  // 1-based, as the compiler reports them
    int column = 1;
    int endLine = 1;
    int endColumn = 1;
    QString severity;  // "error", "warning" or "info"
    QString code;
    QString message;
};

// Colors the Aipo source: comments, strings, numbers, keywords and the engine's modules. The
// vocabulary comes from the engine (the lexer's own keyword table), not from this file.
class AipoHighlighter : public QSyntaxHighlighter {
public:
    explicit AipoHighlighter(QTextDocument* document);
    void setVocabulary(const QStringList& keywords, const QStringList& modules);
    void refreshColors();

protected:
    void highlightBlock(const QString& text) override;

private:
    QSet<QString> keywords_;
    QSet<QString> modules_;
    QTextCharFormat keywordFormat_;
    QTextCharFormat moduleFormat_;
    QTextCharFormat stringFormat_;
    QTextCharFormat numberFormat_;
    QTextCharFormat commentFormat_;
    QTextCharFormat directiveFormat_;
};

// The code editor: line numbers, the highlighter, error underlines, completion from the engine's
// API, auto-indent. Tab indents; Esc then Tab leaves the editor (so the keyboard is never trapped).
class CodeEdit : public QPlainTextEdit {
    Q_OBJECT

public:
    explicit CodeEdit(QWidget* parent = nullptr);

    // The engine's `script.api` answer: keywords, modules, functions and handlers.
    void setApi(const QJsonObject& api);
    void setMarkers(const QList<CodeMarker>& markers);
    const QList<CodeMarker>& markers() const { return markers_; }
    void goToLine(int line, int column);
    void refreshAppearance();

    int lineNumberAreaWidth() const;
    void lineNumberAreaPaintEvent(QPaintEvent* event);

signals:
    void saveRequested();
    // The signature and summary of the completion being looked at.
    void hintChanged(const QString& text);

protected:
    void resizeEvent(QResizeEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    bool event(QEvent* event) override;
    void changeEvent(QEvent* event) override;

private:
    void updateLineNumberAreaWidth();
    void updateLineNumberArea(const QRect& rect, int dy);
    void applySelections();
    void showCompletion(bool forced);
    void insertCompletion(const QString& completion);
    void indentSelection(bool outdent);
    QString wordBeforeCursor(QString* module) const;
    const CodeMarker* markerAt(const QPoint& position) const;

    QWidget* gutter_ = nullptr;
    AipoHighlighter* highlighter_ = nullptr;
    QCompleter* completer_ = nullptr;
    QStringListModel* completionModel_ = nullptr;
    QList<CodeMarker> markers_;
    QStringList globalWords_;
    QHash<QString, QStringList> moduleMembers_;
    QHash<QString, QString> descriptions_;
    QString completionModule_;
    // After Esc the next Tab moves focus instead of indenting.
    bool escapeLatch_ = false;

    friend class LineNumberArea;
};
