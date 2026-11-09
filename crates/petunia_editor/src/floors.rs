//! Floors ("andares"): named levels of a building, saved in the scene as entities with a `Name`
//! and a `Floor` row (elevation and height in meters). Because they are ordinary entities they
//! are saved, undone and listed like everything else; the session only remembers which one is
//! being built.
//!
//! A scene without floors has an implicit ground floor at elevation zero. Going up past the top
//! floor (or down past the lowest) creates the next one, as one undo step.

use petunia_commands::CommandRecord;
use petunia_render::{DrawItem, SpriteItem};
use petunia_world::{Value, World};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::{EditorSession, display_name};
use crate::tools::item_bounds;

/// Floors are between these heights (meters).
const MIN_FLOOR_HEIGHT: f32 = 0.5;
const MAX_FLOOR_HEIGHT: f32 = 50.0;
/// A scene keeps at most this many floors.
const MAX_FLOORS: usize = 64;
/// An object counts as "above" when its bottom is this close to the top of the floor or higher.
const ABOVE_TOLERANCE: f32 = 0.01;
/// Opacity of objects above the floor in ghost mode.
const GHOST_OPACITY: f32 = 0.18;

/// How objects above the floor being built are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AboveFloor {
    Show,
    /// Translucent and not pickable: you see the building, you click the floor you are on.
    Ghost,
    Hide,
}

impl AboveFloor {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "show" => Some(Self::Show),
            "ghost" => Some(Self::Ghost),
            "hide" => Some(Self::Hide),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Ghost => "ghost",
            Self::Hide => "hide",
        }
    }
}

/// One floor as the editor sees it. `entity` is `None` for the implicit ground floor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FloorLevel {
    pub entity: Option<u64>,
    pub name: String,
    pub elevation: f32,
    pub height: f32,
}

impl FloorLevel {
    pub fn top(&self) -> f32 {
        self.elevation + self.height
    }
}

fn float_of(world: &World, handle: petunia_world::EntityHandle, column: &str) -> Option<f32> {
    match world.get(handle, "Floor", column) {
        Ok(Value::Float(value)) if value.is_finite() => Some(*value as f32),
        _ => None,
    }
}

/// Every floor of the scene, lowest first. Heights out of range are clamped, not trusted.
pub(crate) fn floors_of(world: &World) -> Vec<FloorLevel> {
    let mut floors: Vec<FloorLevel> = world
        .entities_with_table("Floor")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|handle| {
            let entity = world.persistent_id(handle).ok()?.0;
            Some(FloorLevel {
                entity: Some(entity),
                name: display_name(world, entity),
                elevation: float_of(world, handle, "elevation")?,
                height: float_of(world, handle, "height")?
                    .clamp(MIN_FLOOR_HEIGHT, MAX_FLOOR_HEIGHT),
            })
        })
        .collect();
    floors.sort_by(|first, second| first.elevation.total_cmp(&second.elevation));
    floors
}

/// Name for a new floor at `position` steps from the ground (0 is the ground floor).
fn floor_name(position: i32) -> String {
    match position {
        0 => "Térreo".to_string(),
        above if above > 0 => format!("Andar {above}"),
        below => format!("Subsolo {}", -below),
    }
}

fn create_floor_record(name: &str, elevation: f32, height: f32) -> CommandRecord {
    CommandRecord::new(
        "world.create",
        json!({
            "rows": {
                "Name": { "text": name },
                "Floor": { "elevation": elevation, "height": height }
            }
        }),
    )
}

impl EditorSession {
    fn implicit_ground(&self) -> FloorLevel {
        FloorLevel {
            entity: None,
            name: floor_name(0),
            elevation: 0.0,
            height: self.floor_height,
        }
    }

    /// The floor being built: the remembered one if it still exists (an undo may remove it),
    /// otherwise the floor at ground level, otherwise the implicit ground.
    pub(crate) fn current_floor(&self) -> FloorLevel {
        let Ok(world) = self.world() else {
            return self.implicit_ground();
        };
        let floors = floors_of(world);
        floors
            .iter()
            .find(|floor| floor.entity.is_some() && floor.entity == self.floor)
            .or_else(|| {
                floors
                    .iter()
                    .find(|floor| floor.elevation.abs() < ABOVE_TOLERANCE)
            })
            .cloned()
            .unwrap_or_else(|| self.implicit_ground())
    }

    pub(crate) fn floor_elevation(&self) -> f32 {
        self.current_floor().elevation
    }

