//! The Script Host, exercised through its public surface only.

// Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use petunia_commands::{CommandRecord, Dispatcher};
use petunia_project::register_standard_tables;
use petunia_script::{API, PlayerInput, ScriptHost, ScriptLimits, api_json, simulate};
use petunia_world::{PersistentId, Value, World};
use serde_json::json;

fn world_with(script: &str) -> (World, u64) {
    let mut world = World::new(7);
    register_standard_tables(&mut world).expect("tables");
    let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
    let created = dispatcher
        .execute(
            &mut world,
            &CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": "Alvo" },
                    "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 0.0 } },
                    "Script": { "file": script, "enabled": true }
                }}),
            ),
        )
        .expect("create");
    let id = created["entity"].as_u64().expect("entity id");
    (world, id)
}

fn add_object(world: &mut World, script: &str) -> u64 {
    let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
    let created = dispatcher
        .execute(
            world,
            &CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": script },
                    "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 0.0 } },
                    "Script": { "file": script, "enabled": true }
                }}),
            ),
        )
        .expect("create");
    created["entity"].as_u64().expect("entity id")
}

fn x_of(host: &ScriptHost, id: u64) -> f64 {
    let world = host.world();
    let handle = world.handle_of(PersistentId(id)).expect("entity");
    match world
        .get(handle, "Transform", "position")
        .expect("position")
    {
        Value::Vec3(position) => f64::from(position.x),
        other => panic!("not a vector: {other:?}"),
    }
}

const WALKER: &str = r#"
fn on_start(entity) {
    log.info("começou", entity)
}

fn on_update(entity, dt) {
    var position = world.get(entity, "Transform", "position")
    position.x = position.x + 1.0
    world.set(entity, "Transform", "position", position)
}
"#;

#[test]
fn on_start_runs_once_and_on_update_every_frame() {
    let (world, id) = world_with("walker");
    let mut host = ScriptHost::new(world).expect("host");
    host.load("walker", WALKER).expect("compiles");
    for _ in 0..5 {
        host.update(1.0 / 60.0);
    }
    assert!(
        (x_of(&host, id) - 5.0).abs() < 1e-6,
        "five frames, one meter each"
    );
    let starts = host
        .take_log()
        .iter()
        .filter(|line| line.text.starts_with("começou"))
        .count();
    assert_eq!(starts, 1, "on_start runs exactly once");
    assert_eq!(host.commands_run(), 5);
    assert!(host.take_faults().is_empty());
}

#[test]
fn a_runaway_script_is_stopped_and_switched_off_then_a_reload_brings_it_back() {
    let (world, id) = world_with("loop");
    let mut host = ScriptHost::new(world).expect("host");
    host.set_limits(ScriptLimits {
        instructions_per_call: 20_000,
    });
    host.load(
        "loop",
        "fn on_update(entity, dt) {\n    while true {\n    }\n}\n",
    )
    .expect("compiles");
    let started = std::time::Instant::now();
    host.update(0.016);
    assert!(
        started.elapsed().as_secs() < 5,
        "the budget must stop the loop quickly"
    );
    let faults = host.take_faults();
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(
        faults[0].code, "SCRIPT_BUDGET_EXCEEDED",
        "{}",
        faults[0].message
    );
    assert!(faults[0].message.contains("20000"));
    assert_eq!(faults[0].entity, Some(id));
    // Switched off: the next frame does not call it again.
    host.update(0.016);
    assert!(host.take_faults().is_empty());
    // A fixed file switches it back on.
    host.load("loop", WALKER).expect("fixed script compiles");
    host.update(0.016);
    assert!((x_of(&host, id) - 1.0).abs() < 1e-6);
}

#[test]
fn a_refused_command_is_a_failure_the_script_can_handle_and_changes_nothing() {
    let (world, id) = world_with("careful");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "careful",
        r#"
fn on_start(entity) {
    attempt {
        world.set(entity, "Transform", "position", "não é um vetor")?
    } failed erro {
        log.info("recusado")
    }
    log.info("seguiu em frente")
}
"#,
    )
    .expect("compiles");
    host.update(0.016);
    assert!(
        host.take_faults().is_empty(),
        "a handled refusal is not a fault"
    );
    assert!(
        x_of(&host, id).abs() < 1e-9,
        "the refused value changed nothing"
    );
    let lines: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert_eq!(
        lines,
        vec!["recusado".to_string(), "seguiu em frente".to_string()]
    );
}

