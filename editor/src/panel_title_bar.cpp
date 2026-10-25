#include "panel_title_bar.h"

#include <QDockWidget>
#include <QHBoxLayout>
#include <QLabel>
#include <QToolButton>

#include "petunia_icons.h"
#include "petunia_theme.h"

PanelTitleBar::PanelTitleBar(QDockWidget* dock) : QWidget(dock) {
    setObjectName(QStringLiteral("PetuniaPanelTitle"));
    setAttribute(Qt::WA_StyledBackground);
    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(8, 2, 4, 2);
    layout->setSpacing(2);

    auto* title = new QLabel(dock->windowTitle(), this);
    title->setProperty("role", QStringLiteral("panelTitle"));
    layout->addWidget(title);
    layout->addStretch(1);

    auto makeButton = [this, layout](const QString& tip) {
        auto* button = new QToolButton(this);
        button->setProperty("role", QStringLiteral("panelButton"));
        button->setToolTip(tip);
        button->setAccessibleName(tip);
        button->setIconSize(QSize(14, 14));
        button->setAutoRaise(true);
        layout->addWidget(button);
        return button;
    };
    floatButton_ = makeButton(QStringLiteral("Soltar ou prender o painel %1").arg(dock->windowTitle()));
    closeButton_ = makeButton(QStringLiteral("Fechar o painel %1 (Exibir reabre)").arg(dock->windowTitle()));
    connect(floatButton_, &QToolButton::clicked, dock, [dock] { dock->setFloating(!dock->isFloating()); });
    connect(closeButton_, &QToolButton::clicked, dock, &QDockWidget::close);
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, [this] { refreshIcons(); });
    refreshIcons();
}

void PanelTitleBar::refreshIcons() {
    floatButton_->setIcon(PetuniaIcons::get(QStringLiteral("picture_in_picture"), PetuniaTheme::instance().textSecondary().name()));
    closeButton_->setIcon(PetuniaIcons::get(QStringLiteral("close"), PetuniaTheme::instance().textSecondary().name()));
}
