#include "inspector_panel.h"

#include <cmath>

#include <QCheckBox>
#include <QColorDialog>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QGridLayout>
#include <QHash>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QToolButton>
#include <QVBoxLayout>

#include "engine_bridge.h"
#include "petunia_icons.h"
#include "petunia_theme.h"
#include "table_icons.h"
#include "wheel_guard.h"
#include "petunia_number_field.h"
#include "petunia_vector3_field.h"

namespace {

constexpr double kFieldLimit = 9.0e15;  // the engine's integers go up to 2^53

QJsonObject vectorJson(double x, double y, double z) {
    return QJsonObject{{"x", x}, {"y", y}, {"z", z}};
}

QColor colorFromJson(const QJsonValue& value) {
    const QJsonObject object = value.toObject();
    return QColor::fromRgbF(
        static_cast<float>(qBound(0.0, object.value("r").toDouble(), 1.0)),
        static_cast<float>(qBound(0.0, object.value("g").toDouble(), 1.0)),
        static_cast<float>(qBound(0.0, object.value("b").toDouble(), 1.0)),
        static_cast<float>(qBound(0.0, object.value("a").toDouble(1.0), 1.0)));
}

QJsonObject colorToJson(const QColor& color) {
    return QJsonObject{{"r", color.redF()}, {"g", color.greenF()}, {"b", color.blueF()}, {"a", color.alphaF()}};
}

QString colorLabel(const QColor& color) {
    return color.name(QColor::HexRgb).toUpper();
}

void paintSwatch(QPushButton* button, const QColor& color) {
    const QColor text = color.lightnessF() > 0.55 ? Qt::black : Qt::white;
    button->setText(colorLabel(color));
    button->setStyleSheet(QStringLiteral("background-color: %1; color: %2;")
                              .arg(color.name(QColor::HexRgb), text.name(QColor::HexRgb)));
}

QString columnTitle(const QString& table, const QString& column) {
    return QStringLiteral("%1 · %2").arg(table, column);
}

bool isReadOnlyColumn(const QString& table, const QString& column) {
    return column == QLatin1String("owner")
        && (table == QLatin1String("Collider") || table == QLatin1String("Script"));
}

// The text in the label column for a column of a table. The engine names columns in English
// snake_case ("rotation_degrees"); the inspector shows something a person reads at a glance.
// The raw name stays in the tooltip, because scripts use it.
QString propertyLabel(const QString& table, const QString& column) {
    // Units go in the field (m, °), not in the label: the label column stays narrow.
    static const QHash<QString, QString> common = {
        {QStringLiteral("text"), QStringLiteral("Nome")},
        {QStringLiteral("position"), QStringLiteral("Posição")},
        {QStringLiteral("rotation_degrees"), QStringLiteral("Rotação")},
        {QStringLiteral("scale"), QStringLiteral("Escala")},
        {QStringLiteral("pivot"), QStringLiteral("Pivô")},
        {QStringLiteral("shape"), QStringLiteral("Forma")},
        {QStringLiteral("size"), QStringLiteral("Tamanho")},
        {QStringLiteral("detail"), QStringLiteral("Detalhe")},
        {QStringLiteral("color"), QStringLiteral("Cor")},
        {QStringLiteral("elevation"), QStringLiteral("Altura do piso")},
        {QStringLiteral("height"), QStringLiteral("Pé-direito")},
        {QStringLiteral("file"), QStringLiteral("Arquivo")},
        {QStringLiteral("enabled"), QStringLiteral("Ativo")},
        {QStringLiteral("faceted"), QStringLiteral("Facetada")},
        {QStringLiteral("cell_size"), QStringLiteral("Célula")},
        {QStringLiteral("base"), QStringLiteral("Base")},
        {QStringLiteral("name"), QStringLiteral("Nome")},
        {QStringLiteral("center"), QStringLiteral("Centro")},
        {QStringLiteral("radius"), QStringLiteral("Raio")},
        {QStringLiteral("trigger"), QStringLiteral("Gatilho")},
        {QStringLiteral("friction"), QStringLiteral("Atrito")},
        {QStringLiteral("restitution"), QStringLiteral("Quique")},
        {QStringLiteral("kind"), QStringLiteral("Tipo")},
        {QStringLiteral("mass"), QStringLiteral("Massa")},
        {QStringLiteral("gravity_scale"), QStringLiteral("Gravidade")},
        {QStringLiteral("linear_damping"), QStringLiteral("Freio")},
        {QStringLiteral("angular_damping"), QStringLiteral("Freio do giro")},
        {QStringLiteral("lock_rotation"), QStringLiteral("Sem girar")},
        {QStringLiteral("intensity"), QStringLiteral("Intensidade")},
        {QStringLiteral("range"), QStringLiteral("Alcance")},
        {QStringLiteral("ambient"), QStringLiteral("Ambiente")},
        {QStringLiteral("speed"), QStringLiteral("Velocidade")},
        {QStringLiteral("jump_speed"), QStringLiteral("Pulo")},
        {QStringLiteral("step_height"), QStringLiteral("Degrau")},
        {QStringLiteral("max_slope_degrees"), QStringLiteral("Rampa máx.")},
        {QStringLiteral("coyote_seconds"), QStringLiteral("Coiote")},
        {QStringLiteral("player"), QStringLiteral("Jogador")},
        {QStringLiteral("layer"), QStringLiteral("Camada")},
        {QStringLiteral("mask"), QStringLiteral("Máscara")},
    };
    if (table == QLatin1String("Collider") && column == QLatin1String("height")) {
        return QStringLiteral("Altura");
    }
    if (const auto found = common.constFind(column); found != common.constEnd()) {
        return found.value();
    }
    QString label = column;
    label.replace(QLatin1Char('_'), QLatin1Char(' '));
    if (!label.isEmpty()) {
        label[0] = label[0].toUpper();
    }
    return label;
}

QLabel* iconLabel(const QString& name, QWidget* parent) {
    auto* label = new QLabel(parent);
    label->setPixmap(PetuniaIcons::get(name).pixmap(16, 16));
    label->setFixedSize(18, 18);
    label->setAlignment(Qt::AlignCenter);
    return label;
}

}  // namespace

