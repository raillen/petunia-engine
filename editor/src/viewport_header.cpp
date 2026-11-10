#include "viewport_header.h"

#include <cmath>

#include <QAction>
#include <QActionGroup>
#include <QButtonGroup>
#include <QCheckBox>
#include <QComboBox>
#include <QFormLayout>
#include <QFrame>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLocale>
#include <QMenu>
#include <QPushButton>
#include <QSignalBlocker>
#include <QToolButton>

#include "petunia_icons.h"
#include "petunia_theme.h"
#include "wheel_guard.h"

namespace {

QString numberText(double value) {
    return QLocale().toString(value, 'g', 4);
}

QFrame* rule(QWidget* parent) {
    auto* line = new QFrame(parent);
    line->setProperty("role", QStringLiteral("vrule"));
    line->setFixedWidth(1);
    return line;
}

}  // namespace

ViewportHeader::ViewportHeader(QWidget* parent) : QWidget(parent) {
    setObjectName(QStringLiteral("PetuniaViewportHeader"));
    setAttribute(Qt::WA_StyledBackground);
    setAccessibleName(QStringLiteral("Opções da cena"));
    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(6, 3, 6, 3);
    layout->setSpacing(2);

    auto makeSegment = [&](const QString& text, const QString& segment, const QString& tip) {
        auto* button = new QPushButton(text, this);
        button->setCheckable(true);
        button->setProperty("segment", segment);
        button->setToolTip(tip);
        button->setAccessibleName(tip);
        layout->addWidget(button);
        return button;
    };

    // Axes of the gizmo: a two-button switch.
    auto* axes = new QButtonGroup(this);
    globalButton_ = makeSegment(QStringLiteral("Global"), QStringLiteral("first"), QStringLiteral("Eixos do gizmo alinhados ao mundo"));
    localButton_ = makeSegment(QStringLiteral("Local"), QStringLiteral("last"),
                               QStringLiteral("Eixos do gizmo alinhados ao objeto (escalar usa sempre estes)"));
    axes->addButton(globalButton_);
    axes->addButton(localButton_);
    globalButton_->setChecked(true);
    connect(globalButton_, &QPushButton::clicked, this, &ViewportHeader::emitTransform);
    connect(localButton_, &QPushButton::clicked, this, &ViewportHeader::emitTransform);
    layout->addSpacing(4);

    // Pivot: icon plus the short name of the current choice; the choices open in a menu.
    pivotButton_ = new QToolButton(this);
    pivotButton_->setProperty("role", QStringLiteral("header"));
    pivotButton_->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    pivotButton_->setPopupMode(QToolButton::InstantPopup);
    pivotButton_->setToolTip(QStringLiteral("Ponto em volta do qual o objeto gira e escala"));
    pivotButton_->setAccessibleName(QStringLiteral("Ponto de pivô"));
    auto* pivotMenu = new QMenu(pivotButton_);
    pivotGroup_ = new QActionGroup(pivotMenu);
    const QList<QPair<QString, QString>> pivots = {{QStringLiteral("object"), QStringLiteral("Pivô do objeto")},
                                                   {QStringLiteral("bounds_center"), QStringLiteral("Centro da caixa")},
                                                   {QStringLiteral("base"), QStringLiteral("Base (fica no chão)")}};
    for (const auto& [id, label] : pivots) {
        QAction* action = pivotMenu->addAction(label);
        action->setCheckable(true);
        action->setData(id);
        pivotGroup_->addAction(action);
        connect(action, &QAction::triggered, this, &ViewportHeader::emitTransform);
    }
    pivotGroup_->actions().constFirst()->setChecked(true);
    pivotButton_->setMenu(pivotMenu);
    layout->addWidget(pivotButton_);

    editPivotButton_ = new QToolButton(this);
    editPivotButton_->setCheckable(true);
    editPivotButton_->setProperty("role", QStringLiteral("appTool"));
    editPivotButton_->setToolTip(QStringLiteral("Editar pivô: arraste o gizmo para mudar onde o objeto gira e escala (Alt+P)"));
    editPivotButton_->setAccessibleName(QStringLiteral("Editar o pivô do objeto"));
    connect(editPivotButton_, &QToolButton::toggled, this, [this](bool on) {
        emit transformRequested(QJsonObject{{"edit_pivot", on}});
    });
    layout->addWidget(editPivotButton_);
    layout->addWidget(rule(this));

    // Snapping: the toggle shows the move step; the panel holds the rest.
    snapButton_ = new QToolButton(this);
    snapButton_->setCheckable(true);
    snapButton_->setProperty("role", QStringLiteral("header"));
    snapButton_->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    snapButton_->setAccessibleName(QStringLiteral("Ligar ou desligar o snap"));
    layout->addWidget(snapButton_);
    snapOptionsButton_ = new QToolButton(this);
    snapOptionsButton_->setProperty("role", QStringLiteral("header"));
    snapOptionsButton_->setToolTip(QStringLiteral("Passos, alvo e inclinação do snap"));
    snapOptionsButton_->setAccessibleName(QStringLiteral("Opções do snap"));
    snapOptionsButton_->setFixedWidth(16);
    layout->addWidget(snapOptionsButton_);

    snapPanel_ = new QFrame(this, Qt::Popup);
    snapPanel_->setProperty("role", QStringLiteral("card"));
    snapPanel_->setAccessibleName(QStringLiteral("Opções do snap"));
    auto* form = new QFormLayout(snapPanel_);
    form->setContentsMargins(12, 10, 12, 10);
    form->setHorizontalSpacing(10);
    form->setVerticalSpacing(6);
    translateBox_ = makeStepBox(snapPanel_, QStringLiteral("Passo de mover, em metros"), {0.1, 0.25, 0.5, 1.0, 2.0, 5.0}, QStringLiteral(" m"));
    rotateBox_ = makeStepBox(snapPanel_, QStringLiteral("Passo de girar, em graus"), {5.0, 15.0, 45.0, 90.0}, QStringLiteral("°"));
    scaleBox_ = makeStepBox(snapPanel_, QStringLiteral("Passo de escala"), {0.05, 0.1, 0.25, 0.5}, QStringLiteral("×"));
    targetBox_ = new QComboBox(snapPanel_);
    targetBox_->setAccessibleName(QStringLiteral("Alvo do snap ao mover"));
    targetBox_->setToolTip(QStringLiteral(
        "Inteligente: canto de outro objeto, senão a superfície, senão a grade.\n"
        "Superfície: o objeto assenta no que está sob o mouse (topos, rampas, andares)."));
    targetBox_->addItem(QStringLiteral("Inteligente"), QStringLiteral("smart"));
    targetBox_->addItem(QStringLiteral("Grade"), QStringLiteral("grid"));
    targetBox_->addItem(QStringLiteral("Incremento"), QStringLiteral("increment"));
    targetBox_->addItem(QStringLiteral("Ponto"), QStringLiteral("point"));
    targetBox_->addItem(QStringLiteral("Superfície"), QStringLiteral("face"));
    WheelGuard::protect(targetBox_);
    slopeBox_ = new QCheckBox(QStringLiteral("Inclinar junto com a superfície"), snapPanel_);
    slopeBox_->setToolTip(QStringLiteral("Ao assentar numa superfície, inclina o objeto junto com a rampa ou o terreno"));
    form->addRow(QStringLiteral("Mover"), translateBox_);
    form->addRow(QStringLiteral("Girar"), rotateBox_);
    form->addRow(QStringLiteral("Escala"), scaleBox_);
    form->addRow(QStringLiteral("Alvo"), targetBox_);
    form->addRow(slopeBox_);
    connect(snapOptionsButton_, &QToolButton::clicked, this, [this] {
        snapPanel_->adjustSize();
        snapPanel_->move(snapButton_->mapToGlobal(QPoint(0, snapButton_->height() + 4)));
        snapPanel_->show();
        translateBox_->setFocus();
    });
    connect(snapButton_, &QToolButton::toggled, this, &ViewportHeader::emitSnap);
    connect(slopeBox_, &QCheckBox::toggled, this, &ViewportHeader::emitSnap);
    for (QComboBox* box : {translateBox_, rotateBox_, scaleBox_, targetBox_}) {
        connect(box, &QComboBox::activated, this, &ViewportHeader::emitSnap);
    }

    viewButton_ = new QToolButton(this);
    viewButton_->setProperty("role", QStringLiteral("header"));
    viewButton_->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    viewButton_->setPopupMode(QToolButton::InstantPopup);
    viewButton_->setText(QStringLiteral("Vistas"));
    viewButton_->setToolTip(QStringLiteral("Vistas predefinidas da câmera"));
    viewButton_->setAccessibleName(QStringLiteral("Vistas predefinidas da câmera"));
    auto* viewPresetsMenu = new QMenu(viewButton_);
    const struct {
        const char* name;
        const char* label;
    } viewPresets[8] = {
        {"front", "Frente"},
        {"back", "Trás"},
        {"left", "Esquerda"},
        {"right", "Direita"},
        {"top", "Topo"},
        {"bottom", "Base"},
        {"isometric", "Isométrica"},
        {"game_2_1", "Isométrica 2:1"},
    };
    for (const auto& preset : viewPresets) {
        QAction* action = viewPresetsMenu->addAction(QString::fromUtf8(preset.label));
        connect(action, &QAction::triggered, this, [this, name = QString::fromLatin1(preset.name)] {
            emit viewRequested(name);
        });
    }
    viewButton_->setMenu(viewPresetsMenu);
    layout->addWidget(rule(this));
    layout->addWidget(viewButton_);

    layout->addStretch(1);

    // Floor: down, which one, up.
    floorDown_ = new QToolButton(this);
    floorDown_->setProperty("role", QStringLiteral("header"));
    floorDown_->setToolTip(QStringLiteral("Descer um andar (Ctrl+PageDown)"));
    floorDown_->setAccessibleName(QStringLiteral("Descer um andar"));
    floorUp_ = new QToolButton(this);
    floorUp_->setProperty("role", QStringLiteral("header"));
    floorUp_->setToolTip(QStringLiteral("Subir um andar: a grade e os objetos novos sobem junto (Ctrl+PageUp)"));
    floorUp_->setAccessibleName(QStringLiteral("Subir um andar"));
    floorBox_ = new QComboBox(this);
    floorBox_->setToolTip(QStringLiteral("Andar em construção. Subir além do último cria um andar novo"));
    floorBox_->setAccessibleName(QStringLiteral("Andar atual"));
    // Sized for "Subsolo 12": it is filled after construction, so it cannot adjust to its contents.
    floorBox_->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
    floorBox_->setMinimumContentsLength(8);
    WheelGuard::protect(floorBox_);
    layout->addWidget(floorDown_);
    layout->addWidget(floorBox_);
    layout->addWidget(floorUp_);
    connect(floorDown_, &QToolButton::clicked, this, [this] { emit floorStepRequested(-1); });
    connect(floorUp_, &QToolButton::clicked, this, [this] { emit floorStepRequested(1); });
    connect(floorBox_, &QComboBox::activated, this, [this](int index) {
        const QVariant entity = floorBox_->itemData(index);
        if (entity.isValid()) {
            emit floorSelectRequested(entity.toDouble());
        }
    });

    refreshIcons();
    updateSnapText();
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, &ViewportHeader::refreshIcons);
}

