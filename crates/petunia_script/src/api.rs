//! The API scripts see, described once as data. The same table builds the compile-time surface
//! (so a typo in `world.fnd` is an error in the editor, not a crash in the game), registers the
//! native functions, and feeds the editor's autocomplete and help. A test checks that the table and
//! the natives agree.

use std::collections::HashMap;

use aipo_runtime::NativeRegistry;
use aipo_sema::{HostFunction, HostParameter, PreludeSurface};
use aipo_vm::Vm;
use serde_json::{Value as Json, json};

/// Hidden global that carries the game state into native functions (a generational handle, so a
/// script cannot forge or keep it past the host's control).
pub(crate) const CONTEXT_GLOBAL: &str = "__petunia";

/// Modules of the standard library a script must **not** see. `sh` runs system programs with no
/// capability check; `fs` and `env` read the machine; `io` writes to the process output. A script
/// is game logic: it gets the world, math, text, collections and a seeded random, nothing else.
pub(crate) const REMOVED_MODULES: &[&str] = &["sh", "fs", "env", "io"];

/// One function of the script API.
pub struct ApiFunction {
    pub module: &'static str,
    pub name: &'static str,
    pub params: &'static [(&'static str, &'static str)],
    pub returns: &'static str,
    pub summary: &'static str,
    /// Capability the call needs (`""` for none).
    pub permission: &'static str,
    pub example: &'static str,
    /// Accepts more arguments than `params` (like `log.info`).
    pub variadic: bool,
}

/// Capabilities a game script is granted.
pub const GAME_CAPABILITIES: &[&str] = &["world.read", "world.write", "game"];

/// Names a script file may define; the host calls them when the thing happens.
pub const HANDLERS: &[(&str, &str)] = &[
    (
        "on_start",
        "on_start(entity): uma vez, antes da primeira atualização do objeto",
    ),
    (
        "on_update",
        "on_update(entity, dt): todo quadro; `dt` são os segundos desde o quadro anterior",
    ),
    (
        "on_event",
        "on_event(entity, nome): quando algo chama `game.emit(nome, entity)`",
    ),
    (
        "on_collision",
        "on_collision(entity, outro, minha, dele): uma colisão deste objeto começou a tocar outra; `minha` e `dele` são os nomes das colisões",
    ),
    (
        "on_collision_end",
        "on_collision_end(entity, outro, minha, dele): as duas colisões deixaram de se tocar",
    ),
];

