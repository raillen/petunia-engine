#pragma once

#include <QString>
#include <QStringList>

// Icon names (":/icons/<name>.svg") for the engine's tables, so the hierarchy and the inspector
// show the same picture for the same kind of data. Unknown tables get a neutral icon.
namespace TableIcons {

inline QString forTable(const QString& table) {
    if (table == QLatin1String("Name")) return QStringLiteral("annotate");
    if (table == QLatin1String("Transform")) return QStringLiteral("transform");
    if (table == QLatin1String("Brush")) return QStringLiteral("cube");
    if (table == QLatin1String("Floor")) return QStringLiteral("grid_overlay");
    if (table == QLatin1String("Door")) return QStringLiteral("connect");
    if (table == QLatin1String("PlayerStart")) return QStringLiteral("cursor_3d");
    if (table == QLatin1String("Sculpt")) return QStringLiteral("generate_volume");
    if (table == QLatin1String("SculptStroke")) return QStringLiteral("paint_brush");
    if (table == QLatin1String("Script")) return QStringLiteral("script");
    return QStringLiteral("parts");
}

// An object is shown by its most telling table: a cave says more than its transform.
inline QString forEntity(const QStringList& tables) {
    for (const char* table : {"PlayerStart", "Floor", "Sculpt", "Door", "Brush", "Script"}) {
        if (tables.contains(QLatin1String(table))) {
            return forTable(QLatin1String(table));
        }
    }
    return QStringLiteral("object_mesh");
}

}  // namespace TableIcons
