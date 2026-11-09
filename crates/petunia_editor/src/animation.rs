//! Playing an imported character's clip in the viewport (phase 4).
//!
//! The engine can already compute a pose and skin a mesh; what was missing was the **link** between
//! an object in the scene and the skeleton and clip saved with it. Three commands close that gap:
//!
//! - `anim.list`: the skeletons and clips of the project.
//! - `anim.play`: one object plays a clip, from a start time, at a speed.
//! - `anim.stop`: the object goes back to its rest pose.
//!
//! A playing clip advances with the editor clock, which the window sends (`anim.advance`). The
//! engine never reads the wall clock, so the same project and the same times give the same pose —
//! the determinism rule from `AGENTS.md` §2.

use petunia_anim::{Clip, Skeleton};
use petunia_world::Value;
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

/// One object playing a clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Playing {
    pub(crate) entity: u64,
    /// The clip row in the `Clip` table.
    pub(crate) clip: u64,
    /// Seconds into the clip.
    pub(crate) time: f32,
    /// How much clip time passes per second of editor time.
    pub(crate) speed: f32,
    pub(crate) looping: bool,
}

impl EditorSession {
    /// `anim.list`: the skeletons and clips that exist in the project.
    pub fn animation_list(&self) -> Result<Json, Failure> {
        let world = self.world()?;
        let mut skeletons = Vec::new();
        for handle in world.entities_with_table("Skeleton").unwrap_or_default() {
            let Ok(id) = world.persistent_id(handle) else {
                continue;
            };
            let joints = match world.get(handle, "Skeleton", "joint_count").ok() {
                Some(Value::Int(count)) => *count,
                _ => 0,
            };
            skeletons.push(json!({ "id": id.0, "joints": joints, "name": name_of(world, handle) }));
        }
        let mut clips = Vec::new();
        for handle in world.entities_with_table("Clip").unwrap_or_default() {
            let Ok(id) = world.persistent_id(handle) else {
                continue;
            };
            let duration = match world.get(handle, "Clip", "duration").ok() {
                Some(Value::Float(seconds)) => *seconds,
                _ => 0.0,
            };
            clips.push(json!({
                "id": id.0,
                "name": name_of(world, handle),
                "duration": duration,
            }));
        }
        Ok(json!({ "skeletons": skeletons, "clips": clips }))
    }

    /// `anim.play`: `entity` plays `clip` from `start`, at `speed`.
    pub fn animation_play(
        &mut self,
        entity: u64,
        clip: u64,
        start: f32,
        speed: f32,
        looping: bool,
    ) -> Result<Json, Failure> {
        let world = self.world()?;
        world
            .handle_of(petunia_world::PersistentId(entity))
            .map_err(|_| not_found(entity))?;
        let clip_handle = world
            .handle_of(petunia_world::PersistentId(clip))
            .map_err(|_| {
                Failure::new(
                    "ANIMATION_NO_CLIP",
                    &format!("o clipe {clip} não existe"),
                    "Escolha um clipe da lista (`anim.list`).",
                )
            })?;
        if world.get(clip_handle, "Clip", "channels").is_err() {
            return Err(Failure::new(
                "ANIMATION_NO_CLIP",
                &format!("a entidade {clip} não é um clipe"),
                "Escolha um clipe da lista (`anim.list`).",
            ));
        }
        if !start.is_finite() || !speed.is_finite() {
            return Err(Failure::new(
                "EDITOR_BAD_NUMBER",
                "o tempo ou a velocidade não é um número finito",
                "Use um número comum, como 0 ou 1.",
            ));
        }
        let playing = Playing {
            entity,
            clip,
            time: start.max(0.0),
            speed: speed.clamp(0.0, 10.0),
            looping,
        };
        self.playing.retain(|other| other.entity != entity);
        self.playing.push(playing);
        Ok(json!({
            "entity": entity,
            "clip": clip,
            "time": playing.time,
            "speed": playing.speed,
        }))
    }

    /// `anim.stop`: the object goes back to its rest pose.
    pub fn animation_stop(&mut self, entity: Option<u64>) -> Result<Json, Failure> {
        match entity {
            Some(entity) => self.playing.retain(|other| other.entity != entity),
            None => self.playing.clear(),
        }
        Ok(json!({ "playing": self.playing.len() }))
    }