pub const API: &[ApiFunction] = &[
    ApiFunction {
        module: "collision",
        name: "list",
        params: &[("entity", "Int")],
        returns: "List",
        summary: "Os nomes das colisões de um objeto.",
        permission: "world.read",
        example: "for nome in collision.list(entity) { log.info(nome) }",
        variadic: false,
    },
    ApiFunction {
        module: "collision",
        name: "get",
        params: &[("entity", "Int"), ("name", "String")],
        returns: "Dict",
        summary: "Uma colisão pelo nome: forma, centro, tamanho, raio, altura, gatilho, atrito, quique.",
        permission: "world.read",
        example: "let pes = collision.get(entity, \"pés\")",
        variadic: false,
    },
    ApiFunction {
        module: "collision",
        name: "set",
        params: &[("entity", "Int"), ("name", "String"), ("column", "String"), ("value", "Value")],
        returns: "None",
        summary: "Muda uma coluna de uma colisão (validado como no inspetor; a física se refaz no próximo quadro).",
        permission: "world.write",
        example: "collision.set(entity, \"pés\", \"radius\", 0.3)",
        variadic: false,
    },
    ApiFunction {
        module: "collision",
        name: "overlaps",
        params: &[("entity", "Int"), ("name", "String")],
        returns: "List",
        summary: "O que toca esta colisão agora: lista de {entity, collider}.",
        permission: "world.read",
        example: "if collision.overlaps(entity, \"pés\").len() > 0 { log.info(\"no chão\") }",
        variadic: false,
    },
    ApiFunction {
        module: "physics",
        name: "raycast",
        params: &[("origin", "Dict"), ("direction", "Dict"), ("distance", "Float")],
        returns: "Value",
        summary: "O primeiro objeto sólido na linha: {entity, collider, point, normal, distance}, ou nada.",
        permission: "world.read",
        example: "let alvo = physics.raycast(transform.get_position(entity), transform.forward(entity), 20.0)",
        variadic: false,
    },
    ApiFunction {
        module: "physics",
        name: "velocity",
        params: &[("entity", "Int")],
        returns: "Dict",
        summary: "A velocidade do corpo, em metros por segundo ({x, y, z}).",
        permission: "world.read",
        example: "let v = physics.velocity(entity)",
        variadic: false,
    },
    ApiFunction {
        module: "physics",
        name: "set_velocity",
        params: &[("entity", "Int"), ("velocity", "Dict")],
        returns: "None",
        summary: "Troca a velocidade do corpo.",
        permission: "world.write",
        example: "physics.set_velocity(entity, {\"x\": 0.0, \"y\": 5.0, \"z\": 0.0})",
        variadic: false,
    },
    ApiFunction {
        module: "physics",
        name: "impulse",
        params: &[("entity", "Int"), ("impulse", "Dict")],
        returns: "None",
        summary: "Um empurrão: muda a velocidade em impulso ÷ massa.",
        permission: "world.write",
        example: "physics.impulse(entity, {\"x\": 0.0, \"y\": 4.0, \"z\": 0.0})",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "get",
        params: &[("entity", "Int"), ("table", "String"), ("column", "String")],
        returns: "Value",
        summary: "Lê o valor de uma coluna de um objeto.",
        permission: "world.read",
        example: "let life = world.get(entity, \"Health\", \"points\")",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "row",
        params: &[("entity", "Int"), ("table", "String")],
        returns: "Dict",
        summary: "Lê todas as colunas de uma tabela de um objeto, como dicionário.",
        permission: "world.read",
        example: "let transform = world.row(entity, \"Transform\")",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "has",
        params: &[("entity", "Int"), ("table", "String")],
        returns: "Bool",
        summary: "Diz se o objeto tem a tabela.",
        permission: "world.read",
        example: "if world.has(entity, \"Brush\") { log.info(\"é um brush\") }",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "exists",
        params: &[("entity", "Int")],
        returns: "Bool",
        summary: "Diz se o objeto existe.",
        permission: "world.read",
        example: "if world.exists(target) { log.info(\"existe\") }",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "find",
        params: &[("table", "String")],
        returns: "List",
        summary: "Lista os objetos que têm a tabela, em ordem de id.",
        permission: "world.read",
        example: "each enemy in world.find(\"Enemy\") { log.info(enemy) }",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "set",
        params: &[
            ("entity", "Int"),
            ("table", "String"),
            ("column", "String"),
            ("value", "Value"),
        ],
        returns: "None",
        summary: "Muda uma coluna. Falha (recuperável) se o valor não serve para a coluna.",
        permission: "world.write",
        example: "world.set(entity, \"Transform\", \"position\", { \"x\": 1.0, \"y\": 0.0, \"z\": 2.0 })",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "create",
        params: &[("rows", "Dict")],
        returns: "Int",
        summary: "Cria um objeto com as tabelas dadas e devolve o id.",
        permission: "world.write",
        example: "let id = world.create({ \"Name\": { \"text\": \"Moeda\" } })",
        variadic: false,
    },
    ApiFunction {
        module: "world",
        name: "remove",
        params: &[("entity", "Int")],
        returns: "None",
        summary: "Remove um objeto.",
        permission: "world.write",
        example: "world.remove(coin)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "get_position",
        params: &[("entity", "Int")],
        returns: "Dict",
        summary: "Lê a posição (x, y, z) de um objeto.",
        permission: "world.read",
        example: "let pos = transform.get_position(entity)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "set_position",
        params: &[
            ("entity", "Int"),
            ("x", "Float"),
            ("y", "Float"),
            ("z", "Float"),
        ],
        returns: "None",
        summary: "Define a posição absoluta (x, y, z) de um objeto.",
        permission: "world.write",
        example: "transform.set_position(entity, 0.0, 1.0, 0.0)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "move",
        params: &[
            ("entity", "Int"),
            ("x", "Float"),
            ("y", "Float"),
            ("z", "Float"),
        ],
        returns: "None",
        summary: "Move um objeto em relação à sua posição atual.",
        permission: "world.write",
        example: "transform.move(entity, 0.0, 0.0, 5.0 * dt)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "move_x",
        params: &[("entity", "Int"), ("delta", "Float")],
        returns: "None",
        summary: "Move um objeto ao longo do eixo X.",
        permission: "world.write",
        example: "transform.move_x(entity, 5.0 * dt)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "move_y",
        params: &[("entity", "Int"), ("delta", "Float")],
        returns: "None",
        summary: "Move um objeto ao longo do eixo Y.",
        permission: "world.write",
        example: "transform.move_y(entity, 5.0 * dt)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "move_z",
        params: &[("entity", "Int"), ("delta", "Float")],
        returns: "None",
        summary: "Move um objeto ao longo do eixo Z.",
        permission: "world.write",
        example: "transform.move_z(entity, 5.0 * dt)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "get_rotation",
        params: &[("entity", "Int")],
        returns: "Dict",
        summary: "Lê a rotação em graus Euler (x, y, z) de um objeto.",
        permission: "world.read",
        example: "let rot = transform.get_rotation(entity)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "set_rotation",
        params: &[
            ("entity", "Int"),
            ("x", "Float"),
            ("y", "Float"),
            ("z", "Float"),
        ],
        returns: "None",
        summary: "Define a rotação absoluta em graus Euler (x, y, z) de um objeto.",
        permission: "world.write",
        example: "transform.set_rotation(entity, 0.0, 180.0, 0.0)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "rotate",
        params: &[
            ("entity", "Int"),
            ("x", "Float"),
            ("y", "Float"),
            ("z", "Float"),
        ],
        returns: "None",
        summary: "Rotaciona um objeto em graus Euler (x, y, z).",
        permission: "world.write",
        example: "transform.rotate(entity, 0.0, 90.0 * dt, 0.0)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "rotate_x",
        params: &[("entity", "Int"), ("degrees", "Float")],
        returns: "None",
        summary: "Rotaciona um objeto ao redor do eixo X (pitch).",
        permission: "world.write",
        example: "transform.rotate_x(entity, 45.0 * dt)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "rotate_y",
        params: &[("entity", "Int"), ("degrees", "Float")],
        returns: "None",
        summary: "Rotaciona um objeto ao redor do eixo Y (yaw).",
        permission: "world.write",
        example: "transform.rotate_y(entity, 90.0 * dt)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "rotate_z",
        params: &[("entity", "Int"), ("degrees", "Float")],
        returns: "None",
        summary: "Rotaciona um objeto ao redor do eixo Z (roll).",
        permission: "world.write",
        example: "transform.rotate_z(entity, 30.0 * dt)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "get_scale",
        params: &[("entity", "Int")],
        returns: "Dict",
        summary: "Lê a escala (x, y, z) de um objeto.",
        permission: "world.read",
        example: "let scl = transform.get_scale(entity)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "set_scale",
        params: &[
            ("entity", "Int"),
            ("x", "Float"),
            ("y", "Float"),
            ("z", "Float"),
        ],
        returns: "None",
        summary: "Define a escala (x, y, z) de um objeto.",
        permission: "world.write",
        example: "transform.set_scale(entity, 2.0, 2.0, 2.0)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "forward",
        params: &[("entity", "Int")],
        returns: "Dict",
        summary: "Devolve o vetor direcional frente (-Z local rotacionado) do objeto.",
        permission: "world.read",
        example: "let fwd = transform.forward(entity)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "right",
        params: &[("entity", "Int")],
        returns: "Dict",
        summary: "Devolve o vetor direcional direita (+X local rotacionado) do objeto.",
        permission: "world.read",
        example: "let rgt = transform.right(entity)",
        variadic: false,
    },
    ApiFunction {
        module: "transform",
        name: "up",
        params: &[("entity", "Int")],
        returns: "Dict",
        summary: "Devolve o vetor direcional cima (+Y local rotacionado) do objeto.",
        permission: "world.read",
        example: "let up = transform.up(entity)",
        variadic: false,
    },
    ApiFunction {
        module: "game",
        name: "tick",
        params: &[],
        returns: "Int",
        summary: "Quantos quadros de jogo já passaram.",
        permission: "game",
        example: "let frame = game.tick()",
        variadic: false,
    },
    ApiFunction {
        module: "game",
        name: "seconds",
        params: &[],
        returns: "Float",
        summary: "Segundos de jogo desde o começo.",
        permission: "game",
        example: "let elapsed = game.seconds()",
        variadic: false,
    },
    ApiFunction {
        module: "game",
        name: "emit",
        params: &[("name", "String"), ("entity", "Int")],
        returns: "None",
        summary: "Avisa um objeto: o `on_event(entity, nome)` dele roda depois deste quadro.",
        permission: "game",
        example: "game.emit(\"open\", door)",
        variadic: false,
    },
    ApiFunction {
        module: "log",
        name: "debug",
        params: &[("message", "Value")],
        returns: "None",
        summary: "Escreve uma linha de depuração no console do editor.",
        permission: "",
        example: "log.debug(\"passo\", game.tick())",
        variadic: true,
    },
    ApiFunction {
        module: "log",
        name: "info",
        params: &[("message", "Value")],
        returns: "None",
        summary: "Escreve uma linha no console do editor.",
        permission: "",
        example: "log.info(\"vida:\", life)",
        variadic: true,
    },
    ApiFunction {
        module: "log",
        name: "warning",
        params: &[("message", "Value")],
        returns: "None",
        summary: "Escreve um aviso no console do editor.",
        permission: "",
        example: "log.warning(\"sem alvo\")",
        variadic: true,
    },
    ApiFunction {
        module: "log",
        name: "error",
        params: &[("message", "Value")],
        returns: "None",
        summary: "Escreve um erro no console do editor.",
        permission: "",
        example: "log.error(\"valor inválido\")",
        variadic: true,
    },
];

