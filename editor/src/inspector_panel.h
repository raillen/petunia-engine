#pragma once

#include <QJsonObject>
#include <QJsonValue>
#include <QSet>
#include <QWidget>

#include <functional>
#include <memory>
#include <vector>

class EngineBridge;
class QJsonArray;
class QVBoxLayout;
struct Reply;

// Shows the selected object's tables and lets you edit every value. Fields are built from the
// column kinds the engine reports, so a new table shows up here with no GUI change. Every edit
// is a `world.set` command, so it is validated by the engine and can be undone.
class InspectorPanel : public QWidget {
    Q_OBJECT

public:
    explicit InspectorPanel(EngineBridge* bridge, QWidget* parent = nullptr);

    // Follows the selection and the world: updates values in place when the layout is the same.
    void reload();

signals:
    // An edit was applied; the owner should refresh the snapshot.
    void edited();
    // A value is being changed live (a drag): redraw the viewport, do not rebuild the panel.
    void previewed();
    void editFailed(const QString& explanation);

private:
    struct Field {
        QString table;
        QString column;
        QJsonValue current;
        std::function<void(const QJsonValue&)> show;
    };

    void rebuild(const QJsonObject& description);
    void refill(const QJsonObject& description);
    void showMessage(const QString& text);
    // One table: a header row (fold, icon, name, menu) and a body with a label column.
    QWidget* makeSection(const QString& tableName, const QJsonArray& columns);
    void clearContent();
    QWidget* makeEditor(const QString& table, const QJsonObject& column, Field* field);
    void apply(Field* field, const QJsonValue& value);
    // Live drag of a value: preview -> commit (one undo step) or cancel.
    void previewValue(Field* field, const QJsonValue& value);
    void commitValue(Field* field, const QJsonValue& value);
    void cancelValue();
    void runCommand(const QString& command, const QJsonObject& payload);
    // An editor operation that is not a plain command (`collider.add`, `collider.fit`).
    void runOperation(const QString& operation, const QJsonObject& fields);

    EngineBridge* bridge_;
    QVBoxLayout* content_;
    QString signature_;
    quint64 entity_ = 0;
    std::vector<std::unique_ptr<Field>> fields_;
    // Tables the user folded; kept while the panel is rebuilt for other objects.
    QSet<QString> collapsed_;
};
