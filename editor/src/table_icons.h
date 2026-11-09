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
    if (table == QLatin1String("Collider")) return QStringLiteral("bounding_box");
    if (table == QLatin1String("RigidBody")) return QStringLiteral("bolt");
    if (table == QLatin1String("Character")) return QStringLiteral("orbit");
    if (table == QLatin1String("Sprite")) return QStringLiteral("texture");
    if (table == QLatin1String("SpriteAnimation")) return QStringLiteral("play");
    if (table == QLatin1String("Light")) return QStringLiteral("view_lit");
    return QStringLiteral("parts");
}

// The name people read for a table. The engine's name (used by scripts) stays in the tooltip.
inline QString displayName(const QString& table) {
    if (table == QLatin1String("Name")) return QStringLiteral("Nome");
    if (table == QLatin1String("Transform")) return QStringLiteral("Transformação");
    if (table == QLatin1String("Brush")) return QStringLiteral("Forma");
    if (table == QLatin1String("Floor")) return QStringLiteral("Andar");
    if (table == QLatin1String("Door")) return QStringLiteral("Porta");
    if (table == QLatin1String("PlayerStart")) return QStringLiteral("Início do jogador");
    if (table == QLatin1String("Sculpt")) return QStringLiteral("Caverna");
    if (table == QLatin1String("SculptStroke")) return QStringLiteral("Traço de caverna");
    if (table == QLatin1String("Script")) return QStringLiteral("Script");
    if (table == QLatin1String("Collider")) return QStringLiteral("Colisão");
    if (table == QLatin1String("RigidBody")) return QStringLiteral("Corpo físico");
    if (table == QLatin1String("Character")) return QStringLiteral("Personagem");
    if (table == QLatin1String("Sprite")) return QStringLiteral("Imagem 2D");
    if (table == QLatin1String("SpriteAnimation")) return QStringLiteral("Animação");
    if (table == QLatin1String("Light")) return QStringLiteral("Luz");
    return table;
}

// An object is shown by its most telling table: a cave says more than its transform.
inline QString forEntity(const QStringList& tables) {
    for (const char* table : {"Sprite", "Light", "Character", "Collider", "PlayerStart", "Floor", "Sculpt", "Door", "Brush", "Script"}) {
        if (tables.contains(QLatin1String(table))) {
            return forTable(QLatin1String(table));
        }
    }
    return QStringLiteral("object_mesh");
}

}  // namespace TableIcons