InspectorPanel::InspectorPanel(EngineBridge* bridge, QWidget* parent)
    : QWidget(parent), bridge_(bridge) {
    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    auto* body = new QWidget(scroll);
    content_ = new QVBoxLayout(body);
    content_->setContentsMargins(0, 0, 0, 0);
    content_->setSpacing(0);
    scroll->setWidget(body);

    auto* outer = new QVBoxLayout(this);
    outer->setContentsMargins(0, 0, 0, 0);
    outer->addWidget(scroll);
    // Icons are drawn in the theme's text color: rebuild when the theme changes.
    connect(&PetuniaTheme::instance(), &PetuniaTheme::changed, this, [this] {
        signature_.clear();
        reload();
    });
    showMessage(QStringLiteral("Nenhum objeto selecionado.\nClique em um objeto no viewport ou na Hierarquia."));
}

InspectorPanel::~InspectorPanel() {
    tearingDown_ = true;
}

void InspectorPanel::setEngineAvailable(bool available) {
    engineAvailable_ = available;
}

// Removes every widget from the panel. They are hidden and deleted LATER: this runs while a
// widget's own signal is still being handled (choosing "Adicionar tabela…" rebuilds the panel
// from inside that combo box), and deleting it right there crashes Qt.
void InspectorPanel::clearContent() {
    tearingDown_ = true;
    while (QLayoutItem* item = content_->takeAt(0)) {
        if (QWidget* widget = item->widget()) {
            widget->hide();
            widget->deleteLater();
        }
        delete item;
    }
    tearingDown_ = false;
}

void InspectorPanel::showMessage(const QString& text) {
    clearContent();
    fields_.clear();
    signature_.clear();
    entity_ = 0;
    auto* label = new QLabel(text);
    label->setWordWrap(true);
    label->setProperty("role", QStringLiteral("muted"));
    label->setContentsMargins(12, 12, 12, 12);
    label->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    content_->addWidget(label);
    content_->addStretch(1);
}

