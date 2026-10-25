#include "viewport_header.h"

#include <cmath>

#include <QButtonGroup>
#include <QComboBox>
#include <QPushButton>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLocale>
#include <QSignalBlocker>
#include <QToolButton>

#include "petunia_icons.h"
#include "wheel_guard.h"
#include "petunia_theme.h"

namespace {

QString numberText(double value) {
    return QLocale().toString(value, 'g', 4);
}

}  // namespace

ViewportHeader::ViewportHeader(QWidget* parent) : QWidget(parent) {
    setObjectName(QStringLiteral("PetuniaViewportHeader"));
    setAttribute(Qt::WA_StyledBackground);
    // One row: snapping on the left, the floor on the right. The units (m, °, ×) name the steps.
    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(6, 3, 6, 3);
    layout->setSpacing(4);

    snapButton_ = new QToolButton(this);
    snapButton_->setCheckable(true);
    snapButton_->setProperty("role", QStringLiteral("header"));
    snapButton_->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    snapButton_->setText(QStringLiteral("Snap"));
    snapButton_->setToolTip(QStringLiteral("Alinha a movimentos, giros e escalas a passos fixos (segure Ctrl para inverter)"));
    snapButton_->setAccessibleName(QStringLiteral("Ligar ou desligar o snap"));
    layout->addWidget(snapButton_);

    translateBox_ = makeStepBox(QStringLiteral("Passo de mover, em metros"), {0.1, 0.25, 0.5, 1.0, 2.0, 5.0}, QStringLiteral(" m"));
    rotateBox_ = makeStepBox(QStringLiteral("Passo de girar, em graus"), {5.0, 15.0, 45.0, 90.0}, QStringLiteral("°"));
    scaleBox_ = makeStepBox(QStringLiteral("Passo de escala"), {0.05, 0.1, 0.25, 0.5}, QStringLiteral("×"));
    layout->addWidget(translateBox_);
    layout->addWidget(rotateBox_);
    layout->addWidget(scaleBox_);

    // What a moving object snaps to (Petunia3D SnapTarget).
    targetBox_ = new QComboBox(this);
    targetBox_->setAccessibleName(QStringLiteral("Alvo do snap ao mover"));
    targetBox_->setToolTip(QStringLiteral(
        "Alvo do snap ao mover.\n"
        "Inteligente: canto de outro objeto, senão a superfície, senão a grade.\n"
        "Superfície: o objeto assenta no que está sob o mouse (topos, rampas, andares)."));
    targetBox_->addItem(QStringLiteral("Inteligente"), QStringLiteral("smart"));
    targetBox_->addItem(QStringLiteral("Grade"), QStringLiteral("grid"));
    targetBox_->addItem(QStringLiteral("Incremento"), QStringLiteral("increment"));
    targetBox_->addItem(QStringLiteral("Ponto"), QStringLiteral("point"));
    targetBox_->addItem(QStringLiteral("Superfície"), QStringLiteral("face"));
    WheelGuard::protect(targetBox_);
    layout->addWidget(targetBox_);
    slopeButton_ = new QToolButton(this);
    slopeButton_->setCheckable(true);
    slopeButton_->setProperty("role", QStringLiteral("header"));
    slopeButton_->setText(QStringLiteral("Inclinação"));
    slopeButton_->setToolTip(QStringLiteral("Ao assentar numa superfície, inclina o objeto junto com a rampa ou o terreno"));
    slopeButton_->setAccessibleName(QStringLiteral("Alinhar o objeto à inclinação da superfície"));
    layout->addWidget(slopeButton_);
    layout->addStretch(1);

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
    // Sized for "Subsolo 12": it is filled after construction, so it cannot adjust to its
    // contents. The elevation is in each item's tooltip, keeping the window narrow.
    floorBox_->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
    floorBox_->setMinimumContentsLength(9);
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

    // Gizmo controls for the top bar (Petunia3D's TransformOrientation and PivotPoint, adapted):
    // a two-button switch for the axes, then the pivot.
    transformControls_ = new QWidget(this);
    auto* transformLayout = new QHBoxLayout(transformControls_);
    transformLayout->setContentsMargins(0, 0, 0, 0);
    transformLayout->setSpacing(0);
    auto* axes = new QButtonGroup(transformControls_);
    auto makeSegment = [&](const QString& text, const QString& segment, const QString& tip) {
        auto* button = new QPushButton(text, transformControls_);
        button->setCheckable(true);
        button->setProperty("segment", segment);
        button->setToolTip(tip);
        button->setAccessibleName(tip);
        axes->addButton(button);
        transformLayout->addWidget(button);
        return button;
    };
    globalButton_ = makeSegment(QStringLiteral("Global"), QStringLiteral("first"),
                                QStringLiteral("Eixos do gizmo alinhados ao mundo"));
    localButton_ = makeSegment(QStringLiteral("Local"), QStringLiteral("last"),
                               QStringLiteral("Eixos do gizmo alinhados ao objeto (escalar usa sempre estes)"));
    globalButton_->setChecked(true);
    transformLayout->addSpacing(6);
    pivotBox_ = new QComboBox(transformControls_);
    pivotBox_->setAccessibleName(QStringLiteral("Ponto de pivô"));
    pivotBox_->setToolTip(QStringLiteral("Ponto em volta do qual o objeto gira e escala"));
    pivotBox_->addItem(QStringLiteral("Pivô do objeto"), QStringLiteral("object"));
    pivotBox_->addItem(QStringLiteral("Centro da caixa"), QStringLiteral("bounds_center"));
    pivotBox_->addItem(QStringLiteral("Base (no chão)"), QStringLiteral("base"));
    WheelGuard::protect(pivotBox_);
    transformLayout->addWidget(pivotBox_);
    transformLayout->addSpacing(2);
    editPivotButton_ = new QToolButton(transformControls_);
    editPivotButton_->setCheckable(true);
    editPivotButton_->setProperty("role", QStringLiteral("appTool"));
    editPivotButton_->setToolTip(QStringLiteral("Editar pivô: arraste o gizmo para mudar onde o objeto gira e escala (Alt+P)"));
    editPivotButton_->setAccessibleName(QStringLiteral("Editar o pivô do objeto"));
    transformLayout->addWidget(editPivotButton_);
    connect(editPivotButton_, &QToolButton::toggled, this, [this](bool on) {
        emit transformRequested(QJsonObject{{"edit_pivot", on}});
    });
    connect(globalButton_, &QPushButton::clicked, this, &ViewportHeader::emitTransform);
    connect(localButton_, &QPushButton::clicked, this, &ViewportHeader::emitTransform);
    connect(pivotBox_, &QComboBox::activated, this, &ViewportHeader::emitTransform);

    refreshIcons();

    auto emitSnap = [this] {
        emit snapRequested(QJsonObject{{"enabled", snapButton_->isChecked()},
                                       {"translate", translateBox_->currentData().toDouble()},
                                       {"rotate_degrees", rotateBox_->currentData().toDouble()},
                                       {"scale", scaleBox_->currentData().toDouble()},
                                       {"target", targetBox_->currentData().toString()},
                                       {"align_to_slope", slopeButton_->isChecked()}});
    };
    connect(snapButton_, &QToolButton::toggled, this, emitSnap);
    connect(slopeButton_, &QToolButton::toggled, this, emitSnap);
    connect(targetBox_, &QComboBox::activated, this, emitSnap);
    for (QComboBox* box : {translateBox_, rotateBox_, scaleBox_}) {
        connect(box, &QComboBox::activated, this, emitSnap);
    }
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, &ViewportHeader::refreshIcons);
}

