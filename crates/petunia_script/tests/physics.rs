//! Collision and physics as scripts see them: named colliders, contacts, rays and pushes.

// Test code: a failed expectation should stop the test with a message.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use petunia_commands::{CommandRecord, Dispatcher};
use petunia_project::register_standard_tables;
use petunia_script::{PhysicsSettings, ScriptHost};
use petunia_world::{PersistentId, Value, World};
use serde_json::{Value as Json, json};

fn create(world: &mut World, dispatcher: &mut Dispatcher, rows: Json) -> u64 {
    let created = dispatcher
        .execute(
            world,
            &CommandRecord::new("world.create", json!({ "rows": rows })),
        )
        .expect("create");
    created["entity"].as_u64().expect("entity id")
}

/// A floor (top at y = 0) and a ball with a script and two named colliders.
fn scene(script: &str) -> (World, u64) {
    let mut world = World::new(7);
    register_standard_tables(&mut world).expect("tables");
    let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
    let floor = create(
        &mut world,
        &mut dispatcher,
        json!({
            "Name": { "text": "Chão" },
            "Transform": { "position": { "x": 0.0, "y": -0.5, "z": 0.0 } }
        }),
    );
    create(
        &mut world,
        &mut dispatcher,
        json!({
            "Collider": { "owner": floor, "name": "chão", "size": { "x": 20.0, "y": 1.0, "z": 20.0 } }
        }),
    );
    let ball = create(
        &mut world,
        &mut dispatcher,
        json!({
            "Name": { "text": "Bola" },
            "Transform": { "position": { "x": 0.0, "y": 3.0, "z": 0.0 } },
            "RigidBody": { "kind": "dynamic", "mass": 1.0 },
            "Script": { "file": script, "enabled": true }
        }),
    );
    create(
        &mut world,
        &mut dispatcher,
        json!({
            "Collider": { "owner": ball, "name": "corpo", "shape": "sphere", "radius": 0.5 }
        }),
    );
    create(
        &mut world,
        &mut dispatcher,
        json!({
            "Collider": { "owner": ball, "name": "sensor", "shape": "sphere", "radius": 0.6, "trigger": true }
        }),
    );
    (world, ball)
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

const BALL: &str = r#"
fn on_start(entity) {
    log.info("colisões", collision.list(entity))
    let below = physics.raycast_from(entity, {"x": 0.0, "y": -1.0, "z": 0.0}, 50.0)
    log.info("raio", below["collider"])
    collision.set(entity, "corpo", "radius", 0.25)
    log.info("raio novo", collision.get(entity, "corpo")["radius"])
}

fn on_collision(entity, other, mine, theirs) {
    log.info("bateu", mine, theirs)
}

fn on_collision_end(entity, other, mine, theirs) {
    log.info("soltou", mine, theirs)
}
"#;

#[test]
fn a_ball_with_named_colliders_falls_hits_and_tells_its_script() {
    let (world, ball) = scene("bola");
    let mut host = ScriptHost::new(world).expect("host");
    host.load("bola", BALL).expect("compiles");
    let problems = host
        .attach_physics(PhysicsSettings::default(), BTreeMap::new())
        .expect("physics");
    assert!(problems.is_empty(), "{problems:?}");
    for _ in 0..240 {
        host.update(1.0 / 60.0);
    }
    assert!(host.take_faults().is_empty(), "no script failed");
    let log: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    let has = |words: &[&str]| {
        log.iter()
            .any(|line| words.iter().all(|word| line.contains(word)))
    };
    assert!(
        has(&["colisões", "corpo", "sensor"]),
        "collision.list: {log:?}"
    );
    assert!(has(&["raio", "chão"]), "the ray sees the floor: {log:?}");
    assert!(
        has(&["raio novo", "0.25"]),
        "collision.set changed the radius: {log:?}"
    );
    assert!(
        has(&["bateu", "corpo", "chão"]),
        "on_collision with both names: {log:?}"
    );
    assert!(
        has(&["bateu", "sensor", "chão"]),
        "the trigger reports too: {log:?}"
    );
    // The smaller radius (0.25) set by the script is the one resting on the floor.
    let height = height_of(&host, ball);
    assert!(
        (height - 0.25).abs() < 0.05,
        "rests on the new radius: {height}"
    );
}

const PUSHER: &str = r#"
fn on_start(entity) {
    physics.impulse(entity, {"x": 0.0, "y": 8.0, "z": 0.0})
}

fn on_update(entity, dt) {
    let v = physics.velocity(entity)
    if game.tick() == 1 {
        log.info("subindo", v["y"] > 0.0)
    }
}
"#;

#[test]
fn scripts_push_bodies_and_read_their_velocity() {
    let (world, ball) = scene("empurra");
    let mut host = ScriptHost::new(world).expect("host");
    host.load("empurra", PUSHER).expect("compiles");
    host.attach_physics(PhysicsSettings::default(), BTreeMap::new())
        .expect("physics");
    for _ in 0..3 {
        host.update(1.0 / 60.0);
    }
    assert!(host.take_faults().is_empty());
    let log: Vec<String> = host.take_log().into_iter().map(|line| line.text).collect();
    assert!(
        log.iter()
            .any(|line| line.contains("subindo") && line.contains("true")),
        "{log:?}"
    );
    assert!(height_of(&host, ball) > 3.0, "the push lifted the ball");
}

const NO_PHYSICS: &str = r#"
fn on_start(entity) {
    let hit = physics.raycast({"x": 0.0, "y": 5.0, "z": 0.0}, {"x": 0.0, "y": -1.0, "z": 0.0}, 10.0)
}
"#;

#[test]
fn without_physics_the_calls_fail_with_an_explanation() {
    let (world, _) = scene("sem");
    let mut host = ScriptHost::new(world).expect("host");
    host.load("sem", NO_PHYSICS).expect("compiles");
    host.update(1.0 / 60.0);
    let faults = host.take_faults();
    assert_eq!(
        faults.len(),
        1,
        "an unhandled failure is reported: {faults:?}"
    );
    assert!(
        faults[0].message.contains("física não está ligada"),
        "{faults:?}"
    );
}

#[test]
fn the_same_game_gives_the_same_fall() {
    let run = || {
        let (world, ball) = scene("bola");
        let mut host = ScriptHost::new(world).expect("host");
        host.load("bola", BALL).expect("compiles");
        host.attach_physics(PhysicsSettings::default(), BTreeMap::new())
            .expect("physics");
        for _ in 0..90 {
            host.update(1.0 / 60.0);
        }
        height_of(&host, ball).to_bits()
    };
    assert_eq!(run(), run());
}
