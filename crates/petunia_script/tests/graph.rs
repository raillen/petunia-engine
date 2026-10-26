//! Node graphs (DR-30) through the public surface: a graph is compiled to Aipo and run by the host.

// Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use petunia_commands::{CommandRecord, Dispatcher};
use petunia_project::register_standard_tables;
use petunia_script::{
    GRAPH_EVENTS, HANDLERS, LogLevel, ScriptHost, compile_graph, compile_graph_with, graph_catalog,
};
use petunia_world::{PersistentId, Value, World};
use serde_json::{Value as Json, json};

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

/// `"3.then"` -> `{ node: 3, pin: "then" }`.
fn pin(text: &str) -> Json {
    let (node, name) = text.split_once('.').expect("node.pin");
    json!({ "node": node.parse::<u32>().expect("node id"), "pin": name })
}

/// A graph file from `(id, kind, params)` nodes and `(from, to)` edges.
fn graph(nodes: &[(u32, &str, Json)], edges: &[(&str, &str)]) -> String {
    json!({
        "schema_version": 1,
        "nodes": nodes.iter().map(|(id, kind, params)| json!({ "id": id, "kind": kind, "params": params })).collect::<Vec<_>>(),
        "edges": edges.iter().map(|(from, to)| json!({ "from": pin(from), "to": pin(to) })).collect::<Vec<_>>(),
    })
    .to_string()
}

fn codes(text: &str) -> Vec<String> {
    compile_graph(text)
        .expect_err("should not compile")
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn every_graph_event_is_a_real_handler() {
    for event in GRAPH_EVENTS {
        assert!(
            HANDLERS.iter().any(|(name, _)| *name == event.handler),
            "{} is not a script handler",
            event.handler
        );
    }
}

#[test]
fn the_catalog_has_one_node_per_api_function() {
    let catalog = graph_catalog();
    for function in petunia_script::API {
        let kind = format!("api.{}.{}", function.module, function.name);
        assert!(catalog.get(&kind).is_some(), "{kind} is missing");
    }
    assert!(catalog.get("flow.if").is_some());
    assert!(catalog.get("event.on_update").is_some());
}

#[test]
fn a_graph_moves_an_object_every_frame() {
    let text = graph(
        &[
            (1, "event.on_update", json!({})),
            (2, "api.transform.move_x", json!({})),
        ],
        &[("1.then", "2.in"), ("1.dt", "2.delta")],
    );
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("fn on_update(entity, dt) {"));
    assert!(compiled.text.contains("transform.move_x(entity, dt)"));
    let (world, id) = world_with("mover");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("mover", &text).expect("loads");
    for _ in 0..4 {
        host.update(0.5);
    }
    assert!(
        (x_of(&host, id) - 2.0).abs() < 1e-6,
        "x = {}",
        x_of(&host, id)
    );
    assert!(host.take_faults().is_empty());
}

#[test]
fn conditions_math_and_variables_keep_state_between_frames() {
    // if counter < 3: counter = counter + 1 ; move_x(1.0)   -> moves exactly three times
    let text = graph(
        &[
            (1, "event.on_update", json!({})),
            (2, "flow.if", json!({})),
            (3, "op.compare", json!({ "operator": "<", "b": 3 })),
            (4, "var.get", json!({ "name": "contador", "fallback": 0 })),
            (5, "var.set", json!({ "name": "contador" })),
            (6, "op.math", json!({ "operator": "+", "b": 1 })),
            (7, "api.transform.move_x", json!({ "delta": 1.0 })),
        ],
        &[
            ("1.then", "2.in"),
            ("4.value", "3.a"),
            ("3.result", "2.condition"),
            ("2.then", "5.in"),
            ("4.value", "6.a"),
            ("6.result", "5.value"),
            ("5.then", "7.in"),
        ],
    );
    let (world, id) = world_with("contador");
    let mut host = ScriptHost::new(world).expect("host");
    let warnings = host.load_graph("contador", &text).expect("loads");
    assert!(warnings.is_empty(), "{warnings:?}");
    for _ in 0..10 {
        host.update(1.0 / 60.0);
    }
    assert!(
        (x_of(&host, id) - 3.0).abs() < 1e-6,
        "x = {}",
        x_of(&host, id)
    );
    assert!(host.take_faults().is_empty());
}

