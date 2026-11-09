//! Editing and reading graphs through the public API, on a small catalog.

// Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use petunia_graph::{
    ApiSpec, Catalog, DataType, EventSpec, GraphDocument, PinRef, Want, add_node, compile, connect,
    disconnect, outline, remove_node, search, set_param, sources,
};
use serde_json::{Value as Json, json};

fn api(
    module: &str,
    name: &str,
    params: &[(&str, &str)],
    returns: &str,
    permission: &str,
) -> ApiSpec {
    ApiSpec {
        module: module.to_string(),
        name: name.to_string(),
        params: params
            .iter()
            .map(|(name, kind)| (name.to_string(), kind.to_string()))
            .collect(),
        returns: returns.to_string(),
        summary: format!("faz {module}.{name}"),
        permission: permission.to_string(),
    }
}

fn catalog() -> Catalog {
    Catalog::new(
        &[
            api(
                "transform",
                "move_x",
                &[("entity", "Int"), ("delta", "Float")],
                "None",
                "world.write",
            ),
            api(
                "transform",
                "position",
                &[("entity", "Int")],
                "Vec3",
                "world.read",
            ),
            api("world", "create", &[("rows", "Dict")], "Int", "world.write"),
            api("log", "info", &[("message", "Value")], "None", ""),
            api(
                "collision",
                "list",
                &[("entity", "Int")],
                "List",
                "world.read",
            ),
        ],
        &[
            EventSpec {
                handler: "on_start".to_string(),
                title: "Ao começar".to_string(),
                summary: "uma vez".to_string(),
                params: vec![("entity".to_string(), "Int".to_string())],
            },
            EventSpec {
                handler: "on_update".to_string(),
                title: "A cada quadro".to_string(),
                summary: "todo quadro".to_string(),
                params: vec![
                    ("entity".to_string(), "Int".to_string()),
                    ("dt".to_string(), "Float".to_string()),
                ],
            },
        ],
    )
}

fn pin(node: u32, name: &str) -> PinRef {
    PinRef {
        node,
        pin: name.to_string(),
    }
}

/// The node ids of a chain in order, read from the outline.
fn chain_nodes(steps: &Json) -> Vec<u64> {
    steps
        .as_array()
        .expect("list")
        .iter()
        .map(|step| step["node"].as_u64().expect("id"))
        .collect()
}

#[test]
fn a_new_node_goes_into_the_chain_where_it_is_added() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let start = add_node(&mut document, &catalog, "event.on_start", None).expect("event");
    let first = add_node(
        &mut document,
        &catalog,
        "api.log.info",
        Some(&pin(start, "then")),
    )
    .expect("first");
    // Added after the event again: it goes between the event and the first.
    let second = add_node(
        &mut document,
        &catalog,
        "api.transform.move_x",
        Some(&pin(start, "then")),
    )
    .expect("second");
    let view = outline(&document, &catalog);
    assert_eq!(
        chain_nodes(&view["events"][0]["steps"]),
        vec![u64::from(second), u64::from(first)]
    );
    assert!(document.check_shape().is_ok());
}

#[test]
fn removing_a_node_closes_the_chain_over_the_gap() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let start = add_node(&mut document, &catalog, "event.on_start", None).expect("event");
    let a = add_node(
        &mut document,
        &catalog,
        "api.log.info",
        Some(&pin(start, "then")),
    )
    .expect("a");
    let b = add_node(
        &mut document,
        &catalog,
        "api.log.info",
        Some(&pin(a, "then")),
    )
    .expect("b");
    let c = add_node(
        &mut document,
        &catalog,
        "api.log.info",
        Some(&pin(b, "then")),
    )
    .expect("c");
    remove_node(&mut document, &catalog, b).expect("removes");
    let view = outline(&document, &catalog);
    assert_eq!(
        chain_nodes(&view["events"][0]["steps"]),
        vec![u64::from(a), u64::from(c)]
    );
    assert!(document.node(b).is_none());
    assert!(remove_node(&mut document, &catalog, 999).is_err());
}