#[test]
fn an_unhandled_failure_is_reported_as_a_fault_not_swallowed() {
    let (world, id) = world_with("sloppy");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "sloppy",
        "fn on_start(entity) {\n    world.set(entity, \"Transform\", \"position\", \"não é um vetor\")\n    log.info(\"não deveria chegar aqui\")\n}\n",
    )
    .expect("compiles");
    host.update(0.016);
    let faults = host.take_faults();
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0].entity, Some(id));
    assert_eq!(faults[0].code, "SCRIPT_FAILURE");
    assert!(x_of(&host, id).abs() < 1e-9);
}

#[test]
fn a_script_cannot_run_programs_read_files_or_the_environment() {
    let (world, _) = world_with("x");
    let mut host = ScriptHost::new(world).expect("host");
    for call in [
        "sh.run(\"touch /tmp/petunia-escaped\")",
        "fs.read_text(\"/etc/hostname\")",
        "env.get(\"HOME\")",
        "io.println(\"escrevendo no processo\")",
    ] {
        let source = format!("fn on_start(entity) {{\n    {call}\n}}\n");
        let outcome = host.load("evil", &source);
        assert!(outcome.is_err(), "`{call}` must be refused at compile time");
    }
    assert!(!std::path::Path::new("/tmp/petunia-escaped").exists());
}

#[test]
fn events_reach_the_object_after_the_frame() {
    let (world, id) = world_with("door");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "door",
        r#"
fn on_update(entity, dt) {
    if game.tick() == 2 {
        game.emit("abrir", entity)
    }
}

fn on_event(entity, name) {
    log.info("evento", name)
}
"#,
    )
    .expect("compiles");
    for _ in 0..4 {
        host.update(0.016);
    }
    let heard: Vec<String> = host
        .take_log()
        .into_iter()
        .map(|line| line.text)
        .filter(|text| text.starts_with("evento"))
        .collect();
    assert_eq!(heard, vec!["evento abrir".to_string()], "{heard:?}");
    host.send_event(id, "manual");
    assert!(
        host.take_log()
            .iter()
            .any(|line| line.text == "evento manual")
    );
}

#[test]
fn scripts_are_isolated_from_each_other_and_share_only_the_world() {
    let (world, _) = world_with("a");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "a",
        "let secret = 41\nfn on_start(entity) {\n    log.info(\"a\", secret)\n}\n",
    )
    .expect("a");
    // `b` cannot compile: `secret` does not exist there.
    assert!(
        host.load("b", "fn peek() {\n    return secret\n}\n")
            .is_err()
    );
}

#[test]
fn creating_finding_and_removing_objects_goes_through_commands() {
    let (world, _) = world_with("maker");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "maker",
        r#"
fn on_start(entity) {
    let coin = world.create({ "Name": { "text": "Moeda" } })
    log.info("achadas", world.find("Name").len())
    world.remove(coin)
    log.info("existe", world.exists(coin))
}
"#,
    )
    .expect("compiles");
    host.update(0.016);
    let lines: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert!(lines.contains(&"achadas 2".to_string()), "{lines:?}");
    assert!(lines.contains(&"existe false".to_string()), "{lines:?}");
    assert_eq!(host.commands_run(), 2);
}

#[test]
fn objects_with_the_same_script_keep_separate_state() {
    let (mut world, first) = world_with("counter");
    let second = add_object(&mut world, "counter");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "counter",
        r#"
var elapsed = 0.0

fn on_update(entity, dt) {
    elapsed = elapsed + dt
    transform.set_position(entity, Vec3{ x: elapsed, y: 0.0, z: 0.0 })
}
"#,
    )
    .expect("compiles");
    host.update(0.5);
    assert!((x_of(&host, first) - 0.5).abs() < 1e-6);
    assert!((x_of(&host, second) - 0.5).abs() < 1e-6);
    host.update(0.5);
    assert!((x_of(&host, first) - 1.0).abs() < 1e-6);
    assert!((x_of(&host, second) - 1.0).abs() < 1e-6);
}

