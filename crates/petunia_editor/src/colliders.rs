//! Named colliders in the editor (DR-27): add one fitted to the object, fit it again, list them
//! under their owner, copy and delete them with it, and draw them on the viewport.
//!
//! A collider is its own entity with a `Collider` row whose `owner` is the object's persistent id,
//! like the strokes of a cave. Every change is a command, so it can be undone.

use petunia_commands::CommandRecord;
use petunia_math::{Color, Transform, Vec3};
use petunia_render::{DrawItem, OverlayItem, item_mesh, model_matrix};
use petunia_world::{PersistentId, Value, World};
use serde_json::{Map, Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

/// Colliders of the selected object, and the one being edited.
const COLLIDER_COLOR: Color = Color::new(0.30, 0.90, 0.45, 0.95);
const COLLIDER_SELECTED_COLOR: Color = Color::new(0.55, 1.0, 0.65, 1.0);
const TRIGGER_COLOR: Color = Color::new(0.95, 0.80, 0.25, 0.95);
/// Segments of a drawn circle.
const CIRCLE_SEGMENTS: usize = 32;

/// The collider row of an entity, read once.
#[derive(Clone, Debug)]
struct ColliderRow {
    name: String,
    shape: String,
    center: Vec3,
    rotation_degrees: Vec3,
    size: Vec3,
    radius: f32,
    height: f32,
    trigger: bool,
}

fn float_of(value: Option<&Value>) -> f32 {
    match value {
        Some(Value::Float(number)) => *number as f32,
        Some(Value::Int(number)) => *number as f32,
        _ => 0.0,
    }
}

fn read_row(world: &World, id: u64) -> Option<ColliderRow> {
    let handle = world.handle_of(PersistentId(id)).ok()?;
    // Only a collider has a collider row: any other object is not one (its name is its `Name`).
    if !world.has_row(handle, "Collider") {
        return None;
    }
    let get = |column: &str| world.get(handle, "Collider", column).ok();
    let vector = |column: &str| match get(column) {
        Some(Value::Vec3(value)) => *value,
        _ => Vec3::ZERO,
    };
    let text = |column: &str| match get(column) {
        Some(Value::Text(value)) => value.clone(),
        _ => String::new(),
    };
    Some(ColliderRow {
        name: text("name"),
        shape: text("shape"),
        center: vector("center"),
        rotation_degrees: vector("rotation_degrees"),
        size: vector("size"),
        radius: float_of(get("radius")),
        height: float_of(get("height")),
        trigger: matches!(get("trigger"), Some(Value::Bool(true))),
    })
}

/// The owner of a collider entity, or `None` when the entity is not a collider.
pub(crate) fn owner_of(world: &World, id: u64) -> Option<u64> {
    let handle = world.handle_of(PersistentId(id)).ok()?;
    match world.get(handle, "Collider", "owner") {
        Ok(Value::Int(owner)) if *owner > 0 => Some(*owner as u64),
        _ => None,
    }
}

/// The name a collider shows in the hierarchy.
pub(crate) fn collider_name(world: &World, id: u64) -> Option<String> {
    read_row(world, id).map(|row| row.name)
}

/// Persistent ids of the colliders of `owner`, in order.
pub(crate) fn colliders_of(world: &World, owner: u64) -> Vec<u64> {
    let mut found: Vec<u64> = world
        .entities_with_table("Collider")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|handle| {
            let id = world.persistent_id(handle).ok()?.0;
            (owner_of(world, id) == Some(owner)).then_some(id)
        })
        .collect();
    found.sort_unstable();
    found
}

/// The box around an item's mesh in its own (unscaled) coordinates.
fn local_bounds(item: &DrawItem) -> Option<(Vec3, Vec3)> {
    let mesh = item_mesh(item).ok()?;
    let mut low = Vec3::splat(f32::MAX);
    let mut high = Vec3::splat(f32::MIN);
    for [x, y, z] in &mesh.positions {
        low = Vec3::new(low.x.min(*x), low.y.min(*y), low.z.min(*z));
        high = Vec3::new(high.x.max(*x), high.y.max(*y), high.z.max(*z));
    }
    (low.x <= high.x).then_some((low, high))
}

