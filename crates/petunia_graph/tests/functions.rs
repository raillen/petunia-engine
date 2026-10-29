//! Functions, globals and locals through the public graph API.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use petunia_graph::{
    ApiSpec, Catalog, EventSpec, GraphDocument, PinRef, add_node, compile, connect, set_param,
    validate,
};
use serde_json::json;

fn catalog() -> Catalog {
    Catalog::new(
        &[ApiSpec {
            module: "log".to_string(),
            name: "info".to_string(),
            params: vec![("message".to_string(), "Value".to_string())],
            returns: "None".to_string(),
            summary: "escreve".to_string(),
            permission: String::new(),
        }],
        &[EventSpec {
            handler: "on_start".to_string(),
            title: "Ao começar".to_string(),
            summary: "uma vez".to_string(),
            params: vec![("entity".to_string(), "Int".to_string())],
        }],
    )
}

fn pin(node: u32, name: &str) -> PinRef {
    PinRef {
        node,
        pin: name.to_string(),
    }
}

#[test]
fn a_function_is_written_once_and_called_with_its_arguments() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let function = add_node(&mut document, &catalog, "flow.function", None).expect("function");
    set_param(
        &mut document,
        &catalog,
        function,
        "name",
        Some(json!("soma")),
    )
    .unwrap();
    set_param(
        &mut document,
        &catalog,
        function,
        "params",
        Some(json!("a, b")),
    )
    .unwrap();
    let returned = add_node(
        &mut document,
        &catalog,
        "flow.return",
        Some(&pin(function, "then")),
    )
    .expect("return");
    let a = add_node(&mut document, &catalog, "flow.param", None).unwrap();
    set_param(&mut document, &catalog, a, "name", Some(json!("a"))).unwrap();
    let b = add_node(&mut document, &catalog, "flow.param", None).unwrap();
    set_param(&mut document, &catalog, b, "name", Some(json!("b"))).unwrap();
    let add = add_node(&mut document, &catalog, "op.math", None).unwrap();
    set_param(&mut document, &catalog, add, "operator", Some(json!("+"))).unwrap();
    connect(&mut document, &catalog, &pin(a, "value"), &pin(add, "a")).unwrap();
    connect(&mut document, &catalog, &pin(b, "value"), &pin(add, "b")).unwrap();
    connect(
        &mut document,
        &catalog,
        &pin(add, "result"),
        &pin(returned, "value"),
    )
    .unwrap();

    let event = add_node(&mut document, &catalog, "event.on_start", None).unwrap();
    let call = add_node(
        &mut document,
        &catalog,
        "flow.call",
        Some(&pin(event, "then")),
    )
    .unwrap();
    set_param(&mut document, &catalog, call, "name", Some(json!("soma"))).unwrap();
    set_param(&mut document, &catalog, call, "arg1", Some(json!(2))).unwrap();
    set_param(&mut document, &catalog, call, "arg2", Some(json!(3))).unwrap();

    let compiled = compile(&document, &catalog).expect("compiles");
    assert!(
        compiled.text.contains("fn soma(a, b) {"),
        "{}",
        compiled.text
    );
    assert!(
        compiled.text.contains("return (a + b)"),
        "{}",
        compiled.text
    );
    assert!(compiled.text.contains("soma(2, 3)"), "{}", compiled.text);
    assert!(validate(&document, &catalog).is_empty());
}

#[test]
fn a_global_is_written_before_the_events_and_can_be_read() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let global = add_node(&mut document, &catalog, "flow.global", None).unwrap();
    set_param(
        &mut document,
        &catalog,
        global,
        "name",
        Some(json!("total")),
    )
    .unwrap();
    set_param(&mut document, &catalog, global, "mode", Some(json!("var"))).unwrap();
    set_param(&mut document, &catalog, global, "value", Some(json!(1))).unwrap();
    let read = add_node(&mut document, &catalog, "global.get", None).unwrap();
    set_param(&mut document, &catalog, read, "name", Some(json!("total"))).unwrap();
    let event = add_node(&mut document, &catalog, "event.on_start", None).unwrap();
    let write = add_node(
        &mut document,
        &catalog,
        "global.set",
        Some(&pin(event, "then")),
    )
    .unwrap();
    set_param(&mut document, &catalog, write, "name", Some(json!("total"))).unwrap();
    connect(
        &mut document,
        &catalog,
        &pin(read, "value"),
        &pin(write, "value"),
    )
    .unwrap();

    let compiled = compile(&document, &catalog).expect("compiles");
    let global_line = compiled.text.find("var total = 1").expect("global line");
    let event_line = compiled.text.find("fn on_start").expect("event line");
    assert!(global_line < event_line, "{}", compiled.text);
    assert!(compiled.text.contains("total = total"), "{}", compiled.text);
}

#[test]
fn a_local_lives_inside_its_function() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let function = add_node(&mut document, &catalog, "flow.function", None).unwrap();
    set_param(
        &mut document,
        &catalog,
        function,
        "name",
        Some(json!("contar")),
    )
    .unwrap();
    set_param(&mut document, &catalog, function, "params", Some(json!(""))).unwrap();
    let local = add_node(
        &mut document,
        &catalog,
        "local.declare",
        Some(&pin(function, "then")),
    )
    .unwrap();
    set_param(
        &mut document,
        &catalog,
        local,
        "name",
        Some(json!("passos")),
    )
    .unwrap();
    set_param(&mut document, &catalog, local, "mode", Some(json!("var"))).unwrap();
    set_param(&mut document, &catalog, local, "value", Some(json!(0))).unwrap();

    let compiled = compile(&document, &catalog).expect("compiles");
    assert!(compiled.text.contains("fn contar() {"), "{}", compiled.text);
    assert!(
        compiled.text.contains("var passos = 0"),
        "{}",
        compiled.text
    );
}

#[test]
fn a_call_to_a_missing_function_is_refused() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let event = add_node(&mut document, &catalog, "event.on_start", None).unwrap();
    let call = add_node(
        &mut document,
        &catalog,
        "flow.call",
        Some(&pin(event, "then")),
    )
    .unwrap();
    set_param(
        &mut document,
        &catalog,
        call,
        "name",
        Some(json!("nao_existe")),
    )
    .unwrap();
    let findings = validate(&document, &catalog);
    assert!(
        findings
            .iter()
            .any(|finding| finding.code == "FUNCTION_UNKNOWN")
    );
}