void InspectorPanel::reload() {
    if (!engineAvailable_ || tearingDown_) {
        return;
    }
    if (!bridge_->hasSelection()) {
        showMessage(QStringLiteral("Nenhum objeto selecionado.\nClique em um objeto no viewport ou na Hierarquia."));
        return;
    }
    const quint64 id = bridge_->selection();
    const Reply reply = bridge_->call(QStringLiteral("entity.describe"), QJsonObject{{"entity", static_cast<double>(id)}});
    if (!reply.ok) {
        showMessage(reply.explanation());
        return;
    }
    const QJsonObject description = reply.result.toObject();

    meshSummary_.clear();
    for (const QJsonValue& tableValue : description.value("tables").toArray()) {
        if (tableValue.toObject().value("name").toString() != QLatin1String("Mesh")) {
            continue;
        }
        const Reply info = bridge_->call(QStringLiteral("mesh.info"),
                                         QJsonObject{{QStringLiteral("entity"), static_cast<double>(id)}});
        if (info.ok) {
            const QJsonObject mesh = info.result.toObject();
            meshSummary_ = QStringLiteral("%1 vértices · %2 arestas · %3 faces · %4")
                               .arg(mesh.value(QStringLiteral("vertices")).toInt())
                               .arg(mesh.value(QStringLiteral("edges")).toInt())
                               .arg(mesh.value(QStringLiteral("faces")).toInt())
                               .arg(mesh.value(QStringLiteral("solid")).toBool() ? QStringLiteral("sólida")
                                                                                 : QStringLiteral("aberta"));
        }
        break;
    }

    QString signature = QString::number(id);
    for (const QJsonValue& tableValue : description.value("tables").toArray()) {
        const QJsonObject table = tableValue.toObject();
        signature += '|' + table.value("name").toString();
        for (const QJsonValue& columnValue : table.value("columns").toArray()) {
            const QJsonObject column = columnValue.toObject();
            signature += ':' + column.value("name").toString() + '/' + column.value("kind").toString();
        }
    }
    for (const QJsonValue& addable : description.value("addable_tables").toArray()) {
        signature += '+' + addable.toString();
    }

    if (signature == signature_) {
        refill(description);
    } else {
        entity_ = id;
        rebuild(description);
        signature_ = signature;
    }
}

void InspectorPanel::refill(const QJsonObject& description) {
    size_t index = 0;
    for (const QJsonValue& tableValue : description.value("tables").toArray()) {
        const QJsonObject table = tableValue.toObject();
        const QString tableName = table.value("name").toString();
        for (const QJsonValue& columnValue : table.value("columns").toArray()) {
            const QJsonObject column = columnValue.toObject();
            if (isReadOnlyColumn(tableName, column.value("name").toString())) {
                continue;
            }
            if (index >= fields_.size()) {
                return;
            }
            Field* field = fields_[index++].get();
            const QJsonValue value = column.value("value");
            field->current = value;
            field->show(value);
        }
    }
}