#[test]
fn the_same_script_on_the_same_world_gives_the_same_result() {
    let run = || {
        let (world, id) = world_with("walker");
        let mut host = ScriptHost::new(world).expect("host");
        host.load("walker", WALKER).expect("compiles");
        for _ in 0..30 {
            host.update(1.0 / 60.0);
        }
        (x_of(&host, id), host.commands_run(), host.world().tick())
    };
    assert_eq!(run(), run());
}

#[test]
fn simulate_runs_on_a_copy_and_reports_what_changed() {
    let (world, id) = world_with("walker");
    let report = simulate(
        &world,
        &[("walker".to_string(), WALKER.to_string())],
        "walker",
        10,
        1.0 / 60.0,
    )
    .expect("runs");
    assert_eq!(report.ticks, 10);
    assert_eq!(report.commands, 10);
    assert_eq!(report.changed, 1, "{report:?}");
    assert!(
        report
            .log
            .iter()
            .any(|(_, _, text)| text.starts_with("começou"))
    );
    // The real world did not move.
    let handle = world.handle_of(PersistentId(id)).expect("entity");
    assert!(matches!(
        world.get(handle, "Transform", "position"),
        Ok(Value::Vec3(position)) if position.x == 0.0
    ));
}

#[test]
fn simulate_gives_an_unattached_script_a_test_object_and_reports_compile_errors() {
    let (world, _) = world_with("other");
    let report = simulate(
        &world,
        &[(
            "lonely".to_string(),
            "fn on_update(entity, dt) {\n    log.info(\"oi\")\n}\n".to_string(),
        )],
        "lonely",
        3,
        0.016,
    )
    .expect("runs");
    // The object that has the unloaded script `other` adds an error line of its own; the test object
    // ran the script once per frame.
    let hellos = report
        .log
        .iter()
        .filter(|(_, _, text)| text == "oi")
        .count();
    assert_eq!(
        hellos, 3,
        "the test object ran the script every frame: {:?}",
        report.log
    );
    let bad = simulate(
        &world,
        &[(
            "broken".to_string(),
            "fn on_update(entity, dt) {\n    let = 1\n}\n".to_string(),
        )],
        "broken",
        3,
        0.016,
    );
    assert!(bad.is_err());
}

#[test]
fn the_api_table_and_the_help_agree() {
    let api = api_json();
    let functions = api["functions"].as_array().expect("functions");
    assert_eq!(
        api["functions"].as_array().expect("functions").len(),
        API.len()
    );
    for name in [
        "world.get(",
        "world.set(",
        "transform.move(",
        "transform.rotate_y(",
        "game.emit(",
        "log.info(",
    ] {
        assert!(
            functions.iter().any(|function| function["signature"]
                .as_str()
                .is_some_and(|s| s.starts_with(name))),
            "{name}"
        );
    }
    assert!(
        api["keywords"]
            .as_array()
            .is_some_and(|k| k.iter().any(|v| v == "each"))
    );
    assert!(api["modules"].to_string().contains("math"));
    assert!(!api["modules"].to_string().contains("\"sh\""));
}

