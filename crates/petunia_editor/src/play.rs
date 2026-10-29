//! Play mode (DR-28): the scene runs on a **copy** of the world with the project's scripts, the
//! physics and the player's character. Stopping throws the copy away, so playing never changes
//! what you built. While it runs, nothing that edits the scene is accepted.

use petunia_render::{FlyCamera, extract_frame};
use petunia_script::{PhysicsSettings, PlayerInput, ScriptHost};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

/// Longest frame the game accepts, so a pause in the window never throws things across the map.
const MAX_FRAME_SECONDS: f32 = 0.1;
/// Third-person camera: how far behind the player and how high above its position.
const CAMERA_DISTANCE: f32 = 4.5;
const CAMERA_HEIGHT: f32 = 1.5;

/// The running game.
pub(crate) struct PlayState {
    pub(crate) host: ScriptHost,
    saved_camera: FlyCamera,
    /// Whether the world has a character the keyboard drives.
    has_player: bool,
}

impl EditorSession {
    pub fn is_playing(&self) -> bool {
        self.play.is_some()
    }

    /// Refuses edits while the game runs.
    pub(crate) fn ensure_not_playing(&self) -> Result<(), Failure> {
        if self.is_playing() {
            return Err(Failure::new(
                "PLAY_ACTIVE",
                "o jogo está rodando",
                "Pare o jogo (Esc ou o botão Parar) antes de editar a cena.",
            ));
        }
        Ok(())
    }

