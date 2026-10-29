//! The catalog of nodes a graph may use.
//!
//! Almost every entry is **generated**: one node per function of the script API, one event node per
//! handler a script may define. A handful of nodes are fixed (if, loops, operators, variables).
//! Nothing here knows the API itself: the caller hands in plain descriptions, so this crate stays a
//! pure domain crate (no scripting, rendering or windowing).

use std::collections::BTreeMap;

/// The type of a value on a data pin. Names follow the script API: a vector is a `Dict` with
/// `x`, `y`, `z`, and an object is an `Int` id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataType {
    Int,
    Float,
    Bool,
    Text,
    Dict,
    List,
    /// Any value; checked when the game runs.
    Any,
}

impl DataType {
    /// The type named by an API signature (`String`, `Value`, ...).
    pub fn from_api(name: &str) -> Self {
        match name {
            "Int" => Self::Int,
            "Float" => Self::Float,
            "Bool" => Self::Bool,
            "String" => Self::Text,
            "Dict" => Self::Dict,
            "List" => Self::List,
            _ => Self::Any,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Int => "Inteiro",
            Self::Float => "Decimal",
            Self::Bool => "Sim/Não",
            Self::Text => "Texto",
            Self::Dict => "Dicionário",
            Self::List => "Lista",
            Self::Any => "Qualquer",
        }
    }

