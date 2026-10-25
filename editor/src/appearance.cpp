#include "appearance.h"

#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QtGlobal>

#include "petunia_theme.h"

namespace Appearance {

namespace {

QJsonObject readTheme(const QString& id) {
    QFile file(QStringLiteral(":/themes/%1.json").arg(id));
    if (!file.open(QIODevice::ReadOnly)) {
        return {};
    }
    return QJsonDocument::fromJson(file.readAll()).object();
}

}  // namespace

QList<ThemeInfo> themes() {
    QList<ThemeInfo> found;
    for (const QString& id : {QStringLiteral("petunia-light"), QStringLiteral("petunia-dark"),
                              QStringLiteral("petunia-high-contrast")}) {
        const QJsonObject theme = readTheme(id);
        if (!theme.isEmpty()) {
            found.append({id, theme.value(QStringLiteral("name")).toString(id)});
        }
    }
    return found;
}

bool applyTheme(const QString& id) {
    const QJsonObject theme = readTheme(id);
    if (theme.isEmpty()) {
        return false;
    }
    PetuniaTheme& tokens = PetuniaTheme::instance();
    tokens.updateFromSnapshot(theme.value(QStringLiteral("colors")).toObject(), id);
    tokens.applyToApplication();
    return true;
}

double scaleForStep(int step) {
    return 1.0 + 0.1 * qBound(kMinTextStep, step, kMaxTextStep);
}

}  // namespace Appearance