QComboBox* ViewportHeader::makeStepBox(QWidget* parent, const QString& accessibleName, const QList<double>& steps, const QString& suffix) {
    auto* box = new QComboBox(parent);
    box->setAccessibleName(accessibleName);
    box->setToolTip(accessibleName);
    WheelGuard::protect(box);
    for (const double step : steps) {
        box->addItem(numberText(step) + suffix, step);
    }
    return box;
}

void ViewportHeader::select(QComboBox* box, double value) {
    for (int index = 0; index < box->count(); ++index) {
        // The engine stores f32: 0.1 arrives as 0.10000000149, so compare with a tolerance.
        if (std::abs(box->itemData(index).toDouble() - value) < 1.0e-5) {
            box->setCurrentIndex(index);
            return;
        }
    }
}

void ViewportHeader::emitTransform() {
    const QAction* pivot = pivotGroup_->checkedAction();
    emit transformRequested(QJsonObject{
        {"orientation", localButton_->isChecked() ? QStringLiteral("local") : QStringLiteral("global")},
        {"pivot", pivot != nullptr ? pivot->data().toString() : QStringLiteral("object")}});
}

void ViewportHeader::emitSnap() {
    updateSnapText();
    emit snapRequested(QJsonObject{{"enabled", snapButton_->isChecked()},
                                   {"translate", translateBox_->currentData().toDouble()},
                                   {"rotate_degrees", rotateBox_->currentData().toDouble()},
                                   {"scale", scaleBox_->currentData().toDouble()},
                                   {"target", targetBox_->currentData().toString()},
                                   {"align_to_slope", slopeBox_->isChecked()}});
}