#[test]
fn wiring_rules_explain_themselves() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let start = add_node(&mut document, &catalog, "event.on_update", None).expect("event");
    let mover = add_node(
        &mut document,
        &catalog,
        "api.transform.move_x",
        Some(&pin(start, "then")),
    )
    .expect("mover");

    // A decimal into a decimal input: fine, and it replaces a constant.
    set_param(&mut document, &catalog, mover, "delta", Some(json!(2.0))).expect("constant");
    connect(
        &mut document,
        &catalog,
        &pin(start, "dt"),
        &pin(mover, "delta"),
    )
    .expect("wire");
    assert!(
        !document
            .node(mover)
            .expect("node")
            .params
            .contains_key("delta"),
        "the wire replaced the constant"
    );
    // A constant replaces the wire back.
    set_param(&mut document, &catalog, mover, "delta", Some(json!(1.5))).expect("constant again");
    assert!(
        document
            .edges
            .iter()
            .all(|edge| edge.to != pin(mover, "delta"))
    );

    // A decimal into an Int input: refused with the types named.
    let error = connect(
        &mut document,
        &catalog,
        &pin(start, "dt"),
        &pin(mover, "entity"),
    )
    .expect_err("types");
    assert!(
        error.0.contains("Decimal") && error.0.contains("Inteiro"),
        "{error}"
    );

    // An exec output already used, an exec input already used, and a loop back.
    let other = add_node(&mut document, &catalog, "api.log.info", None).expect("loose");
    let error = connect(
        &mut document,
        &catalog,
        &pin(start, "then"),
        &pin(other, "in"),
    )
    .expect_err("busy");
    assert!(error.0.contains("já leva"), "{error}");
    let tail = add_node(
        &mut document,
        &catalog,
        "api.log.info",
        Some(&pin(mover, "then")),
    )
    .expect("tail");
    let error = connect(
        &mut document,
        &catalog,
        &pin(tail, "then"),
        &pin(mover, "in"),
    )
    .expect_err("loop");
    assert!(
        error.0.contains("já roda depois") || error.0.contains("voltar"),
        "{error}"
    );
    let error = connect(
        &mut document,
        &catalog,
        &pin(mover, "then"),
        &pin(mover, "delta"),
    )
    .expect_err("self");
    assert!(!error.0.is_empty());

    // Order cannot be wired to a value.
    let error = connect(
        &mut document,
        &catalog,
        &pin(tail, "then"),
        &pin(other, "message"),
    )
    .expect_err("kinds");
    assert!(error.0.contains("não se ligam"), "{error}");

    // Disconnect takes the wire away.
    assert_eq!(disconnect(&mut document, &pin(tail, "in")), 1);
}

#[test]
fn constants_are_checked_against_the_pin() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let mover = add_node(&mut document, &catalog, "api.transform.move_x", None).expect("node");
    assert!(
        set_param(
            &mut document,
            &catalog,
            mover,
            "delta",
            Some(json!("texto"))
        )
        .is_err()
    );
    assert!(set_param(&mut document, &catalog, mover, "entity", Some(json!(1.5))).is_err());
    assert!(set_param(&mut document, &catalog, mover, "nada", Some(json!(1))).is_err());
    assert!(set_param(&mut document, &catalog, mover, "delta", Some(json!(3))).is_ok());
    set_param(&mut document, &catalog, mover, "delta", None).expect("clears");
    assert!(document.node(mover).expect("node").params.is_empty());

    let compare = add_node(&mut document, &catalog, "op.compare", None).expect("compare");
    assert!(
        set_param(
            &mut document,
            &catalog,
            compare,
            "operator",
            Some(json!("parece"))
        )
        .is_err()
    );
    assert!(
        set_param(
            &mut document,
            &catalog,
            compare,
            "operator",
            Some(json!(">="))
        )
        .is_ok()
    );
}

#[test]
fn an_event_can_only_be_added_once_and_values_do_not_join_chains() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let start = add_node(&mut document, &catalog, "event.on_start", None).expect("event");
    assert!(add_node(&mut document, &catalog, "event.on_start", None).is_err());
    let error =
        add_node(&mut document, &catalog, "op.not", Some(&pin(start, "then"))).expect_err("value");
    assert!(error.0.contains("só calcula"), "{error}");
    assert!(add_node(&mut document, &catalog, "nao.existe", None).is_err());
}

#[test]
fn the_outline_reads_like_a_sentence() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let start = add_node(&mut document, &catalog, "event.on_update", None).expect("event");
    let branch = add_node(
        &mut document,
        &catalog,
        "flow.if",
        Some(&pin(start, "then")),
    )
    .expect("if");
    let compare = add_node(&mut document, &catalog, "op.compare", None).expect("compare");
    set_param(
        &mut document,
        &catalog,
        compare,
        "operator",
        Some(json!("<")),
    )
    .expect("operator");
    set_param(&mut document, &catalog, compare, "b", Some(json!(3))).expect("b");
    set_param(&mut document, &catalog, compare, "a", Some(json!(1))).expect("a");
    connect(
        &mut document,
        &catalog,
        &pin(compare, "result"),
        &pin(branch, "condition"),
    )
    .expect("condition");
    let mover = add_node(
        &mut document,
        &catalog,
        "api.transform.move_x",
        Some(&pin(branch, "then")),
    )
    .expect("mover");
    connect(
        &mut document,
        &catalog,
        &pin(start, "dt"),
        &pin(mover, "delta"),
    )
    .expect("dt");

    let view = outline(&document, &catalog);
    let event = &view["events"][0];
    assert_eq!(event["text"], "QUANDO A cada quadro");
    let step = &event["steps"][0];
    let text = step["text"].as_str().expect("text");
    assert!(text.starts_with("SE Comparar é menor que"), "{text}");
    assert!(text.contains("B = 3"), "{text}");
    // The comparison is a nested row of the condition input, ready to be edited in place.
    assert_eq!(step["inputs"][0]["node"]["node"], json!(compare));
    // The branch holds the move, whose delta says where it comes from.
    let inside = &step["branches"][0]["steps"][0];
    assert!(
        inside["text"].as_str().expect("text").contains("Mover"),
        "{inside}"
    );
    assert_eq!(inside["inputs"][1]["text"], "dt (A cada quadro)");
    assert!(
        view["diagnostics"]
            .as_array()
            .expect("list")
            .iter()
            .all(|d| d["severity"] != "error"),
        "{view}"
    );
    assert!(compile(&document, &catalog).is_ok());
}