/// Installs Aipo's standard library into `vm` and takes away what a game must not touch.
pub(crate) fn install_standard(vm: &mut Vm, registry: &mut NativeRegistry) {
    aipo_stdlib::register_stdlib(vm, registry);
    for module in REMOVED_MODULES {
        vm.globals.remove(*module);
    }
}

/// The standard environment a script gets: Aipo's standard library minus the modules a game
/// must not touch.
pub(crate) fn standard_environment() -> (Vm, NativeRegistry) {
    let mut vm = Vm::new();
    let mut registry = NativeRegistry::new();
    install_standard(&mut vm, &mut registry);
    (vm, registry)
}

/// What the compiler may accept: the standard surface (as the Aipo CLI derives it) plus the
/// engine modules of [`API`]. Removed modules are not in it, so `sh.run(...)` does not compile.
pub fn script_surface() -> PreludeSurface {
    let (vm, registry) = standard_environment();
    let mut surface = PreludeSurface::fundamental();
    for name in vm.globals.keys() {
        match registry.get(None, name) {
            Some(meta) => surface.add_function(name, meta.arity, meta.arity),
            None => surface.add_variable(name),
        }
    }
    for module in registry.list_modules() {
        if REMOVED_MODULES.contains(&module.as_str()) || module == "log" {
            continue;
        }
        let functions: HashMap<String, HostFunction> = registry
            .list_module(&module)
            .into_iter()
            .map(|meta| {
                let params = (0..meta.arity)
                    .map(|index| HostParameter {
                        name: format!("arg{index}"),
                        ty: "Value".to_string(),
                        nullable: true,
                        optional: false,
                    })
                    .collect();
                (
                    meta.name.clone(),
                    HostFunction {
                        params,
                        variadic: meta.is_variadic,
                    },
                )
            })
            .collect();
        if !functions.is_empty() {
            surface.add_host_module(&module, functions);
        }
    }
    let mut modules: HashMap<&str, HashMap<String, HostFunction>> = HashMap::new();
    for function in API {
        let params = function
            .params
            .iter()
            .map(|(name, ty)| HostParameter {
                name: (*name).to_string(),
                ty: (*ty).to_string(),
                nullable: true,
                optional: false,
            })
            .collect();
        modules.entry(function.module).or_default().insert(
            function.name.to_string(),
            HostFunction {
                params,
                variadic: function.variadic,
            },
        );
    }
    for (module, functions) in modules {
        surface.add_host_module(module, functions);
    }
    surface
}

