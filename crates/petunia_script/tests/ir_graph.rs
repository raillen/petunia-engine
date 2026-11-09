//! The first IR → graph path (DR-34): only the explicit subset, with end-to-end execution.

// Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use petunia_commands::Dispatcher;
use petunia_project::register_standard_tables;
use petunia_script::ir_graph::to_graph;
use petunia_script::source_ir::{emit_aipo, parse_source};
use petunia_script::{ScriptHost, compile_graph};
use petunia_world::{PersistentId, Value, World};
use serde_json::json;

fn world_with(script: &str) -> (World, u64) {
    let mut world = World::new(7);
    register_standard_tables(&mut world).expect("tables");
    let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
    let created = dispatcher
        .execute(
            &mut world,
            &petunia_commands::CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": "Alvo" },
                    "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 0.0 } },
                    "Script": { "file": script, "enabled": true }
                }}),
            ),
        )
        .expect("create");
    (world, created["entity"].as_u64().expect("entity id"))
}

fn add_object(world: &mut World, name: &str) -> u64 {
    let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
    let created = dispatcher
        .execute(
            world,
            &petunia_commands::CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": name },
                    "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 0.0 } }
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

#[test]
fn a_linear_handler_becomes_a_runnable_graph() {
    let program = parse_source(
        "linear.aipo",
        r#"
fn on_update(entity, dt) {
    transform.move_x(entity, dt)
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("the existing compiler accepts the graph");
    assert!(
        compiled.text.contains("transform.move_x(entity, dt)"),
        "{text}"
    );

    let (world, id) = world_with("linear");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("linear", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 0.5).abs() < 1e-6);
    assert!(host.take_faults().is_empty());
}

#[test]
fn a_supported_program_survives_the_full_round_trip() {
    let text = r#"
fn on_update(entity, dt) {
    if dt > 0.0 {
        transform.move_x(entity, dt)
    }
}
"#;
    let first = parse_source("round_trip.aipo", text).expect("IR");
    let document = to_graph(&first).expect("graph");
    let compiled = compile_graph(&document.to_text()).expect("compiles");
    let second = parse_source("round_trip.generated.aipo", &compiled.text)
        .expect("the generated text is in the subset");
    assert_eq!(emit_aipo(&first), emit_aipo(&second), "{}", compiled.text);
}

#[test]
fn a_condition_becomes_an_if_node_and_runs() {
    let program = parse_source(
        "condition.aipo",
        r#"
fn on_update(entity, dt) {
    if dt > 0.0 {
        transform.move_x(entity, dt)
    }
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("if (dt > 0.0)"), "{text}");
    assert!(
        compiled.text.contains("transform.move_x(entity, dt)"),
        "{text}"
    );

    let (world, id) = world_with("condition");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("condition", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 0.5).abs() < 1e-6);
    assert!(host.take_faults().is_empty());
}

#[test]
fn an_elif_becomes_a_nested_if_and_runs() {
    let program = parse_source(
        "elif.aipo",
        r#"
fn on_update(entity, dt) {
    if dt > 1.0 {
        transform.move_x(entity, 1.0 * dt)
    } elif dt > 0.0 {
        transform.move_x(entity, 2.0 * dt)
    } else {
        transform.move_x(entity, 3.0 * dt)
    }
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    assert_eq!(
        document
            .nodes
            .iter()
            .filter(|node| node.kind == "flow.if")
            .count(),
        2
    );
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");

    let (world, id) = world_with("elif");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("elif", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 1.0).abs() < 1e-6, "{}", compiled.text);
    assert!(host.take_faults().is_empty());
}

#[test]
fn a_repeat_becomes_a_repeat_node_and_runs() {
    let program = parse_source(
        "repeat.aipo",
        r#"
fn on_update(entity, dt) {
    repeat 3 as i {
        transform.move_x(entity, i * dt)
    }
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("repeat 3 as i"), "{text}");

    let (world, id) = world_with("repeat");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("repeat", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 1.5).abs() < 1e-6, "{}", compiled.text);
    assert!(host.take_faults().is_empty());
}

#[test]
fn a_value_call_becomes_a_value_node() {
    let program = parse_source(
        "tick.aipo",
        r#"
fn on_start(entity) {
    if game.tick() >= 0 {
        log.info("começou")
    }
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("fn on_start(entity)"), "{text}");
    assert!(compiled.text.contains("game.tick()"), "{text}");
    assert!(compiled.text.contains("log.info(\"começou\")"), "{text}");
}

#[test]
fn variables_become_object_state_and_run() {
    let program = parse_source(
        "variables.aipo",
        r#"
fn on_update(entity, dt) {
    var speed = 1.0
    speed += 1.0
    transform.move_x(entity, speed * dt)
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("graph_state_set"), "{text}");
    assert!(compiled.text.contains("on_update/speed"), "{text}");

    let (world, id) = world_with("variables");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("variables", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 1.0).abs() < 1e-6);
    assert!(host.take_faults().is_empty());
}

#[test]
fn the_same_local_name_in_two_handlers_gets_two_slots() {
    let program = parse_source(
        "scopes.aipo",
        r#"
fn on_start(entity) {
    var speed = 1.0
}

fn on_update(entity, dt) {
    var speed = 2.0
    transform.move_x(entity, speed * dt)
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let compiled = compile_graph(&document.to_text()).expect("compiles");
    assert!(
        compiled.text.contains("on_start/speed"),
        "{}",
        compiled.text
    );
    assert!(
        compiled.text.contains("on_update/speed"),
        "{}",
        compiled.text
    );
}

#[test]
fn assigning_to_a_let_is_refused() {
    let program = parse_source(
        "immutable.aipo",
        r#"
fn on_update(entity, dt) {
    let speed = 1.0
    speed = 2.0
}
"#,
    )
    .expect("IR");
    let diagnostics = to_graph(&program).expect_err("let cannot be assigned");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "IR_GRAPH_UNSUPPORTED");
    assert!(diagnostics[0].message.contains("`let`"), "{diagnostics:?}");
}

#[test]
fn a_while_becomes_a_while_node_and_runs() {
    let program = parse_source(
        "while.aipo",
        r#"
fn on_update(entity, dt) {
    var steps = 0
    while steps < 3 {
        steps += 1
        transform.move_x(entity, dt)
    }
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("while ("), "{}", compiled.text);

    let (world, id) = world_with("while");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("while", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 1.5).abs() < 1e-6, "{}", compiled.text);
    assert!(host.take_faults().is_empty());
}

#[test]
fn a_loop_with_break_and_continue_runs() {
    let program = parse_source(
        "loop.aipo",
        r#"
fn on_update(entity, dt) {
    var steps = 0
    loop {
        steps += 1
        if steps >= 3 {
            break
        }
        continue
        transform.move_x(entity, 100.0)
    }
    transform.move_x(entity, dt)
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("loop {"), "{}", compiled.text);
    assert!(compiled.text.contains("break"), "{}", compiled.text);
    assert!(compiled.text.contains("continue"), "{}", compiled.text);

    let (world, id) = world_with("loop");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("loop", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 0.5).abs() < 1e-6, "{}", compiled.text);
    assert!(host.take_faults().is_empty());
}

#[test]
fn each_with_one_and_two_variables_runs() {
    let one = parse_source(
        "each_one.aipo",
        r#"
fn on_update(entity, dt) {
    each item in world.find("Name") {
        transform.move_x(item, dt)
    }
}
"#,
    )
    .expect("IR");
    let document = to_graph(&one).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("each item"), "{}", compiled.text);

    let (mut world, id) = world_with("each_one");
    let second = add_object(&mut world, "Outro");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("each_one", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 0.5).abs() < 1e-6, "{}", compiled.text);
    assert!(
        (x_of(&host, second) - 0.5).abs() < 1e-6,
        "{}",
        compiled.text
    );
    assert!(host.take_faults().is_empty());

    let two = parse_source(
        "each_two.aipo",
        r#"
fn on_update(entity, dt) {
    each index, item in world.find("Name") {
        transform.move_x(item, (index + 1) * dt)
    }
}
"#,
    )
    .expect("IR");
    let document = to_graph(&two).expect("graph");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("each position"), "{}", compiled.text);

    let (mut world, id) = world_with("each_two");
    let second = add_object(&mut world, "Outro");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("each_two", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 0.5).abs() < 1e-6, "{}", compiled.text);
    assert!(
        (x_of(&host, second) - 1.0).abs() < 1e-6,
        "{}",
        compiled.text
    );
    assert!(host.take_faults().is_empty());
}

#[test]
fn break_outside_a_loop_and_helpers_are_refused() {
    let program = parse_source(
        "break.aipo",
        r#"
fn on_update(entity, dt) {
    break
}
"#,
    )
    .expect("IR");
    let diagnostics = to_graph(&program).expect_err("break is outside a loop");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "IR_GRAPH_UNSUPPORTED");
    assert!(
        diagnostics[0].message.contains("`break`"),
        "{diagnostics:?}"
    );
}

#[test]
fn a_helper_function_is_converted_and_called() {
    let program = parse_source(
        "helpers.aipo",
        r#"
fn mover(entity) {
    transform.move_x(entity, 1.0)
}
fn on_update(entity, dt) {
    mover(entity)
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("helper function");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("fn mover(entity)"), "{text}");
    assert!(compiled.text.contains("mover(entity)"), "{text}");

    let (world, id) = world_with("helpers");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("helpers", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 1.0).abs() < 1e-6);
    assert!(host.take_faults().is_empty());
}

#[test]
fn a_global_and_a_function_local_keep_their_scope() {
    let program = parse_source(
        "scopes.aipo",
        r#"
var total = 1
fn mover(entity) {
    let passos = 2
    transform.move_x(entity, total + passos)
}
fn on_update(entity, dt) {
    total = total + 1
    mover(entity)
}
"#,
    )
    .expect("IR");
    let document = to_graph(&program).expect("globals and locals");
    let text = document.to_text();
    let compiled = compile_graph(&text).expect("compiles");
    assert!(compiled.text.contains("var total = 1"), "{text}");
    assert!(compiled.text.contains("let passos = 2"), "{text}");

    let (world, id) = world_with("scopes");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("scopes", &text).expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 4.0).abs() < 1e-6);
    assert!(host.take_faults().is_empty());
}

#[test]
fn fields_indices_lists_and_dicts_run_from_a_graph() {
    let text = r#"
fn on_update(entity, dt) {
    var position = transform.position(entity)
    var x = position.x
    var first = [1, 2, 3][0]
    var point = { "x": 1.0, "y": 2.0 }
    var y = point["y"]
    transform.move_x(entity, x + first + y)
}
"#;
    let first = parse_source("values.aipo", text).expect("IR");
    let document = to_graph(&first).expect("graph");
    let (world, id) = world_with("values");
    let mut host = ScriptHost::new(world).expect("host");
    host.load_graph("values", &document.to_text())
        .expect("loads");
    host.update(0.5);
    assert!((x_of(&host, id) - 3.0).abs() < 1e-6, "{}", x_of(&host, id));
    assert!(host.take_faults().is_empty(), "{:?}", host.take_faults());
}