#[test]
fn loose_parts_and_missing_values_are_listed() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let _ = add_node(&mut document, &catalog, "api.log.info", None).expect("loose action");
    let _ = add_node(&mut document, &catalog, "op.not", None).expect("loose value");
    let view = outline(&document, &catalog);
    assert_eq!(view["loose"].as_array().expect("list").len(), 1);
    assert_eq!(view["loose_values"].as_array().expect("list").len(), 1);
    let loose_step = &view["loose"][0]["steps"][0];
    assert_eq!(loose_step["inputs"][0]["source"], "missing");
    assert!(!loose_step["problems"].as_array().expect("list").is_empty());
}

#[test]
fn sources_list_only_what_exists_when_the_node_runs() {
    let catalog = catalog();
    let mut document = GraphDocument::new();
    let start = add_node(&mut document, &catalog, "event.on_update", None).expect("event");
    let created = add_node(
        &mut document,
        &catalog,
        "api.world.create",
        Some(&pin(start, "then")),
    )
    .expect("create");
    let repeat = add_node(
        &mut document,
        &catalog,
        "flow.repeat",
        Some(&pin(created, "then")),
    )
    .expect("repeat");
    let inside = add_node(
        &mut document,
        &catalog,
        "api.transform.move_x",
        Some(&pin(repeat, "body")),
    )
    .expect("inside");
    let after = add_node(
        &mut document,
        &catalog,
        "api.transform.move_x",
        Some(&pin(repeat, "done")),
    )
    .expect("after");

    let labels = |document: &GraphDocument, node: u32| -> Vec<String> {
        sources(document, &catalog, node, "delta")
            .iter()
            .map(|source| {
                format!(
                    "{}.{}",
                    source["from"]["node"],
                    source["from"]["pin"].as_str().unwrap_or("")
                )
            })
            .collect()
    };
    // Inside the loop: dt of the event, the loop index (Int widens to decimal) and the result of
    // world.create (an Int too).
    let inside_sources = labels(&document, inside);
    assert!(
        inside_sources.contains(&format!("{start}.dt")),
        "{inside_sources:?}"
    );
    assert!(
        inside_sources.contains(&format!("{repeat}.index")),
        "{inside_sources:?}"
    );
    assert!(
        inside_sources.contains(&format!("{created}.result")),
        "{inside_sources:?}"
    );
    // After the loop the index is gone.
    let after_sources = labels(&document, after);
    assert!(
        !after_sources.contains(&format!("{repeat}.index")),
        "{after_sources:?}"
    );
    assert!(after_sources.contains(&format!("{start}.dt")));
    // A pure node asks the question for the chain node that reads it; none reads it yet.
    let position =
        add_node(&mut document, &catalog, "api.transform.position", None).expect("pure");
    assert!(sources(&document, &catalog, position, "entity").is_empty());
}

#[test]
fn search_ignores_accents_and_filters_by_what_is_wanted() {
    let catalog = catalog();
    let titles = |query: &str, want: Want| -> Vec<String> {
        search(&catalog, query, want)
            .iter()
            .map(|spec| spec.title.clone())
            .collect()
    };
    assert!(
        titles("posicao", Want::Any)
            .iter()
            .any(|t| t == "Posição do objeto")
    );
    assert!(titles("COMECAR", Want::Event).contains(&"Ao começar".to_string()));
    assert!(
        titles("", Want::Event)
            .iter()
            .all(|t| t == "Ao começar" || t == "A cada quadro" || t == "Função")
    );
    // For a list input only nodes that give a list.
    let for_list = titles("", Want::Value(DataType::List));
    assert!(
        for_list.contains(&"Listar colisões".to_string()),
        "{for_list:?}"
    );
    assert!(!for_list.contains(&"Posição do objeto".to_string()));
    // Several words, any order.
    assert!(titles("objeto posicao", Want::Any).contains(&"Posição do objeto".to_string()));
    assert!(titles("zzzz", Want::Any).is_empty());
}