void ViewportHeader::emitTransform() {
    emit transformRequested(QJsonObject{
        {"orientation", localButton_->isChecked() ? QStringLiteral("local") : QStringLiteral("global")},
        {"pivot", pivotBox_->currentData().toString()}});
}

void ViewportHeader::setControlsEnabled(bool enabled) {
    setEnabled(enabled);
    transformControls_->setEnabled(enabled);
}

QComboBox* ViewportHeader::makeStepBox(const QString& accessibleName, const QList<double>& steps, const QString& suffix) {
    auto* box = new QComboBox(this);
    box->setAccessibleName(accessibleName);
    WheelGuard::protect(box);
    box->setToolTip(accessibleName);
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

void ViewportHeader::showSnap(const QJsonObject& snap) {
    const QSignalBlocker a(snapButton_);
    const QSignalBlocker b(translateBox_);
    const QSignalBlocker c(rotateBox_);
    const QSignalBlocker d(scaleBox_);
    snapButton_->setChecked(snap.value(QStringLiteral("enabled")).toBool());
    select(translateBox_, snap.value(QStringLiteral("translate")).toDouble());
    select(rotateBox_, snap.value(QStringLiteral("rotate_degrees")).toDouble());
    select(scaleBox_, snap.value(QStringLiteral("scale")).toDouble());
    const QSignalBlocker e(targetBox_);
    const QSignalBlocker f(slopeButton_);
    targetBox_->setCurrentIndex(qMax(0, targetBox_->findData(snap.value(QStringLiteral("target")).toString())));
    slopeButton_->setChecked(snap.value(QStringLiteral("align_to_slope")).toBool());
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
    const QSignalBlocker c(pivotBox_);
    const QSignalBlocker d(editPivotButton_);
    editPivotButton_->setChecked(transform.value(QStringLiteral("edit_pivot")).toBool());
    const bool local = transform.value(QStringLiteral("orientation")).toString() == QLatin1String("local");
    localButton_->setChecked(local);
    globalButton_->setChecked(!local);
    pivotBox_->setCurrentIndex(qMax(0, pivotBox_->findData(transform.value(QStringLiteral("pivot")).toString())));
}

void ViewportHeader::refreshIcons() {
    snapButton_->setIcon(PetuniaIcons::get(QStringLiteral("snap_magnet")));
    editPivotButton_->setIcon(PetuniaIcons::get(QStringLiteral("pivot_median")));
    floorDown_->setIcon(PetuniaIcons::get(QStringLiteral("chevron_down")));
    floorUp_->setIcon(PetuniaIcons::get(QStringLiteral("chevron_up")));
}
