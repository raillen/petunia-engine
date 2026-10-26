#include "petunia_theme.h"

#include <QApplication>
#include <QFile>
#include <QHash>
#include <QStyle>
#include <QStyleFactory>
#include <QDir>
#include <QSaveFile>
#include <QStandardPaths>
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

// The style sheet cannot color an SVG with `currentColor`: it reads the file as is. This writes a
// copy of the icon in the theme color into the user's cache folder, one file per icon and color,
// reused by every run (a temporary folder was left behind whenever the editor was killed).
QString tintedIcon(const QString &name, const QColor &color) {
    const QString fallback = QStringLiteral(":/icons/%1.svg").arg(name);
    const QString folder = QStandardPaths::writableLocation(QStandardPaths::CacheLocation) + QStringLiteral("/tinted-icons");
    if (folder.startsWith(QLatin1Char('/')) == false || !QDir().mkpath(folder))
        return fallback;
    const QString path = QStringLiteral("%1/%2-%3.svg").arg(folder, name, color.name(QColor::HexRgb).mid(1));
    if (!QFile::exists(path)) {
        QFile source(fallback);
        QSaveFile target(path);
        if (!source.open(QIODevice::ReadOnly) || !target.open(QIODevice::WriteOnly))
            return fallback;
        QByteArray svg = source.readAll();
        svg.replace("currentColor", color.name(QColor::HexRgb).toUtf8());
        target.write(svg);
        if (!target.commit())
            return fallback;
    }
    return path;
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

// Sizes follow the Unity editor design system (12 px text, 16 px icons) without going below the
// 24 x 24 px pointer target of WCAG 2.5.8. Comfortable density is there for anyone who wants more.
int PetuniaTheme::controlHeight() const {
    switch (m_density) {
    case Density::Compact: return std::max(24, scaled(24));
    case Density::Standard: return std::max(24, scaled(28));
    case Density::Comfortable: return std::max(24, scaled(32));
    }
    return std::max(24, scaled(28));
}

int PetuniaTheme::fieldHeight() const {
    switch (m_density) {
    case Density::Compact: return std::max(24, scaled(24));
    case Density::Standard: return std::max(24, scaled(26));
    case Density::Comfortable: return std::max(24, scaled(30));
    }
    return std::max(24, scaled(26));
}

int PetuniaTheme::compactControlHeight() const {
    return std::max(24, controlHeight() - scaled(4));
}

int PetuniaTheme::fontPx(TextRole role) const {
    const int body = m_density == Density::Compact ? 12 : m_density == Density::Standard ? 13 : 14;
    // Operational text never below 12 logical px (U08), at any scale.
    switch (role) {
    case TextRole::Caption: return std::max(12, scaled(body - 1));
    case TextRole::Body: return std::max(12, scaled(body));
    case TextRole::Title: return std::max(13, scaled(body + 1));
    case TextRole::Mono: return std::max(12, scaled(body));
    }
    return std::max(12, scaled(body));
}

int PetuniaTheme::space(int step) const {
    return scaled(4 * step);
}

int PetuniaTheme::radius() const {
    return scaled(4);
}

int PetuniaTheme::iconPx() const {
    return scaled(m_density == Density::Comfortable ? 18 : 16);
}

int PetuniaTheme::railWidth() const {
    return scaled(m_density == Density::Comfortable ? 40 : 34);
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
    // One rule per role. Quiet by default: transparent buttons, 1 px borders, one accent color.
    // Fields carry a stronger bottom edge (3:1, WCAG 1.4.11) like Fluent 2 inputs.
    QString s = QStringLiteral(R"QSS(
QWidget { color: @textPrimary@; font-size: @fontBody@px; }
QMainWindow { background-color: @bgCanvas@; }
QDialog { background-color: @bgPanel@; }
QToolTip { background-color: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; border-radius: @radius@px; padding: @s1@px @s2@px; }
QLabel { background: transparent; }
QLabel[role="caption"] { color: @textSecondary@; font-size: @fontCaption@px; }
QLabel[role="muted"] { color: @textMuted@; font-size: @fontCaption@px; }
QLabel[role="title"] { font-size: @fontTitle@px; font-weight: 600; }
QLabel[role="error"] { color: @statusError@; }
QLabel[role="keycap"] { background-color: @bgSurfaceActive@; color: @textSecondary@; border: none; border-radius: @radius@px; padding: 0px @s1@px; font-size: @fontCaption@px; }
QLabel[role="liveTool"] { background-color: @bgSurfaceActive@; color: @textPrimary@; border-radius: @radius@px; padding: 1px @s2@px; font-size: @fontCaption@px; font-weight: 600; }
QLabel[role="chip"] { background-color: @bgSurfaceActive@; color: @textSecondary@; border-radius: @s2@px; padding: 0px @s2@px; font-size: @fontCaption@px; }
QLabel[role="entityId"] { color: @textMuted@; font-size: @fontCaption@px; }
QLabel[role="entityTitle"] { font-size: @fontTitle@px; font-weight: 600; }
QLabel[role="panelTitle"] { font-weight: 600; color: @textSecondary@; font-size: @fontCaption@px; }
QLabel[role="propertyLabel"] { color: @textSecondary@; }
QLabel[role="sectionTitle"] { font-weight: 600; }
QFrame[role="card"] { background-color: @bgPanel@; border: 1px solid @borderSubtle@; border-radius: @radiusL@px; }
QFrame[role="overlay"] { background-color: @hud@; border: 1px solid @borderSubtle@; border-radius: @radiusL@px; }
QFrame[role="separator"], QFrame[role="vrule"] { background-color: @borderSubtle@; border: none; min-width: 1px; max-width: 1px; }
QFrame[role="vrule"] { margin: @s1@px @s1@px; }
QFrame[role="hrule"] { background-color: @borderSubtle@; border: none; min-height: 1px; max-height: 1px; margin: 2px 4px; }

QPushButton, QToolButton { background-color: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; border-radius: @radius@px; padding: 0px @s2@px; min-height: @controlH@px; }
QPushButton:hover, QToolButton:hover { background-color: @bgSurfaceHover@; }
QPushButton:pressed, QToolButton:pressed { background-color: @bgSurfaceActive@; }
QPushButton:checked, QToolButton:checked { background-color: @accentSoft@; border-color: @accentEdge@; }
QPushButton:focus, QToolButton:focus { border: @ring@px solid @borderFocus@; }
QPushButton:disabled, QToolButton:disabled { color: @textMuted@; background-color: transparent; }
QPushButton[role="primary"] { background-color: @accent@; color: @textActive@; border-color: @accent@; font-weight: 600; }
QPushButton[role="primary"]:hover { background-color: @accentHover@; border-color: @accentHover@; }
QPushButton[role="danger"] { color: @statusError@; }
QToolButton::menu-indicator, QPushButton::menu-indicator { image: none; width: 0px; }

QToolButton[role="appTool"], QToolButton[role="header"], QPushButton[role="quiet"], QToolButton[role="quiet"], QPushButton[role="ghost"], QToolButton[role="ghost"] { background: transparent; border: 1px solid transparent; }
QToolButton[role="appTool"] { padding: 0px; min-width: @controlH@px; }
QToolButton[role="appTool"]:hover, QToolButton[role="header"]:hover, QPushButton[role="quiet"]:hover, QToolButton[role="quiet"]:hover, QPushButton[role="ghost"]:hover, QToolButton[role="ghost"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="appTool"]:pressed, QToolButton[role="header"]:pressed { background-color: @bgSurfaceActive@; }
QToolButton[role="appTool"]:checked, QToolButton[role="header"]:checked, QPushButton[role="quiet"]:checked { background-color: @accentSoft@; border-color: transparent; }
QToolButton[role="appTool"]:focus, QToolButton[role="header"]:focus, QPushButton[role="quiet"]:focus, QPushButton[role="ghost"]:focus { border: @ring@px solid @borderFocus@; }
QToolButton[role="header"] { padding: 0px @s1@px; }
QPushButton[role="ghost"], QToolButton[role="ghost"] { text-align: left; padding: 0px @s2@px; }
QPushButton[role="search"] { background-color: @bgSurfaceHover@; color: @textMuted@; border: 1px solid transparent; border-radius: @radius@px; text-align: left; padding: 0px @s2@px; }
QPushButton[role="search"]:hover { border-color: @borderSubtle@; }
QPushButton[role="search"]:focus { border: @ring@px solid @borderFocus@; }
QToolButton[role="panelButton"] { background: transparent; border: 1px solid transparent; padding: 0px; min-height: 20px; min-width: 20px; }
QToolButton[role="panelButton"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="panelButton"]:focus { border: @ring@px solid @borderFocus@; }
QToolButton[role="sectionFold"] { background: transparent; border: 1px solid transparent; text-align: left; padding: 0px @s1@px; font-weight: 600; }
QToolButton[role="sectionFold"]:hover { background-color: @bgSurfaceHover@; }
QToolButton[role="sectionFold"]:focus { border: @ring@px solid @borderFocus@; }
QPushButton[role="addRow"] { background: transparent; border: 1px dashed @borderSubtle@; text-align: center; color: @textSecondary@; margin: @s2@px; }
QPushButton[role="addRow"]:hover { background-color: @bgSurfaceHover@; color: @textPrimary@; }

QPushButton[segment] { background-color: @bgSurfaceHover@; color: @textSecondary@; border: 1px solid transparent; border-radius: 0px; padding: 0px @s2@px; }
QPushButton[segment]:hover { color: @textPrimary@; background-color: @bgSurfaceActive@; }
QPushButton[segment]:checked { background-color: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; font-weight: 600; }
QPushButton[segment]:focus { border: @ring@px solid @borderFocus@; }
QPushButton[segment="first"] { border-top-left-radius: @radius@px; border-bottom-left-radius: @radius@px; }
QPushButton[segment="last"] { border-top-right-radius: @radius@px; border-bottom-right-radius: @radius@px; }
QPushButton[segment="only"] { border-radius: @radius@px; }
QPushButton[role="workspace"] { padding: 0px @s3@px; }

QLineEdit, QSpinBox, QDoubleSpinBox, QComboBox { background-color: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; border-bottom-color: @borderStrong@; border-radius: @radius@px; padding: 0px @s2@px; min-height: @fieldH@px; selection-background-color: @accent@; selection-color: @textActive@; }
QLineEdit:hover, QSpinBox:hover, QDoubleSpinBox:hover, QComboBox:hover { border-color: @borderStrongSoft@; border-bottom-color: @borderStrong@; }
QLineEdit:focus, QSpinBox:focus, QDoubleSpinBox:focus, QComboBox:focus { border: @ring@px solid @borderFocus@; padding: 0px @s2Ring@px; }
QLineEdit[invalid="true"] { border: @ring@px dashed @statusError@; }
QLineEdit:disabled, QComboBox:disabled, QSpinBox:disabled, QDoubleSpinBox:disabled { color: @textMuted@; background-color: transparent; }
QComboBox::drop-down { border: none; width: @s4@px; }
QComboBox::down-arrow { image: url(@iconChevronDown@); width: 10px; height: 10px; }
QSpinBox::up-button, QSpinBox::down-button, QDoubleSpinBox::up-button, QDoubleSpinBox::down-button { width: 0px; border: none; }
QComboBox QAbstractItemView { background-color: @bgSurface@; border: 1px solid @borderSubtle@; selection-background-color: @accentSoft@; selection-color: @textPrimary@; outline: 0; padding: @s1@px; }

QCheckBox, QRadioButton { spacing: @s2@px; min-height: @fieldH@px; }
QCheckBox::indicator { width: 14px; height: 14px; border-radius: 3px; border: 1px solid @borderStrong@; background-color: @bgSurface@; }
QCheckBox::indicator:hover { background-color: @bgSurfaceHover@; }
QCheckBox::indicator:checked { background-color: @accent@; border-color: @accent@; image: url(@iconCheck@); }
QCheckBox::indicator:disabled { background-color: transparent; border-color: @borderSubtle@; }
QCheckBox:focus, QRadioButton:focus { outline: @ring@px solid @borderFocus@; }
QRadioButton::indicator { width: @s4@px; height: @s4@px; }

QMenuBar { background-color: transparent; color: @textPrimary@; }
QMenuBar::item { padding: @s1@px @s2@px; border-radius: @radius@px; background: transparent; }
QMenuBar::item:selected { background-color: @bgSurfaceHover@; }
QMenu { background-color: @bgSurface@; border: 1px solid @borderSubtle@; border-radius: @radiusL@px; padding: @s1@px; }
QMenu::item { padding: 3px @s6@px 3px @s2@px; margin: 0px; border-radius: @radius@px; min-height: @menuH@px; }
QMenu::item:selected { background-color: @accentSoft@; color: @textPrimary@; }
QMenu::item:disabled { color: @textMuted@; }
QMenu::icon { padding-left: @s2@px; }
QMenu::indicator { width: @s4@px; height: @s4@px; }
QMenu::separator { height: 1px; background: @borderSubtle@; margin: @s1@px @s2@px; }
QMenu::right-arrow { image: url(@iconChevronRight@); width: 10px; height: 10px; }

QScrollBar:vertical { background: transparent; width: 8px; margin: 0px; }
QScrollBar:horizontal { background: transparent; height: 8px; margin: 0px; }
QScrollBar::handle { background: @borderSubtle@; border-radius: 4px; min-height: @s6@px; min-width: @s6@px; }
QScrollBar::handle:hover { background: @borderStrong@; }
QScrollBar::add-line, QScrollBar::sub-line { width: 0px; height: 0px; }
QScrollBar::add-page, QScrollBar::sub-page { background: transparent; }

QTabBar { background: transparent; }
QTabBar::tab { background: transparent; color: @textSecondary@; padding: 0px @s3@px; border: none; border-bottom: 2px solid transparent; min-height: @controlH@px; }
QTabBar::tab:hover { color: @textPrimary@; }
QTabBar::tab:selected { color: @textPrimary@; border-bottom: 2px solid @accent@; font-weight: 600; }
QTabBar::tab:focus { border-bottom: @ring@px solid @borderFocus@; }
QTabWidget::pane { border: none; background: transparent; }
QMainWindow > QTabBar { background-color: @bgPanel@; }

QSplitter::handle { background: transparent; }
QSplitter::handle:hover { background: @accentSoft@; }
QMainWindow::separator { background: @bgCanvas@; width: @gap@px; height: @gap@px; }
QMainWindow::separator:hover { background: @accentSoft@; }

QWidget#PetuniaAppBar { background-color: @bgCanvas@; }
QWidget#PetuniaViewportHeader, QToolBar#PetuniaToolOptions { background-color: @bgPanel@; border: none; border-bottom: 1px solid @borderSubtle@; }
QToolBar#PetuniaToolOptions { padding: 2px @s2@px; spacing: @s1@px; }
QFrame#PetuniaToolPalette { background-color: @hud@; border: 1px solid @borderSubtle@; border-radius: @radiusL@px; }
QWidget#PetuniaPanelTitle { background-color: @bgPanel@; border-top-left-radius: @radiusL@px; border-top-right-radius: @radiusL@px; }
QWidget#PetuniaDockBody { background-color: @bgPanel@; border-bottom-left-radius: @radiusL@px; border-bottom-right-radius: @radiusL@px; }
QWidget#PetuniaPanelBar { background-color: @bgPanel@; }
QWidget#PetuniaInspectorHeader { background-color: @bgPanel@; border-bottom: 1px solid @borderSubtle@; }
QWidget#PetuniaSectionHeader { background-color: @bgPanel@; }
QWidget#PetuniaSectionBody { background-color: @bgPanel@; }
QFrame#PetuniaSectionRule { background-color: @borderSubtle@; border: none; max-height: 1px; min-height: 1px; margin: 0px @s2@px; }
QWidget#PetuniaWorkspacePage { background-color: @bgPanel@; border-radius: @radiusL@px; }

QTreeView, QListView, QTableView { background-color: transparent; alternate-background-color: @bgPanelHeader@; border: 1px solid transparent; outline: 0; selection-background-color: @accentSoft@; selection-color: @textPrimary@; }
QTreeView { show-decoration-selected: 0; }
QTreeView::item, QListView::item { min-height: @rowH@px; padding: 0px @s1@px; border: none; }
QListView::item { border-radius: @radius@px; }
QTreeView::branch { background: transparent; }
QTreeView::branch:selected { background: transparent; }
QTreeView::item:hover, QListView::item:hover { background-color: @bgSurfaceHover@; }
QTreeView::item:selected, QListView::item:selected { background-color: @accentSoft@; color: @textPrimary@; }
QTreeView::item:selected:active, QListView::item:selected:active { background-color: @accent@; color: @textActive@; }
QTreeView:focus, QListView:focus, QTableView:focus { border: 1px solid @borderFocus@; border-radius: @radius@px; }
QHeaderView { background: transparent; }
QHeaderView::section { background-color: transparent; color: @textMuted@; border: none; border-bottom: 1px solid @borderSubtle@; padding: @s1@px @s2@px; font-size: @fontCaption@px; }

QStatusBar { background-color: @bgCanvas@; color: @textMuted@; font-size: @fontCaption@px; }
QStatusBar::item { border: none; }
QStatusBar QLabel { color: @textMuted@; font-size: @fontCaption@px; }
QDockWidget { color: @textPrimary@; }
QDockWidget > QWidget { background-color: @bgPanel@; }
QToolBar { background-color: @bgPanel@; border: none; spacing: @s1@px; padding: @s1@px; }
QToolBar::separator { background: @borderSubtle@; width: 1px; margin: @s1@px @s1@px; }
QGroupBox { border: none; border-top: 1px solid @borderSubtle@; margin-top: @s4@px; padding-top: @s2@px; font-weight: 600; }
QGroupBox::title { subcontrol-origin: margin; left: 0px; padding: 0px @s1@px; }

QSlider::groove:horizontal { height: 4px; background: @borderSubtle@; border-radius: 2px; }
QSlider::sub-page:horizontal { background: @accent@; border-radius: 2px; }
QSlider::handle:horizontal { width: 12px; height: 12px; margin: -4px 0px; border-radius: 6px; background: @bgSurface@; border: 1px solid @borderStrong@; }
QSlider::handle:horizontal:focus { border: @ring@px solid @borderFocus@; }
QTextEdit, QPlainTextEdit { background: @bgSurface@; color: @textPrimary@; border: 1px solid @borderSubtle@; border-radius: @radius@px; padding: @s1@px; selection-background-color: @accent@; selection-color: @textActive@; }
QTextEdit:focus, QPlainTextEdit:focus { border-color: @borderFocus@; }
QProgressBar { background: @bgSurfaceHover@; border: none; border-radius: @radius@px; min-height: 6px; max-height: 6px; text-align: center; }
QProgressBar::chunk { background: @accent@; border-radius: @radius@px; }
QAbstractScrollArea { background: transparent; }
QScrollArea > QWidget > QWidget { background: transparent; }
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
        // Hover moves the accent away from the text on it: darker in light themes, lighter in dark.
        {QStringLiteral("accentHover"), css(luminance(accent) < 0.3 ? accent.darker(112) : accent.lighter(110))},
        {QStringLiteral("iconChevronDown"), tintedIcon(QStringLiteral("chevron_down"), m_textSecondary)},
        {QStringLiteral("iconChevronRight"), tintedIcon(QStringLiteral("chevron_right"), m_textSecondary)},
        {QStringLiteral("iconCheck"), tintedIcon(QStringLiteral("check"), m_textActive)},
        {QStringLiteral("accentSoft"), css(withAlpha(accent, isHighContrast() ? 0.35 : 0.16))},
        {QStringLiteral("accentEdge"), css(withAlpha(accent, 0.55))},
        {QStringLiteral("borderStrongSoft"), css(withAlpha(m_borderStrong, 0.6))},
        {QStringLiteral("hud"), css(withAlpha(m_bgPanel, isHighContrast() ? 1.0 : 0.94))},
        {QStringLiteral("rowH"), QString::number(std::max(22, scaled(m_density == Density::Comfortable ? 28 : 22)))},
        {QStringLiteral("gap"), QString::number(scaled(5))},
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
        {QStringLiteral("radiusL"), QString::number(radius() + scaled(4))},
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
