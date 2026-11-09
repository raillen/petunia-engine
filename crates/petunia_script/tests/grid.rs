//! `grid.*` through a real script: the values a game actually handles.
//!
//! Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use petunia_commands::{CommandRecord, Dispatcher};
use petunia_project::register_standard_tables;
use petunia_script::ScriptHost;
use petunia_world::World;
use serde_json::json;

fn host_with(script: &str) -> ScriptHost {
    let mut world = World::new(11);
    register_standard_tables(&mut world).expect("tables");
    let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
    dispatcher
        .execute(
            &mut world,
            &CommandRecord::new(
                "world.create",
                json!({ "rows": {
                    "Name": { "text": "Alvo" },
                    "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 0.0 } },
                    "Script": { "file": "grade", "enabled": true }
                }}),
            ),
        )
        .expect("create");
    let mut host = ScriptHost::new(world).expect("host");
    host.load("grade", script).expect("compiles");
    host
}

fn lines(host: &mut ScriptHost) -> Vec<String> {
    host.take_log().into_iter().map(|line| line.text).collect()
}

#[test]
fn a_hex_grid_answers_distance_neighbors_range_and_path() {
    let mut host = host_with(
        r#"
fn on_start(entity) {
    let grade = grid.hex(2.0)
    log.info("distancia", grid.distance(grade, { "q": 0, "r": 0 }, { "q": 3, "r": -1 }))
    log.info("vizinhas", len(grid.neighbors(grade, { "q": 0, "r": 0 })))
    log.info("alcance", len(grid.range(grade, { "q": 0, "r": 0 }, 2)))
    attempt {
        let caminho = grid.path(grade, { "q": 0, "r": 0 }, { "q": 2, "r": 0 }, [{ "q": 1, "r": 0 }])?
        log.info("passos", len(caminho))
    } failed erro {
        log.info("erro", erro)
    }
}
"#,
    );
    host.update(0.016);
    assert!(host.take_faults().is_empty(), "{:?}", host.take_faults());
    let lines = lines(&mut host);
    assert!(lines.contains(&"distancia 3".to_string()), "{lines:?}");
    assert!(lines.contains(&"vizinhas 6".to_string()), "{lines:?}");
    assert!(lines.contains(&"alcance 19".to_string()), "{lines:?}");
    // A blocked first step: the path goes around it, three steps plus the start.
    assert!(lines.contains(&"passos 4".to_string()), "{lines:?}");
}

#[test]
fn a_blocked_goal_gives_none_and_a_world_point_becomes_a_cell() {
    let mut host = host_with(
        r#"
fn on_start(entity) {
    let grade = grid.square(1.0)
    let caminho = grid.path(grade, { "q": 0, "r": 0 }, { "q": 1, "r": 0 }, [{ "q": 1, "r": 0 }])
    log.info("sem caminho", caminho == none)
    let celula = grid.from_world(grade, { "x": 2.4, "y": 0.0, "z": -1.2 })
    log.info("celula", celula["q"], celula["r"])
    let metros = grid.to_world(grade, { "q": 2, "r": -1 })
    log.info("metros", metros.x, metros.z)
}
"#,
    );
    host.update(0.016);
    let faults = host.take_faults();
    assert!(faults.is_empty(), "{faults:?}");
    let lines = lines(&mut host);
    assert!(lines.contains(&"sem caminho true".to_string()), "{lines:?}");
    assert!(lines.contains(&"celula 2 -1".to_string()), "{lines:?}");
    assert!(lines.contains(&"metros 2.0 -1.0".to_string()), "{lines:?}");
}

#[test]
fn a_bad_grid_or_cell_is_a_handled_failure_not_a_crash() {
    let mut host = host_with(
        r#"
fn on_start(entity) {
    attempt {
        let ruim = grid.square(0.0)?
        log.info("não devia passar", ruim)
    } failed erro {
        log.info("tamanho recusado")
    }
    let grade = grid.square(1.0)
    attempt {
        let fora = grid.to_world(grade, { "q": 2000000, "r": 0 })?
        log.info("não devia passar", fora)
    } failed erro {
        log.info("celula recusada")
    }
}
"#,
    );
    host.update(0.016);
    assert!(host.take_faults().is_empty(), "{:?}", host.take_faults());
    let lines = lines(&mut host);
    assert!(lines.contains(&"tamanho recusado".to_string()), "{lines:?}");
    assert!(lines.contains(&"celula recusada".to_string()), "{lines:?}");
}