void InspectorPanel::rebuild(const QJsonObject& description) {
    clearContent();
    fields_.clear();

    // Header: what kind of object, its name, its id and its tags.
    QStringList tableNames;
    for (const QJsonValue& tableValue : description.value("tables").toArray()) {
        tableNames << tableValue.toObject().value("name").toString();
    }
    auto* header = new QWidget;
    header->setObjectName(QStringLiteral("PetuniaInspectorHeader"));
    header->setAttribute(Qt::WA_StyledBackground);
    auto* headerLayout = new QVBoxLayout(header);
    headerLayout->setContentsMargins(10, 8, 10, 8);
    headerLayout->setSpacing(4);
    auto* titleRow = new QHBoxLayout;
    titleRow->setSpacing(6);
    titleRow->addWidget(iconLabel(TableIcons::forEntity(tableNames), header));
    auto* title = new QLabel(description.value("name").toString(), header);
    title->setProperty("role", QStringLiteral("entityTitle"));
    title->setAccessibleName(QStringLiteral("Objeto selecionado"));
    title->setTextInteractionFlags(Qt::TextSelectableByMouse);
    titleRow->addWidget(title, 1);
    auto* id = new QLabel(QStringLiteral("#%1").arg(entity_), header);
    id->setProperty("role", QStringLiteral("entityId"));
    id->setToolTip(QStringLiteral("Número do objeto; scripts e o histórico usam este número"));
    titleRow->addWidget(id);
    headerLayout->addLayout(titleRow);
    const QJsonArray tags = description.value("tags").toArray();
    if (!tags.isEmpty()) {
        auto* chips = new QHBoxLayout;
        chips->setSpacing(4);
        for (const QJsonValue& tag : tags) {
            auto* chip = new QLabel(tag.toString(), header);
            chip->setProperty("role", QStringLiteral("chip"));
            chips->addWidget(chip);
        }
        chips->addStretch(1);
        headerLayout->addLayout(chips);
    }
    content_->addWidget(header);

    for (const QJsonValue& tableValue : description.value("tables").toArray()) {
        const QJsonObject table = tableValue.toObject();
        content_->addWidget(makeSection(table.value("name").toString(), table.value("columns").toArray()));
    }

    // A brush can become an editable mesh without going through Tab: the operation is the same.
    if (tableNames.contains(QStringLiteral("Brush"))) {
        auto* convert = new QPushButton(QStringLiteral("Tornar editável"));
        convert->setProperty("role", QStringLiteral("addRow"));
        convert->setIcon(PetuniaIcons::get(QStringLiteral("mode_edit")));
        convert->setAccessibleName(QStringLiteral("Transformar este brush em malha editável"));
        convert->setToolTip(QStringLiteral("Transforma o brush em malha editável; Tab entra no modo de edição"));
        connect(convert, &QPushButton::clicked, this, [this] {
            runOperation(QStringLiteral("mesh.convert"), QJsonObject{{"entity", static_cast<double>(entity_)}});
        });
        content_->addWidget(convert);
    }

    const QJsonArray addable = description.value("addable_tables").toArray();
    addableTables_.clear();
    for (const QJsonValue& name : addable) {
        addableTables_ << name.toString();
    }
    if (!addable.isEmpty()) {
        auto* add = new QPushButton(QStringLiteral("Adicionar tabela"));
        add->setProperty("role", QStringLiteral("addRow"));
        add->setIcon(PetuniaIcons::get(QStringLiteral("plus")));
        add->setAccessibleName(QStringLiteral("Adicionar uma tabela a este objeto"));
        add->setToolTip(QStringLiteral("Busca uma tabela pelo nome e acrescenta neste objeto"));
        connect(add, &QPushButton::clicked, this, &InspectorPanel::addTable);
        content_->addWidget(add);
    }
    // Named colliders (DR-27): each is its own entity, listed under the object in the hierarchy.
    if (!tableNames.contains(QStringLiteral("Collider"))) {
        auto* addCollider = new QPushButton(QStringLiteral("Adicionar colisão"));
        addCollider->setProperty("role", QStringLiteral("addRow"));
        addCollider->setIcon(PetuniaIcons::get(QStringLiteral("bounding_box")));
        addCollider->setAccessibleName(QStringLiteral("Adicionar uma colisão a este objeto"));
        addCollider->setToolTip(QStringLiteral("Uma forma de colisão com nome, já ajustada ao objeto; um objeto pode ter várias"));
        auto* shapes = new QMenu(addCollider);
        const QList<QPair<QString, QString>> kinds = {{QStringLiteral("box"), QStringLiteral("Caixa")},
                                                      {QStringLiteral("sphere"), QStringLiteral("Esfera")},
                                                      {QStringLiteral("capsule"), QStringLiteral("Cápsula (personagens)")},
                                                      {QStringLiteral("convex"), QStringLiteral("Convexa (pela malha)")}};
        for (const auto& [shape, label] : kinds) {
            connect(shapes->addAction(label), &QAction::triggered, this, [this, shape = shape] {
                runOperation(QStringLiteral("collider.add"), QJsonObject{{"shape", shape}});
            });
        }
        addCollider->setMenu(shapes);
        content_->addWidget(addCollider);
    }

    content_->addStretch(1);
}