    /// `anim.advance`: moves every playing clip forward by `seconds` of editor time.
    pub fn animation_advance(&mut self, seconds: f32) -> Result<Json, Failure> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Ok(json!({ "advanced": 0 }));
        }
        // The same clock drives the rain, so a scene with weather and nothing playing still moves.
        self.advance_precipitation(seconds);
        let mut finished = Vec::new();
        let clips: Vec<(u64, f32)> = self
            .playing
            .iter()
            .map(|playing| {
                (
                    playing.clip,
                    self.clip_duration(playing.clip).unwrap_or(0.0),
                )
            })
            .collect();
        for playing in &mut self.playing {
            playing.time += seconds * playing.speed;
            if let Some(duration) = clips
                .iter()
                .find(|(clip, _)| *clip == playing.clip)
                .map(|(_, duration)| *duration)
                .filter(|duration| *duration > 0.0)
                && playing.time > duration
            {
                if playing.looping {
                    playing.time %= duration;
                } else {
                    playing.time = duration;
                    finished.push(playing.entity);
                }
            }
        }
        self.playing
            .retain(|playing| !finished.contains(&playing.entity));
        Ok(json!({ "playing": self.playing.len(), "finished": finished }))
    }

    fn clip_duration(&self, clip: u64) -> Option<f32> {
        let world = self.world().ok()?;
        let handle = world.handle_of(petunia_world::PersistentId(clip)).ok()?;
        match world.get(handle, "Clip", "duration").ok()? {
            Value::Float(seconds) if seconds.is_finite() => Some(*seconds as f32),
            _ => None,
        }
    }

    /// Fills one draw item's skin from the tables, at the pose of its playing clip.
    ///
    /// Called by the frame builder for every object. An object with no `Skin` row is left alone, so
    /// an ordinary mesh costs nothing here.
    pub(crate) fn apply_skin_to(&self, item: &mut petunia_render::DrawItem) {
        let Ok(world) = self.world() else {
            return;
        };
        // The skin is its own row (`owner` = the object), like a collider: the mesh entity does not
        // carry the weights itself.
        let Some(weights_text) = skin_weights_of(world, item.entity) else {
            return;
        };
        let Some((skeleton_row, weights)) = crate::rig::skin_from_text(&weights_text) else {
            return;
        };
        let Some(skeleton) = self.read_skeleton(skeleton_row) else {
            return;
        };
        // The pose: the playing clip if there is one, the first key otherwise (rest is the bind
        // pose, which is what a character with animation but no playback should show).
        let (matrices, _) = match self.playing_for(item.entity) {
            Some(playing) => match self.read_clip(playing.clip) {
                Some(clip) => (
                    skeleton.skinning_matrices(&clip, playing.time, 1.0),
                    Some(playing.time),
                ),
                None => (skeleton.skinning_matrices(&empty_clip(), 0.0, 0.0), None),
            },
            None => match self.first_clip() {
                Some(clip) => (skeleton.skinning_matrices(&clip, 0.0, 1.0), Some(0.0)),
                None => (skeleton.skinning_matrices(&empty_clip(), 0.0, 0.0), None),
            },
        };
        item.skin = weights;
        item.skin_matrices = matrices;
    }

    fn read_skeleton(&self, row: u64) -> Option<Skeleton> {
        let world = self.world().ok()?;
        let handle = world.handle_of(petunia_world::PersistentId(row)).ok()?;
        let Value::Text(text) = world.get(handle, "Skeleton", "joints").ok()? else {
            return None;
        };
        crate::rig::skeleton_from_text(text)
    }

    fn read_clip(&self, row: u64) -> Option<Clip> {
        let world = self.world().ok()?;
        let handle = world.handle_of(petunia_world::PersistentId(row)).ok()?;
        let Value::Text(text) = world.get(handle, "Clip", "channels").ok()? else {
            return None;
        };
        let name = name_of(world, handle);
        crate::rig::clip_from_text(&name, text)
    }

    fn first_clip(&self) -> Option<Clip> {
        let world = self.world().ok()?;
        let handle = world.entities_with_table("Clip").ok()?.into_iter().next()?;
        let id = world.persistent_id(handle).ok()?.0;
        self.read_clip(id)
    }

    fn playing_for(&self, entity: u64) -> Option<Playing> {
        self.playing
            .iter()
            .find(|playing| playing.entity == entity)
            .copied()
    }
}

/// The weights text of the skin row that belongs to `entity`, if it has one.
fn skin_weights_of(world: &petunia_world::World, entity: u64) -> Option<String> {
    for handle in world.entities_with_table("Skin").ok()? {
        let Ok(Value::Int(owner)) = world.get(handle, "Skin", "owner") else {
            continue;
        };
        if *owner < 0 || *owner as u64 != entity {
            continue;
        }
        if let Ok(Value::Text(text)) = world.get(handle, "Skin", "weights") {
            return Some(text.clone());
        }
    }
    None
}

fn empty_clip() -> Clip {
    Clip {
        name: String::new(),
        channels: Vec::new(),
    }
}

fn name_of(world: &petunia_world::World, handle: petunia_world::EntityHandle) -> String {
    match world.get(handle, "Name", "text") {
        Ok(Value::Text(text)) => text.clone(),
        _ => String::new(),
    }
}

