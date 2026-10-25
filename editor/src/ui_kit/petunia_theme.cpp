#include "petunia_theme.h"

#include <QApplication>
#include <QHash>
#include <QStyle>
#include <QStyleFactory>
#include <algorithm>
#include <cmath>
#include <tuple>

namespace {

QColor parseToken(const QJsonObject &colors, const char *key, const QColor &fallback) {
    const QString value = colors.value(QLatin1String(key)).toString();
    if (value.isEmpty())
        return fallback;
    const QColor parsed(value);
    return parsed.isValid() ? parsed : fallback;
}

QString css(const QColor &color) {
    if (color.alpha() == 255)
        return color.name(QColor::HexRgb);
    return QStringLiteral("rgba(%1,%2,%3,%4)")
        .arg(color.red())
        .arg(color.green())
        .arg(color.blue())
        .arg(QString::number(color.alphaF(), 'f', 3));
}

QColor withAlpha(QColor color, double alpha) {
    color.setAlphaF(alpha);
    return color;
}

double luminance(const QColor &color) {
    auto channel = [](double c) {
        return c <= 0.03928 ? c / 12.92 : std::pow((c + 0.055) / 1.055, 2.4);
    };
    return 0.2126 * channel(color.redF()) + 0.7152 * channel(color.greenF()) +
           0.0722 * channel(color.blueF());
}

} // namespace

PetuniaTheme &PetuniaTheme::instance() {
    static PetuniaTheme theme;
    return theme;
}

PetuniaTheme::PetuniaTheme() : QObject(nullptr) {}

void PetuniaTheme::markDirty() {
    m_styleDirty = true;
    emit changed();
}

void PetuniaTheme::updateFromSnapshot(const QJsonObject &colors, const QString &themeId) {
    const auto before = std::make_tuple(
        m_themeId, m_bgCanvas, m_bgHeader, m_bgPanel, m_bgPanelHeader, m_bgSurface,
        m_bgSurfaceHover, m_bgSurfaceActive, m_textPrimary, m_textSecondary, m_textMuted,
        m_textActive, m_accentPrimary, m_accentSelection, m_accentSelectionActive,
        m_borderSubtle, m_borderStrong, m_borderFocus);
    const auto statusBefore = std::make_tuple(m_statusInfo, m_statusWarning, m_statusError,
                                              m_statusSuccess, m_axisX, m_axisY, m_axisZ);

    m_themeId = themeId.isEmpty() ? QStringLiteral("petunia-dark") : themeId;
    m_bgCanvas = parseToken(colors, "bg_canvas", QColor(QStringLiteral("#121212")));
    m_bgHeader = parseToken(colors, "bg_header", QColor(QStringLiteral("#191919")));
    m_bgPanel = parseToken(colors, "bg_panel", QColor(QStringLiteral("#202020")));
    m_bgPanelHeader = parseToken(colors, "bg_panel_header", QColor(QStringLiteral("#252525")));
    m_bgSurface = parseToken(colors, "bg_surface", QColor(QStringLiteral("#292929")));
    m_bgSurfaceHover = parseToken(colors, "bg_surface_hover", QColor(QStringLiteral("#353535")));
    m_bgSurfaceActive = parseToken(colors, "bg_surface_active", QColor(QStringLiteral("#404040")));

    m_textPrimary = parseToken(colors, "text_primary", QColor(QStringLiteral("#eeeeee")));
    m_textSecondary = parseToken(colors, "text_secondary", QColor(QStringLiteral("#b5b5b5")));
    m_textMuted = parseToken(colors, "text_muted", QColor(QStringLiteral("#8a8a8a")));
    m_textActive = parseToken(colors, "text_active", QColor(QStringLiteral("#121212")));

    // `accent_blue` guarda o roxo floral de interação; `accent_orange` é a
    // seleção 3D (legado de nomes preservado por UI-003).
    m_accentPrimary = parseToken(colors, "accent_blue", QColor(QStringLiteral("#b58cff")));
    m_accentSelection = parseToken(colors, "accent_orange", QColor(QStringLiteral("#e96a00")));
    m_accentSelectionActive = parseToken(colors, "accent_hover", QColor(QStringLiteral("#ffaf29")));

    m_borderSubtle = parseToken(colors, "border_subtle", QColor(QStringLiteral("#404040")));
    m_borderStrong = parseToken(colors, "border_strong", QColor(QStringLiteral("#5a5a5a")));
    m_borderFocus = parseToken(colors, "border_focus", QColor(QStringLiteral("#b58cff")));

    m_statusInfo = parseToken(colors, "status_info", QColor(QStringLiteral("#6cb6ff")));
    m_statusWarning = parseToken(colors, "status_warning", QColor(QStringLiteral("#e5bd67")));
    m_statusError = parseToken(colors, "status_error", QColor(QStringLiteral("#ef6b73")));
    m_statusSuccess = parseToken(colors, "status_success", QColor(QStringLiteral("#73d59b")));

    if (isHighContrast()) {
        m_axisX = QColor(QStringLiteral("#ff3333"));
        m_axisY = QColor(QStringLiteral("#33ff33"));
        m_axisZ = QColor(QStringLiteral("#3399ff"));
    } else {
        m_axisX = QColor(QStringLiteral("#ff6e6e"));
        m_axisY = QColor(QStringLiteral("#69db7c"));
        m_axisZ = QColor(QStringLiteral("#4dabf7"));
    }

    const auto after = std::make_tuple(
        m_themeId, m_bgCanvas, m_bgHeader, m_bgPanel, m_bgPanelHeader, m_bgSurface,
        m_bgSurfaceHover, m_bgSurfaceActive, m_textPrimary, m_textSecondary, m_textMuted,
        m_textActive, m_accentPrimary, m_accentSelection, m_accentSelectionActive,
        m_borderSubtle, m_borderStrong, m_borderFocus);
    const auto statusAfter = std::make_tuple(m_statusInfo, m_statusWarning, m_statusError,
                                             m_statusSuccess, m_axisX, m_axisY, m_axisZ);
    if (before != after || statusBefore != statusAfter)
        markDirty();
}