QWidget* InspectorPanel::makeSection(const QString& tableName, const QJsonArray& columns) {
    auto* section = new QWidget;
    auto* sectionLayout = new QVBoxLayout(section);
    sectionLayout->setContentsMargins(0, 0, 0, 0);
    sectionLayout->setSpacing(0);

    auto* header = new QWidget(section);
    header->setObjectName(QStringLiteral("PetuniaSectionHeader"));
    header->setAttribute(Qt::WA_StyledBackground);
    header->setAttribute(Qt::WA_Hover);
    auto* headerLayout = new QHBoxLayout(header);
    headerLayout->setContentsMargins(4, 0, 4, 0);
    headerLayout->setSpacing(2);

    const bool folded = collapsed_.contains(tableName);
    auto* fold = new QToolButton(header);
    fold->setProperty("role", QStringLiteral("sectionFold"));
    fold->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    fold->setIcon(PetuniaIcons::get(folded ? QStringLiteral("chevron_right") : QStringLiteral("chevron_down")));
    fold->setIconSize(QSize(12, 12));
    fold->setText(TableIcons::displayName(tableName));
    fold->setToolTip(QStringLiteral("Tabela %1 (nome usado nos scripts)").arg(tableName));
    fold->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    fold->setAccessibleName(QStringLiteral("Mostrar ou esconder a tabela %1").arg(tableName));
    fold->setAccessibleDescription(folded ? QStringLiteral("recolhida") : QStringLiteral("aberta"));
    headerLayout->addWidget(iconLabel(TableIcons::forTable(tableName), header));
    headerLayout->addWidget(fold, 1);

    auto* more = new QToolButton(header);
    more->setProperty("role", QStringLiteral("panelButton"));
    more->setIcon(PetuniaIcons::get(QStringLiteral("more_horiz")));
    more->setPopupMode(QToolButton::InstantPopup);
    more->setToolTip(QStringLiteral("Mais ações da tabela %1").arg(tableName));
    more->setAccessibleName(more->toolTip());
    auto* menu = new QMenu(more);
    QAction* remove = menu->addAction(PetuniaIcons::get(QStringLiteral("trash")), QStringLiteral("Remover %1").arg(TableIcons::displayName(tableName).toLower()));
    connect(remove, &QAction::triggered, this, [this, tableName] {
        runCommand(QStringLiteral("row.remove"), QJsonObject{{"entity", static_cast<double>(entity_)}, {"table", tableName}});
    });
    if (tableName == QLatin1String("Character") || tableName == QLatin1String("Script")) {
        QAction* editScript = menu->addAction(PetuniaIcons::get(QStringLiteral("script")), QStringLiteral("Editar script"));
        connect(editScript, &QAction::triggered, this, [this, tableName] {
            if (tableName == QLatin1String("Script")) {
                for (const auto& field : fields_) {
                    if (field->table == QLatin1String("Script") && field->column == QLatin1String("file") &&
                        !field->current.toString().isEmpty()) {
                        emit scriptOpenRequested(field->current.toString());
                        return;
                    }
                }
            }
            attachScript();
        });
    }
    if (tableName == QLatin1String("Collider")) {
        // Removing the row would leave an empty entity: a collider is deleted as a whole.
        remove->setText(QStringLiteral("Excluir esta colisão"));
        disconnect(remove, nullptr, this, nullptr);
        connect(remove, &QAction::triggered, this, [this] {
            runCommand(QStringLiteral("world.remove"), QJsonObject{{"entity", static_cast<double>(entity_)}});
        });
        QAction* fit = menu->addAction(PetuniaIcons::get(QStringLiteral("frame_selection")), QStringLiteral("Ajustar ao objeto"));
        connect(fit, &QAction::triggered, this, [this] { runOperation(QStringLiteral("collider.fit"), {}); });
    }
    more->setMenu(menu);
    headerLayout->addWidget(more);
    sectionLayout->addWidget(header);

    auto* body = new QWidget(section);
    body->setObjectName(QStringLiteral("PetuniaSectionBody"));
    body->setAttribute(Qt::WA_StyledBackground);
    auto* grid = new QGridLayout(body);
    grid->setContentsMargins(12, 0, 8, 8);
    grid->setHorizontalSpacing(8);
    grid->setVerticalSpacing(4);
    // The values get the room: the label column is as wide as its longest label.
    grid->setColumnStretch(1, 1);
    int row = 0;
    for (const QJsonValue& columnValue : columns) {
        const QJsonObject column = columnValue.toObject();
        auto field = std::make_unique<Field>();
        field->table = tableName;
        field->column = column.value("name").toString();
        // Which object a collider belongs to is set when it is created; not an editable number.
        if (isReadOnlyColumn(tableName, field->column)) {
            continue;
        }
        field->current = column.value("value");
        QWidget* editor = makeEditor(tableName, column, field.get());
        field->show(field->current);
        auto* label = new QLabel(propertyLabel(tableName, field->column), body);
        label->setProperty("role", QStringLiteral("propertyLabel"));
        static const QHash<QString, QString> help = {
            {QStringLiteral("range"), QStringLiteral("Até onde a luz pontual chega, em metros (o sol ignora)")},
            {QStringLiteral("ambient"), QStringLiteral("Luz suave em tudo, de 0 a 1 (vale o valor da primeira luz da cena)")},
            {QStringLiteral("intensity"), QStringLiteral("Força da luz, de 0 a 16")},
            {QStringLiteral("player"), QStringLiteral("O teclado controla este personagem no modo Jogar (W A S D, Espaço pula)")},
            {QStringLiteral("coyote_seconds"), QStringLiteral("Tempo de coiote: o pulo ainda funciona por este tempo depois de sair de uma borda (ajuda de acessibilidade)")},
            {QStringLiteral("step_height"), QStringLiteral("Altura do degrau que o personagem sobe sem pular, em metros")},
            {QStringLiteral("max_slope_degrees"), QStringLiteral("Inclinação máxima da rampa que o personagem sobe, em graus")},
            {QStringLiteral("layer"), QStringLiteral("Camadas em que a colisão está (bits; 1 é a camada 1)")},
            {QStringLiteral("mask"), QStringLiteral("Só toca colisões cuja camada divide um bit com esta máscara (todos os bits ligados = toca tudo)")},
        };
        label->setToolTip(QStringLiteral("%1.%2%3").arg(tableName, field->column,
                                                        help.contains(field->column) ? QStringLiteral("\n") + help.value(field->column) : QString()));
        label->setBuddy(editor);
        grid->addWidget(label, row, 0, Qt::AlignVCenter);
        grid->addWidget(editor, row, 1);
        fields_.push_back(std::move(field));
        ++row;
    }
    body->setVisible(!folded);
    sectionLayout->addWidget(body);

    connect(fold, &QToolButton::clicked, this, [this, tableName, body, fold] {
        const bool nowFolded = body->isVisible();
        body->setVisible(!nowFolded);
        fold->setIcon(PetuniaIcons::get(nowFolded ? QStringLiteral("chevron_right") : QStringLiteral("chevron_down")));
        fold->setAccessibleDescription(nowFolded ? QStringLiteral("recolhida") : QStringLiteral("aberta"));
        if (nowFolded) {
            collapsed_.insert(tableName);
        } else {
            collapsed_.remove(tableName);
        }
    });
    return section;
}