/// Columns that make a collider of `shape` wrap the box `low..high`.
fn fitted(shape: &str, low: Vec3, high: Vec3) -> Map<String, Json> {
    let size = high - low;
    let center = (low + high) * 0.5;
    let vector = |value: Vec3| json!({ "x": value.x, "y": value.y, "z": value.z });
    let mut columns = Map::new();
    columns.insert("shape".to_string(), json!(shape));
    columns.insert("center".to_string(), vector(center));
    columns.insert("rotation_degrees".to_string(), vector(Vec3::ZERO));
    let minimum = 0.01f32;
    columns.insert(
        "size".to_string(),
        vector(Vec3::new(
            size.x.max(minimum),
            size.y.max(minimum),
            size.z.max(minimum),
        )),
    );
    let radius = match shape {
        "sphere" => size.x.max(size.y).max(size.z) * 0.5,
        _ => size.x.max(size.z) * 0.5,
    };
    columns.insert("radius".to_string(), json!(radius.max(minimum)));
    columns.insert(
        "height".to_string(),
        json!(size.y.max(radius * 2.0).max(minimum)),
    );
    columns
}

impl EditorSession {
    /// The object a collider command acts on: the selection, or the owner of a selected collider.
    fn collider_target(&self) -> Result<u64, Failure> {
        let selection = self.selection.ok_or_else(|| {
            Failure::new(
                "EDITOR_NO_SELECTION",
                "nenhum objeto selecionado",
                "Selecione o objeto que vai ganhar a colisão.",
            )
        })?;
        let world = self.world()?;
        Ok(owner_of(world, selection).unwrap_or(selection))
    }

    /// `collider.add`: a new collider on the selected object, fitted to its shape, with a name not
    /// yet used on it ("colisão", "colisão 2"…). One undo step; the new collider is selected.
    pub fn add_collider(&mut self, shape: &str) -> Result<Json, Failure> {
        if !petunia_project::COLLIDER_SHAPES.contains(&shape) {
            return Err(Failure::bad_request(&format!(
                "forma `{shape}` desconhecida (box, sphere, capsule ou convex)"
            )));
        }
        let owner = self.collider_target()?;
        let (low, high) = self
            .item_of(owner)
            .and_then(|item| local_bounds(&item))
            .unwrap_or((Vec3::splat(-0.5), Vec3::splat(0.5)));
        let name = {
            let world = self.world()?;
            let taken: Vec<String> = colliders_of(world, owner)
                .into_iter()
                .filter_map(|id| collider_name(world, id))
                .collect();
            (1..)
                .map(|index| {
                    if index == 1 {
                        "colisão".to_string()
                    } else {
                        format!("colisão {index}")
                    }
                })
                .find(|candidate| !taken.contains(candidate))
                .unwrap_or_else(|| "colisão".to_string())
        };
        let mut columns = fitted(shape, low, high);
        columns.insert("owner".to_string(), json!(owner));
        columns.insert("name".to_string(), json!(name));
        let result = self.execute(&CommandRecord::new(
            "world.create",
            json!({ "rows": { "Collider": columns } }),
        ))?;
        let entity = result.get("entity").and_then(Json::as_u64);
        self.selection = entity;
        Ok(json!({ "entity": entity, "owner": owner, "name": name }))
    }

