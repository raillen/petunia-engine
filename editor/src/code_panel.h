#pragma once

#include <QJsonArray>
#include <QString>
#include <QWidget>

class CodeEdit;
class EngineBridge;
class QComboBox;
class QLabel;
class QListWidget;
class QPlainTextEdit;
class QPushButton;
class QToolButton;
class QSpinBox;
class QTabWidget;
class QTimer;

// The embedded code editor: the project's scripts, checked as you type by the engine's own
// compiler, tried on a copy of the world (nothing in the scene changes), formatted and saved with
// a backup of the previous version.
class CodePanel : public QWidget {
    Q_OBJECT

public:
    CodePanel(EngineBridge* bridge, QWidget* parent = nullptr);

    // Asks what to do with unsaved text. `false` means the person chose to stay.
    bool maybeSave();
    // Puts the cursor in the editor.
    void focusEditor();
    // Opens a script by name (after it was created or attached elsewhere).
    void open(const QString& name);

private:
    void onSnapshot();
    void reloadList(const QString& select = QString());
    int rowOf(const QString& name) const;
    void load(const QString& name);
    void save();
    void format();
    void run();
    void create();
    void attach();
    void scheduleCheck();
    void check();
    void showProblems(const QJsonArray& diagnostics);
    void setDirty(bool dirty);
    void updateTitle();
    void console(const QString& line);
    void say(const QString& message);

    EngineBridge* bridge_;
    CodeEdit* editor_;
    // Scripts of the project, one per row; the row data is the file name.
    QListWidget* scripts_;
    // Shown instead of the list while the project has no script: what to do next.
    QLabel* noScripts_;
    QToolButton* saveButton_;
    QToolButton* formatButton_;
    QToolButton* runButton_;
    QToolButton* attachButton_;
    QSpinBox* frames_;
    QLabel* status_;
    QLabel* hint_;
    QTabWidget* output_;
    QListWidget* problems_;
    QPlainTextEdit* console_;
    QTimer* checkTimer_;
    QString current_;
    QString projectKey_;
    bool dirty_ = false;
    bool loading_ = false;
};