QWidget* InspectorPanel::makeEditor(const QString& table, const QJsonObject& column, Field* field) {
    const QString kind = column.value("kind").toString();
    const QString name = column.value("name").toString();
    const QString accessible = columnTitle(table, name);

    if (kind == "bool") {
        auto* box = new QCheckBox;
        box->setAccessibleName(accessible);
        connect(box, &QCheckBox::toggled, this, [this, field](bool on) { apply(field, on); });
        field->show = [box](const QJsonValue& value) {
            const QSignalBlocker blocker(box);
            box->setChecked(value.toBool());
        };
        return box;
    }
    if (kind == "int" || kind == "float") {
        const bool isInt = kind == "int";
        auto* number = new PetuniaNumberField;
        number->setLabel(accessible);
        number->setRange(-kFieldLimit, kFieldLimit);
        number->setMaxDecimals(isInt ? 0 : 3);
        number->setSingleStep(isInt ? 1.0 : 0.1);
        auto toJson = [isInt](double value) {
            return isInt ? QJsonValue(static_cast<qint64>(std::llround(value))) : QJsonValue(value);
        };
        connect(number, &PetuniaNumberField::previewRequested, this,
                [this, field, toJson](const QString&, double value) { previewValue(field, toJson(value)); });
        connect(number, &PetuniaNumberField::commitRequested, this,
                [this, field, toJson](const QString&, double value, const QString&) { commitValue(field, toJson(value)); });
        connect(number, &PetuniaNumberField::editCanceled, this, [this](const QString&) { cancelValue(); });
        field->show = [number](const QJsonValue& value) {
            if (!number->isEditing() && !number->isScrubbing()) {
                const QSignalBlocker blocker(number);
                number->setValue(value.toDouble());
            }
        };
        return number;
    }
    if (kind == "floats" || kind == "spline" || kind == "mesh") {
        auto* summary = new QLabel;
        summary->setAccessibleName(accessible);
        summary->setTextInteractionFlags(Qt::TextSelectableByMouse);
        field->show = [this, summary, kind](const QJsonValue& value) {
            if (kind == "floats") {
                summary->setText(QStringLiteral("%1 números").arg(value.toArray().size()));
            } else if (kind == "spline") {
                summary->setText(QStringLiteral("%1 pontos%2")
                                     .arg(value.toObject().value(QStringLiteral("points")).toArray().size())
                                     .arg(value.toObject().value(QStringLiteral("closed")).toBool()
                                              ? QStringLiteral(", fechada")
                                              : QStringLiteral(", aberta")));
            } else {
                summary->setText(meshSummary_.isEmpty() ? QStringLiteral("malha editável") : meshSummary_);
            }
        };
        return summary;
    }
    if (kind == "text") {
        const QJsonArray choices = column.value("choices").toArray();
        if (!choices.isEmpty()) {
            auto* combo = new QComboBox;
            combo->setAccessibleName(accessible);
            WheelGuard::protect(combo);
            for (const QJsonValue& choice : choices) {
                combo->addItem(choice.toString());
            }
            connect(combo, &QComboBox::activated, this, [this, field, combo](int index) {
                apply(field, combo->itemText(index));
            });
            field->show = [combo](const QJsonValue& value) {
                const QSignalBlocker blocker(combo);
                const QString text = value.toString();
                if (combo->findText(text) < 0) {
                    combo->addItem(text);
                }
                combo->setCurrentText(text);
            };
            return combo;
        }
        auto* edit = new QLineEdit;
        edit->setAccessibleName(accessible);
        connect(edit, &QLineEdit::editingFinished, this, [this, field, edit] { apply(field, edit->text()); });
        field->show = [edit](const QJsonValue& value) {
            const QSignalBlocker blocker(edit);
            if (!edit->hasFocus()) {
                edit->setText(value.toString());
            }
        };
        return edit;
    }
    if (kind == "vec3") {
        auto* vector = new PetuniaVector3Field;
        vector->setLabel(accessible);
        vector->setRange(-kFieldLimit, kFieldLimit);
        vector->setMaxDecimals(3);
        vector->setSingleStep(0.1);
        auto compose = [vector](int axis, double value) {
            double parts[3] = {vector->value(0), vector->value(1), vector->value(2)};
            parts[axis] = value;
            return vectorJson(parts[0], parts[1], parts[2]);
        };
        connect(vector, &PetuniaVector3Field::previewRequested, this,
                [this, field, compose](int axis, double value) { previewValue(field, compose(axis, value)); });
        connect(vector, &PetuniaVector3Field::commitRequested, this,
                [this, field, compose](int axis, double value, const QString&) { commitValue(field, compose(axis, value)); });
        connect(vector, &PetuniaVector3Field::editCanceled, this, [this](int) { cancelValue(); });
        field->show = [vector](const QJsonValue& value) {
            for (int axis = 0; axis < 3; ++axis) {
                if (vector->field(axis)->isEditing() || vector->field(axis)->isScrubbing()) {
                    return;
                }
            }
            const QJsonObject object = value.toObject();
            const QSignalBlocker blocker(vector);
            vector->setValues(object.value("x").toDouble(), object.value("y").toDouble(), object.value("z").toDouble());
        };
        return vector;
    }
    // color
    auto* button = new QPushButton;
    button->setAccessibleName(accessible);
    connect(button, &QPushButton::clicked, this, [this, field, button] {
        const QColor chosen = QColorDialog::getColor(colorFromJson(field->current), this,
                                                     QStringLiteral("Escolher cor"),
                                                     QColorDialog::ShowAlphaChannel);
        if (chosen.isValid()) {
            apply(field, colorToJson(chosen));
            paintSwatch(button, chosen);
        }
    });
    field->show = [button](const QJsonValue& value) { paintSwatch(button, colorFromJson(value)); };
    return button;
}