    /// Goes up or down `delta` floors. Past the top or the bottom it creates the missing floors
    /// (one undo step). The grid, new objects and surface snapping follow, and the camera
    /// rises with the floor so the view stays the same.
    pub fn floor_step(&mut self, delta: i32) -> Result<Json, Failure> {
        let before = self.current_floor();
        let mut floors = floors_of(self.world()?);
        if floors.is_empty() {
            floors.push(self.implicit_ground());
        }
        let current = floors
            .iter()
            .position(|floor| floor.entity == before.entity)
            .unwrap_or(0);
        let ground = floors
            .iter()
            .position(|floor| floor.elevation.abs() < ABOVE_TOLERANCE)
            .unwrap_or(0) as i32;
        let wanted = current as i32 + delta;
        let last = floors.len() as i32 - 1;
        let mut records = Vec::new();
        // The implicit ground becomes a real floor as soon as a second floor exists.
        if wanted != current as i32 && floors.iter().any(|floor| floor.entity.is_none()) {
            records.push(create_floor_record(&before.name, 0.0, before.height));
        }
        let (target_elevation, created) = if wanted > last {
            let mut top = floors
                .last()
                .cloned()
                .unwrap_or_else(|| self.implicit_ground());
            for step in 1..=(wanted - last) {
                let elevation = top.top();
                records.push(create_floor_record(
                    &floor_name(last + step - ground),
                    elevation,
                    top.height,
                ));
                top.elevation = elevation;
            }
            (top.elevation, true)
        } else if wanted < 0 {
            let mut bottom = floors
                .first()
                .cloned()
                .unwrap_or_else(|| self.implicit_ground());
            for step in 1..=-wanted {
                bottom.elevation -= bottom.height;
                records.push(create_floor_record(
                    &floor_name(-step - ground),
                    bottom.elevation,
                    bottom.height,
                ));
            }
            (bottom.elevation, true)
        } else {
            let target = floors
                .get(wanted as usize)
                .map_or(before.elevation, |floor| floor.elevation);
            (target, false)
        };
        if floors.iter().filter(|floor| floor.entity.is_some()).count() + records.len() > MAX_FLOORS
        {
            return Err(Failure::bad_request(&format!(
                "a cena já tem o máximo de {MAX_FLOORS} andares"
            )));
        }
        match records.len() {
            0 => {}
            1 => {
                let record = records.remove(0);
                self.execute(&record)?;
            }
            _ => {
                let batch = CommandRecord::new(
                    petunia_commands::BATCH_COMMAND,
                    json!({ "records": records }),
                );
                self.execute(&batch)?;
            }
        }
        // Remember the floor by entity, found by its elevation (fresh entities included).
        self.floor = floors_of(self.world()?)
            .into_iter()
            .find(|floor| (floor.elevation - target_elevation).abs() < ABOVE_TOLERANCE)
            .and_then(|floor| floor.entity);
        let after = self.current_floor();
        self.camera.camera.position.y += after.elevation - before.elevation;
        let mut reply = self.floor_json();
        if let Json::Object(fields) = &mut reply {
            fields.insert("created".to_string(), json!(created));
        }
        Ok(reply)
    }

    /// Changes the height of the floor being built (undoable when the floor is saved in the
    /// scene) and the default for new floors.
    pub fn set_floor_height(&mut self, meters: f32) -> Result<Json, Failure> {
        if !(meters.is_finite() && (MIN_FLOOR_HEIGHT..=MAX_FLOOR_HEIGHT).contains(&meters)) {
            return Err(Failure::bad_request(
                "a altura do andar vai de 0,5 a 50 metros",
            ));
        }
        self.floor_height = meters;
        if let Some(entity) = self.current_floor().entity {
            let record = CommandRecord::new(
                "world.set",
                json!({ "entity": entity, "table": "Floor", "column": "height", "value": meters }),
            );
            self.execute(&record)?;
        }
        Ok(self.floor_json())
    }

    pub fn set_above_floor(&mut self, mode: &str) -> Result<Json, Failure> {
        self.above_floor = AboveFloor::parse(mode).ok_or_else(|| {
            Failure::bad_request("use `show`, `ghost` ou `hide` para os objetos acima do andar")
        })?;
        Ok(self.floor_json())
    }