    /// `collider.fit`: wraps the selected collider around its object again (keeping its shape).
    pub fn fit_collider(&mut self) -> Result<Json, Failure> {
        let selection = self.selection.ok_or_else(|| {
            Failure::new(
                "EDITOR_NO_SELECTION",
                "nenhuma colisão selecionada",
                "Clique numa colisão na Hierarquia.",
            )
        })?;
        let (owner, shape) = {
            let world = self.world()?;
            let owner = owner_of(world, selection).ok_or_else(|| {
                Failure::new(
                    "EDITOR_NOT_A_COLLIDER",
                    "o que está selecionado não é uma colisão",
                    "Clique numa colisão (aparece sob o objeto na Hierarquia).",
                )
            })?;
            (
                owner,
                read_row(world, selection)
                    .map(|row| row.shape)
                    .unwrap_or_else(|| "box".to_string()),
            )
        };
        let (low, high) = self
            .item_of(owner)
            .and_then(|item| local_bounds(&item))
            .ok_or_else(|| {
                Failure::new(
                    "EDITOR_NO_SHAPE",
                    "o objeto não tem forma para ajustar a colisão",
                    "Ajuste o tamanho no inspetor.",
                )
            })?;
        let mut columns = fitted(&shape, low, high);
        columns.remove("shape");
        self.execute(&CommandRecord::new(
            "row.set",
            json!({ "entity": selection, "table": "Collider", "values": columns }),
        ))
    }

    /// `world.remove` records for the colliders of `owner` (deleting an object deletes them).
    pub(crate) fn remove_collider_records(&self, owner: u64) -> Vec<CommandRecord> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        colliders_of(world, owner)
            .into_iter()
            .map(|id| CommandRecord::new("world.remove", json!({ "entity": id })))
            .collect()
    }

    /// `world.create` records that copy the colliders of `owner` onto the entity created by
    /// command number `copy` of the same batch (duplicating an object duplicates them).
    pub(crate) fn copy_collider_records(&self, owner: u64, copy: usize) -> Vec<CommandRecord> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        colliders_of(world, owner)
            .into_iter()
            .filter_map(|id| {
                let handle = world.handle_of(PersistentId(id)).ok()?;
                let values = world.row(handle, "Collider").ok()?;
                let mut row = world.row_to_json("Collider", values).ok()?;
                row.insert("owner".to_string(), json!({ "$entity_of": copy }));
                Some(CommandRecord::new(
                    "world.create",
                    json!({ "rows": { "Collider": row } }),
                ))
            })
            .collect()
    }

    /// Green wires of the colliders of the selected object (or of the selected collider's
    /// object); triggers in yellow; the selected collider brighter and thicker.
    pub(crate) fn collider_overlay(&self, items: &[DrawItem], out: &mut Vec<OverlayItem>) {
        let (Some(selection), Ok(world)) = (self.selection, self.world()) else {
            return;
        };
        let owner = owner_of(world, selection).unwrap_or(selection);
        let ids = colliders_of(world, owner);
        if ids.is_empty() {
            return;
        }
        let Some(item) = items.iter().find(|item| item.entity == owner) else {
            return;
        };
        for id in ids {
            let Some(row) = read_row(world, id) else {
                continue;
            };
            let (color, width) = if id == selection {
                (COLLIDER_SELECTED_COLOR, 2.5)
            } else if row.trigger {
                (TRIGGER_COLOR, 1.5)
            } else {
                (COLLIDER_COLOR, 1.5)
            };
            collider_lines(item, &row, color, width, out);
        }
    }
}