void InspectorPanel::apply(Field* field, const QJsonValue& value) {
    if (!engineAvailable_ || tearingDown_) {
        return;
    }
    if (value == field->current) {
        return;  // spin boxes also report "no change" when they lose focus
    }
    runCommand(QStringLiteral("world.set"),
               QJsonObject{{"entity", static_cast<double>(entity_)},
                           {"table", field->table},
                           {"column", field->column},
                           {"value", value}});
}

void InspectorPanel::previewValue(Field* field, const QJsonValue& value) {
    if (!engineAvailable_ || tearingDown_) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("gesture.preview"),
                                      QJsonObject{{"entity", static_cast<double>(entity_)},
                                                  {"table", field->table},
                                                  {"column", field->column},
                                                  {"value", value}});
    if (reply.ok) {
        emit previewed();
    } else {
        emit editFailed(reply.explanation());
    }
}

void InspectorPanel::commitValue(Field* field, const QJsonValue& value) {
    if (!engineAvailable_ || tearingDown_) {
        return;
    }
    // Same path for a typed value and for the end of a drag: preview the final value, then commit.
    previewValue(field, value);
    const Reply reply = bridge_->call(QStringLiteral("gesture.commit"));
    if (!reply.ok) {
        emit editFailed(reply.explanation());
    }
    emit edited();
}

void InspectorPanel::cancelValue() {
    if (!engineAvailable_ || tearingDown_) {
        return;
    }
    bridge_->call(QStringLiteral("gesture.cancel"));
    emit edited();
}