void PetuniaTheme::setDensity(Density density) {
    if (m_density == density)
        return;
    m_density = density;
    markDirty();
}

void PetuniaTheme::setUiScale(double scale) {
    const double clamped = std::clamp(scale, 0.75, 2.0);
    if (std::abs(clamped - m_uiScale) < 1e-6)
        return;
    m_uiScale = clamped;
    markDirty();
}

void PetuniaTheme::setReducedMotion(bool reduced) {
    if (m_reducedMotion == reduced)
        return;
    m_reducedMotion = reduced;
    markDirty();
}

int PetuniaTheme::scaled(int px) const {
    return std::max(1, int(std::lround(px * m_uiScale)));
}

int PetuniaTheme::controlHeight() const {
    // Alvo mínimo de 28 px lógicos mesmo com escala de interface abaixo de 100%.
    switch (m_density) {
    case Density::Compact: return std::max(28, scaled(28));
    case Density::Standard: return std::max(28, scaled(32));
    case Density::Comfortable: return std::max(28, scaled(40));
    }
    return std::max(28, scaled(32));
}

int PetuniaTheme::fieldHeight() const {
    switch (m_density) {
    case Density::Compact:
        return std::max(28, scaled(28));
    case Density::Standard: return std::max(28, scaled(30));
    case Density::Comfortable: return std::max(28, scaled(36));
    }
    return std::max(28, scaled(28));
}

int PetuniaTheme::compactControlHeight() const {
    return std::max(28, controlHeight() - scaled(8));
}

int PetuniaTheme::fontPx(TextRole role) const {
    const int body = m_density == Density::Compact ? 13 : m_density == Density::Standard ? 14 : 15;
    // Texto operacional nunca abaixo de 12 px lógicos (U08), em qualquer escala.
    switch (role) {
    case TextRole::Caption: return std::max(12, scaled(body - 1));
    case TextRole::Body: return std::max(12, scaled(body));
    case TextRole::Title: return std::max(14, scaled(body + 2));
    case TextRole::Mono: return std::max(12, scaled(body - 1));
    }
    return std::max(12, scaled(body));
}

int PetuniaTheme::space(int step) const {
    // Densidade muda controles e tipografia, não o ritmo da grade inteira.
    return scaled(4 * step);
}

int PetuniaTheme::radius() const {
    // Cantos discretos: o visual plano pede bordas finas e raio pequeno.
    return scaled(3);
}

int PetuniaTheme::iconPx() const {
    return scaled(m_density == Density::Compact ? 16 : m_density == Density::Standard ? 18 : 20);
}