    /// `play.start`: runs the scene on a copy of the world.
    pub fn play_start(&mut self) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        let world = self.world()?.clone();
        let mut host = ScriptHost::new(world).map_err(|message| {
            Failure::new(
                "PLAY_HOST",
                &format!("não foi possível iniciar o jogo: {message}"),
                "Reabra o projeto.",
            )
        })?;
        // Someone to play, on the copy only: a stand-in hero at the PlayerStart if there is none.
        let made_hero = petunia_project::ensure_player(&mut host.world_mut())
            .map_err(|error| {
                Failure::new(
                    "PLAY_HOST",
                    &format!("não foi possível criar o jogador: {error}"),
                    "Reabra o projeto.",
                )
            })?
            .is_some();
        let mut problems: Vec<String> = Vec::new();
        // Play mode reads the marks the graphs write (N5): which node ran and why a condition
        // failed. Cheap and bounded, so it is always on while playing.
        host.set_debug_marks(true);
        let mut loaded = Vec::new();
        for name in self.script_names() {
            let Ok(text) = self.read_script_file(&name) else {
                continue;
            };
            match host.load(&name, &text) {
                Ok(_) => loaded.push(name),
                Err(diagnostics) => {
                    for diagnostic in diagnostics
                        .iter()
                        .filter(|diagnostic| diagnostic.is_error())
                    {
                        problems.push(format!(
                            "script `{name}` (linha {}): {}",
                            diagnostic.line, diagnostic.message
                        ));
                    }
                }
            }
        }
        for name in self.graph_names() {
            let Ok(text) = self.read_graph_file(&name) else {
                continue;
            };
            match host.load_graph(&name, &text) {
                Ok(_) => loaded.push(name),
                Err(diagnostics) => {
                    for diagnostic in diagnostics
                        .iter()
                        .filter(|diagnostic| diagnostic.is_error())
                    {
                        problems.push(format!("grafo `{name}`: {}", diagnostic.message));
                    }
                }
            }
        }
        // Everything drawn is solid unless it has colliders of its own.
        let physical = true;
        {
            let solids = petunia_render::solid_shapes(&host.world());
            match host.attach_physics(PhysicsSettings::default(), solids) {
                Ok(found) => problems.extend(
                    found
                        .into_iter()
                        .map(|problem| format!("física: {problem}")),
                ),
                Err(message) => problems.push(format!("física desligada: {message}")),
            }
        }
        let has_player = host.player_position().is_some();
        let saved_camera = self.camera;
        self.play = Some(Box::new(PlayState {
            host,
            saved_camera,
            has_player,
        }));
        self.follow_player();
        Ok(json!({
            "playing": true,
            "scripts": loaded,
            "has_player": has_player,
            "stand_in_player": made_hero,
            "physics": physical,
            "problems": problems,
        }))
    }

    /// `play.stop`: throws the copy away and gives the editor camera back.
    pub fn play_stop(&mut self) -> Result<Json, Failure> {
        let Some(state) = self.play.take() else {
            return Ok(json!({ "playing": false }));
        };
        self.camera = state.saved_camera;
        Ok(json!({ "playing": false }))
    }

    /// `play.tick`: one frame. `forward` and `right` go from -1 to 1; `look_x` and `look_y` are
    /// mouse movement in pixels. Returns what scripts wrote to the console and what failed.
    pub fn play_tick(
        &mut self,
        seconds: f32,
        forward: f32,
        right: f32,
        jump: bool,
        look: (f32, f32),
    ) -> Result<Json, Failure> {
        if self.play.is_none() {
            return Err(Failure::new(
                "PLAY_NOT_ACTIVE",
                "o jogo não está rodando",
                "Aperte Jogar.",
            ));
        }
        let seconds = if seconds.is_finite() {
            seconds.clamp(0.0, MAX_FRAME_SECONDS)
        } else {
            0.0
        };
        self.camera.look(look.0, look.1);
        // The camera turns the opposite way around the vertical axis from a Transform.
        let yaw_degrees = -self.camera.camera.yaw_degrees;
        let (log, faults, debug) = {
            let Some(state) = self.play.as_mut() else {
                return Ok(Json::Null);
            };
            state.host.set_player_input(PlayerInput {
                forward,
                right,
                jump,
                yaw_degrees,
            });
            state.host.update(f64::from(seconds));
            let log: Vec<Json> = state
                .host
                .take_log()
                .into_iter()
                .map(|line| json!({ "level": line.level.name(), "tick": line.tick, "text": line.text }))
                .collect();
            let faults: Vec<Json> = state
                .host
                .take_faults()
                .into_iter()
                .map(|fault| json!({ "script": fault.script, "function": fault.function, "code": fault.code, "message": fault.message }))
                .collect();
            (log, faults, state.host.debug_snapshot())
        };
        self.follow_player();
        Ok(json!({ "log": log, "faults": faults, "debug": debug }))
    }

    /// `play.debug`: what ran in the newest frame, for the canvas highlight and "por que esta
    /// condição falhou?". Cheap enough to ask every frame while playing.
    pub fn play_debug(&self) -> Result<Json, Failure> {
        let Some(state) = self.play.as_ref() else {
            return Ok(json!({ "playing": false, "tick": 0, "marks": [] }));
        };
        let mut snapshot = state.host.debug_snapshot();
        if let Some(object) = snapshot.as_object_mut() {
            object.insert("playing".to_string(), json!(true));
        }
        Ok(snapshot)
    }

    /// Puts the camera behind the player (when there is one), looking where the mouse looks.
    fn follow_player(&mut self) {
        let Some(state) = self.play.as_ref() else {
            return;
        };
        if !state.has_player {
            return;
        }
        let forward = self.camera.camera.forward();
        if let Some(position) =
            state
                .host
                .third_person_camera(forward, CAMERA_DISTANCE, CAMERA_HEIGHT)
        {
            self.camera.camera.position = position;
        }
    }

    /// What to draw while the game runs: the copy of the world, with nothing selected.
    pub(crate) fn play_frame(&self) -> Option<petunia_render::RenderFrame> {
        let state = self.play.as_ref()?;
        let world = state.host.world();
        let mut frame = extract_frame(&world, self.camera.camera);
        frame.sculpts = self.sculpt_items();
        frame.show_grid = false;
        Some(frame)
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

    #[test]
    fn playing_runs_a_copy_with_a_walking_player_and_stopping_leaves_the_scene_untouched() {
        let root =
            std::env::temp_dir().join(format!("petunia_editor_test_play_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "jogar" }),
        );

        // A 40 m floor (the sample's floor is #1) and a hero made of a capsule collider.
        call(&mut session, json!({ "op": "selection.set", "entity": 1 }));
        call(
            &mut session,
            json!({ "op": "collider.add", "shape": "box" }),
        );
        let hero = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
            "Name": { "text": "Herói" },
            "Transform": { "position": { "x": 0.0, "y": 1.2, "z": 0.0 } },
            "Brush": { "shape": "cylinder", "size": { "x": 0.6, "y": 1.8, "z": 0.6 } },
            "Character": { "player": true }
        } } }),
        )["entity"]
            .as_u64()
            .expect("hero id");
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": hero }),
        );
        call(
            &mut session,
            json!({ "op": "collider.add", "shape": "capsule" }),
        );
        let before = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"].clone();

        // Not playing: ticking is an error, and edits are fine.
        let early = session.handle(&json!({ "op": "play.tick", "seconds": 0.016 }));
        assert_eq!(early["ok"], false);

        let started = call(&mut session, json!({ "op": "play.start" }));
        assert_eq!(started["has_player"], true);
        assert_eq!(started["physics"], true);
        assert!(
            started["problems"].as_array().expect("list").is_empty(),
            "{started}"
        );
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["playing"],
            true
        );

        // Edits are refused while playing.
        let refused = session.handle(&json!({ "op": "edit.delete" }));
        assert_eq!(refused["ok"], false);
        assert_eq!(refused["error"]["code"], "PLAY_ACTIVE");
        assert_eq!(
            session.handle(&json!({ "op": "play.start" }))["ok"],
            false,
            "already playing"
        );

        // Walk forward for a second with the camera looking along -Z.
        for _ in 0..60 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0, "forward": 1.0 }),
            );
        }
        let frame = session.frame().expect("a frame while playing");
        let moved = frame
            .items
            .iter()
            .any(|item| item.entity == hero && item.transform.position.z < -2.0);
        assert!(
            moved,
            "the hero walked forward (toward -Z): {:?}",
            frame
                .items
                .iter()
                .find(|item| item.entity == hero)
                .map(|item| item.transform.position)
        );
        assert!(!frame.show_grid, "no editing grid while playing");

        // Stop: the scene is exactly as it was, and the hero is where it was built.
        call(&mut session, json!({ "op": "play.stop" }));
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["playing"],
            false
        );
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"],
            before
        );
        let frame = session.frame().expect("a frame");
        let hero_item = frame
            .items
            .iter()
            .find(|item| item.entity == hero)
            .expect("hero is drawn");
        assert!(
            (hero_item.transform.position.z).abs() < 1e-4,
            "the built scene did not change"
        );
        call(&mut session, json!({ "op": "edit.undo" }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_node_graph_in_the_scripts_folder_runs_when_playing() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_play_graph_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "grafo" }),
        );
        // on_update: turn the object by 90 degrees per second (dt * 90).
        let graph = json!({
            "schema_version": 1,
            "nodes": [
                { "id": 1, "kind": "event.on_update" },
                { "id": 2, "kind": "op.math", "params": { "operator": "*", "b": 90.0 } },
                { "id": 3, "kind": "api.transform.rotate_y" }
            ],
            "edges": [
                { "from": { "node": 1, "pin": "then" }, "to": { "node": 3, "pin": "in" } },
                { "from": { "node": 1, "pin": "dt" }, "to": { "node": 2, "pin": "a" } },
                { "from": { "node": 2, "pin": "result" }, "to": { "node": 3, "pin": "degrees" } }
            ]
        });
        std::fs::create_dir_all(root.join("scripts")).expect("scripts folder");
        std::fs::write(root.join("scripts/gira.graph.json"), graph.to_string()).expect("graph");
        let spinner = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Name": { "text": "Giratório" },
                "Transform": { "position": { "x": 0.0, "y": 3.0, "z": 0.0 } },
                "Brush": { "shape": "box", "size": { "x": 1.0, "y": 1.0, "z": 1.0 } },
                "Script": { "file": "gira", "enabled": true }
            } } }),
        )["entity"]
            .as_u64()
            .expect("id");

        let started = call(&mut session, json!({ "op": "play.start" }));
        assert!(
            started["scripts"]
                .as_array()
                .expect("list")
                .iter()
                .any(|name| name == "gira"),
            "{started}"
        );
        assert!(
            started["problems"].as_array().expect("list").is_empty(),
            "{started}"
        );
        for _ in 0..60 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
            );
        }
        let frame = session.frame().expect("a frame while playing");
        let turned = frame
            .items
            .iter()
            .find(|item| item.entity == spinner)
            .map(|item| item.transform.rotation_degrees.y)
            .expect("the object is drawn");
        assert!(
            (turned.abs() - 90.0).abs() < 1.0,
            "turned {turned} degrees in a second"
        );
        call(&mut session, json!({ "op": "play.stop" }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_drawn_character_without_a_collider_is_solid_and_walks() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_auto_character_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "auto" }),
        );
        let hero = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Name": { "text": "Jogador" },
                "Transform": { "position": { "x": 0.0, "y": 1.0, "z": 0.0 } },
                "Brush": { "shape": "cylinder", "size": { "x": 0.6, "y": 1.8, "z": 0.6 } },
                "Character": { "player": true }
            } } }),
        )["entity"]
            .as_u64()
            .expect("hero");
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert_eq!(started["has_player"], true, "{started}");
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
        let resting = session
            .frame()
            .expect("frame")
            .items
            .into_iter()
            .find(|item| item.entity == hero)
            .map(|item| item.transform.position)
            .expect("hero is drawn");
        assert!(
            resting.y > 0.5 && resting.y < 1.5,
            "the automatic capsule keeps the hero on the floor: {resting:?}"
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
            .find(|item| item.entity == hero)
            .map(|item| item.transform.position)
            .expect("hero is drawn");
        assert!(
            walked.z < resting.z - 1.0,
            "the hero walks: {resting:?} -> {walked:?}"
        );
        call(&mut session, json!({ "op": "play.stop" }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_character_with_a_dynamic_body_is_corrected_to_kinematic() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_dynamic_character_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "dynamic" }),
        );
        let hero = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Name": { "text": "Jogador" },
                "Transform": { "position": { "x": 0.0, "y": 1.0, "z": 0.0 } },
                "Brush": { "shape": "cylinder", "size": { "x": 0.6, "y": 1.8, "z": 0.6 } },
                "Character": { "player": true },
                "RigidBody": { "kind": "dynamic" }
            } } }),
        )["entity"]
            .as_u64()
            .expect("hero");
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert_eq!(started["has_player"], true, "{started}");
        assert!(
            started["problems"]
                .as_array()
                .expect("list")
                .iter()
                .any(|problem| problem
                    .as_str()
                    .is_some_and(|text| text.contains("usei cinemático"))),
            "the correction is reported: {started}"
        );
        for _ in 0..90 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
            );
        }
        let position = session
            .frame()
            .expect("frame")
            .items
            .into_iter()
            .find(|item| item.entity == hero)
            .map(|item| item.transform.position)
            .expect("hero is drawn");
        assert!(
            position.y > 0.5 && position.y < 1.5,
            "the corrected character stands: {position:?}"
        );
        call(&mut session, json!({ "op": "play.stop" }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_character_with_only_a_trigger_collider_gets_the_walking_capsule() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_trigger_character_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "trigger" }),
        );
        let hero = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Name": { "text": "Jogador" },
                "Transform": { "position": { "x": 0.0, "y": 1.0, "z": 0.0 } },
                "Brush": { "shape": "cylinder", "size": { "x": 0.6, "y": 1.8, "z": 0.6 } },
                "Character": { "player": true }
            } } }),
        )["entity"]
            .as_u64()
            .expect("hero");
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Collider": { "owner": hero, "name": "gatilho", "shape": "sphere", "radius": 1.0, "trigger": true }
            } } }),
        );
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert_eq!(started["has_player"], true, "{started}");
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
        let position = session
            .frame()
            .expect("frame")
            .items
            .into_iter()
            .find(|item| item.entity == hero)
            .map(|item| item.transform.position)
            .expect("hero is drawn");
        assert!(
            position.y > 0.5 && position.y < 1.5,
            "the automatic capsule stands: {position:?}"
        );
        call(&mut session, json!({ "op": "play.stop" }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_sample_room_is_walkable_without_any_setup() {
        let root =
            std::env::temp_dir().join(format!("petunia_editor_test_walk_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "sala" }),
        );
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert_eq!(
            started["stand_in_player"], true,
            "no hero was built: one is made for the copy"
        );
        assert!(
            started["problems"].as_array().expect("list").is_empty(),
            "{started}"
        );

        // The camera starts behind the hero, in front of the south wall (#3), never inside it.
        let first = session.frame().expect("frame");
        let south = first
            .items
            .iter()
            .find(|item| item.entity == 3)
            .expect("south wall");
        let south_inner = south.transform.position.z - south.size.z * 0.5;
        assert!(
            first.camera.position.z < south_inner,
            "camera {:?} is behind the south wall (inner face {south_inner})",
            first.camera.position
        );

        // The north wall (#2): where its inner face is.
        let north = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": 2 }),
        )
        .to_string();
        let wall = session
            .frame()
            .expect("frame")
            .items
            .into_iter()
            .find(|item| item.entity == 2)
            .expect("wall");
        let inner_face = wall.transform.position.z + wall.size.z * 0.5;
        assert!(north.contains("Parede"), "sanity: {north}");

        // Stand still for a second: the hero must rest on the floor, not fall through it.
        for _ in 0..60 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
            );
        }
        let hero_y = |session: &EditorSession| {
            session
                .frame()
                .expect("frame")
                .items
                .iter()
                .find(|item| {
                    item.transform.position.y > -50.0 && item.size.y > 1.7 && item.size.y < 1.9
                })
                .map(|item| item.transform.position)
        };
        let standing = hero_y(&session).expect("the stand-in hero is drawn");
        assert!(
            standing.y > 0.5 && standing.y < 1.5,
            "stands on the floor: {standing:?}"
        );

        // Walk forward (toward the north wall) for 8 s: it stops at the wall, never inside it.
        for _ in 0..480 {
            call(
                &mut session,
                json!({ "op": "play.tick", "seconds": 1.0 / 60.0, "forward": 1.0 }),
            );
        }
        let end = hero_y(&session).expect("hero");
        assert!(
            end.z < standing.z - 5.0,
            "walked across the room: {standing:?} -> {end:?}"
        );
        assert!(
            end.z - 0.3 >= inner_face - 0.06,
            "stopped at the wall (inner face at {inner_face}): {end:?}"
        );
        assert!(end.y > 0.5 && end.y < 1.5, "still on the floor: {end:?}");
        call(&mut session, json!({ "op": "play.stop" }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn playing_reports_which_nodes_ran_and_why_a_condition_was_false() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_play_debug_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "depurar" }),
        );
        call(&mut session, json!({ "op": "graph.new", "name": "cond" }));
        let event = call(
            &mut session,
            json!({ "op": "graph.add", "kind": "event.on_update" }),
        )["node"]
            .as_u64()
            .expect("event");
        let decide = call(
            &mut session,
            json!({ "op": "graph.add", "kind": "flow.if", "after": { "node": event, "pin": "then" } }),
        )["node"]
            .as_u64()
            .expect("if");
        let compare = call(
            &mut session,
            json!({ "op": "graph.add", "kind": "op.compare", "into": { "node": decide, "pin": "condition" } }),
        )["node"]
            .as_u64()
            .expect("compare");
        for (name, value) in [
            ("operator", json!(">")),
            ("a", json!(1.0)),
            ("b", json!(2.0)),
        ] {
            call(
                &mut session,
                json!({ "op": "graph.set", "node": compare, "name": name, "value": value }),
            );
        }
        let inside = call(
            &mut session,
            json!({ "op": "graph.add", "kind": "api.log.info", "after": { "node": decide, "pin": "then" } }),
        )["node"]
            .as_u64()
            .expect("log");
        call(
            &mut session,
            json!({ "op": "graph.set", "node": inside, "name": "message", "value": "entrou" }),
        );

        let runner = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Name": { "text": "Alvo" },
                "Transform": { "position": { "x": 0.0, "y": 1.0, "z": 0.0 } },
                "Brush": { "shape": "box", "size": { "x": 1.0, "y": 1.0, "z": 1.0 } }
            } } }),
        )["entity"]
            .as_u64()
            .expect("entity");
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": runner }),
        );
        call(
            &mut session,
            json!({ "op": "script.attach", "name": "cond" }),
        );
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert!(
            started["problems"].as_array().expect("list").is_empty(),
            "{started}"
        );
        call(
            &mut session,
            json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
        );
        let debug = call(&mut session, json!({ "op": "play.debug" }));
        assert_eq!(debug["playing"], true, "{debug}");
        let marks = debug["marks"].as_array().expect("marks").clone();
        assert!(
            marks.iter().any(|mark| mark["node"] == decide
                && mark["kind"] == "check"
                && mark["value"] == false),
            "the If decided false: {debug}"
        );
        assert!(
            marks
                .iter()
                .any(|mark| mark["node"] == compare && mark["kind"] == "value"),
            "the values the condition used: {debug}"
        );
        assert!(
            !marks
                .iter()
                .any(|mark| mark["node"] == inside && mark["kind"] == "ran"),
            "the 'then' branch did not run: {debug}"
        );
        call(&mut session, json!({ "op": "play.stop" }));
        assert_eq!(
            call(&mut session, json!({ "op": "play.debug" }))["playing"],
            false
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
