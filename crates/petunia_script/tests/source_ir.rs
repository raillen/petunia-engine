//! The first Aipo ⇄ IR subset (DR-34): supported code round-trips, everything else is refused.

// Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use petunia_script::compile_graph;
use petunia_script::source_ir::{emit_aipo, parse_source};
use serde_json::json;

#[test]
fn supported_code_round_trips_without_changing_the_program() {
    let text = r#"
var elapsed = 0.0
let gate = true

fn on_update(entity, dt) {
    elapsed = elapsed + dt
    if elapsed > 1.0 {
        transform.move_x(entity, 2.0)
        elapsed = 0.0
    }
}
"#;
    let first = parse_source("round_trip.aipo", text).expect("supported subset");
    let written = emit_aipo(&first);
    assert_eq!(
        written,
        "var elapsed = 0.0\nlet gate = true\n\nfn on_update(entity, dt) {\n    elapsed = (elapsed + dt)\n    if (elapsed > 1.0) {\n        transform.move_x(entity, 2.0)\n        elapsed = 0.0\n    }\n}\n"
    );
    let second = parse_source("round_trip.aipo", &written).expect("written text parses");
    assert_eq!(written, emit_aipo(&second));
}

#[test]
fn compound_assignment_and_while_round_trip() {
    let text = r#"
var total = 0.0
var steps = 0

fn on_update(entity, dt) {
    total += dt
    while steps < 3 {
        steps += 1
        if steps == 2 {
            continue
        }
        transform.move_x(entity, 1.0)
    }
}
"#;
    let first = parse_source("loop.aipo", text).expect("supported subset");
    let written = emit_aipo(&first);
    let second = parse_source("loop.aipo", &written).expect("written text parses");
    assert_eq!(written, emit_aipo(&second));
    assert!(written.contains("total += dt"), "{written}");
    assert!(written.contains("while (steps < 3) {"), "{written}");
}

#[test]
fn repeat_round_trips() {
    let text = r#"
fn on_update(entity, dt) {
    repeat 3 as i {
        transform.move_x(entity, i * dt)
    }
}
"#;
    let first = parse_source("repeat.aipo", text).expect("supported subset");
    let written = emit_aipo(&first);
    assert!(written.contains("repeat 3 as i {"), "{written}");
    let second = parse_source("repeat.aipo", &written).expect("written text parses");
    assert_eq!(written, emit_aipo(&second));
}

#[test]
fn loop_and_each_round_trip() {
    let text = r#"
fn on_update(entity, dt) {
    var steps = 0
    loop {
        steps += 1
        if steps >= 3 {
            break
        }
        continue
    }
    each item in world.find("Alvo") {
        transform.move_x(item, dt)
    }
    each index, item in world.find("Alvo") {
        transform.move_x(item, index * dt)
    }
}
"#;
    let first = parse_source("loops.aipo", text).expect("supported subset");
    let written = emit_aipo(&first);
    assert!(written.contains("loop {"), "{written}");
    assert!(
        written.contains("each item in world.find(\"Alvo\") {"),
        "{written}"
    );
    assert!(
        written.contains("each index, item in world.find(\"Alvo\") {"),
        "{written}"
    );
    let second = parse_source("loops.aipo", &written).expect("written text parses");
    assert_eq!(written, emit_aipo(&second));
}

#[test]
fn unsupported_code_is_refused_with_a_line_and_a_reason() {
    let diagnostics = parse_source("unsupported.aipo", "struct Ponto {\n    x\n}\n")
        .expect_err("structs are outside the subset");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "IR_UNSUPPORTED");
    assert_eq!(diagnostics[0].line, 1);
    assert!(
        diagnostics[0].message.contains("declaração"),
        "{diagnostics:?}"
    );
}

#[test]
fn code_generated_by_the_node_graph_reads_into_the_same_ir() {
    let graph = json!({
        "schema_version": 1,
        "nodes": [
            { "id": 1, "kind": "event.on_update" },
            { "id": 2, "kind": "api.transform.move_x" }
        ],
        "edges": [
            { "from": { "node": 1, "pin": "then" }, "to": { "node": 2, "pin": "in" } },
            { "from": { "node": 1, "pin": "dt" }, "to": { "node": 2, "pin": "delta" } }
        ]
    })
    .to_string();
    let compiled = compile_graph(&graph).expect("graph compiles");
    let program = parse_source("generated.aipo", &compiled.text)
        .expect("the current graph output fits the first subset");
    let written = emit_aipo(&program);
    let again = parse_source("generated.aipo", &written).expect("written text parses");
    assert_eq!(written, emit_aipo(&again));
    assert!(
        written.contains("transform.move_x(entity, dt)"),
        "{written}"
    );
}