#[test]
fn loops_give_their_index_and_item_inside_the_body() {
    // repeat 3: move_x(index)  -> 0 + 1 + 2
    let text = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "flow.repeat", json!({ "count": 3 })),
            (3, "api.transform.move_x", json!({})),
        ],
        &[
            ("1.then", "2.in"),
            ("2.body", "3.in"),
            ("2.index", "3.delta"),
        ],
    );
    let (world, id) = world_with("laco");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("laco", &text).expect("loads");
    host.update(0.016);
    assert!(
        (x_of(&host, id) - 3.0).abs() < 1e-6,
        "x = {}",
        x_of(&host, id)
    );

    // each over world.find("Script"): one log line per object with a script
    let text = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "api.world.find", json!({ "table": "Script" })),
            (3, "flow.each", json!({})),
            (4, "api.log.info", json!({})),
        ],
        &[
            ("1.then", "3.in"),
            ("2.result", "3.list"),
            ("3.body", "4.in"),
            ("3.item", "4.message"),
        ],
    );
    let (world, _) = world_with("cada");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("cada", &text).expect("loads");
    host.update(0.016);
    let lines = host.take_log();
    assert_eq!(
        lines.iter().filter(|l| l.level == LogLevel::Info).count(),
        1
    );
}

#[test]
fn text_constants_survive_quotes_and_newlines() {
    let message = "ele disse \"oi\"\ne \\ saiu {com chaves}";
    let text = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "api.log.info", json!({ "message": message })),
        ],
        &[("1.then", "2.in")],
    );
    let (world, _) = world_with("texto");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("texto", &text).expect("loads");
    host.update(0.016);
    let lines = host.take_log();
    assert!(
        lines.iter().any(|line| line.text.contains(message)),
        "{lines:?}"
    );
}

#[test]
fn the_same_graph_always_writes_the_same_text_whatever_the_node_order() {
    let nodes_a = [
        (1, "event.on_update", json!({})),
        (2, "api.transform.move_x", json!({ "delta": 0.5 })),
        (3, "api.transform.move_y", json!({ "delta": 0.25 })),
    ];
    let edges = [("1.then", "2.in"), ("2.then", "3.in")];
    let mut nodes_b = nodes_a.clone();
    nodes_b.reverse();
    let first = compile_graph(&graph(&nodes_a, &edges)).expect("a");
    let second = compile_graph(&graph(&nodes_b, &edges)).expect("b");
    assert_eq!(first.text, second.text);
    assert_eq!(first.line_nodes, second.line_nodes);
}

#[test]
fn every_line_knows_its_node() {
    let text = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "api.transform.move_x", json!({ "delta": 1.0 })),
        ],
        &[("1.then", "2.in")],
    );
    let compiled = compile_graph(&text).expect("compiles");
    let lines: Vec<&str> = compiled.text.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.contains("transform.move_x"))
        .expect("call line");
    assert_eq!(compiled.line_nodes.get(at).copied().flatten(), Some(2));
}