    /// Whether a pin of type `self` accepts a wire from a pin of type `source`. The only automatic
    /// conversion is whole number to decimal.
    pub fn accepts(self, source: DataType) -> bool {
        self == Self::Any
            || source == Self::Any
            || self == source
            || (self == Self::Float && source == Self::Int)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinKind {
    /// Order of execution: what runs after what.
    Exec,
    Data(DataType),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PinDefault {
    /// Must be wired or given a constant.
    Required,
    /// Left empty, it means the object the event is about (the `entity` of the handler).
    ThisObject,
    /// May stay empty; the node decides what an empty value means.
    Optional,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PinSpec {
    pub name: String,
    pub label: String,
    pub direction: Direction,
    pub kind: PinKind,
    pub default: PinDefault,
}

/// A setting that is not a pin: an operator, a variable name.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingSpec {
    pub name: String,
    pub label: String,
    pub choices: SettingChoices,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingChoices {
    /// One of these `(value, label)` pairs.
    OneOf(Vec<(String, String)>),
    /// Free text (a variable name).
    Text,
    /// Free text that may be empty (a function's parameter list).
    TextOptional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeClass {
    /// Starts a chain when something happens.
    Event,
    /// Does something; sits in a chain.
    Action,
    /// Only computes a value; wired by data.
    Value,
    /// Chooses or repeats; sits in a chain.
    Flow,
}

/// What the compiler writes for a node.
#[derive(Clone, Debug, PartialEq)]
pub enum Lowering {
    Event { handler: String },
    Api { module: String, name: String },
    If,
    Repeat,
    Each,
    While,
    Loop,
    Break,
    Continue,
    Wait,
    Field,
    Index,
    List,
    Dict,
    Function,
    Call,
    CallValue,
    Param,
    Return,
    Global,
    GlobalGet,
    GlobalSet,
    LocalGet,
    LocalSet,
    LocalDeclare,
    Compare,
    Logic,
    Not,
    Math,
    Join,
    Vector,
    VectorPart,
    VarGet,
    VarSet,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    pub kind: String,
    pub title: String,
    pub category: String,
    pub summary: String,
    pub class: NodeClass,
    pub pins: Vec<PinSpec>,
    pub settings: Vec<SettingSpec>,
    /// Capability the call needs (empty for none).
    pub permission: String,
    pub lowering: Lowering,
}

impl NodeSpec {
    pub fn pin(&self, name: &str, direction: Direction) -> Option<&PinSpec> {
        self.pins
            .iter()
            .find(|pin| pin.name == name && pin.direction == direction)
    }

    pub fn setting(&self, name: &str) -> Option<&SettingSpec> {
        self.settings.iter().find(|setting| setting.name == name)
    }

    /// Whether the node sits in an execution chain (has an `in` exec pin).
    pub fn has_exec_in(&self) -> bool {
        self.pins
            .iter()
            .any(|pin| pin.direction == Direction::In && pin.kind == PinKind::Exec)
    }
}

/// One function of the script API, as plain data.
#[derive(Clone, Debug)]
pub struct ApiSpec {
    pub module: String,
    pub name: String,
    pub params: Vec<(String, String)>,
    pub returns: String,
    pub summary: String,
    pub permission: String,
}

/// One handler a script may define, with its typed parameters.
#[derive(Clone, Debug)]
pub struct EventSpec {
    pub handler: String,
    pub title: String,
    pub summary: String,
    pub params: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Catalog {
    nodes: BTreeMap<String, NodeSpec>,
}

fn pin(name: &str, label: &str, direction: Direction, kind: PinKind) -> PinSpec {
    PinSpec {
        name: name.to_string(),
        label: label.to_string(),
        direction,
        kind,
        default: PinDefault::Required,
    }
}

fn exec_in() -> PinSpec {
    pin("in", "Executar", Direction::In, PinKind::Exec)
}

fn exec_out(name: &str, label: &str) -> PinSpec {
    pin(name, label, Direction::Out, PinKind::Exec)
}

fn data_in(name: &str, label: &str, data_type: DataType) -> PinSpec {
    pin(name, label, Direction::In, PinKind::Data(data_type))
}

fn data_out(name: &str, label: &str, data_type: DataType) -> PinSpec {
    pin(name, label, Direction::Out, PinKind::Data(data_type))
}

fn choice_setting(name: &str, label: &str, choices: &[(&str, &str)]) -> SettingSpec {
    SettingSpec {
        name: name.to_string(),
        label: label.to_string(),
        choices: SettingChoices::OneOf(
            choices
                .iter()
                .map(|(value, text)| (value.to_string(), text.to_string()))
                .collect(),
        ),
    }
}

fn text_setting(name: &str, label: &str) -> SettingSpec {
    SettingSpec {
        name: name.to_string(),
        label: label.to_string(),
        choices: SettingChoices::Text,
    }
}

fn optional_text_setting(name: &str, label: &str) -> SettingSpec {
    SettingSpec {
        name: name.to_string(),
        label: label.to_string(),
        choices: SettingChoices::TextOptional,
    }
}

fn optional_data_in(name: &str, label: &str, data_type: DataType) -> PinSpec {
    let mut pin = data_in(name, label, data_type);
    pin.default = PinDefault::Optional;
    pin
}

fn item_inputs(count: usize) -> Vec<PinSpec> {
    (1..=count)
        .map(|index| {
            optional_data_in(
                &format!("item{index}"),
                &format!("Item {index}"),
                DataType::Any,
            )
        })
        .collect()
}

fn dict_inputs(count: usize) -> Vec<PinSpec> {
    let mut pins = Vec::new();
    for index in 1..=count {
        pins.push(optional_data_in(
            &format!("key{index}"),
            &format!("Chave {index}"),
            DataType::Any,
        ));
        pins.push(optional_data_in(
            &format!("value{index}"),
            &format!("Valor {index}"),
            DataType::Any,
        ));
    }
    pins
}

fn call_inputs() -> Vec<PinSpec> {
    (1..=8)
        .map(|index| {
            data_in(
                &format!("arg{index}"),
                &format!("Argumento {index}"),
                DataType::Any,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn fixed(
    kind: &str,
    title: &str,
    category: &str,
    summary: &str,
    class: NodeClass,
    pins: Vec<PinSpec>,
    settings: Vec<SettingSpec>,
    lowering: Lowering,
) -> NodeSpec {
    NodeSpec {
        kind: kind.to_string(),
        title: title.to_string(),
        category: category.to_string(),
        summary: summary.to_string(),
        class,
        pins,
        settings,
        permission: String::new(),
        lowering,
    }
}

/// Portuguese label of an API parameter; the API's own name when there is none.
fn parameter_label(name: &str) -> String {
    match name {
        "entity" => "Objeto",
        "x" => "X",
        "y" => "Y",
        "z" => "Z",
        "table" => "Tabela",
        "column" => "Coluna",
        "name" => "Nome",
        "value" => "Valor",
        "rows" => "Linhas",
        "origin" => "Origem",
        "direction" => "Direção",
        "distance" => "Distância",
        "velocity" => "Velocidade",
        "impulse" => "Impulso",
        "delta" => "Quanto",
        "degrees" => "Graus",
        "message" => "Mensagem",
        other => other,
    }
    .to_string()
}

fn category_of(module: &str) -> &'static str {
    match module {
        "world" => "Mundo",
        "transform" => "Transformação",
        "physics" => "Física",
        "collision" => "Colisão",
        "character" => "Personagem",
        "game" => "Jogo",
        "log" => "Registro",
        _ => "Outros",
    }
}

/// Portuguese title of an API function; `module.name` when none is known.
fn api_title(module: &str, name: &str) -> String {
    let known = match (module, name) {
        ("character", "move") => "Mover personagem",
        ("character", "jump") => "Pular",
        ("character", "grounded") => "Personagem está no chão",
        ("collision", "list") => "Listar colisões",
        ("collision", "get") => "Ler colisão",
        ("collision", "set") => "Mudar colisão",
        ("collision", "overlaps") => "Quem toca esta colisão",
        ("physics", "raycast") => "Lançar raio",
        ("physics", "raycast_from") => "Lançar raio a partir do objeto",
        ("physics", "velocity") => "Velocidade do objeto",
        ("physics", "set_velocity") => "Definir velocidade",
        ("physics", "impulse") => "Dar impulso",
        ("world", "get") => "Ler valor do objeto",
        ("world", "row") => "Ler tabela do objeto",
        ("world", "has") => "Objeto tem tabela",
        ("world", "exists") => "Objeto existe",
        ("world", "find") => "Achar objetos com tabela",
        ("world", "set") => "Mudar valor do objeto",
        ("world", "create") => "Criar objeto",
        ("world", "remove") => "Apagar objeto",
        ("transform", "get_position") => "Posição do objeto",
        ("transform", "set_position") => "Definir posição",
        ("transform", "move") => "Mover objeto",
        ("transform", "move_x") => "Mover no X",
        ("transform", "move_y") => "Mover no Y",
        ("transform", "move_z") => "Mover no Z",
        ("transform", "get_rotation") => "Rotação do objeto",
        ("transform", "set_rotation") => "Definir rotação",
        ("transform", "rotate") => "Girar objeto",
        ("transform", "rotate_x") => "Girar no X",
        ("transform", "rotate_y") => "Girar no Y",
        ("transform", "rotate_z") => "Girar no Z",
        ("transform", "get_scale") => "Escala do objeto",
        ("transform", "set_scale") => "Definir escala",
        ("transform", "forward") => "Direção da frente",
        ("transform", "right") => "Direção da direita",
        ("transform", "up") => "Direção de cima",
        ("game", "tick") => "Quadro atual",
        ("game", "seconds") => "Segundos de jogo",
        ("game", "emit") => "Emitir evento",
        ("log", "debug") => "Registrar (depuração)",
        ("log", "info") => "Registrar (informação)",
        ("log", "warning") => "Registrar (aviso)",
        ("log", "error") => "Registrar (erro)",
        _ => "",
    };
    if known.is_empty() {
        format!("{module}.{name}")
    } else {
        known.to_string()
    }
}

impl Catalog {
    /// Builds the catalog from the API functions and the handlers, plus the fixed nodes.
    pub fn new(api: &[ApiSpec], events: &[EventSpec]) -> Self {
        let mut nodes = BTreeMap::new();
        for event in events {
            let mut pins = vec![exec_out("then", "Então")];
            for (name, type_name) in &event.params {
                pins.push(data_out(
                    name,
                    &parameter_label(name),
                    DataType::from_api(type_name),
                ));
            }
            let kind = format!("event.{}", event.handler);
            nodes.insert(
                kind.clone(),
                NodeSpec {
                    kind,
                    title: event.title.clone(),
                    category: "Eventos".to_string(),
                    summary: event.summary.clone(),
                    class: NodeClass::Event,
                    pins,
                    settings: Vec::new(),
                    permission: String::new(),
                    lowering: Lowering::Event {
                        handler: event.handler.clone(),
                    },
                },
            );
        }
        for function in api {
            let effect = function.returns == "None" || function.permission == "world.write";
            let mut pins = Vec::new();
            if effect {
                pins.push(exec_in());
            }
            for (name, type_name) in &function.params {
                let data_type = DataType::from_api(type_name);
                let mut spec = data_in(name, &parameter_label(name), data_type);
                if name == "entity" && data_type == DataType::Int {
                    spec.default = PinDefault::ThisObject;
                }
                pins.push(spec);
            }
            if effect {
                pins.push(exec_out("then", "Depois"));
            }
            if function.returns != "None" {
                pins.push(data_out(
                    "result",
                    "Resultado",
                    DataType::from_api(&function.returns),
                ));
            }
            let kind = format!("api.{}.{}", function.module, function.name);
            nodes.insert(
                kind.clone(),
                NodeSpec {
                    kind,
                    title: api_title(&function.module, &function.name),
                    category: category_of(&function.module).to_string(),
                    summary: function.summary.clone(),
                    class: if effect {
                        NodeClass::Action
                    } else {
                        NodeClass::Value
                    },
                    pins,
                    settings: Vec::new(),
                    permission: function.permission.clone(),
                    lowering: Lowering::Api {
                        module: function.module.clone(),
                        name: function.name.clone(),
                    },
                },
            );
        }
        for spec in fixed_nodes() {
            nodes.insert(spec.kind.clone(), spec);
        }
        Self { nodes }
    }

    pub fn get(&self, kind: &str) -> Option<&NodeSpec> {
        self.nodes.get(kind)
    }

    /// Every node, in kind order.
    pub fn iter(&self) -> impl Iterator<Item = &NodeSpec> {
        self.nodes.values()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

fn fixed_nodes() -> Vec<NodeSpec> {
    let comparisons = [
        ("==", "é igual a"),
        ("!=", "é diferente de"),
        ("<", "é menor que"),
        ("<=", "é menor ou igual a"),
        (">", "é maior que"),
        (">=", "é maior ou igual a"),
    ];
    vec![
        fixed(
            "flow.if",
            "Se",
            "Fluxo",
            "Roda um caminho se a condição for verdadeira e outro se não for; depois continua.",
            NodeClass::Flow,
            vec![
                exec_in(),
                data_in("condition", "Condição", DataType::Bool),
                exec_out("then", "Então"),
                exec_out("else", "Senão"),
                exec_out("done", "Depois do Se"),
            ],
            Vec::new(),
            Lowering::If,
        ),
        fixed(
            "flow.repeat",
            "Repetir",
            "Fluxo",
            "Roda o corpo algumas vezes; o número da volta começa em 0.",
            NodeClass::Flow,
            vec![
                exec_in(),
                data_in("count", "Quantas vezes", DataType::Int),
                exec_out("body", "A cada volta"),
                exec_out("done", "Depois de repetir"),
                data_out("index", "Volta", DataType::Int),
            ],
            Vec::new(),
            Lowering::Repeat,
        ),
        fixed(
            "flow.each",
            "Para cada item",
            "Fluxo",
            "Roda o corpo uma vez para cada item de uma lista ou dicionário. Com posição, a primeira variável é o índice (lista) ou a chave (dicionário) e a segunda é o item (lista) ou o valor (dicionário).",
            NodeClass::Flow,
            vec![
                exec_in(),
                data_in("list", "Lista ou dicionário", DataType::Any),
                exec_out("body", "Para cada"),
                exec_out("done", "Depois da lista"),
                data_out("item", "Item ou chave", DataType::Any),
                data_out("index", "Posição ou chave", DataType::Any),
            ],
            vec![choice_setting(
                "with_index",
                "Variáveis",
                &[("true", "Com posição/chave"), ("false", "Só o item/chave")],
            )],
            Lowering::Each,
        ),
        fixed(
            "flow.while",
            "Enquanto",
            "Fluxo",
            "Repete o corpo enquanto a condição for verdadeira.",
            NodeClass::Flow,
            vec![
                exec_in(),
                data_in("condition", "Condição", DataType::Bool),
                exec_out("body", "A cada volta"),
                exec_out("done", "Depois de repetir"),
            ],
            Vec::new(),
            Lowering::While,
        ),
        fixed(
            "flow.loop",
            "Repetir sem parar",
            "Fluxo",
            "Repete o corpo até um nó 'Parar o laço' ser alcançado.",
            NodeClass::Flow,
            vec![
                exec_in(),
                exec_out("body", "A cada volta"),
                exec_out("done", "Depois de parar"),
            ],
            Vec::new(),
            Lowering::Loop,
        ),
        fixed(
            "flow.break",
            "Parar o laço",
            "Fluxo",
            "Sai do laço mais próximo.",
            NodeClass::Flow,
            vec![exec_in()],
            Vec::new(),
            Lowering::Break,
        ),
        fixed(
            "flow.continue",
            "Pular para a próxima volta",
            "Fluxo",
            "Vai direto para a próxima volta do laço mais próximo.",
            NodeClass::Flow,
            vec![exec_in()],
            Vec::new(),
            Lowering::Continue,
        ),
        fixed(
            "flow.function",
            "Função",
            "Funções",
            "Uma função do script, com os parâmetros que ela recebe. O que vem depois roda quando a função é chamada.",
            NodeClass::Event,
            vec![exec_out("then", "Fazer")],
            vec![
                text_setting("name", "Nome"),
                optional_text_setting("params", "Parâmetros (separados por vírgula)"),
            ],
            Lowering::Function,
        ),
        fixed(
            "flow.call",
            "Chamar função",
            "Funções",
            "Chama uma função do script e segue depois que ela termina.",
            NodeClass::Action,
            std::iter::once(exec_in())
                .chain(call_inputs())
                .chain(std::iter::once(exec_out("then", "Depois")))
                .collect(),
            vec![text_setting("name", "Função")],
            Lowering::Call,
        ),
        fixed(
            "flow.call_value",
            "Valor de função",
            "Funções",
            "Chama uma função do script e usa o resultado dela como valor.",
            NodeClass::Value,
            call_inputs()
                .into_iter()
                .chain(std::iter::once(data_out(
                    "result",
                    "Resultado",
                    DataType::Any,
                )))
                .collect(),
            vec![text_setting("name", "Função")],
            Lowering::CallValue,
        ),
        fixed(
            "flow.param",
            "Parâmetro",
            "Funções",
            "O valor de um parâmetro da função atual.",
            NodeClass::Value,
            vec![data_out("value", "Valor", DataType::Any)],
            vec![text_setting("name", "Parâmetro")],
            Lowering::Param,
        ),
        fixed(
            "flow.return",
            "Devolver",
            "Funções",
            "Termina a função, opcionalmente devolvendo um valor.",
            NodeClass::Flow,
            vec![exec_in(), optional_data_in("value", "Valor", DataType::Any)],
            Vec::new(),
            Lowering::Return,
        ),
        fixed(
            "flow.global",
            "Global",
            "Variáveis",
            "Uma variável do módulo, compartilhada por todos os objetos e funções do script.",
            NodeClass::Action,
            vec![data_in("value", "Valor inicial", DataType::Any)],
            vec![
                text_setting("name", "Nome"),
                choice_setting(
                    "mode",
                    "Tipo",
                    &[("let", "Não muda"), ("var", "Pode mudar")],
                ),
            ],
            Lowering::Global,
        ),
        fixed(
            "global.get",
            "Ler global",
            "Variáveis",
            "Lê uma variável do módulo.",
            NodeClass::Value,
            vec![data_out("value", "Valor", DataType::Any)],
            vec![text_setting("name", "Nome")],
            Lowering::GlobalGet,
        ),
        fixed(
            "global.set",
            "Gravar global",
            "Variáveis",
            "Grava uma variável do módulo que foi declarada com 'Pode mudar'.",
            NodeClass::Action,
            vec![
                exec_in(),
                data_in("value", "Valor", DataType::Any),
                exec_out("then", "Depois"),
            ],
            vec![text_setting("name", "Nome")],
            Lowering::GlobalSet,
        ),
        fixed(
            "local.get",
            "Ler variável local",
            "Variáveis",
            "Lê uma variável que existe só durante esta chamada de função.",
            NodeClass::Value,
            vec![data_out("value", "Valor", DataType::Any)],
            vec![text_setting("name", "Nome")],
            Lowering::LocalGet,
        ),
        fixed(
            "local.set",
            "Gravar variável local",
            "Variáveis",
            "Grava uma variável local que já foi declarada nesta função.",
            NodeClass::Action,
            vec![
                exec_in(),
                data_in("value", "Valor", DataType::Any),
                exec_out("then", "Depois"),
            ],
            vec![text_setting("name", "Nome")],
            Lowering::LocalSet,
        ),
        fixed(
            "local.declare",
            "Declarar variável local",
            "Variáveis",
            "Cria uma variável que existe só durante esta chamada de função.",
            NodeClass::Action,
            vec![
                exec_in(),
                data_in("value", "Valor", DataType::Any),
                exec_out("then", "Depois"),
            ],
            vec![
                text_setting("name", "Nome"),
                choice_setting(
                    "mode",
                    "Tipo",
                    &[("let", "Não muda"), ("var", "Pode mudar")],
                ),
            ],
            Lowering::LocalDeclare,
        ),
        fixed(
            "op.field",
            "Campo",
            "Valores",
            "Lê um campo de um objeto, dicionário ou lista.",
            NodeClass::Value,
            vec![
                data_in("target", "Objeto", DataType::Any),
                data_out("result", "Valor", DataType::Any),
            ],
            vec![text_setting("name", "Campo")],
            Lowering::Field,
        ),
        fixed(
            "op.index",
            "Índice",
            "Valores",
            "Lê um item de uma lista ou dicionário pela posição ou chave.",
            NodeClass::Value,
            vec![
                data_in("target", "Lista", DataType::Any),
                data_in("index", "Posição ou chave", DataType::Any),
                data_out("result", "Valor", DataType::Any),
            ],
            Vec::new(),
            Lowering::Index,
        ),
        fixed(
            "op.list",
            "Lista",
            "Valores",
            "Monta uma lista com os itens ligados, na ordem.",
            NodeClass::Value,
            item_inputs(16)
                .into_iter()
                .chain(std::iter::once(data_out("result", "Lista", DataType::Any)))
                .collect(),
            Vec::new(),
            Lowering::List,
        ),
        fixed(
            "op.dict",
            "Dicionário",
            "Valores",
            "Monta um dicionário com os pares chave/valor ligados.",
            NodeClass::Value,
            dict_inputs(8)
                .into_iter()
                .chain(std::iter::once(data_out(
                    "result",
                    "Dicionário",
                    DataType::Any,
                )))
                .collect(),
            Vec::new(),
            Lowering::Dict,
        ),
        fixed(
            "flow.wait",
            "Esperar",
            "Fluxo",
            "Espera alguns segundos de jogo e então continua com o que vem depois. Depois da espera só o próprio objeto continua disponível: guarde o que precisar numa variável antes. Esperar de novo antes do fim reinicia a contagem.",
            NodeClass::Flow,
            vec![
                exec_in(),
                data_in("seconds", "Segundos", DataType::Float),
                exec_out("done", "Depois de esperar"),
            ],
            Vec::new(),
            Lowering::Wait,
        ),
        fixed(
            "op.compare",
            "Comparar",
            "Operadores",
            "Compara dois valores e dá sim ou não.",
            NodeClass::Value,
            vec![
                data_in("a", "A", DataType::Any),
                data_in("b", "B", DataType::Any),
                data_out("result", "Resultado", DataType::Bool),
            ],
            vec![choice_setting("operator", "Comparação", &comparisons)],
            Lowering::Compare,
        ),
        fixed(
            "op.logic",
            "E / Ou",
            "Operadores",
            "Junta duas condições: 'e' pede as duas, 'ou' pede uma delas.",
            NodeClass::Value,
            vec![
                data_in("a", "A", DataType::Bool),
                data_in("b", "B", DataType::Bool),
                data_out("result", "Resultado", DataType::Bool),
            ],
            vec![choice_setting(
                "operator",
                "Operação",
                &[("and", "e"), ("or", "ou")],
            )],
            Lowering::Logic,
        ),
        fixed(
            "op.not",
            "Não",
            "Operadores",
            "Troca sim por não e não por sim.",
            NodeClass::Value,
            vec![
                data_in("a", "Valor", DataType::Bool),
                data_out("result", "Resultado", DataType::Bool),
            ],
            Vec::new(),
            Lowering::Not,
        ),
        fixed(
            "op.math",
            "Conta",
            "Operadores",
            "Soma, subtrai, multiplica, divide ou dá o resto. A divisão sempre dá decimal.",
            NodeClass::Value,
            vec![
                data_in("a", "A", DataType::Float),
                data_in("b", "B", DataType::Float),
                data_out("result", "Resultado", DataType::Float),
            ],
            vec![choice_setting(
                "operator",
                "Conta",
                &[
                    ("+", "somar"),
                    ("-", "subtrair"),
                    ("*", "multiplicar"),
                    ("/", "dividir"),
                    ("%", "resto"),
                ],
            )],
            Lowering::Math,
        ),
        fixed(
            "op.join",
            "Juntar textos",
            "Operadores",
            "Cola dois valores como texto.",
            NodeClass::Value,
            vec![
                data_in("a", "A", DataType::Any),
                data_in("b", "B", DataType::Any),
                data_out("result", "Texto", DataType::Text),
            ],
            Vec::new(),
            Lowering::Join,
        ),
        fixed(
            "op.vector",
            "Montar vetor",
            "Operadores",
            "Junta X, Y e Z num vetor, que é o que posições e direções usam.",
            NodeClass::Value,
            vec![
                data_in("x", "X", DataType::Float),
                data_in("y", "Y", DataType::Float),
                data_in("z", "Z", DataType::Float),
                data_out("result", "Vetor", DataType::Dict),
            ],
            Vec::new(),
            Lowering::Vector,
        ),
        fixed(
            "op.vector_part",
            "Parte do vetor",
            "Operadores",
            "Tira o X, o Y ou o Z de um vetor.",
            NodeClass::Value,
            vec![
                data_in("vector", "Vetor", DataType::Dict),
                data_out("result", "Valor", DataType::Float),
            ],
            vec![choice_setting(
                "axis",
                "Eixo",
                &[("x", "X"), ("y", "Y"), ("z", "Z")],
            )],
            Lowering::VectorPart,
        ),
        fixed(
            "var.get",
            "Ler variável",
            "Variáveis",
            "Lê uma variável deste objeto (vale entre quadros). Sem valor guardado ainda, dá o valor inicial.",
            NodeClass::Value,
            vec![
                data_in("fallback", "Valor inicial", DataType::Any),
                data_out("value", "Valor", DataType::Any),
            ],
            vec![text_setting("name", "Nome da variável")],
            Lowering::VarGet,
        ),
        fixed(
            "var.set",
            "Guardar variável",
            "Variáveis",
            "Guarda um valor numa variável deste objeto; vale entre quadros enquanto o jogo roda.",
            NodeClass::Action,
            vec![
                exec_in(),
                data_in("value", "Valor", DataType::Any),
                exec_out("then", "Depois"),
            ],
            vec![text_setting("name", "Nome da variável")],
            Lowering::VarSet,
        ),
    ]
}
