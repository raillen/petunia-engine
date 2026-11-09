//! Prefabs (DR-31): ready-made objects with their behaviour, placed in front of the camera.
//!
//! A prefab here is a **mold of rows** (the entities to create, with their tables) plus, when it
//! has behaviour, the **node graphs** it needs. Spawning creates the rows in one undo step and
//! puts each graph in `scripts/` when it is not there yet (a graph the person already has, and
//! maybe edited, is never overwritten). The behaviour is then ordinary graphs: open one in the
//! Logic workspace and change it.
//!
//! The molds are built in (Porta, Item, Ponto de nascimento). Prefabs of the project itself, with
//! revision and instances that follow the mold, come later.

use petunia_commands::{BATCH_COMMAND, CommandRecord};
use petunia_graph::{GraphDocument, GraphError, PinRef, add_node, connect, set_param};
use petunia_math::Vec3;
use petunia_project::{atomic_write, resolve_inside};
use petunia_script::{GRAPH_SUFFIX, SCRIPTS_DIRECTORY, graph_catalog};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::{EditorSession, vector_json};

/// A new prefab goes this far in front of the camera.
const PLACE_DISTANCE: f32 = 5.0;
/// How high the door lifts, in meters (a bit more than its height).
const DOOR_LIFT: f64 = 3.2;
/// Seconds the door stays open.
const DOOR_OPEN_SECONDS: f64 = 3.0;

/// A prefab of the built-in set.
pub struct PrefabInfo {
    pub id: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
}

pub const PREFABS: &[PrefabInfo] = &[
    PrefabInfo {
        id: "door",
        title: "Porta",
        summary: "Uma porta que sobe quando um personagem chega perto, espera alguns segundos e desce. Comportamento no grafo `porta`.",
    },
    PrefabInfo {
        id: "pickup",
        title: "Item coletável",
        summary: "Uma esfera que some quando um personagem a toca, e avisa no console. Comportamento no grafo `item`.",
    },
    PrefabInfo {
        id: "spawn",
        title: "Ponto de nascimento",
        summary: "Onde o jogador nasce ao jogar (tabela PlayerStart), com uma marca que não bloqueia ninguém. Não é o personagem; use Personagem jogável para isso.",
    },
    PrefabInfo {
        id: "player",
        title: "Personagem jogável",
        summary: "Um personagem controlado pelo teclado (W A S D, Espaço), com uma cápsula de colisão sólida.",
    },
];

fn refuse(error: &GraphError) -> Failure {
    Failure::new(
        "PREFAB_GRAPH",
        &format!("o grafo do prefab não pôde ser montado: {}", error.0),
        "Isto é um defeito do Petunia, não do projeto. Avise quem mantém o editor.",
    )
}

fn pin(node: u32, name: &str) -> PinRef {
    PinRef {
        node,
        pin: name.to_string(),
    }
}

/// The graph of the item: touched by a character, say so and disappear.
fn pickup_graph() -> Result<GraphDocument, GraphError> {
    let catalog = graph_catalog();
    let mut graph = GraphDocument::new();
    let event = add_node(&mut graph, catalog, "event.on_collision", None)?;
    let branch = add_node(&mut graph, catalog, "flow.if", Some(&pin(event, "then")))?;
    let has = add_node(&mut graph, catalog, "api.world.has", None)?;
    connect(
        &mut graph,
        catalog,
        &pin(event, "other"),
        &pin(has, "entity"),
    )?;
    set_param(&mut graph, catalog, has, "table", Some(json!("Character")))?;
    connect(
        &mut graph,
        catalog,
        &pin(has, "result"),
        &pin(branch, "condition"),
    )?;
    let say = add_node(
        &mut graph,
        catalog,
        "api.log.info",
        Some(&pin(branch, "then")),
    )?;
    set_param(
        &mut graph,
        catalog,
        say,
        "message",
        Some(json!("Item coletado")),
    )?;
    add_node(
        &mut graph,
        catalog,
        "api.world.remove",
        Some(&pin(say, "then")),
    )?;
    Ok(graph)
}

