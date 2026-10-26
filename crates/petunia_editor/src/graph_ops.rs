//! Node graphs in the editor (DR-30): the files `scripts/<name>.graph.json`, opened one at a time.
//!
//! Every edit goes through `petunia_graph::edit`, which checks the catalog and refuses with the
//! reason, then the file is saved at once (atomic), so playing always runs the latest and nothing
//! is lost to a crash. Undo has its own history of snapshots: a graph file is not part of the scene,
//! so the scene's undo does not cover it. The first change of a session also keeps the previous
//! file in `scripts/.backup/`.

use std::path::PathBuf;

use petunia_graph::{
    Catalog, GraphDocument, GraphError, PinRef, Want, add_node, compile, connect, disconnect,
    outline, remove_node, search, set_param, sources, spec_json,
};
use petunia_project::{atomic_write, read_limited, resolve_inside};
use petunia_script::{GRAPH_SUFFIX, SCRIPTS_DIRECTORY, graph_catalog, graph_max_bytes};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

const BACKUP_DIRECTORY: &str = ".backup";
/// Snapshots kept for undo.
const MAX_HISTORY: usize = 100;
/// The most nodes a search returns.
const MAX_FOUND: usize = 60;

/// The graph being edited and its undo history.
pub struct OpenGraph {
    name: String,
    document: GraphDocument,
    past: Vec<GraphDocument>,
    future: Vec<GraphDocument>,
    backed_up: bool,
}

fn graph_failure(error: &GraphError) -> Failure {
    Failure::new(
        "GRAPH_REFUSED",
        &error.0,
        "Nada foi alterado. Leia a mensagem e tente outra ligação.",
    )
}

fn io_failure(action: &str, error: &dyn std::fmt::Display) -> Failure {
    Failure::new(
        "GRAPH_IO",
        &format!("{action}: {error}"),
        "Confira se a pasta do projeto pode ser escrita e se há espaço no disco.",
    )
}

fn no_graph() -> Failure {
    Failure::new(
        "GRAPH_NOT_OPEN",
        "nenhum grafo está aberto",
        "Abra um grafo da lista ou crie um novo.",
    )
}

/// `{ "node": 3, "pin": "then" }` in a request field.
fn pin_of(request: &Json, key: &str) -> Result<PinRef, Failure> {
    let field = request
        .get(key)
        .ok_or_else(|| Failure::bad_request(&format!("falta o campo `{key}`")))?;
    let node = field
        .get("node")
        .and_then(Json::as_u64)
        .and_then(|node| u32::try_from(node).ok())
        .ok_or_else(|| Failure::bad_request(&format!("`{key}.node` precisa ser um número")))?;
    let pin = field
        .get("pin")
        .and_then(Json::as_str)
        .ok_or_else(|| Failure::bad_request(&format!("`{key}.pin` precisa ser um texto")))?;
    Ok(PinRef {
        node,
        pin: pin.to_string(),
    })
}

fn optional_pin(request: &Json, key: &str) -> Result<Option<PinRef>, Failure> {
    match request.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(_) => pin_of(request, key).map(Some),
    }
}

fn node_of(request: &Json, key: &str) -> Result<u32, Failure> {
    request
        .get(key)
        .and_then(Json::as_u64)
        .and_then(|node| u32::try_from(node).ok())
        .ok_or_else(|| Failure::bad_request(&format!("falta o número `{key}`")))
}

impl EditorSession {
    fn graph_path(&self, name: &str) -> Result<PathBuf, Failure> {
        if !crate::scripts::valid_name(name) {
            return Err(Failure::new(
                "SCRIPT_BAD_NAME",
                &format!("`{name}` não serve de nome de grafo"),
                "Use letras, números, `_` e `-`, começando por uma letra (até 40 caracteres).",
            ));
        }
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let directory = resolve_inside(&project.root, SCRIPTS_DIRECTORY)
            .map_err(|error| io_failure("a pasta de scripts", &error))?;
        Ok(directory.join(format!("{name}{GRAPH_SUFFIX}")))
    }