void ViewportHeader::updateSnapText() {
    snapButton_->setText(translateBox_->currentText());
    snapButton_->setToolTip(QStringLiteral("Snap %1: mover %2, girar %3, escala %4, alvo %5 (segure Ctrl para inverter)")
                                .arg(snapButton_->isChecked() ? QStringLiteral("ligado") : QStringLiteral("desligado"),
                                     translateBox_->currentText(), rotateBox_->currentText(), scaleBox_->currentText(),
                                     targetBox_->currentText().toLower()));
}

void ViewportHeader::showSnap(const QJsonObject& snap) {
    const QSignalBlocker a(snapButton_);
    const QSignalBlocker b(translateBox_);
    const QSignalBlocker c(rotateBox_);
    const QSignalBlocker d(scaleBox_);
    const QSignalBlocker e(targetBox_);
    const QSignalBlocker f(slopeBox_);
    snapButton_->setChecked(snap.value(QStringLiteral("enabled")).toBool());
    select(translateBox_, snap.value(QStringLiteral("translate")).toDouble());
    select(rotateBox_, snap.value(QStringLiteral("rotate_degrees")).toDouble());
    select(scaleBox_, snap.value(QStringLiteral("scale")).toDouble());
    targetBox_->setCurrentIndex(qMax(0, targetBox_->findData(snap.value(QStringLiteral("target")).toString())));
    slopeBox_->setChecked(snap.value(QStringLiteral("align_to_slope")).toBool());
    updateSnapText();
}