    /// Jumps to a floor by its entity (the list in the GUI).
    pub fn floor_select(&mut self, entity: u64) -> Result<Json, Failure> {
        let before = self.current_floor();
        let target = floors_of(self.world()?)
            .into_iter()
            .find(|floor| floor.entity == Some(entity))
            .ok_or_else(|| Failure::bad_request("esse objeto não é um andar"))?;
        self.floor = target.entity;
        self.camera.camera.position.y += target.elevation - before.elevation;
        Ok(self.floor_json())
    }

    pub(crate) fn floor_json(&self) -> Json {
        let current = self.current_floor();
        let list: Vec<Json> = self
            .world()
            .map(floors_of)
            .unwrap_or_default()
            .into_iter()
            .map(|floor| {
                json!({
                    "entity": floor.entity,
                    "name": floor.name,
                    "elevation": floor.elevation,
                    "height": floor.height,
                })
            })
            .collect();
        json!({
            "entity": current.entity,
            "name": current.name,
            "elevation": current.elevation,
            "height": current.height,
            "above": self.above_floor.name(),
            "floors": list,
        })
    }

    /// Whether an object sits wholly above the floor being built.
    fn is_above(item: &DrawItem, floor_top: f32) -> bool {
        item_bounds(item).is_some_and(|(low, _)| low.y >= floor_top - ABOVE_TOLERANCE)
    }

    /// Applies the "above the floor" mode to what is drawn. The selection is never hidden.
    pub(crate) fn apply_floor_visibility(&self, items: &mut Vec<DrawItem>) {
        if self.above_floor == AboveFloor::Show {
            return;
        }
        let top = self.current_floor().top();
        let selection = self.selection;
        match self.above_floor {
            AboveFloor::Hide => {
                items.retain(|item| Some(item.entity) == selection || !Self::is_above(item, top))
            }
            _ => {
                for item in items.iter_mut() {
                    if Some(item.entity) != selection && Self::is_above(item, top) {
                        item.color.a = item.color.a.min(GHOST_OPACITY);
                    }
                }
            }
        }
    }

    /// What the mouse can hit: objects above the floor are out of reach unless shown.
    pub(crate) fn pickable(&self, mut items: Vec<DrawItem>) -> Vec<DrawItem> {
        if self.above_floor != AboveFloor::Show {
            let top = self.current_floor().top();
            let selection = self.selection;
            items.retain(|item| Some(item.entity) == selection || !Self::is_above(item, top));
        }
        items
    }

    /// Whether a sprite reaches down to (or below) the floor being built. The quad may be turned,
    /// so its half diagonal is used: the test never hides a sprite that is still visible.
    fn sprite_reaches_below(sprite: &SpriteItem, floor_top: f32) -> bool {
        let scale = sprite.transform.scale;
        let half_width = sprite.width * scale.x.abs() * 0.5;
        let half_height = sprite.height * scale.y.abs() * 0.5;
        let reach = (half_width * half_width + half_height * half_height).sqrt();
        sprite.transform.position.y - reach < floor_top - ABOVE_TOLERANCE
    }

    /// The same rule as [`Self::apply_floor_visibility`], for the 2D sprites of the scene.
    pub(crate) fn apply_sprite_visibility(&self, sprites: &mut Vec<SpriteItem>) {
        if self.above_floor == AboveFloor::Show {
            return;
        }
        let top = self.current_floor().top();
        let selection = self.selection;
        match self.above_floor {
            AboveFloor::Hide => sprites.retain(|sprite| {
                Some(sprite.entity) == selection || Self::sprite_reaches_below(sprite, top)
            }),
            _ => {
                for sprite in sprites.iter_mut() {
                    if Some(sprite.entity) != selection && !Self::sprite_reaches_below(sprite, top)
                    {
                        sprite.color.a = sprite.color.a.min(GHOST_OPACITY);
                    }
                }
            }
        }
    }

    /// What the mouse can hit among the sprites: those above the floor are out of reach unless
    /// shown.
    pub(crate) fn pickable_sprites(&self, mut sprites: Vec<SpriteItem>) -> Vec<SpriteItem> {
        if self.above_floor != AboveFloor::Show {
            let top = self.current_floor().top();
            let selection = self.selection;
            sprites.retain(|sprite| {
                Some(sprite.entity) == selection || Self::sprite_reaches_below(sprite, top)
            });
        }
        sprites
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
    fn going_up_creates_floors_that_save_and_undo_as_one_step() {
        let root =
            std::env::temp_dir().join(format!("petunia_editor_test_floors_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Andares" }),
        );
        let ground = call(&mut session, json!({ "op": "editor.snapshot" }))["floor"].clone();
        assert_eq!(
            ground["entity"],
            Json::Null,
            "a new scene has the implicit ground"
        );

        // Up from the implicit ground: the ground and the first floor are created together.
        let first = call(&mut session, json!({ "op": "floor.step", "delta": 1 }));
        assert_eq!(first["created"], true);
        assert_eq!(first["name"], "Andar 1");
        assert_eq!(first["elevation"], 3.0);
        assert_eq!(first["floors"].as_array().map(Vec::len), Some(2));

        // A box built on the first floor sits on it.
        let created = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let upstairs = created["entity"].as_u64().expect("entity");
        let described = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": upstairs }),
        );
        assert!(described.to_string().contains("\"y\":4.0"), "{described}");