int PetuniaTheme::railWidth() const {
    return scaled(m_density == Density::Compact ? 38 : m_density == Density::Standard ? 42 : 46);
}

QColor PetuniaTheme::axisColor(int axis) const {
    switch (axis) {
    case 0: return m_axisX;
    case 1: return m_axisY;
    case 2: return m_axisZ;
    default: return m_textPrimary;
    }
}

QString PetuniaTheme::axisLetter(int axis) {
    switch (axis) {
    case 0: return QStringLiteral("X");
    case 1: return QStringLiteral("Y");
    case 2: return QStringLiteral("Z");
    default: return QString();
    }
}

QColor PetuniaTheme::overlaySurface() const {
    // O texto do HUD não deve competir com linhas do viewport por transparência.
    return m_bgPanel;
}

double PetuniaTheme::contrastRatio(const QColor &a, const QColor &b) {
    const double la = luminance(a);
    const double lb = luminance(b);
    const double hi = std::max(la, lb);
    const double lo = std::min(la, lb);
    return (hi + 0.05) / (lo + 0.05);
}

QPalette PetuniaTheme::palette() const {
    QPalette p;
    p.setColor(QPalette::Window, m_bgPanel);
    p.setColor(QPalette::WindowText, m_textPrimary);
    p.setColor(QPalette::Base, m_bgSurface);
    p.setColor(QPalette::AlternateBase, m_bgPanelHeader);
    p.setColor(QPalette::Text, m_textPrimary);
    p.setColor(QPalette::Button, m_bgSurface);
    p.setColor(QPalette::ButtonText, m_textPrimary);
    p.setColor(QPalette::BrightText, m_textPrimary);
    p.setColor(QPalette::Highlight, m_accentPrimary);
    p.setColor(QPalette::HighlightedText, m_textActive);
    p.setColor(QPalette::ToolTipBase, m_bgSurface);
    p.setColor(QPalette::ToolTipText, m_textPrimary);
    p.setColor(QPalette::PlaceholderText, m_textMuted);
    p.setColor(QPalette::Link, m_statusInfo);
    p.setColor(QPalette::Light, m_bgSurfaceHover);
    p.setColor(QPalette::Midlight, m_bgSurfaceHover);
    p.setColor(QPalette::Mid, m_borderSubtle);
    p.setColor(QPalette::Dark, m_bgCanvas);
    p.setColor(QPalette::Shadow, m_bgCanvas);
    p.setColor(QPalette::Disabled, QPalette::WindowText, m_textMuted);
    p.setColor(QPalette::Disabled, QPalette::Text, m_textMuted);
    p.setColor(QPalette::Disabled, QPalette::ButtonText, m_textMuted);
    return p;
}

const QString &PetuniaTheme::styleSheet() const {
    if (m_styleDirty) {
        m_styleSheet = buildStyleSheet();
        m_styleDirty = false;
    }
    return m_styleSheet;
}

void PetuniaTheme::applyToApplication() const {
    if (auto *app = qobject_cast<QApplication *>(QCoreApplication::instance())) {
        if (app->style()->name().compare(QLatin1String("fusion"), Qt::CaseInsensitive) != 0) {
            if (auto *fusion = QStyleFactory::create(QStringLiteral("Fusion")))
                app->setStyle(fusion);
        }
        app->setPalette(palette());
        app->setStyleSheet(styleSheet());
    }
}