    fn open_graph_mut(&mut self) -> Result<&mut OpenGraph, Failure> {
        self.graph.as_mut().ok_or_else(no_graph)
    }

    /// `graph.list`: the graph files of the project and the one open.
    pub fn graph_list(&self) -> Result<Json, Failure> {
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let directory = project.root.join(SCRIPTS_DIRECTORY);
        Ok(json!({
            "graphs": petunia_script::graph_names(&directory),
            "open": self.graph.as_ref().map(|graph| graph.name.clone()),
        }))
    }

    /// `graph.new`: creates an empty graph file (already saved, so it shows in lists) and opens it.
    pub fn graph_new(&mut self, name: &str) -> Result<Json, Failure> {
        let path = self.graph_path(name)?;
        let aipo = path.with_file_name(format!("{name}.aipo"));
        if path.exists() || aipo.exists() {
            return Err(Failure::new(
                "GRAPH_EXISTS",
                &format!("já existe um script ou grafo chamado `{name}`"),
                "Escolha outro nome.",
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| io_failure("criar a pasta de scripts", &error))?;
        }
        let document = GraphDocument::new();
        atomic_write(&path, document.to_text().as_bytes())
            .map_err(|error| io_failure("gravar o grafo", &error))?;
        self.graph = Some(OpenGraph {
            name: name.to_string(),
            document,
            past: Vec::new(),
            future: Vec::new(),
            backed_up: true,
        });
        self.record("graph", &format!("novo {name}"), true, "");
        self.refresh_script_diagnostics();
        self.graph_view()
    }

    /// `graph.open`: reads a graph file. A file that cannot be read says why and changes nothing.
    pub fn graph_open(&mut self, name: &str) -> Result<Json, Failure> {
        let path = self.graph_path(name)?;
        let bytes = read_limited(&path, graph_max_bytes() as u64 + 1).map_err(|error| {
            Failure::new(
                "GRAPH_NOT_FOUND",
                &format!("não consegui ler o grafo `{name}`: {error}"),
                "Veja a lista de grafos, ou crie um novo.",
            )
        })?;
        let text = String::from_utf8(bytes).map_err(|_| {
            Failure::new(
                "GRAPH_NOT_TEXT",
                &format!("o grafo `{name}` não é texto UTF-8"),
                "Abra o arquivo num editor externo e salve como UTF-8.",
            )
        })?;
        let document = GraphDocument::parse(&text).map_err(|error| {
            Failure::new(
                "GRAPH_INVALID",
                &format!("o grafo `{name}` não pôde ser aberto: {error}"),
                "O arquivo não foi alterado. Corrija-o num editor de texto ou restaure de scripts/.backup/.",
            )
        })?;
        self.graph = Some(OpenGraph {
            name: name.to_string(),
            document,
            past: Vec::new(),
            future: Vec::new(),
            backed_up: false,
        });
        self.record("graph", &format!("abrir {name}"), true, "");
        self.graph_view()
    }

    /// `graph.close`.
    pub fn graph_close(&mut self) -> Result<Json, Failure> {
        self.graph = None;
        Ok(json!({ "open": Json::Null }))
    }

    /// `graph.view`: the outline of the open graph (see `petunia_graph::outline`).
    pub fn graph_view(&self) -> Result<Json, Failure> {
        let graph = self.graph.as_ref().ok_or_else(no_graph)?;
        Ok(json!({
            "name": graph.name,
            "outline": outline(&graph.document, graph_catalog()),
            "can_undo": !graph.past.is_empty(),
            "can_redo": !graph.future.is_empty(),
        }))
    }

    /// Applies an edit to a copy; on success the copy becomes the graph, the old one goes into the
    /// undo history, and the file is saved. On failure nothing changes.
    fn graph_edit<T>(
        &mut self,
        edit: impl FnOnce(&mut GraphDocument, &Catalog) -> Result<T, GraphError>,
    ) -> Result<T, Failure> {
        let path = {
            let name = self.graph.as_ref().ok_or_else(no_graph)?.name.clone();
            self.graph_path(&name)?
        };
        let graph = self.open_graph_mut()?;
        let mut copy = graph.document.clone();
        let outcome = edit(&mut copy, graph_catalog()).map_err(|error| graph_failure(&error))?;
        copy.check_shape().map_err(|error| graph_failure(&error))?;
        if !graph.backed_up {
            if path.exists()
                && let Some(parent) = path.parent()
            {
                let backup_directory = parent.join(BACKUP_DIRECTORY);
                if std::fs::create_dir_all(&backup_directory).is_ok() {
                    // A failed backup must not lose the save: the old text is still on disk.
                    let _ = std::fs::copy(
                        &path,
                        backup_directory.join(format!("{}{GRAPH_SUFFIX}", graph.name)),
                    );
                }
            }
            graph.backed_up = true;
        }
        atomic_write(&path, copy.to_text().as_bytes())
            .map_err(|error| io_failure("gravar o grafo", &error))?;
        let before = std::mem::replace(&mut graph.document, copy);
        graph.past.push(before);
        if graph.past.len() > MAX_HISTORY {
            graph.past.remove(0);
        }
        graph.future.clear();
        self.refresh_script_diagnostics();
        Ok(outcome)
    }

    /// `graph.add`: a node. `after` puts it into a chain; `into` (an input pin) also wires its
    /// result there, for value nodes.
    pub fn graph_add(
        &mut self,
        kind: &str,
        after: Option<PinRef>,
        into: Option<PinRef>,
    ) -> Result<Json, Failure> {
        let id = self.graph_edit(|document, catalog| {
            let id = add_node(document, catalog, kind, after.as_ref())?;
            if let Some(target) = &into {
                let spec = catalog
                    .get(kind)
                    .ok_or_else(|| GraphError(format!("não existe o nó '{kind}'")))?;
                let output = spec
                    .pins
                    .iter()
                    .filter(|pin| pin.direction == petunia_graph::Direction::Out)
                    .find(|pin| matches!(pin.kind, petunia_graph::PinKind::Data(_)))
                    .ok_or_else(|| {
                        GraphError(format!("'{}' não dá nenhum valor para ligar", spec.title))
                    })?;
                connect(
                    document,
                    catalog,
                    &PinRef {
                        node: id,
                        pin: output.name.clone(),
                    },
                    target,
                )?;
            }
            Ok(id)
        })?;
        Ok(json!({ "node": id }))
    }

    pub fn graph_remove(&mut self, node: u32) -> Result<Json, Failure> {
        self.graph_edit(|document, catalog| remove_node(document, catalog, node))?;
        Ok(json!({ "removed": node }))
    }

    pub fn graph_set(
        &mut self,
        node: u32,
        name: &str,
        value: Option<Json>,
    ) -> Result<Json, Failure> {
        self.graph_edit(|document, catalog| set_param(document, catalog, node, name, value))?;
        Ok(json!({ "node": node }))
    }

    pub fn graph_connect(&mut self, from: PinRef, to: PinRef) -> Result<Json, Failure> {
        self.graph_edit(|document, catalog| connect(document, catalog, &from, &to))?;
        Ok(json!({ "connected": true }))
    }

    pub fn graph_disconnect(&mut self, pin: PinRef) -> Result<Json, Failure> {
        let removed = self.graph_edit(|document, _| Ok(disconnect(document, &pin)))?;
        Ok(json!({ "removed": removed }))
    }

    pub fn graph_undo(&mut self) -> Result<Json, Failure> {
        let path = {
            let name = self.graph.as_ref().ok_or_else(no_graph)?.name.clone();
            self.graph_path(&name)?
        };
        let graph = self.open_graph_mut()?;
        let Some(previous) = graph.past.pop() else {
            return Err(Failure::new(
                "GRAPH_NOTHING_TO_UNDO",
                "não há nada para desfazer neste grafo",
                "O histórico vale só enquanto o grafo está aberto.",
            ));
        };
        atomic_write(&path, previous.to_text().as_bytes())
            .map_err(|error| io_failure("gravar o grafo", &error))?;
        let current = std::mem::replace(&mut graph.document, previous);
        graph.future.push(current);
        self.refresh_script_diagnostics();
        Ok(json!({ "undone": true }))
    }

    pub fn graph_redo(&mut self) -> Result<Json, Failure> {
        let path = {
            let name = self.graph.as_ref().ok_or_else(no_graph)?.name.clone();
            self.graph_path(&name)?
        };
        let graph = self.open_graph_mut()?;
        let Some(next) = graph.future.pop() else {
            return Err(Failure::new(
                "GRAPH_NOTHING_TO_REDO",
                "não há nada para refazer neste grafo",
                "Refazer vale logo depois de desfazer.",
            ));
        };
        atomic_write(&path, next.to_text().as_bytes())
            .map_err(|error| io_failure("gravar o grafo", &error))?;
        let current = std::mem::replace(&mut graph.document, next);
        graph.past.push(current);
        self.refresh_script_diagnostics();
        Ok(json!({ "redone": true }))
    }

    /// `graph.search`: nodes for the palette. `want` is `chain`, `event`, `value` or `any`; for
    /// `value`, `for_node` and `for_pin` name the input the value is for.
    pub fn graph_search(
        &self,
        query: &str,
        want: &str,
        target: Option<PinRef>,
    ) -> Result<Json, Failure> {
        let graph = self.graph.as_ref().ok_or_else(no_graph)?;
        let catalog = graph_catalog();
        let want = match want {
            "chain" => Want::Chain,
            "event" => Want::Event,
            "any" | "" => Want::Any,
            "value" => {
                let target = target.ok_or_else(|| {
                    Failure::bad_request("`want: value` precisa de `for` (o pino de entrada)")
                })?;
                let data_type = graph
                    .document
                    .node(target.node)
                    .and_then(|node| catalog.get(&node.kind))
                    .and_then(|spec| spec.pin(&target.pin, petunia_graph::Direction::In))
                    .and_then(|pin| match pin.kind {
                        petunia_graph::PinKind::Data(data_type) => Some(data_type),
                        petunia_graph::PinKind::Exec => None,
                    })
                    .ok_or_else(|| Failure::bad_request("esse pino não recebe valores"))?;
                Want::Value(data_type)
            }
            other => {
                return Err(Failure::bad_request(&format!(
                    "`want` precisa ser chain, event, value ou any (veio `{other}`)"
                )));
            }
        };
        let found: Vec<Json> = search(catalog, query, want)
            .into_iter()
            .take(MAX_FOUND)
            .map(spec_json)
            .collect();
        Ok(json!({ "nodes": found }))
    }

    /// `graph.sources`: what an input could be wired to.
    pub fn graph_sources(&self, target: &PinRef) -> Result<Json, Failure> {
        let graph = self.graph.as_ref().ok_or_else(no_graph)?;
        Ok(
            json!({ "sources": sources(&graph.document, graph_catalog(), target.node, &target.pin) }),
        )
    }

    /// `graph.code`: "see as code": the Aipo text and which node wrote each line, or the problems.
    pub fn graph_code(&self) -> Result<Json, Failure> {
        let graph = self.graph.as_ref().ok_or_else(no_graph)?;
        match compile(&graph.document, graph_catalog()) {
            Ok(compiled) => Ok(json!({
                "ok": true,
                "text": compiled.text,
                "line_nodes": compiled.line_nodes,
            })),
            Err(findings) => Ok(json!({
                "ok": false,
                "problems": findings.iter().map(|finding| json!({
                    "node": finding.node,
                    "pin": finding.pin,
                    "message": finding.message,
                })).collect::<Vec<_>>(),
            })),
        }
    }

    /// Request helpers for the protocol.
    pub(crate) fn graph_request_pin(request: &Json, key: &str) -> Result<PinRef, Failure> {
        pin_of(request, key)
    }

    pub(crate) fn graph_request_optional_pin(
        request: &Json,
        key: &str,
    ) -> Result<Option<PinRef>, Failure> {
        optional_pin(request, key)
    }

    pub(crate) fn graph_request_node(request: &Json, key: &str) -> Result<u32, Failure> {
        node_of(request, key)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value as Json, json};

    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn error_code(session: &mut EditorSession, request: Json) -> String {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], false, "{request} -> {reply}");
        reply["error"]["code"].as_str().unwrap_or("").to_string()
    }

    fn project(name: &str) -> (EditorSession, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("petunia_editor_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": name }),
        );
        (session, root)
    }

    #[test]
    fn a_graph_is_built_edited_undone_saved_and_played_through_operations() {
        let (mut session, root) = project("grafo_ops");
        assert_eq!(
            call(&mut session, json!({ "op": "graph.list" }))["graphs"],
            json!([])
        );
        call(&mut session, json!({ "op": "graph.new", "name": "gira" }));
        let file = root.join("scripts/gira.graph.json");
        assert!(file.is_file(), "a new graph is saved at once");
        assert_eq!(
            error_code(&mut session, json!({ "op": "graph.new", "name": "gira" })),
            "GRAPH_EXISTS"
        );
        assert_eq!(
            error_code(
                &mut session,
                json!({ "op": "graph.new", "name": "../fora" })
            ),
            "SCRIPT_BAD_NAME"
        );

        // The palette: events first, then what fits a chain.
        let events = call(
            &mut session,
            json!({ "op": "graph.search", "query": "quadro", "want": "event" }),
        );
        assert_eq!(events["nodes"][0]["kind"], "event.on_update");
        let event = call(
            &mut session,
            json!({ "op": "graph.add", "kind": "event.on_update" }),
        )["node"]
            .as_u64()
            .expect("id");
        let found = call(
            &mut session,
            json!({ "op": "graph.search", "query": "girar y", "want": "chain" }),
        );
        assert_eq!(found["nodes"][0]["kind"], "api.transform.rotate_y");
        let turn = call(
            &mut session,
            json!({ "op": "graph.add", "kind": "api.transform.rotate_y", "after": { "node": event, "pin": "then" } }),
        )["node"]
            .as_u64()
            .expect("id");

        // `degrees` is empty: the outline says so, and the code view refuses.
        let view = call(&mut session, json!({ "op": "graph.view" }));
        let step = &view["outline"]["events"][0]["steps"][0];
        assert_eq!(step["inputs"][1]["source"], "missing");
        assert_eq!(
            call(&mut session, json!({ "op": "graph.code" }))["ok"],
            false
        );

        // What can feed `degrees`: the event's dt. Then a value node built in place: dt * 90.
        let sources = call(
            &mut session,
            json!({ "op": "graph.sources", "for": { "node": turn, "pin": "degrees" } }),
        );
        assert_eq!(
            sources["sources"][0]["from"],
            json!({ "node": event, "pin": "dt" })
        );
        let value_search = call(
            &mut session,
            json!({ "op": "graph.search", "query": "conta", "want": "value", "for": { "node": turn, "pin": "degrees" } }),
        );
        assert_eq!(value_search["nodes"][0]["kind"], "op.math");
        let math = call(
            &mut session,
            json!({ "op": "graph.add", "kind": "op.math", "into": { "node": turn, "pin": "degrees" } }),
        )["node"]
            .as_u64()
            .expect("id");
        call(
            &mut session,
            json!({ "op": "graph.set", "node": math, "name": "operator", "value": "*" }),
        );
        call(
            &mut session,
            json!({ "op": "graph.set", "node": math, "name": "b", "value": 90.0 }),
        );
        call(
            &mut session,
            json!({ "op": "graph.connect", "from": { "node": event, "pin": "dt" }, "to": { "node": math, "pin": "a" } }),
        );
        let code = call(&mut session, json!({ "op": "graph.code" }));
        assert_eq!(code["ok"], true, "{code}");
        assert!(
            code["text"]
                .as_str()
                .expect("text")
                .contains("transform.rotate_y(entity, (dt * 90.0))"),
            "{code}"
        );

        // A refused edit changes nothing, and is not a step in the history.
        let before = std::fs::read_to_string(&file).expect("file");
        assert_eq!(
            error_code(
                &mut session,
                json!({ "op": "graph.connect", "from": { "node": event, "pin": "dt" }, "to": { "node": turn, "pin": "entity" } })
            ),
            "GRAPH_REFUSED"
        );
        assert_eq!(std::fs::read_to_string(&file).expect("file"), before);

        // Undo walks back one edit at a time and the file follows; redo returns.

        call(&mut session, json!({ "op": "graph.undo" }));
        assert_ne!(std::fs::read_to_string(&file).expect("file"), before);
        call(&mut session, json!({ "op": "graph.redo" }));
        assert_eq!(std::fs::read_to_string(&file).expect("file"), before);
        assert_eq!(
            error_code(&mut session, json!({ "op": "graph.redo" })),
            "GRAPH_NOTHING_TO_REDO"
        );

        // Reopening an existing graph: its first change keeps the file as it was.
        call(&mut session, json!({ "op": "graph.open", "name": "gira" }));
        assert!(!root.join("scripts/.backup/gira.graph.json").exists());
        call(
            &mut session,
            json!({ "op": "graph.set", "node": math, "name": "b", "value": 90.0 }),
        );
        assert!(
            root.join("scripts/.backup/gira.graph.json").is_file(),
            "the first change of a session kept a backup"
        );

        // The graph plays: attach it to a drawn object and turn it for a second.
        let spinner = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Name": { "text": "Giratório" },
                "Transform": { "position": { "x": 0.0, "y": 3.0, "z": 0.0 } },
                "Brush": { "shape": "box", "size": { "x": 1.0, "y": 1.0, "z": 1.0 } }
            } } }),
        )["entity"]
            .as_u64()
            .expect("id");
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": spinner }),
        );
        call(
            &mut session,
            json!({ "op": "script.attach", "name": "gira" }),
        );
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert!(
            started["problems"].as_array().expect("list").is_empty(),
            "{started}"
        );
        // Graph edits are allowed while playing (they apply next time); scene edits are not.
        call(&mut session, json!({ "op": "graph.view" }));
        for _ in 0..60 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
            );
        }
        let frame = session.frame().expect("frame");
        let turned = frame
            .items
            .iter()
            .find(|item| item.entity == spinner)
            .map(|item| item.transform.rotation_degrees.y)
            .expect("drawn");
        assert!((turned.abs() - 90.0).abs() < 1.0, "turned {turned}");
        call(&mut session, json!({ "op": "play.stop" }));

        // Removing the multiplication cuts it out and leaves `degrees` empty again.
        call(&mut session, json!({ "op": "graph.remove", "node": math }));
        let view = call(&mut session, json!({ "op": "graph.view" }));
        assert_eq!(
            view["outline"]["events"][0]["steps"][0]["inputs"][1]["source"],
            "missing"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_hostile_graph_file_is_refused_and_the_open_graph_stays() {
        let (mut session, root) = project("grafo_hostil");
        call(&mut session, json!({ "op": "graph.new", "name": "bom" }));
        std::fs::write(root.join("scripts/ruim.graph.json"), "{ nao e json").expect("write");
        assert_eq!(
            error_code(&mut session, json!({ "op": "graph.open", "name": "ruim" })),
            "GRAPH_INVALID"
        );
        assert_eq!(
            call(&mut session, json!({ "op": "graph.view" }))["name"],
            "bom"
        );
        std::fs::write(
            root.join("scripts/futuro.graph.json"),
            r#"{"schema_version":99,"nodes":[],"edges":[]}"#,
        )
        .expect("write");
        assert_eq!(
            error_code(
                &mut session,
                json!({ "op": "graph.open", "name": "futuro" })
            ),
            "GRAPH_INVALID"
        );
        assert_eq!(
            error_code(
                &mut session,
                json!({ "op": "graph.open", "name": "nao_existe" })
            ),
            "GRAPH_NOT_FOUND"
        );
        // Not open: edits say so.
        call(&mut session, json!({ "op": "graph.close" }));
        assert_eq!(
            error_code(
                &mut session,
                json!({ "op": "graph.add", "kind": "flow.if" })
            ),
            "GRAPH_NOT_OPEN"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