#[test]
fn transform_movement_rotation_and_direction_methods_work() {
    let (world, id) = world_with("mover");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "mover",
        r#"
fn on_start(entity) {
    transform.move_x(entity, delta = 3.0)
    transform.move_y(entity, delta = 1.5)
    transform.move_z(entity, delta = -2.0)
    transform.rotate_y(entity, degrees = 90.0)
    transform.set_scale(entity, Vec3{ x: 2.0, y: 2.0, z: 2.0 })

    let pos = transform.position(entity)
    let rot = transform.rotation(entity)
    let scl = transform.scale(entity)
    let fwd = transform.forward(entity)
    let rgt = transform.right(entity)
    let up = transform.up(entity)

    log.info("pos", pos.x, pos.y, pos.z)
    log.info("rot", rot.x, rot.y, rot.z)
    log.info("scl", scl.x, scl.y, scl.z)
    log.info("fwd", fwd.x, fwd.y, fwd.z)
    log.info("rgt", rgt.x, rgt.y, rgt.z)
    log.info("up", up.x, up.y, up.z)
}

fn on_update(entity, dt) {
    transform.move(entity, Vec3{ x: -1.0, y: 0.5, z: 1.0 })
    transform.rotate(entity, Vec3{ x: 10.0, y: 10.0, z: 10.0 })
}
"#,
    )
    .expect("compiles");

    host.update(0.016);

    let (pos, rot, scale) = {
        let world = host.world();
        let handle = world.handle_of(PersistentId(id)).expect("entity");
        let pos = match world.get(handle, "Transform", "position").expect("pos") {
            Value::Vec3(v) => *v,
            other => panic!("not a vec3: {other:?}"),
        };
        let rot = match world
            .get(handle, "Transform", "rotation_degrees")
            .expect("rot")
        {
            Value::Vec3(v) => *v,
            other => panic!("not a vec3: {other:?}"),
        };
        let scale = match world.get(handle, "Transform", "scale").expect("scale") {
            Value::Vec3(v) => *v,
            other => panic!("not a vec3: {other:?}"),
        };
        (pos, rot, scale)
    };

    // After on_start (move_x 3, move_y 1.5, move_z -2) + on_update (move -1, 0.5, 1)
    assert!((pos.x - 2.0).abs() < 1e-5);
    assert!((pos.y - 2.0).abs() < 1e-5);
    assert!((pos.z - (-1.0)).abs() < 1e-5);

    // After on_start (rotate_y 90) + on_update (rotate 10, 10, 10)
    assert!((rot.x - 10.0).abs() < 1e-5);
    assert!((rot.y - 100.0).abs() < 1e-5);
    assert!((rot.z - 10.0).abs() < 1e-5);

    // Scale set in on_start
    assert!((scale.x - 2.0).abs() < 1e-5);
    assert!((scale.y - 2.0).abs() < 1e-5);
    assert!((scale.z - 2.0).abs() < 1e-5);

    let lines: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("pos 3 1.5 -2") || l.starts_with("pos 3.0 1.5 -2.0")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("rot 0 90 0") || l.starts_with("rot 0.0 90.0 0.0")),
        "{lines:?}"
    );
    for (label, expected) in [
        ("fwd", [-1.0, 0.0, 0.0]),
        ("rgt", [0.0, 0.0, -1.0]),
        ("up", [0.0, 1.0, 0.0]),
    ] {
        let line = lines
            .iter()
            .find(|line| line.split_whitespace().next() == Some(label))
            .expect("direction log");
        let components: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .map(|component| component.parse().expect("number"))
            .collect();
        assert_eq!(components.len(), 3);
        for (actual, expected) in components.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-6, "{line}");
        }
    }
    assert!(host.take_faults().is_empty());
}

#[test]
fn transform_on_missing_entity_is_a_recoverable_failure() {
    let (world, _) = world_with("miss");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "miss",
        r#"
fn on_start(entity) {
    attempt {
        let pos = transform.position(999999)?
    } failed erro {
        log.info("recuperado", erro.message)
    }
}
"#,
    )
    .expect("compiles");
    host.update(0.016);
    let lines: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert!(
        lines.iter().any(|l| l.starts_with("recuperado")),
        "{lines:?}"
    );
    assert!(host.take_faults().is_empty());
}

#[test]
fn transform_with_overflowing_float_is_a_type_fault() {
    let (world, _id) = world_with("huge");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "huge",
        r#"
fn on_start(entity) {
    transform.move_x(entity, delta = 1.0e300)
}
"#,
    )
    .expect("compiles");
    host.update(0.016);
    let faults = host.take_faults();
    assert!(!faults.is_empty(), "overflowing float must be a type fault");
    assert!(
        faults
            .iter()
            .any(|f| f.message.contains("Float finito em transform.move_x")),
        "{faults:?}"
    );
}

#[test]
fn transform_on_entity_without_transform_table_is_recoverable_failure() {
    let mut world = World::new(7);
    petunia_project::register_standard_tables(&mut world).expect("tables");
    let mut dispatcher = petunia_commands::Dispatcher::with_builtin().expect("dispatcher");
    let _created = dispatcher
        .execute(
            &mut world,
            &petunia_commands::CommandRecord::new(
                "world.create",
                serde_json::json!({
                    "rows": {
                        "Name": { "text": "NoTransform" },
                        "Script": { "file": "no_transform", "enabled": true }
                    }
                }),
            ),
        )
        .expect("create");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "no_transform",
        r#"
fn on_start(entity) {
    attempt {
        let fwd = transform.forward(entity)?
    } failed erro {
        log.info("erro_tabela", erro.message)
    }
}
"#,
    )
    .expect("compiles");
    host.update(0.016);
    let lines: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("a entidade não tem linha em `Transform`")),
        "{lines:?}"
    );
    assert!(host.take_faults().is_empty());
}