QString PetuniaTheme::buildStyleSheet() const {
    QString s = QStringLiteral(R"QSS(
QWidget { color: @textPrimary@; font-size: @fontBody@px; }
QMainWindow, QDialog { background-color: @bgPanel@; }
QToolTip { background-color: @bgSurface@; color: @textPrimary@; border: 1px solid @borderStrong@; padding: @s1@px @s2@px; }
QLabel { background: transparent; }
QLabel[role="caption"] { color: @textSecondary@; font-size: @fontCaption@px; }
QLabel[role="muted"] { color: @textMuted@; font-size: @fontCaption@px; }
QLabel[role="title"] { font-size: @fontTitle@px; font-weight: 600; }
QLabel[role="error"] { color: @statusError@; }
QLabel[role="keycap"] { background-color: @bgSurfaceActive@; color: @textPrimary@; border: 1px solid @borderStrong@; border-radius: @radius@px; padding: 1px @s1@px; font-size: @fontCaption@px; font-weight: 600; }
QLabel[role="liveTool"] { background-color: @bgSurfaceActive@; color: @textPrimary@; border: 1px solid @borderStrong@; border-radius: @radius@px; padding: 1px @s2@px; font-size: @fontCaption@px; font-weight: 600; }
QFrame[role="card"] { background-color: @bgSurface@; border: 1px solid @borderSubtle@; border-radius: @radiusL@px; }
QFrame[role="overlay"] { background-color: @overlay@; border: 1px solid @borderStrong@; border-radius: @radiusL@px; }
QFrame[role="separator"] { background-color: @borderSubtle@; border: none; }

QPushButton, QToolButton { background-color: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; border-radius: @radius@px; padding: 0 @s3@px; min-height: @controlH@px; }
QPushButton:hover, QToolButton:hover { background-color: @bgSurfaceHover@; border-color: @borderStrong@; }
QPushButton:pressed, QToolButton:pressed { background-color: @bgSurfaceActive@; }
QPushButton:checked, QToolButton:checked { background-color: @bgSurfaceActive@; color: @textPrimary@; border-color: @borderStrong@; }
QPushButton:focus, QToolButton:focus { border: @ring@px solid @borderFocus@; padding: 0 @s3Ring@px; }
QPushButton:disabled, QToolButton:disabled { background-color: @bgPanel@; color: @textMuted@; border-color: @borderSubtle@; }
QPushButton[role="primary"] { background-color: @accent@; color: @textActive@; border-color: @accent@; font-weight: 600; }
QPushButton[role="primary"]:hover { background-color: @accentHover@; border-color: @accentHover@; }
QPushButton[role="quiet"], QToolButton[role="quiet"] { background-color: transparent; border-color: transparent; }
QPushButton[role="quiet"]:hover, QToolButton[role="quiet"]:hover { background-color: @bgSurfaceHover@; }
QPushButton[role="ghost"], QToolButton[role="ghost"] { background-color: transparent; border: 1px solid transparent; border-radius: @radius@px; padding: 0 @s2@px; text-align: left; color: @textPrimary@; }
QPushButton[role="ghost"]:hover, QToolButton[role="ghost"]:hover { background-color: @bgSurfaceHover@; border-color: @borderSubtle@; }
QPushButton[role="ghost"]:pressed, QToolButton[role="ghost"]:pressed, QPushButton[role="ghost"]:checked, QToolButton[role="ghost"]:checked { background-color: @bgSurfaceActive@; border-color: @accent@; }
QPushButton[role="ghost"]:disabled, QToolButton[role="ghost"]:disabled { background-color: transparent; border-color: transparent; color: @textMuted@; }
QPushButton[role="danger"] { color: @statusError@; border-color: @statusError@; }
QToolButton::menu-indicator { image: none; }
QPushButton[role="quiet"]:checked, QToolButton[role="quiet"]:checked { background-color: @accentSoft@; color: @textPrimary@; border-color: @accent@; }

QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox { background-color: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; border-radius: @radius@px; padding: 0 @s2@px; min-height: @fieldH@px; selection-background-color: @accent@; selection-color: @textActive@; }
QLineEdit:hover, QSpinBox:hover, QDoubleSpinBox:hover, QComboBox:hover { border-color: @borderStrong@; }
QLineEdit:focus, QSpinBox:focus, QDoubleSpinBox:focus, QComboBox:focus { border: @ring@px solid @borderFocus@; padding: 0 @s2Ring@px; }
QLineEdit[invalid="true"] { border: @ring@px dashed @statusError@; }
QLineEdit:disabled, QComboBox:disabled { color: @textMuted@; background-color: @bgPanel@; }
QComboBox::drop-down { border: none; width: @s5@px; }
QComboBox QAbstractItemView { background-color: @bgSurface@; border: 1px solid @borderStrong@; selection-background-color: @accent@; selection-color: @textActive@; outline: 0; }

QCheckBox, QRadioButton { spacing: @s2@px; min-height: @fieldH@px; }
QCheckBox:focus, QRadioButton:focus { color: @textPrimary@; }

QMenuBar { background-color: transparent; color: @textPrimary@; }
QMenuBar::item { padding: @s1@px @s2@px; border-radius: @radius@px; background: transparent; }
QMenuBar::item:selected { background-color: @bgSurfaceHover@; }
QMenu { background-color: @bgSurface@; border: 1px solid @borderStrong@; border-radius: @radiusL@px; padding: @s1@px; }
QMenu::item { padding: 2px @s5@px 2px @s2@px; margin: 0; min-height: @menuH@px; border-radius: @radius@px; }
QMenu::item:selected { background-color: @accentSoft@; color: @textPrimary@; }
QMenu::icon { padding-left: @s1@px; }
QMenu::indicator { width: @s4@px; height: @s4@px; }
QMenu::item:disabled { color: @textMuted@; }
QMenu::separator { height: 1px; background: @borderSubtle@; margin: @s1@px @s2@px; }

QScrollBar:vertical { background: transparent; width: 10px; margin: 0; }
QScrollBar:horizontal { background: transparent; height: 10px; margin: 0; }
QScrollBar::handle { background: @borderStrong@; border-radius: @radius@px; min-height: @s6@px; min-width: @s6@px; }
QScrollBar::handle:hover { background: @textMuted@; }
QScrollBar::add-line, QScrollBar::sub-line { width: 0; height: 0; }
QScrollBar::add-page, QScrollBar::sub-page { background: transparent; }

QTabBar::tab { background: transparent; color: @textSecondary@; padding: @s2@px @s3@px; border: none; border-bottom: 2px solid transparent; min-height: @controlH@px; }
QTabBar::tab:hover { color: @textPrimary@; background: @bgSurfaceHover@; }
QTabBar::tab:selected { color: @textPrimary@; border-bottom: 2px solid @accent@; }
QTabBar::tab:focus { border-bottom: @ring@px solid @borderFocus@; }
QTabWidget::pane { border: none; background: @bgPanel@; }

QSplitter::handle { background: transparent; }
QSplitter::handle:hover { background: @accentSoft@; }
QSplitter::handle:horizontal { image: none; border-left: 1px solid @borderSubtle@; }
QSplitter::handle:vertical { image: none; border-top: 1px solid @borderSubtle@; }

QWidget#PetuniaAppBar { background-color: @bgHeader@; border-bottom: 1px solid @borderStrong@; }
QWidget#PetuniaAppBar QMenuBar { background: transparent; }
QWidget#PetuniaViewportHeader { background-color: @bgPanel@; border-bottom: 1px solid @borderSubtle@; }
QToolBar#PetuniaToolOptions { background-color: @bgPanelHeader@; border: none; border-bottom: 1px solid @borderSubtle@; padding: 2px @s2@px; }
QFrame[role="vrule"] { background-color: @borderStrong@; border: none; min-width: 1px; max-width: 1px; margin: @s2@px @s1@px; }
QToolButton[role="appTool"] { background: transparent; border: 1px solid transparent; border-radius: @radius@px; padding: 0px; min-height: @compactH@px; min-width: @compactH@px; }
QToolButton[role="appTool"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="appTool"]:pressed { background-color: @bgSurfaceActive@; }
QToolButton[role="appTool"]:checked { background-color: @accentSoft@; border-color: @accent@; }
QToolButton[role="appTool"]:focus { border: @ring@px solid @borderFocus@; }
QToolButton[role="appTool"]::menu-indicator { image: none; }
QWidget#PetuniaPanelTitle { background-color: @bgPanelHeader@; border-bottom: 1px solid @borderSubtle@; }
QLabel[role="panelTitle"] { font-weight: 600; color: @textPrimary@; }
QToolButton[role="panelButton"] { background: transparent; border: none; border-radius: @radius@px; padding: 0px; min-height: 20px; min-width: 20px; }
QToolButton[role="panelButton"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="panelButton"]:focus { border: @ring@px solid @borderFocus@; }
QWidget#PetuniaPanelBar { background-color: @bgPanel@; border-bottom: 1px solid @borderSubtle@; }
QWidget#PetuniaInspectorHeader { background-color: @bgPanel@; border-bottom: 1px solid @borderSubtle@; }
QWidget#PetuniaSectionHeader { background-color: @bgPanel@; border-top: 1px solid @borderSubtle@; }
QWidget#PetuniaSectionHeader:hover { background-color: @bgPanelHeader@; }
QWidget#PetuniaSectionBody { background-color: @bgPanel@; }
QToolButton[role="sectionFold"] { background: transparent; border: 1px solid transparent; border-radius: @radius@px; text-align: left; padding: 0 @s1@px; font-weight: 600; min-height: @compactH@px; }
QToolButton[role="sectionFold"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="sectionFold"]:focus { border: @ring@px solid @borderFocus@; }
QLabel[role="chip"] { background-color: @bgSurfaceActive@; color: @textPrimary@; border-radius: @s2@px; padding: 1px @s2@px; font-size: @fontCaption@px; }
QLabel[role="entityId"] { color: @textMuted@; font-size: @fontCaption@px; }
QMainWindow > QTabBar { background-color: @bgPanelHeader@; border-bottom: 1px solid @borderSubtle@; }
QMainWindow > QTabBar::tab { min-height: @compactH@px; padding: 0 @s3@px; }
QMainWindow > QTabBar::tab:selected { background-color: @bgPanel@; }
QWidget#PetuniaToolRail, QWidget#PetuniaToolRailColumn { background-color: @bgHeader@; }
QWidget#PetuniaDrawerBar { background-color: @bgPanelHeader@; border-top: 1px solid @borderSubtle@; }
QFrame#PetuniaPropertySection { background: transparent; border: none; border-bottom: 1px solid @borderSubtle@; }
QMenuBar#PetuniaMenuBar { background: transparent; }

QToolButton[role="rail"] { background: transparent; border: none; border-radius: @radius@px; padding: 0 @s1@px; min-height: @compactH@px; text-align: left; }
QToolButton[role="rail"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="rail"]:checked { background-color: @accentSoft@; color: @textPrimary@; border-left: 3px solid @accent@; }
QToolButton[role="rail"]:focus { border: @ring@px solid @borderFocus@; padding: 0 @s2Ring@px; }
QToolButton[role="header"] { background: transparent; border: 1px solid transparent; border-radius: @radius@px; padding: 0 @s1@px; min-height: @compactH@px; }
QToolButton[role="header"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="header"]:checked { background-color: @bgSurfaceActive@; border-color: @borderStrong@; color: @textPrimary@; }
QToolButton[role="header"]:focus { border: @ring@px solid @borderFocus@; padding: 0 @s2Ring@px; }
QToolButton[role="header"]::menu-indicator { image: none; }
#PetuniaPropertiesPanel QLabel { font-size: @fontCaption@px; color: @textSecondary@; }
#PetuniaPropertiesPanel QPushButton, #PetuniaPropertiesPanel QToolButton { min-height: @fieldH@px; }
QToolButton[role="sectionHeader"] { background-color: transparent; border: none; border-radius: @radius@px; text-align: left; padding: 0 @s3@px; font-weight: 600; min-height: @controlH@px; }
QToolButton[role="sectionHeader"]:checked { background-color: transparent; color: @textPrimary@; }
QToolButton[role="sectionHeader"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="sectionHeader"]:focus { border: @ring@px solid @borderFocus@; padding: 0 @s3Ring@px; }

QPushButton[segment] { background-color: @bgPanelHeader@; color: @textSecondary@; border: 1px solid @borderSubtle@; border-radius: 0px; padding: 0 @s2@px; }
QPushButton[segment]:hover { background-color: @bgSurfaceHover@; color: @textPrimary@; }
QPushButton[segment]:checked { background-color: @bgSurfaceActive@; color: @textPrimary@; border-color: @accent@; font-weight: 600; }
QPushButton[segment="first"] { border-top-left-radius: @radius@px; border-bottom-left-radius: @radius@px; }
QPushButton[segment="last"] { border-top-right-radius: @radius@px; border-bottom-right-radius: @radius@px; }
QPushButton[segment="only"] { border-radius: @radius@px; }
QPushButton[role="workspace"] { background: transparent; border-color: transparent; border-radius: @radius@px; color: @textSecondary@; }
QPushButton[role="workspace"]:hover { background-color: @bgSurfaceHover@; color: @textPrimary@; }
QPushButton[role="workspace"]:checked { background-color: @accentSoft@; color: @textPrimary@; border-bottom: 2px solid @accent@; font-weight: 600; }
QPushButton[role="workspace"]:focus { border: @ring@px solid @borderFocus@; }

QTreeView, QListView, QTableView { background-color: @bgPanel@; alternate-background-color: @bgPanelHeader@; border: 1px solid transparent; outline: 0; selection-background-color: @accent@; selection-color: @textActive@; }
QTreeView::item, QListView::item { min-height: @compactH@px; padding: 0 @s2@px; border: none; }
QTreeView[dense="true"]::item { min-height: @compactH@px; }
QTreeView::item:hover, QListView::item:hover { background-color: @bgSurfaceHover@; }
QTreeView::item:selected, QListView::item:selected { background-color: @accent@; color: @textActive@; }
QTreeView:focus, QListView:focus, QTableView:focus { border: 1px solid @borderFocus@; }
QHeaderView::section { background-color: @bgPanelHeader@; color: @textSecondary@; border: none; border-bottom: 1px solid @borderSubtle@; padding: @s1@px @s2@px; font-weight: 600; }

QStatusBar { background-color: @bgHeader@; color: @textSecondary@; border-top: 1px solid @borderSubtle@; }
QStatusBar::item { border: none; }
QDockWidget { color: @textPrimary@; font-weight: 600; }
QDockWidget > QWidget { font-weight: 400; }
QDockWidget::title { background-color: @bgPanelHeader@; padding: @s1@px @s2@px; border-bottom: 1px solid @borderSubtle@; }
QDockWidget::close-button, QDockWidget::float-button { border: none; background: transparent; padding: 0px; }
QDockWidget::close-button:hover, QDockWidget::float-button:hover { background: @bgSurfaceHover@; }
QMainWindow::separator { background: @borderSubtle@; width: 1px; height: 1px; }
QMainWindow::separator:hover { background: @accent@; }
QToolBar { background-color: @bgHeader@; border: none; border-bottom: 1px solid @borderSubtle@; spacing: @s1@px; padding: @s1@px @s2@px; }
QToolBar::separator { background: @borderSubtle@; width: 1px; margin: @s1@px @s2@px; }
QToolBar QToolButton { background: transparent; border: 1px solid transparent; }
QToolBar QToolButton:hover { background: @bgSurfaceHover@; border-color: @borderSubtle@; }
QToolBar QToolButton:checked { background: @accent@; color: @textActive@; border-color: @accent@; }
QGroupBox { border: none; border-top: 1px solid @borderSubtle@; margin-top: @s4@px; padding-top: @s2@px; font-weight: 600; }
QGroupBox::title { subcontrol-origin: margin; left: 0px; padding: 0 @s1@px; }
QWidget[role="sectionHeader"] { background-color: @bgPanelHeader@; border-top: 1px solid @borderSubtle@; border-bottom: 1px solid @borderSubtle@; }
QLabel[role="sectionTitle"] { font-weight: 600; color: @textPrimary@; }
QLabel[role="propertyLabel"] { color: @textSecondary@; }
QLabel[role="entityTitle"] { font-size: @fontTitle@px; font-weight: 600; }
QToolButton[role="sectionToggle"], QToolButton[role="sectionMenu"] { background: transparent; border: none; padding: 0px; min-height: @compactH@px; min-width: @compactH@px; }
QToolButton[role="sectionToggle"]:hover, QToolButton[role="sectionMenu"]:hover { background: @bgSurfaceHover@; }
QToolButton[role="sectionMenu"]::menu-indicator { image: none; }
QPushButton[role="addRow"] { background: transparent; border: none; border-top: 1px solid @borderSubtle@; border-radius: 0px; text-align: left; padding: 0 @s3@px; font-weight: 600; min-height: @controlH@px; }
QPushButton[role="addRow"]:hover { background: @bgSurfaceHover@; }
QPushButton[role="addRow"]::menu-indicator { image: none; }

QSlider::groove:horizontal { height: 4px; background: @borderSubtle@; border-radius: 2px; }
QSlider::sub-page:horizontal { background: @textSecondary@; border-radius: 2px; }
QSlider::handle:horizontal { width: @s4@px; height: @s4@px; margin: -@s1@px 0; border-radius: @s2@px; background: @textPrimary@; border: 1px solid @borderStrong@; }
QSlider::handle:horizontal:focus { border: @ring@px solid @borderFocus@; }
QSlider::sub-page:horizontal:disabled, QSlider::handle:horizontal:disabled { background: @borderSubtle@; }
QTextEdit, QPlainTextEdit { background: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; border-radius: @radius@px; padding: @s2@px; selection-background-color: @accent@; selection-color: @textActive@; }
QTextEdit:focus, QPlainTextEdit:focus { border-color: @borderFocus@; }
QProgressBar { background: @bgSurface@; border: 1px solid @borderSubtle@; border-radius: @radius@px; min-height: @fieldH@px; text-align: center; }
QProgressBar::chunk { background: @accentSoft@; border-radius: @radius@px; }
QCheckBox::indicator { width: 14px; height: 14px; border-radius: 2px; border: 1px solid @borderStrong@; background-color: @bgSurface@; }
QCheckBox::indicator:hover { border-color: @textSecondary@; background-color: @bgSurfaceHover@; }
QCheckBox::indicator:checked { background-color: @accent@; border-color: @accent@; }
QCheckBox::indicator:disabled { background-color: @bgPanel@; border-color: @borderSubtle@; }
QRadioButton::indicator { width: @s4@px; height: @s4@px; }
QCheckBox:focus, QRadioButton:focus { outline: @ring@px solid @borderFocus@; }
QAbstractScrollArea { selection-background-color: @accentSoft@; selection-color: @textPrimary@; }
)QSS");

    const QColor accent = m_accentPrimary;
    const QHash<QString, QString> values = {
        {QStringLiteral("textPrimary"), css(m_textPrimary)},
        {QStringLiteral("textSecondary"), css(m_textSecondary)},
        {QStringLiteral("textMuted"), css(m_textMuted)},
        {QStringLiteral("textActive"), css(m_textActive)},
        {QStringLiteral("bgPanel"), css(m_bgPanel)},
        {QStringLiteral("bgCanvas"), css(m_bgCanvas)},
        {QStringLiteral("bgHeader"), css(m_bgHeader)},
        {QStringLiteral("bgPanelHeader"), css(m_bgPanelHeader)},
        {QStringLiteral("bgSurface"), css(m_bgSurface)},
        {QStringLiteral("bgSurfaceHover"), css(m_bgSurfaceHover)},
        {QStringLiteral("bgSurfaceActive"), css(m_bgSurfaceActive)},
        {QStringLiteral("borderSubtle"), css(m_borderSubtle)},
        {QStringLiteral("borderStrong"), css(m_borderStrong)},
        {QStringLiteral("borderFocus"), css(m_borderFocus)},
        {QStringLiteral("statusError"), css(m_statusError)},
        {QStringLiteral("accent"), css(accent)},
        {QStringLiteral("accentHover"), css(accent.lighter(115))},
        {QStringLiteral("accentSoft"), css(withAlpha(accent, isHighContrast() ? 0.35 : 0.12))},
        {QStringLiteral("overlay"), css(overlaySurface())},
        {QStringLiteral("fontBody"), QString::number(fontPx(TextRole::Body))},
        {QStringLiteral("fontCaption"), QString::number(fontPx(TextRole::Caption))},
        {QStringLiteral("fontTitle"), QString::number(fontPx(TextRole::Title))},
        {QStringLiteral("controlH"), QString::number(controlHeight() - 2)},
        {QStringLiteral("compactH"), QString::number(compactControlHeight() - 2)},
        {QStringLiteral("fieldH"), QString::number(fieldHeight() - 2)},
        // B14: linhas de menu densas, sem descer do alvo de 24 px (WCAG 2.5.8).
        {QStringLiteral("menuH"), QString::number(std::max(20, scaled(m_density == Density::Comfortable ? 24 : 20)))},
        {QStringLiteral("radius"), QString::number(radius())},
        {QStringLiteral("radiusL"), QString::number(radius() + scaled(3))},
        {QStringLiteral("ring"), QString::number(focusRingWidth())},
        {QStringLiteral("s1"), QString::number(space(1))},
        {QStringLiteral("s2"), QString::number(space(2))},
        {QStringLiteral("s3"), QString::number(space(3))},
        {QStringLiteral("s4"), QString::number(space(4))},
        {QStringLiteral("s5"), QString::number(space(5))},
        {QStringLiteral("s6"), QString::number(space(6))},
        // O anel de foco troca 1px de borda por `ring`px: compensa no padding
        // para o controle não "pular" ao receber foco (UI-013).
        {QStringLiteral("s2Ring"), QString::number(std::max(0, space(2) - (focusRingWidth() - 1)))},
        {QStringLiteral("s3Ring"), QString::number(std::max(0, space(3) - (focusRingWidth() - 1)))},
    };
    for (auto it = values.cbegin(); it != values.cend(); ++it)
        s.replace(QLatin1Char('@') + it.key() + QLatin1Char('@'), it.value());
    return s;
}