/// Keywords of the language (the table of `aipo-lexer`), for highlighting.
pub const KEYWORDS: &[&str] = &[
    "let",
    "var",
    "fixed",
    "fn",
    "async",
    "await",
    "with",
    "struct",
    "enum",
    "interface",
    "init",
    "invariant",
    "if",
    "elif",
    "else",
    "then",
    "match",
    "when",
    "loop",
    "while",
    "repeat",
    "as",
    "each",
    "in",
    "break",
    "continue",
    "return",
    "fail",
    "or_else",
    "attempt",
    "failed",
    "import",
    "export",
    "is",
    "not",
    "and",
    "or",
    "true",
    "false",
    "none",
    "self",
    "do",
];

/// The API as JSON for the editor: functions with signatures and examples, handlers, keywords and
/// the names a script can use without a module.
pub fn api_json() -> Json {
    let functions: Vec<Json> = API
        .iter()
        .map(|function| {
            let params: Vec<String> = function
                .params
                .iter()
                .map(|(name, ty)| format!("{name}: {ty}"))
                .collect();
            let tail = if function.variadic { ", ..." } else { "" };
            json!({
                "module": function.module,
                "name": function.name,
                "signature": format!(
                    "{}.{}({}{}) -> {}",
                    function.module,
                    function.name,
                    params.join(", "),
                    tail,
                    function.returns
                ),
                "summary": function.summary,
                "permission": function.permission,
                "example": function.example,
            })
        })
        .collect();
    let (vm, registry) = standard_environment();
    let mut globals: Vec<String> = vm.globals.keys().cloned().collect();
    globals.sort();
    let mut modules: Vec<Json> = Vec::new();
    for module in registry.list_modules() {
        if REMOVED_MODULES.contains(&module.as_str()) || module == "log" {
            continue;
        }
        let mut members: Vec<String> = registry
            .list_module(&module)
            .into_iter()
            .map(|meta| meta.name.clone())
            .collect();
        members.sort();
        modules.push(json!({ "module": module, "members": members }));
    }
    json!({
        "functions": functions,
        "handlers": HANDLERS.iter().map(|(name, help)| json!({ "name": name, "help": help })).collect::<Vec<_>>(),
        "keywords": KEYWORDS,
        "globals": globals,
        "modules": modules,
    })
}