/// The graph of the door: a character reaches the sensor and the door is not open yet, so it
/// lifts, waits, and comes back down. The variable `aberta` keeps it from opening twice.
fn door_graph() -> Result<GraphDocument, GraphError> {
    let catalog = graph_catalog();
    let mut graph = GraphDocument::new();
    let event = add_node(&mut graph, catalog, "event.on_collision", None)?;
    let branch = add_node(&mut graph, catalog, "flow.if", Some(&pin(event, "then")))?;

    let from_sensor = add_node(&mut graph, catalog, "op.compare", None)?;
    set_param(
        &mut graph,
        catalog,
        from_sensor,
        "operator",
        Some(json!("==")),
    )?;
    connect(
        &mut graph,
        catalog,
        &pin(event, "mine"),
        &pin(from_sensor, "a"),
    )?;
    set_param(&mut graph, catalog, from_sensor, "b", Some(json!("sensor")))?;

    let is_character = add_node(&mut graph, catalog, "api.world.has", None)?;
    connect(
        &mut graph,
        catalog,
        &pin(event, "other"),
        &pin(is_character, "entity"),
    )?;
    set_param(
        &mut graph,
        catalog,
        is_character,
        "table",
        Some(json!("Character")),
    )?;

    let both = add_node(&mut graph, catalog, "op.logic", None)?;
    set_param(&mut graph, catalog, both, "operator", Some(json!("and")))?;
    connect(
        &mut graph,
        catalog,
        &pin(from_sensor, "result"),
        &pin(both, "a"),
    )?;
    connect(
        &mut graph,
        catalog,
        &pin(is_character, "result"),
        &pin(both, "b"),
    )?;

    let open = add_node(&mut graph, catalog, "var.get", None)?;
    set_param(&mut graph, catalog, open, "name", Some(json!("aberta")))?;
    set_param(&mut graph, catalog, open, "fallback", Some(json!(false)))?;
    let closed = add_node(&mut graph, catalog, "op.not", None)?;
    connect(&mut graph, catalog, &pin(open, "value"), &pin(closed, "a"))?;

    let allowed = add_node(&mut graph, catalog, "op.logic", None)?;
    set_param(&mut graph, catalog, allowed, "operator", Some(json!("and")))?;
    connect(
        &mut graph,
        catalog,
        &pin(both, "result"),
        &pin(allowed, "a"),
    )?;
    connect(
        &mut graph,
        catalog,
        &pin(closed, "result"),
        &pin(allowed, "b"),
    )?;
    connect(
        &mut graph,
        catalog,
        &pin(allowed, "result"),
        &pin(branch, "condition"),
    )?;

    let mark_open = add_node(&mut graph, catalog, "var.set", Some(&pin(branch, "then")))?;
    set_param(
        &mut graph,
        catalog,
        mark_open,
        "name",
        Some(json!("aberta")),
    )?;
    set_param(&mut graph, catalog, mark_open, "value", Some(json!(true)))?;
    let lift = add_node(
        &mut graph,
        catalog,
        "api.transform.move_y",
        Some(&pin(mark_open, "then")),
    )?;
    set_param(&mut graph, catalog, lift, "delta", Some(json!(DOOR_LIFT)))?;
    let say = add_node(
        &mut graph,
        catalog,
        "api.log.info",
        Some(&pin(lift, "then")),
    )?;
    set_param(
        &mut graph,
        catalog,
        say,
        "message",
        Some(json!("A porta abriu")),
    )?;
    let wait = add_node(&mut graph, catalog, "flow.wait", Some(&pin(say, "then")))?;
    set_param(
        &mut graph,
        catalog,
        wait,
        "seconds",
        Some(json!(DOOR_OPEN_SECONDS)),
    )?;
    let lower = add_node(
        &mut graph,
        catalog,
        "api.transform.move_y",
        Some(&pin(wait, "done")),
    )?;
    set_param(&mut graph, catalog, lower, "delta", Some(json!(-DOOR_LIFT)))?;
    let mark_closed = add_node(&mut graph, catalog, "var.set", Some(&pin(lower, "then")))?;
    set_param(
        &mut graph,
        catalog,
        mark_closed,
        "name",
        Some(json!("aberta")),
    )?;
    set_param(
        &mut graph,
        catalog,
        mark_closed,
        "value",
        Some(json!(false)),
    )?;
    Ok(graph)
}