QString InspectorPanel::chooseName(const QString& title, const QStringList& names, const QString& empty) const {
    QDialog dialog(const_cast<InspectorPanel*>(this));
    dialog.setWindowTitle(title);
    dialog.setAccessibleName(title);
    auto* layout = new QVBoxLayout(&dialog);
    auto* search = new QLineEdit(&dialog);
    search->setPlaceholderText(QStringLiteral("Buscar pelo nome"));
    search->setAccessibleName(QStringLiteral("Buscar pelo nome"));
    search->setClearButtonEnabled(true);
    layout->addWidget(search);
    auto* list = new QListWidget(&dialog);
    list->setAccessibleName(title);
    for (const QString& name : names) {
        list->addItem(name);
    }
    if (names.isEmpty()) {
        auto* note = new QListWidgetItem(empty);
        note->setFlags(Qt::NoItemFlags);
        list->addItem(note);
    }
    layout->addWidget(list);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &dialog);
    buttons->button(QDialogButtonBox::Ok)->setText(QStringLiteral("Escolher"));
    buttons->button(QDialogButtonBox::Cancel)->setText(QStringLiteral("Cancelar"));
    layout->addWidget(buttons);
    connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
    connect(search, &QLineEdit::textChanged, list, [list](const QString& text) {
        const QString needle = text.trimmed();
        for (int row = 0; row < list->count(); ++row) {
            QListWidgetItem* item = list->item(row);
            const bool usable = item->flags().testFlag(Qt::ItemIsEnabled);
            item->setHidden(usable && !needle.isEmpty() && !item->text().contains(needle, Qt::CaseInsensitive));
        }
    });
    connect(list, &QListWidget::itemActivated, &dialog, &QDialog::accept);
    search->setFocus();
    if (dialog.exec() != QDialog::Accepted || list->currentItem() == nullptr ||
        !list->currentItem()->flags().testFlag(Qt::ItemIsEnabled)) {
        return {};
    }
    return list->currentItem()->text();
}

void InspectorPanel::addTable() {
    QStringList labels;
    QHash<QString, QString> tables;
    for (const QString& table : addableTables_) {
        const QString label = QStringLiteral("%1 (%2)").arg(TableIcons::displayName(table), table);
        labels << label;
        tables.insert(label, table);
    }
    const QString chosen = chooseName(QStringLiteral("Adicionar tabela"), labels, QStringLiteral("Nenhuma tabela disponível"));
    if (chosen.isEmpty()) {
        return;
    }
    const QString table = tables.value(chosen);
    if (table == QLatin1String("Script")) {
        attachScript();
        return;
    }
    runCommand(QStringLiteral("row.create"), QJsonObject{{"entity", static_cast<double>(entity_)}, {"table", table}});
}

void InspectorPanel::attachScript() {
    const Reply listed = bridge_->call(QStringLiteral("script.list"));
    if (!listed.ok) {
        emit editFailed(listed.explanation());
        return;
    }
    QStringList names;
    names << QStringLiteral("Novo script…");
    for (const QJsonValue& value : listed.result.toObject().value(QStringLiteral("scripts")).toArray()) {
        names << value.toObject().value(QStringLiteral("name")).toString();
    }
    const QString chosen = chooseName(QStringLiteral("Escolher script"), names, QStringLiteral("Nenhum script no projeto"));
    if (chosen.isEmpty()) {
        return;
    }
    QString name = chosen;
    if (chosen == QLatin1String("Novo script…")) {
        bool accepted = false;
        name = QInputDialog::getText(this, QStringLiteral("Novo script"), QStringLiteral("Nome do script"), QLineEdit::Normal, QStringLiteral("teste"), &accepted).trimmed();
        if (!accepted || name.isEmpty()) {
            return;
        }
        const Reply created = bridge_->call(QStringLiteral("script.new"), QJsonObject{{"name", name}});
        if (!created.ok) {
            emit editFailed(created.explanation());
            return;
        }
    }
    const Reply attached = bridge_->call(QStringLiteral("script.attach"), QJsonObject{{"name", name}});
    if (!attached.ok) {
        emit editFailed(attached.explanation());
        return;
    }
    emit edited();
    emit scriptOpenRequested(name);
}

void InspectorPanel::runOperation(const QString& operation, const QJsonObject& fields) {
    if (!engineAvailable_ || tearingDown_) {
        return;
    }
    const Reply reply = bridge_->call(operation, fields);
    if (!reply.ok) {
        emit editFailed(reply.explanation());
    }
    emit edited();
}

void InspectorPanel::runCommand(const QString& command, const QJsonObject& payload) {
    if (!engineAvailable_ || tearingDown_) {
        return;
    }
    const Reply reply = bridge_->call(QStringLiteral("command.execute"),
                                      QJsonObject{{"command", command}, {"payload", payload}});
    if (!reply.ok) {
        emit editFailed(reply.explanation());
    }
    emit edited();  // on failure too: the panel then shows the real value again
}