#[test]
fn mistakes_are_reported_in_words_aimed_at_a_node() {
    // A node kind that is no longer in the API: kept in the file, refused to run.
    assert!(
        codes(&graph(&[(1, "api.world.sumiu", json!({}))], &[]))
            .contains(&"NODE_GHOST".to_string())
    );

    // A required pin with nothing on it.
    let missing = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "api.transform.move_x", json!({})),
        ],
        &[("1.then", "2.in")],
    );
    assert!(codes(&missing).contains(&"PIN_MISSING".to_string()));

    // Text into a decimal pin.
    let mismatch = graph(
        &[
            (1, "event.on_collision", json!({})),
            (2, "api.transform.move_x", json!({})),
        ],
        &[("1.then", "2.in"), ("1.mine", "2.delta")],
    );
    assert!(codes(&mismatch).contains(&"TYPE_MISMATCH".to_string()));

    // The loop index used after the loop ended.
    let early = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "flow.repeat", json!({ "count": 2 })),
            (3, "api.transform.move_x", json!({})),
        ],
        &[
            ("1.then", "2.in"),
            ("2.done", "3.in"),
            ("2.index", "3.delta"),
        ],
    );
    assert!(codes(&early).contains(&"VALUE_NOT_READY".to_string()));

    // Two events of the same kind.
    let twice = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "event.on_start", json!({})),
        ],
        &[],
    );
    assert!(codes(&twice).contains(&"EVENT_DUPLICATE".to_string()));

    // One exit that leads to two nodes.
    let fan = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "api.character.jump", json!({})),
            (3, "api.character.jump", json!({})),
        ],
        &[("1.then", "2.in"), ("1.then", "3.in")],
    );
    assert!(codes(&fan).contains(&"EXEC_FANOUT".to_string()));

    // Values wired in a circle.
    let circle = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "flow.if", json!({})),
            (3, "op.not", json!({})),
            (4, "op.not", json!({})),
            (5, "api.character.jump", json!({})),
        ],
        &[
            ("1.then", "2.in"),
            ("3.result", "4.a"),
            ("4.result", "3.a"),
            ("4.result", "2.condition"),
            ("2.then", "5.in"),
        ],
    );
    assert!(codes(&circle).contains(&"DATA_CYCLE".to_string()));

    // A setting outside its options.
    let bad_option = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "flow.if", json!({})),
            (
                3,
                "op.compare",
                json!({ "operator": "parece", "a": 1, "b": 2 }),
            ),
            (4, "api.character.jump", json!({})),
        ],
        &[
            ("1.then", "2.in"),
            ("3.result", "2.condition"),
            ("2.then", "4.in"),
        ],
    );
    assert!(codes(&bad_option).contains(&"SETTING_INVALID".to_string()));
}

#[test]
fn nodes_nobody_runs_are_a_warning_not_an_error() {
    let text = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "api.character.jump", json!({})),
        ],
        &[],
    );
    let compiled = compile_graph(&text).expect("still compiles");
    assert!(compiled.warnings.iter().any(|w| w.code == "NODE_LOOSE"));
}