fn not_found(entity: u64) -> Failure {
    Failure::new(
        "EDITOR_NO_ENTITY",
        &format!("o objeto {entity} não existe"),
        "Escolha um objeto na Hierarquia.",
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{Value as Json, json};

    use crate::rig::{clip_to_text, skeleton_to_text, skin_to_text};
    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    /// A project with one object that carries a skin, a skeleton and a clip — the shape the importer
    /// produces, written directly so this test does not need a GLB.
    #[allow(clippy::too_many_lines)]
    fn rigged(label: &str) -> (EditorSession, std::path::PathBuf, u64, u64) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_anim_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": label }),
        );
        // A skeleton with two joints: the root at the origin, the child one meter up.
        let skeleton = petunia_anim::Skeleton::new(vec![
            petunia_anim::Joint {
                parent: None,
                rest: petunia_anim::Mat4::translation(petunia_math::Vec3::ZERO),
                inverse_bind: petunia_anim::Mat4::IDENTITY,
            },
            petunia_anim::Joint {
                parent: Some(0),
                rest: petunia_anim::Mat4::translation(petunia_math::Vec3::new(0.0, 1.0, 0.0)),
                inverse_bind: petunia_anim::Mat4::translation(petunia_math::Vec3::new(
                    0.0, -1.0, 0.0,
                )),
            },
        ])
        .expect("skeleton");
        let clip = petunia_anim::Clip {
            name: "anda".to_string(),
            channels: vec![petunia_anim::Channel {
                joint: 0,
                values: petunia_anim::ChannelValues::Translation(vec![
                    petunia_anim::Vec3Key {
                        time: 0.0,
                        value: petunia_math::Vec3::ZERO,
                    },
                    petunia_anim::Vec3Key {
                        time: 1.0,
                        value: petunia_math::Vec3::new(4.0, 0.0, 0.0),
                    },
                ]),
                interpolation: petunia_anim::Interpolation::Linear,
            }],
        };
        let rig = json!({ "records": [
            { "command": "world.create", "payload": { "rows": {
                "Name": { "text": "esqueleto" },
                "Skeleton": {
                    "joints": skeleton_to_text(&skeleton),
                    "joint_count": 2
                } } } },
            { "command": "world.create", "payload": { "rows": {
                "Name": { "text": "anda" },
                "Clip": { "channels": clip_to_text(&clip), "duration": 1.0 } } } }
        ] });
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.batch", "payload": rig }),
        );
        let listed = call(&mut session, json!({ "op": "anim.list" }));
        let skeleton_row = listed["skeletons"][0]["id"].as_u64().expect("skeleton");
        let clip_row = listed["clips"][0]["id"].as_u64().expect("clip");

        // An object with a mesh and a skin: every vertex belongs to joint 0.
        let made = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let entity = made["entity"].as_u64().expect("entity");
        let skin = petunia_import::skin::ImportSkin {
            skeleton: 0,
            weights: vec![
                petunia_import::skin::VertexWeights {
                    joints: [0, 0, 0, 0],
                    weights: [1.0, 0.0, 0.0, 0.0],
                };
                24
            ],
        };
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Skin": {
                    "owner": entity as i64,
                    "skeleton": skeleton_row,
                    "weights": skin_to_text(skeleton_row, &skin)
                } } } }),
        );
        // The brush is a shape, not a `Mesh` row: convert it so the frame carries a mesh to skin.
        call(
            &mut session,
            json!({ "op": "mesh.convert", "entity": entity }),
        );
        (session, root, entity, clip_row)
    }

    #[test]
    fn playing_a_clip_moves_the_mesh_and_advancing_time_keeps_it_going() {
        let (mut session, root, entity, clip) = rigged("play");
        call(
            &mut session,
            json!({ "op": "anim.play", "entity": entity, "clip": clip, "start": 0.0, "speed": 1.0 }),
        );

        let at_zero = skinned_item(&session, entity);
        let start_x = at_zero.0[12];
        call(
            &mut session,
            json!({ "op": "anim.advance", "seconds": 0.5 }),
        );
        let at_half = skinned_item(&session, entity);
        assert!(
            at_half.0[12] > start_x,
            "half a second moves the joint: {start_x} -> {}",
            at_half.0[12]
        );
        call(
            &mut session,
            json!({ "op": "anim.advance", "seconds": 0.5 }),
        );
        let at_one = skinned_item(&session, entity);
        assert!(
            (at_one.0[12] - 4.0).abs() < 0.2,
            "one second reaches the end of the clip: {}",
            at_one.0[12]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stopping_returns_the_object_to_its_first_pose() {
        let (mut session, root, entity, clip) = rigged("stop");
        call(
            &mut session,
            json!({ "op": "anim.play", "entity": entity, "clip": clip, "start": 0.0, "speed": 1.0 }),
        );
        call(
            &mut session,
            json!({ "op": "anim.advance", "seconds": 0.8 }),
        );
        let moving = skinned_item(&session, entity);
        assert!(moving.0[12] > 0.5, "it moved: {}", moving.0[12]);

        call(&mut session, json!({ "op": "anim.stop", "entity": entity }));
        let stopped = skinned_item(&session, entity);
        assert!(
            stopped.0[12].abs() < 0.2,
            "back to the first key: {}",
            stopped.0[12]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    fn skinned_item(session: &EditorSession, entity: u64) -> petunia_anim::Mat4 {
        let frame = session.frame().expect("frame");
        let item = frame
            .items
            .iter()
            .find(|item| item.entity == entity)
            .expect("the object is drawn");
        assert!(!item.skin.is_empty(), "the frame carries the skin weights");
        item.skin_matrices
            .first()
            .copied()
            .expect("the first joint matrix")
    }
}