void ViewportHeader::showFloor(const QJsonObject& floor) {
    const QSignalBlocker a(floorBox_);
    auto add = [this](const QJsonObject& level, const QVariant& entity) {
        floorBox_->addItem(level.value(QStringLiteral("name")).toString(), entity);
        const QString where = QStringLiteral("Piso a %1 m, altura %2 m")
            .arg(QLocale().toString(level.value(QStringLiteral("elevation")).toDouble(), 'f', 1),
                 QLocale().toString(level.value(QStringLiteral("height")).toDouble(), 'f', 1));
        floorBox_->setItemData(floorBox_->count() - 1, where, Qt::ToolTipRole);
    };
    floorBox_->clear();
    const QJsonArray floors = floor.value(QStringLiteral("floors")).toArray();
    if (floors.isEmpty()) {
        // The implicit ground floor: nothing saved yet, nothing to pick.
        add(floor, QVariant());
    }
    for (const QJsonValue& value : floors) {
        const QJsonObject level = value.toObject();
        add(level, level.value(QStringLiteral("entity")).toDouble());
    }
    const QJsonValue current = floor.value(QStringLiteral("entity"));
    if (!current.isNull()) {
        floorBox_->setCurrentIndex(qMax(0, floorBox_->findData(current.toDouble())));
    }
}

void ViewportHeader::showTransform(const QJsonObject& transform) {
    const QSignalBlocker a(globalButton_);
    const QSignalBlocker b(localButton_);
    const QSignalBlocker c(editPivotButton_);
    editPivotButton_->setChecked(transform.value(QStringLiteral("edit_pivot")).toBool());
    const bool local = transform.value(QStringLiteral("orientation")).toString() == QLatin1String("local");
    localButton_->setChecked(local);
    globalButton_->setChecked(!local);
    const QString pivot = transform.value(QStringLiteral("pivot")).toString();
    for (QAction* action : pivotGroup_->actions()) {
        if (action->data().toString() == pivot) {
            action->setChecked(true);
        }
    }
    const QAction* checked = pivotGroup_->checkedAction();
    // The short word on the button; the full name is in the menu and the tooltip.
    const QString full = checked != nullptr ? checked->text() : QString();
    pivotButton_->setText(full.section(QLatin1Char(' '), 0, 0));
    pivotButton_->setToolTip(QStringLiteral("Pivô: %1. Ponto em volta do qual o objeto gira e escala").arg(full.toLower()));
}

void ViewportHeader::refreshIcons() {
    snapButton_->setIcon(PetuniaIcons::get(QStringLiteral("snap_magnet")));
    snapOptionsButton_->setIcon(PetuniaIcons::get(QStringLiteral("chevron_down")));
    snapOptionsButton_->setIconSize(QSize(10, 10));
    pivotButton_->setIcon(PetuniaIcons::get(QStringLiteral("origin")));
    editPivotButton_->setIcon(PetuniaIcons::get(QStringLiteral("pivot_median")));
    floorDown_->setIcon(PetuniaIcons::get(QStringLiteral("chevron_down")));
    floorUp_->setIcon(PetuniaIcons::get(QStringLiteral("chevron_up")));
    viewButton_->setIcon(PetuniaIcons::get(QStringLiteral("view_front")));
}