#[test]
fn hostile_files_are_refused_before_anything_runs() {
    assert_eq!(codes("isto não é json"), vec!["GRAPH_INVALID"]);
    assert_eq!(codes(&"x".repeat(2 << 20)), vec!["GRAPH_INVALID"]);
    // An extra field, a newer version, repeated ids, a wire to nowhere, too many nodes.
    assert_eq!(
        codes(r#"{"schema_version":1,"nodes":[],"edges":[],"extra":1}"#),
        vec!["GRAPH_INVALID"]
    );
    assert_eq!(
        codes(r#"{"schema_version":99,"nodes":[],"edges":[]}"#),
        vec!["GRAPH_INVALID"]
    );
    assert_eq!(
        codes(
            r#"{"schema_version":1,"nodes":[{"id":1,"kind":"flow.if"},{"id":1,"kind":"flow.if"}],"edges":[]}"#
        ),
        vec!["GRAPH_INVALID"]
    );
    assert_eq!(
        codes(
            r#"{"schema_version":1,"nodes":[{"id":1,"kind":"flow.if"}],"edges":[{"from":{"node":1,"pin":"then"},"to":{"node":9,"pin":"in"}}]}"#
        ),
        vec!["GRAPH_INVALID"]
    );
    let many: Vec<Json> = (0..600)
        .map(|id| json!({ "id": id, "kind": "flow.if" }))
        .collect();
    let text = json!({ "schema_version": 1, "nodes": many, "edges": [] }).to_string();
    assert_eq!(codes(&text), vec!["GRAPH_INVALID"]);
    // A list hidden in a parameter.
    assert_eq!(
        codes(
            r#"{"schema_version":1,"nodes":[{"id":1,"kind":"event.on_start","params":{"x":[1,2]}}],"edges":[]}"#
        ),
        vec!["GRAPH_INVALID"]
    );
}

#[test]
fn a_graph_file_loads_from_the_scripts_folder_and_clashes_are_reported() {
    let root = std::env::temp_dir().join(format!("petunia_graph_test_dir_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("folder");
    let text = graph(
        &[
            (1, "event.on_update", json!({})),
            (2, "api.transform.move_x", json!({ "delta": 1.0 })),
        ],
        &[("1.then", "2.in")],
    );
    std::fs::write(root.join("anda.graph.json"), &text).expect("write graph");
    std::fs::write(root.join("dupla.aipo"), "fn on_start(entity) { }\n").expect("write script");
    std::fs::write(root.join("dupla.graph.json"), &text).expect("write clash");
    let (world, id) = world_with("anda");
    let mut host = ScriptHost::new(world).expect("host");
    let report = host.load_directory(&root);
    assert!(
        report.loaded.contains(&"anda".to_string()),
        "{:?}",
        report.loaded
    );
    assert!(
        report
            .failed
            .iter()
            .any(|(name, found)| name == "dupla"
                && found.iter().any(|d| d.code == "GRAPH_NAME_CLASH")),
        "{:?}",
        report.failed
    );
    host.update(0.016);
    assert!((x_of(&host, id) - 1.0).abs() < 1e-6);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn waiting_continues_the_chain_later_for_each_object_on_its_own() {
    // on_start: wait 1 s, then move_x(2) ; and again: wait 1 s, move_x(1).
    let text = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "flow.wait", json!({ "seconds": 1.0 })),
            (3, "api.transform.move_x", json!({ "delta": 2.0 })),
            (4, "flow.wait", json!({ "seconds": 1.0 })),
            (5, "api.transform.move_x", json!({ "delta": 1.0 })),
        ],
        &[
            ("1.then", "2.in"),
            ("2.done", "3.in"),
            ("3.then", "4.in"),
            ("4.done", "5.in"),
        ],
    );
    let compiled = compile_graph(&text).expect("compiles");
    assert!(
        compiled.text.contains("graph_wait_start(entity, 2, 1.0)"),
        "{}",
        compiled.text
    );
    assert!(compiled.text.contains("fn graph_after_2(entity) {"));
    assert!(
        compiled.text.contains("fn on_update(entity, dt) {"),
        "an on_update is made to count down"
    );
    let (world, id) = world_with("espera");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("espera", &text).expect("loads");
    host.update(0.5);
    assert!(
        x_of(&host, id).abs() < 1e-6,
        "nothing yet: x = {}",
        x_of(&host, id)
    );
    host.update(0.5);
    assert!(
        (x_of(&host, id) - 2.0).abs() < 1e-6,
        "after 1 s: x = {}",
        x_of(&host, id)
    );
    host.update(0.5);
    host.update(0.5);
    assert!(
        (x_of(&host, id) - 3.0).abs() < 1e-6,
        "after 2 s: x = {}",
        x_of(&host, id)
    );
    for _ in 0..10 {
        host.update(0.5);
    }
    assert!((x_of(&host, id) - 3.0).abs() < 1e-6, "it does not repeat");
    assert!(host.take_faults().is_empty());
}

#[test]
fn waiting_again_before_the_end_restarts_the_count() {
    // on_update: wait 0.5 s then move_x. The wait starts again every frame, so it never ends.
    let text = graph(
        &[
            (1, "event.on_update", json!({})),
            (2, "flow.wait", json!({ "seconds": 0.5 })),
            (3, "api.transform.move_x", json!({ "delta": 1.0 })),
        ],
        &[("1.then", "2.in"), ("2.done", "3.in")],
    );
    let compiled = compile_graph(&text).expect("compiles");
    assert_eq!(
        compiled.text.matches("fn on_update(").count(),
        1,
        "the graph's own on_update counts the waits down:\n{}",
        compiled.text
    );
    let (world, id) = world_with("reinicia");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("reinicia", &text).expect("loads");
    for _ in 0..20 {
        host.update(0.1);
    }
    assert!(
        x_of(&host, id).abs() < 1e-6,
        "the wait never ends: x = {}",
        x_of(&host, id)
    );
    assert!(host.take_faults().is_empty());
}

#[test]
fn after_a_wait_only_the_object_is_still_there() {
    let text = graph(
        &[
            (1, "event.on_collision", json!({})),
            (2, "flow.wait", json!({ "seconds": 1.0 })),
            (3, "api.transform.move_x", json!({})),
        ],
        &[
            ("1.then", "2.in"),
            ("2.done", "3.in"),
            ("1.other", "3.entity"),
        ],
    );
    assert!(codes(&text).contains(&"VALUE_NOT_READY".to_string()));
    // The object itself is fine.
    let fine = graph(
        &[
            (1, "event.on_collision", json!({})),
            (2, "flow.wait", json!({ "seconds": 1.0 })),
            (3, "api.transform.move_x", json!({ "delta": 1.0 })),
        ],
        &[
            ("1.then", "2.in"),
            ("2.done", "3.in"),
            ("1.entity", "3.entity"),
        ],
    );
    assert!(compile_graph(&fine).is_ok());
}

#[test]
fn a_wait_inside_a_loop_warns() {
    let text = graph(
        &[
            (1, "event.on_start", json!({})),
            (2, "flow.repeat", json!({ "count": 3 })),
            (3, "flow.wait", json!({ "seconds": 1.0 })),
            (4, "api.transform.move_x", json!({ "delta": 1.0 })),
        ],
        &[("1.then", "2.in"), ("2.body", "3.in"), ("3.done", "4.in")],
    );
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.warnings.iter().any(|w| w.code == "WAIT_IN_LOOP"));
}

#[test]
fn play_mode_reports_what_ran_and_why_a_condition_was_false() {
    let text = graph(
        &[
            (1, "event.on_update", json!({})),
            (2, "flow.if", json!({})),
            (
                3,
                "op.compare",
                json!({ "operator": ">", "a": 1.0, "b": 2.0 }),
            ),
            (4, "api.log.info", json!({ "message": "entrou" })),
        ],
        &[
            ("1.then", "2.in"),
            ("3.result", "2.condition"),
            ("2.then", "4.in"),
        ],
    );
    let plain = compile_graph(&text).expect("compiles");
    assert!(!plain.text.contains("graph_debug"), "{}", plain.text);
    assert_eq!(
        compile_graph_with(&text, false).expect("compiles").text,
        plain.text,
        "the marks are the only difference"
    );
    let (world, _) = world_with("depurar");
    let mut host = ScriptHost::new(world).expect("host");
    host.set_debug_marks(true);
    host.load_graph("depurar", &text).expect("loads");
    host.update(0.5);
    let snapshot = host.debug_snapshot();
    let marks = snapshot["marks"].as_array().expect("marks").clone();
    let kind = |node: u32, wanted: &str| {
        marks
            .iter()
            .any(|mark| mark["node"] == node && mark["kind"] == wanted)
    };
    assert!(kind(1, "ran"), "the event ran: {snapshot}");
    assert!(kind(2, "ran"), "the If ran: {snapshot}");
    assert!(
        marks
            .iter()
            .any(|mark| mark["node"] == 2 && mark["kind"] == "check" && mark["value"] == false),
        "the condition decided false: {snapshot}"
    );
    assert_eq!(
        marks
            .iter()
            .filter(|mark| mark["node"] == 3 && mark["kind"] == "value")
            .count(),
        2,
        "both values the condition used: {snapshot}"
    );
    assert!(!kind(4, "ran"), "the 'then' branch did not run: {snapshot}");
}