/// The lines of one collider, placed like the physics places it: inside the owner's transform,
/// with the owner's scale stretching boxes and convex hulls, spheres taking the largest scale and
/// capsules the largest of x and z.
fn collider_lines(
    owner: &DrawItem,
    row: &ColliderRow,
    color: Color,
    width_pixels: f32,
    out: &mut Vec<OverlayItem>,
) {
    let matrix = model_matrix(&owner.transform);
    let entry = |index: usize| matrix.get(index).copied().unwrap_or(0.0);
    let to_world = |point: Vec3| {
        Vec3::new(
            entry(0) * point.x + entry(4) * point.y + entry(8) * point.z + entry(12),
            entry(1) * point.x + entry(5) * point.y + entry(9) * point.z + entry(13),
            entry(2) * point.x + entry(6) * point.y + entry(10) * point.z + entry(14),
        )
    };
    let own = Transform {
        rotation_degrees: row.rotation_degrees,
        ..Transform::IDENTITY
    };
    let turn = |direction: Vec3| own.rotation_direction(direction) * direction.length();
    let mut line = |from: Vec3, to: Vec3| {
        out.push(OverlayItem::Line {
            from,
            to,
            color,
            width_pixels,
        });
    };
    let scale = owner.transform.scale;
    let scale = Vec3::new(scale.x.abs(), scale.y.abs(), scale.z.abs());
    match row.shape.as_str() {
        "sphere" | "capsule" => {
            // Round shapes do not stretch: draw them in world units around the world center.
            let center = to_world(row.center);
            let world_axis = |direction: Vec3| {
                owner
                    .transform
                    .rotation_direction(own.rotation_direction(direction))
            };
            let (x_axis, y_axis, z_axis) = (
                world_axis(Vec3::new(1.0, 0.0, 0.0)),
                world_axis(Vec3::new(0.0, 1.0, 0.0)),
                world_axis(Vec3::new(0.0, 0.0, 1.0)),
            );
            let circle = |middle: Vec3,
                          first: Vec3,
                          second: Vec3,
                          radius: f32,
                          from_angle: f32,
                          to_angle: f32,
                          line: &mut dyn FnMut(Vec3, Vec3)| {
                let point = |angle: f32| {
                    middle + first * (angle.cos() * radius) + second * (angle.sin() * radius)
                };
                for index in 0..CIRCLE_SEGMENTS {
                    let a = from_angle
                        + (to_angle - from_angle) * index as f32 / CIRCLE_SEGMENTS as f32;
                    let b = from_angle
                        + (to_angle - from_angle) * (index + 1) as f32 / CIRCLE_SEGMENTS as f32;
                    line(point(a), point(b));
                }
            };
            let full = std::f32::consts::TAU;
            let half = std::f32::consts::PI;
            if row.shape == "sphere" {
                let radius = row.radius * scale.x.max(scale.y).max(scale.z);
                circle(center, x_axis, z_axis, radius, 0.0, full, &mut line);
                circle(center, x_axis, y_axis, radius, 0.0, full, &mut line);
                circle(center, y_axis, z_axis, radius, 0.0, full, &mut line);
            } else {
                let radius = row.radius * scale.x.max(scale.z);
                let rise = (row.height * scale.y * 0.5 - radius).max(0.0);
                let top = center + y_axis * rise;
                let bottom = center - y_axis * rise;
                circle(top, x_axis, z_axis, radius, 0.0, full, &mut line);
                circle(bottom, x_axis, z_axis, radius, 0.0, full, &mut line);
                for side in [x_axis, z_axis, x_axis * -1.0, z_axis * -1.0] {
                    line(top + side * radius, bottom + side * radius);
                }
                circle(top, x_axis, y_axis, radius, 0.0, half, &mut line);
                circle(top, z_axis, y_axis, radius, 0.0, half, &mut line);
                circle(bottom, x_axis, y_axis, radius, half, full, &mut line);
                circle(bottom, z_axis, y_axis, radius, half, full, &mut line);
            }
        }
        _ => {
            // Boxes (and convex hulls, drawn as the box around the object's mesh: an outline,
            // not the exact hull) stretch with the owner, so they are built in its own units.
            let half = if row.shape == "convex" {
                local_bounds(owner).map_or(Vec3::splat(0.5), |(low, high)| (high - low) * 0.5)
            } else {
                row.size * 0.5
            };
            let middle = if row.shape == "convex" {
                local_bounds(owner).map_or(Vec3::ZERO, |(low, high)| (low + high) * 0.5)
            } else {
                row.center
            };
            let corner = |x: f32, y: f32, z: f32| {
                to_world(middle + turn(Vec3::new(half.x * x, half.y * y, half.z * z)))
            };
            let signs = [-1.0f32, 1.0];
            for a in signs {
                for b in signs {
                    line(corner(-1.0, a, b), corner(1.0, a, b));
                    line(corner(a, -1.0, b), corner(a, 1.0, b));
                    line(corner(a, b, -1.0), corner(a, b, 1.0));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value as Json, json};

    use crate::session::EditorSession;
    use petunia_render::OverlayItem;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn opened(label: &str) -> (EditorSession, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": label }),
        );
        (session, root)
    }

    fn collider_count(session: &mut EditorSession) -> usize {
        let snapshot = call(session, json!({ "op": "editor.snapshot" }));
        snapshot["entities"].as_array().map_or(0, |entities| {
            entities
                .iter()
                .filter(|entity| entity.get("owner").is_some())
                .count()
        })
    }

    #[test]
    fn colliders_are_added_fitted_listed_drawn_copied_and_deleted_with_their_object() {
        let (mut session, root) = opened("colliders");
        // Crate #9 of the sample is a 1 m box.
        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        // An ordinary object keeps its own name in the inspector, not an empty collider name.
        let crate_name = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": 9 }),
        );
        assert_eq!(crate_name["name"], "Caixa grande");
        let first = call(
            &mut session,
            json!({ "op": "collider.add", "shape": "box" }),
        );
        assert_eq!(first["name"], "colisão");
        assert_eq!(first["owner"], 9);
        let collider = first["entity"].as_u64().expect("collider id");
        let described = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": collider }),
        );
        let size = described.to_string();
        assert!(
            size.contains("\"x\":1.0") || size.contains("\"x\":1"),
            "fitted to the 1 m crate: {size}"
        );

        // A second one gets the next free name, also when the collider itself is selected.
        let second = call(
            &mut session,
            json!({ "op": "collider.add", "shape": "sphere" }),
        );
        assert_eq!(second["name"], "colisão 2");
        assert_eq!(second["owner"], 9);

        let snapshot = call(&mut session, json!({ "op": "editor.snapshot" }));
        let listed: Vec<&Json> = snapshot["entities"]
            .as_array()
            .expect("list")
            .iter()
            .filter(|entity| entity["owner"] == 9)
            .collect();
        assert_eq!(listed.len(), 2, "listed under their owner");
        assert!(listed.iter().any(|entity| entity["name"] == "colisão 2"));

        // Drawn in green around the object while it (or one of its colliders) is selected.
        let lines = session
            .frame()
            .expect("frame")
            .overlay
            .iter()
            .filter(|item| matches!(item, OverlayItem::Line { color, .. } if color.g > 0.85 && color.r < 0.6))
            .count();
        assert!(lines >= 12 + 3 * 32, "a box and a sphere: {lines}");

        // Fit again after a change, as one undo step.
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": collider }),
        );
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.set", "payload": { "entity": collider, "table": "Collider", "column": "size", "value": { "x": 5.0, "y": 5.0, "z": 5.0 } } }),
        );
        call(&mut session, json!({ "op": "collider.fit" }));
        let refitted = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": collider }),
        )
        .to_string();
        assert!(
            !refitted.contains("5.0"),
            "back to the crate's size: {refitted}"
        );

        // Duplicating the crate copies its colliders onto the copy; deleting removes them.
        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        call(&mut session, json!({ "op": "edit.duplicate" }));
        assert_eq!(collider_count(&mut session), 4);
        call(&mut session, json!({ "op": "edit.delete" }));
        assert_eq!(collider_count(&mut session), 2);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(
            collider_count(&mut session),
            4,
            "undo brings the copy back with its colliders"
        );

        let unknown = session.handle(&json!({ "op": "collider.add", "shape": "torus" }));
        assert_eq!(unknown["ok"], false);
        let _ = std::fs::remove_dir_all(&root);
    }
}
