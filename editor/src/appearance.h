#pragma once

#include <QList>
#include <QString>

// Themes, density, text scale and motion: the look of the editor, in one place. The tokens and
// the style sheet come from PetuniaTheme (copied from Petunia3D); this only chooses and remembers.
namespace Appearance {

struct ThemeInfo {
    QString id;
    QString name;
};

// Themes embedded in the executable (":/themes/<id>.json").
QList<ThemeInfo> themes();

// Loads a theme by id and applies it to the whole application. False if it does not exist.
bool applyTheme(const QString& id);

// Text size steps: scale = 1.0 + 0.1 * step.
constexpr int kMinTextStep = -2;
constexpr int kMaxTextStep = 10;
double scaleForStep(int step);

}  // namespace Appearance