/// What a prefab needs: the rows to create, and the graphs as `(name, graph)`.
pub type Built = (Vec<Json>, Vec<(String, GraphDocument)>);

/// The rows to create for a prefab placed with its feet at `at`, and the graphs it needs as
/// `(name, graph)`. The first row set is the prefab's main object; the others are its colliders,
/// which point at it.
pub fn build(id: &str, at: Vec3) -> Result<Built, Failure> {
    match id {
        "door" => {
            let rows = vec![
                json!({
                    "Name": { "text": "Porta" },
                    "Transform": { "position": vector_json(Vec3::new(at.x, at.y + 1.5, at.z)) },
                    "Brush": { "shape": "box", "size": { "x": 2.0, "y": 3.0, "z": 0.3 },
                               "color": { "r": 0.55, "g": 0.38, "b": 0.22, "a": 1.0 } },
                    // Kinematic: the script moves it and the physics follows (a static body would not).
                    "RigidBody": { "kind": "kinematic" },
                    "Script": { "file": "porta", "enabled": true }
                }),
                json!({ "Collider": { "owner": { "$entity_of": 0 }, "name": "corpo", "shape": "box",
                                      "size": { "x": 2.0, "y": 3.0, "z": 0.3 } } }),
                json!({ "Collider": { "owner": { "$entity_of": 0 }, "name": "sensor", "shape": "box",
                                      "size": { "x": 4.0, "y": 3.0, "z": 3.0 }, "trigger": true } }),
            ];
            Ok((
                rows,
                vec![(
                    "porta".to_string(),
                    door_graph().map_err(|error| refuse(&error))?,
                )],
            ))
        }
        "pickup" => {
            let rows = vec![
                json!({
                    "Name": { "text": "Item" },
                    "Transform": { "position": vector_json(Vec3::new(at.x, at.y + 0.8, at.z)) },
                    "Brush": { "shape": "sphere", "size": { "x": 0.5, "y": 0.5, "z": 0.5 },
                               "color": { "r": 0.95, "g": 0.75, "b": 0.15, "a": 1.0 } },
                    "Script": { "file": "item", "enabled": true }
                }),
                json!({ "Collider": { "owner": { "$entity_of": 0 }, "name": "sensor", "shape": "sphere",
                                      "radius": 0.8, "trigger": true } }),
            ];
            Ok((
                rows,
                vec![(
                    "item".to_string(),
                    pickup_graph().map_err(|error| refuse(&error))?,
                )],
            ))
        }
        "player" => {
            let rows = vec![
                json!({
                    "Name": { "text": "Jogador" },
                    "Transform": { "position": vector_json(Vec3::new(at.x, at.y + 0.9, at.z)) },
                    "Brush": { "shape": "cylinder", "size": { "x": 0.6, "y": 1.8, "z": 0.6 },
                               "color": { "r": 0.95, "g": 0.55, "b": 0.75, "a": 1.0 } },
                    "Character": { "player": true }
                }),
                json!({ "Collider": { "owner": { "$entity_of": 0 }, "name": "corpo", "shape": "capsule",
                                      "radius": 0.3, "height": 1.8 } }),
            ];
            Ok((rows, Vec::new()))
        }
        "spawn" => {
            let rows = vec![
                json!({
                    "Name": { "text": "Ponto de nascimento" },
                    "Transform": { "position": vector_json(Vec3::new(at.x, at.y + 0.1, at.z)) },
                    "Brush": { "shape": "cylinder", "size": { "x": 0.8, "y": 0.2, "z": 0.8 },
                               "color": { "r": 0.2, "g": 0.7, "b": 0.4, "a": 1.0 } },
                    "PlayerStart": { "yaw_degrees": 0.0 }
                }),
                // A collider of its own opts the marker out of the automatic solids: nobody trips on it.
                json!({ "Collider": { "owner": { "$entity_of": 0 }, "name": "marca", "shape": "box",
                                      "size": { "x": 0.8, "y": 0.2, "z": 0.8 }, "trigger": true } }),
            ];
            Ok((rows, Vec::new()))
        }
        other => Err(Failure::new(
            "PREFAB_UNKNOWN",
            &format!("não existe o prefab `{other}`"),
            "Veja a lista com prefab.list.",
        )),
    }
}