        // Back on the ground floor the box is ghosted and out of reach of the mouse.
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": null }),
        );
        let down = call(&mut session, json!({ "op": "floor.step", "delta": -1 }));
        assert_eq!(down["created"], false);
        assert_eq!(down["name"], "Térreo");
        let frame = session.frame().expect("frame");
        let ghost = frame
            .items
            .iter()
            .find(|item| item.entity == upstairs)
            .expect("drawn");
        assert!(ghost.color.a < 0.5);
        assert!(
            session
                .draw_items()
                .iter()
                .all(|item| item.entity != upstairs)
        );
        call(&mut session, json!({ "op": "floor.above", "mode": "hide" }));
        let frame = session.frame().expect("frame");
        assert!(frame.items.iter().all(|item| item.entity != upstairs));

        // Saved with the scene.
        call(&mut session, json!({ "op": "project.save" }));
        let mut reopened = EditorSession::new().expect("session");
        call(
            &mut reopened,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        let floors =
            call(&mut reopened, json!({ "op": "editor.snapshot" }))["floor"]["floors"].clone();
        assert_eq!(floors.as_array().map(Vec::len), Some(2), "{floors}");

        // One undo removes the box; the next removes both floors created by one step.
        call(&mut session, json!({ "op": "edit.undo" }));
        call(&mut session, json!({ "op": "edit.undo" }));
        let floor = call(&mut session, json!({ "op": "editor.snapshot" }))["floor"].clone();
        assert_eq!(floor["floors"].as_array().map(Vec::len), Some(0), "{floor}");
        assert_eq!(floor["elevation"], 0.0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_sprite_above_the_floor_is_ghosted_hidden_and_out_of_reach() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_sprite_floor_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Sprites" }),
        );
        call(&mut session, json!({ "op": "floor.step", "delta": 1 }));
        let created = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.create", "payload": { "rows": {
                "Name": { "text": "Balao" },
                "Transform": { "position": { "x": 0.0, "y": 6.0, "z": 0.0 } },
                "Sprite": { "size": { "x": 2.0, "y": 2.0, "z": 0.0 } }
            } } }),
        );
        let sprite = created["entity"].as_u64().expect("sprite");
        // Back on the ground floor the sprite stays drawn, ghosted, and out of reach.
        call(&mut session, json!({ "op": "floor.step", "delta": -1 }));
        let frame = session.frame().expect("frame");
        let ghost = frame
            .sprites
            .iter()
            .find(|sprite_item| sprite_item.entity == sprite)
            .expect("drawn");
        assert!(ghost.color.a < 0.5, "{}", ghost.color.a);
        assert!(
            session
                .sprite_items()
                .iter()
                .all(|sprite_item| sprite_item.entity != sprite)
        );
        call(&mut session, json!({ "op": "floor.above", "mode": "hide" }));
        let frame = session.frame().expect("frame");
        assert!(
            frame
                .sprites
                .iter()
                .all(|sprite_item| sprite_item.entity != sprite)
        );
        // The selection is never hidden.
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": sprite }),
        );
        let frame = session.frame().expect("frame");
        assert!(
            frame
                .sprites
                .iter()
                .any(|sprite_item| sprite_item.entity == sprite)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn going_down_from_the_ground_makes_a_basement() {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_basement_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Subsolo" }),
        );
        call(&mut session, json!({ "op": "floor.height", "meters": 4.0 }));
        let basement = call(&mut session, json!({ "op": "floor.step", "delta": -1 }));
        assert_eq!(basement["name"], "Subsolo 1");
        assert_eq!(basement["elevation"], -4.0);
        let refused = session.handle(&json!({ "op": "floor.above", "mode": "sometimes" }));
        assert_eq!(refused["ok"], false);
        let _ = std::fs::remove_dir_all(&root);
    }
}