/// DR-40: a project runs the files of its own programming method. A leftover of the other one is
/// not loaded, so the Doctor's warning ("this does not run") is true instead of a lie.
#[test]
fn only_the_projects_own_method_is_loaded() {
    let root =
        std::env::temp_dir().join(format!("petunia_script_test_method_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("dir");
    std::fs::write(
        root.join("script.aipo"),
        "fn on_start(entity) {\n    log.info(\"aipo\", entity)\n}\n",
    )
    .expect("aipo");
    std::fs::write(
        root.join("graph.graph.json"),
        r#"{"schema_version":2,"nodes":[{"id":1,"kind":"event.on_start","params":{}}],"edges":[]}"#,
    )
    .expect("graph");

    // An Aipo project loads the script and not the graph.
    let mut aipo_world = World::new(7);
    register_standard_tables(&mut aipo_world).expect("tables");
    let mut aipo = ScriptHost::new(aipo_world).expect("host");
    let report = aipo.load_directory(&root, "aipo");
    assert_eq!(report.loaded, vec!["script".to_string()], "{report:?}");

    // A pNodes project loads the graph and not the script.
    let mut nodes_world = World::new(7);
    register_standard_tables(&mut nodes_world).expect("tables");
    let mut nodes = ScriptHost::new(nodes_world).expect("host");
    let report = nodes.load_directory(&root, "nodes");
    assert_eq!(report.loaded, vec!["graph".to_string()], "{report:?}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn pressing_interact_reaches_the_nearest_door_and_zero_life_is_one_event() {
    let mut world = World::new(7);
    register_standard_tables(&mut world).expect("tables");
    let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
    dispatcher
        .execute(
            &mut world,
            &CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": "Jogador" },
                    "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 0.0 } },
                    "Character": { "player": true }
                }}),
            ),
        )
        .expect("player");
    dispatcher
        .execute(
            &mut world,
            &CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": "Porta" },
                    "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 1.0 } },
                    "Script": { "file": "porta", "enabled": true },
                    "Interact": { "radius": 2.0, "prompt": "Abrir" },
                    "Health": { "current": 30.0, "maximum": 100.0 }
                }}),
            ),
        )
        .expect("door");
    let far = dispatcher
        .execute(
            &mut world,
            &CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": "Longe" },
                    "Transform": { "position": { "x": 20.0, "y": 0.0, "z": 0.0 } },
                    "Script": { "file": "porta", "enabled": true },
                    "Interact": { "radius": 2.0, "prompt": "Longe" }
                }}),
            ),
        )
        .expect("far");
    let far_id = far["entity"].as_u64().expect("id");
    let mut host = ScriptHost::new(world).expect("host");
    host.load(
        "porta",
        r#"
fn on_event(entity, name) {
    log.info(name, entity)
    if name == "interact" {
        health.damage(entity, 30.0)
    }
}
"#,
    )
    .expect("compiles");
    assert_eq!(host.interact_prompt().as_deref(), Some("Abrir"));
    host.set_player_input(PlayerInput {
        interact: true,
        ..PlayerInput::default()
    });
    host.update(0.016);
    let lines: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert!(
        lines.iter().any(|line| line.starts_with("interact ")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.starts_with("died ")),
        "{lines:?}"
    );
    let world = host.world();
    let door = lines
        .iter()
        .find(|line| line.starts_with("interact "))
        .expect("interact line");
    let id: u64 = door.split(' ').nth(1).expect("id").parse().expect("number");
    assert_ne!(id, far_id);
    let handle = world.handle_of(PersistentId(id)).expect("door");
    let Value::Float(life) = world.get(handle, "Health", "current").expect("life") else {
        panic!("life is not a number");
    };
    assert!(*life <= 0.0, "{life}");
    drop(world);
    host.set_player_input(PlayerInput {
        interact: true,
        ..PlayerInput::default()
    });
    host.update(0.016);
    let again: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert!(
        again.iter().all(|line| !line.starts_with("interact ")),
        "holding the key must not fire again: {again:?}"
    );
}