impl EditorSession {
    /// `prefab.list`.
    pub fn prefab_list(&self) -> Json {
        let mut prefabs: Vec<Json> = PREFABS
            .iter()
            .map(|prefab| {
                json!({
                    "id": prefab.id,
                    "kind": "builtin",
                    "title": prefab.title,
                    "summary": prefab.summary,
                })
            })
            .collect();
        prefabs.extend(self.project_prefabs());
        json!({ "prefabs": prefabs })
    }

    /// `prefab.spawn`: creates the prefab in front of the camera, on the current floor (one undo
    /// step), and puts the graphs it needs in `scripts/` when they are not there yet.
    pub fn prefab_spawn(&mut self, id: &str) -> Result<Json, Failure> {
        // `project:<name>` is a prefab saved from the project itself.
        if let Some(name) = id.strip_prefix(crate::user_prefabs::PREFAB_PREFIX) {
            return self.prefab_spawn_project(name);
        }
        let project_root = self
            .project
            .as_ref()
            .ok_or_else(Failure::no_project)?
            .root
            .clone();
        let camera = self.camera.camera;
        let forward = camera.forward();
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalized_or_zero();
        let flat = if flat.length_squared() == 0.0 {
            Vec3::new(0.0, 0.0, -1.0)
        } else {
            flat
        };
        let target = camera.position + flat * PLACE_DISTANCE;
        let at = Vec3::new(target.x, self.floor_elevation(), target.z);
        let (rows, graphs) = build(id, at)?;

        // Graphs first: if writing fails, the scene has not changed.
        let directory = resolve_inside(&project_root, SCRIPTS_DIRECTORY).map_err(|error| {
            Failure::new(
                "PREFAB_IO",
                &format!("a pasta de scripts: {error}"),
                "Confira se a pasta do projeto pode ser escrita.",
            )
        })?;
        let mut written = Vec::new();
        let mut kept = Vec::new();
        for (name, graph) in &graphs {
            let path = directory.join(format!("{name}{GRAPH_SUFFIX}"));
            let aipo = directory.join(format!("{name}.aipo"));
            if path.exists() || aipo.exists() {
                // Someone's own graph (maybe edited): never overwritten.
                kept.push(name.clone());
                continue;
            }
            std::fs::create_dir_all(&directory).map_err(|error| {
                Failure::new(
                    "PREFAB_IO",
                    &format!("criar a pasta de scripts: {error}"),
                    "Confira se a pasta do projeto pode ser escrita.",
                )
            })?;
            atomic_write(&path, graph.to_text().as_bytes()).map_err(|error| {
                Failure::new(
                    "PREFAB_IO",
                    &format!("gravar o grafo `{name}`: {error}"),
                    "Confira se há espaço no disco.",
                )
            })?;
            written.push(name.clone());
        }

        let records: Vec<CommandRecord> = rows
            .into_iter()
            .map(|row| CommandRecord::new("world.create", json!({ "rows": row })))
            .collect();
        let result = self.execute(&CommandRecord::new(
            BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        let entity = result
            .get(0)
            .and_then(|first| first.get("entity"))
            .and_then(Json::as_u64);
        self.selection = entity;
        self.refresh_script_diagnostics();
        Ok(json!({
            "entity": entity,
            "graphs_written": written,
            "graphs_kept": kept,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use petunia_commands::{BATCH_COMMAND, CommandRecord, Dispatcher};
    use petunia_math::Vec3;
    use petunia_project::register_standard_tables;
    use petunia_script::{PhysicsSettings, ScriptHost, compile_graph};
    use petunia_world::{PersistentId, Value, World};
    use serde_json::{Value as Json, json};

    use super::{build, door_graph, pickup_graph};
    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    #[test]
    fn the_player_prefab_spawns_a_working_keyboard_character() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_player_prefab_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "player" }),
        );
        let spawned = call(
            &mut session,
            json!({ "op": "prefab.spawn", "id": "player" }),
        );
        let player = spawned["entity"].as_u64().expect("player id");
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert_eq!(started["has_player"], true, "{started}");
        assert_eq!(started["stand_in_player"], false, "{started}");
        assert!(
            started["problems"].as_array().expect("list").is_empty(),
            "{started}"
        );
        for _ in 0..90 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
            );
        }
        let standing = session
            .frame()
            .expect("frame")
            .items
            .into_iter()
            .find(|item| item.entity == player)
            .map(|item| item.transform.position)
            .expect("the player is drawn");
        assert!(
            standing.y > 0.5 && standing.y < 1.5,
            "the player stands on the floor: {standing:?}"
        );
        for _ in 0..60 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0, "forward": 1.0 }),
            );
        }
        let walked = session
            .frame()
            .expect("frame")
            .items
            .into_iter()
            .find(|item| item.entity == player)
            .map(|item| item.transform.position)
            .expect("the player is drawn");
        assert!(
            walked.z < standing.z - 1.0,
            "the player walks: {standing:?} -> {walked:?}"
        );
        call(&mut session, json!({ "op": "play.stop" }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_graphs_of_the_prefabs_compile_without_a_single_warning() {
        for graph in [door_graph().expect("door"), pickup_graph().expect("pickup")] {
            let compiled = compile_graph(&graph.to_text()).expect("compiles");
            assert!(
                compiled.warnings.is_empty(),
                "{:?}\n{}",
                compiled.warnings,
                compiled.text
            );
        }
    }

    /// A floor, a walking hero (a script pushes it along +x) and the prefab placed 4 m ahead.
    fn scene(prefab: &str) -> (ScriptHost, u64, u64) {
        let mut world = World::new(7);
        register_standard_tables(&mut world).expect("tables");
        let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
        let mut create = |world: &mut World, rows: Json| -> u64 {
            dispatcher
                .execute(
                    world,
                    &CommandRecord::new("world.create", json!({ "rows": rows })),
                )
                .expect("create")["entity"]
                .as_u64()
                .expect("id")
        };
        let floor = create(
            &mut world,
            json!({ "Transform": { "position": { "x": 0.0, "y": -0.5, "z": 0.0 } } }),
        );
        create(
            &mut world,
            json!({ "Collider": { "owner": floor, "name": "chão", "size": { "x": 40.0, "y": 1.0, "z": 40.0 } } }),
        );
        let hero = create(
            &mut world,
            json!({
                "Name": { "text": "Herói" },
                "Transform": { "position": { "x": 0.0, "y": 0.9, "z": 0.0 } },
                "Character": { "player": true },
                "Script": { "file": "anda", "enabled": true }
            }),
        );
        create(
            &mut world,
            json!({ "Collider": { "owner": hero, "name": "corpo", "shape": "capsule", "radius": 0.3, "height": 1.8 } }),
        );
        let (rows, graphs) = build(prefab, Vec3::new(4.0, 0.0, 0.0)).expect("prefab");
        let records: Vec<CommandRecord> = rows
            .into_iter()
            .map(|row| CommandRecord::new("world.create", json!({ "rows": row })))
            .collect();
        let made = dispatcher
            .execute(
                &mut world,
                &CommandRecord::new(BATCH_COMMAND, json!({ "records": records })),
            )
            .expect("batch");
        let main = made[0]["entity"].as_u64().expect("main object");
        let mut host = ScriptHost::new(world).expect("host");
        host.load(
            "anda",
            "fn on_update(entity, dt) { character.move(entity, 3.0, 0.0) }\n",
        )
        .expect("walker");
        for (name, graph) in graphs {
            host.load_graph(&name, &graph.to_text())
                .expect("graph loads");
        }
        let problems = host
            .attach_physics(PhysicsSettings::default(), BTreeMap::new())
            .expect("physics");
        assert!(problems.is_empty(), "{problems:?}");
        (host, hero, main)
    }

    fn height_of(host: &ScriptHost, id: u64) -> f32 {
        let world = host.world();
        let handle = world.handle_of(PersistentId(id)).expect("entity");
        match world
            .get(handle, "Transform", "position")
            .expect("position")
        {
            Value::Vec3(position) => position.y,
            other => panic!("not a vector: {other:?}"),
        }
    }

    #[test]
    fn the_item_vanishes_when_the_hero_walks_into_it() {
        let (mut host, _, item) = scene("pickup");
        for _ in 0..180 {
            host.update(1.0 / 60.0);
        }
        assert!(host.take_faults().is_empty());
        let gone = {
            let world = host.world();
            world
                .handle_of(PersistentId(item))
                .map_or(true, |handle| !world.is_alive(handle))
        };
        assert!(gone, "the item was collected");
        let log: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
        assert!(
            log.iter().any(|line| line.contains("Item coletado")),
            "{log:?}"
        );
    }

    #[test]
    fn the_door_lifts_for_the_hero_waits_and_comes_back_down() {
        let (mut host, _, door) = scene("door");
        let rest = height_of(&host, door);
        let mut highest = rest;
        let mut open_at = None;
        let mut closed_at = None;
        for frame in 0..600 {
            host.update(1.0 / 60.0);
            let height = height_of(&host, door);
            highest = highest.max(height);
            if open_at.is_none() && height > rest + 3.0 {
                open_at = Some(frame);
            }
            if open_at.is_some() && closed_at.is_none() && (height - rest).abs() < 0.01 {
                closed_at = Some(frame);
            }
        }
        assert!(host.take_faults().is_empty());
        let opened = open_at.expect("the door opened");
        assert!(
            highest > rest + 3.0,
            "lifted by 3.2 m: {highest} from {rest}"
        );
        let closed = closed_at.expect("and closed again");
        let seconds = f64::from(closed - opened) / 60.0;
        assert!(
            (seconds - 3.0).abs() < 0.2,
            "open for about 3 s, was {seconds}"
        );
        let log: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
        assert_eq!(
            log.iter()
                .filter(|line| line.contains("A porta abriu"))
                .count(),
            1,
            "it opens once per visit: {log:?}"
        );
    }

    #[test]
    fn spawning_places_the_prefab_in_front_of_the_camera_in_one_undo_step_and_keeps_edited_graphs()
    {
        let root =
            std::env::temp_dir().join(format!("petunia_editor_test_prefab_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "prefab" }),
        );
        let listed = call(&mut session, json!({ "op": "prefab.list" }));
        assert_eq!(listed["prefabs"].as_array().expect("list").len(), 4);
        let before = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"]
            .as_u64()
            .expect("count");

        let made = call(&mut session, json!({ "op": "prefab.spawn", "id": "door" }));
        assert_eq!(made["graphs_written"], json!(["porta"]));
        assert!(root.join("scripts/porta.graph.json").is_file());
        let count = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"]
            .as_u64()
            .expect("count");
        assert_eq!(count, before + 3, "the door and its two colliders");
        let entity = made["entity"].as_u64().expect("door id");
        let described = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": entity }),
        );
        assert_eq!(described["name"], "Porta");

        // A graph the person already has is never overwritten.
        std::fs::write(root.join("scripts/porta.graph.json"), "meu\n").expect("edit");
        let second = call(&mut session, json!({ "op": "prefab.spawn", "id": "door" }));
        assert_eq!(second["graphs_written"], json!([]));
        assert_eq!(second["graphs_kept"], json!(["porta"]));
        assert_eq!(
            std::fs::read_to_string(root.join("scripts/porta.graph.json")).expect("read"),
            "meu\n"
        );

        // One undo takes the whole second door away (and its colliders).
        call(&mut session, json!({ "op": "edit.undo" }));
        let after_undo = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"]
            .as_u64()
            .expect("count");
        assert_eq!(after_undo, before + 3);

        let reply = session.handle(&json!({ "op": "prefab.spawn", "id": "nao_existe" }));
        assert_eq!(reply["error"]["code"], "PREFAB_UNKNOWN");
        call(&mut session, json!({ "op": "prefab.spawn", "id": "spawn" }));
        call(
            &mut session,
            json!({ "op": "prefab.spawn", "id": "pickup" }),
        );
        assert!(root.join("scripts/item.graph.json").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }
}
