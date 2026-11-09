//! Per-face colour (phase 2 pendency): one object, more than one paint.
//!
//! The colour lives in its own table, not in the mesh, so a scene written before this feature still
//! opens and the geometry keeps its own format. A row is `owner` (the object) plus the face index
//! and the colour; a face without a row keeps the object's colour. That is the "defaults → profile
//! → overrides" shape the AGENTS file asks for: the object colour is the default, this table is the
//! override.

use crate::value::Value;
use crate::world::World;
use petunia_math::Color;

/// Table that carries the overrides.
pub const FACE_COLOR_TABLE: &str = "FaceColor";

/// The colour of each face, by index, for the object with persistent id `owner`.
///
/// A face index that does not exist is ignored: the mesh may have been edited after the paint, and
/// a stale override must never make drawing fail.
pub fn face_colors(world: &World, owner: u64, face_count: usize) -> Vec<Option<Color>> {
    let mut colors = vec![None; face_count];
    let Ok(handles) = world.entities_with_table(FACE_COLOR_TABLE) else {
        return colors;
    };
    for handle in handles {
        let Ok(row_owner) = world.get(handle, FACE_COLOR_TABLE, "owner") else {
            continue;
        };
        let Value::Int(row_owner) = row_owner else {
            continue;
        };
        if *row_owner < 0 || *row_owner as u64 != owner {
            continue;
        }
        let Ok(Value::Int(face)) = world.get(handle, FACE_COLOR_TABLE, "face") else {
            continue;
        };
        let Ok(Value::Color(color)) = world.get(handle, FACE_COLOR_TABLE, "color") else {
            continue;
        };
        if *face < 0 {
            continue;
        }
        if let Some(slot) = colors.get_mut(*face as usize) {
            *slot = Some(*color);
        }
    }
    colors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use crate::world::World;

    fn world_with_face_color() -> World {
        let mut world = World::new(7);
        world
            .define_table(
                crate::table::TableSpec::new(FACE_COLOR_TABLE)
                    .column("owner", crate::value::ValueKind::Int, Value::Int(0))
                    .column("face", crate::value::ValueKind::Int, Value::Int(0))
                    .column(
                        "color",
                        crate::value::ValueKind::Color,
                        Value::Color(Color::rgb(0.8, 0.8, 0.8)),
                    ),
            )
            .expect("table");
        world
    }

    fn paint(world: &mut World, owner: u64, face: i64, color: Color) {
        let handle = world.spawn().expect("room");
        world
            .insert_row_json(
                handle,
                FACE_COLOR_TABLE,
                &serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    &format!(
                        r#"{{"owner": {owner}, "face": {face}, "color": {{"r": {}, "g": {}, "b": {}, "a": {}}}}}"#,
                        color.r, color.g, color.b, color.a
                    ),
                )
                .expect("row"),
            )
            .expect("row fits");
        let _ = world.persistent_id(handle).expect("alive");
    }

    #[test]
    fn a_painted_face_overrides_only_itself() {
        let mut world = world_with_face_color();
        let red = Color::rgb(1.0, 0.0, 0.0);
        paint(&mut world, 4, 1, red);
        let colors = face_colors(&world, 4, 3);
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], None, "face 0 keeps the object colour");
        assert_eq!(colors[1], Some(red), "face 1 was painted");
        assert_eq!(colors[2], None);
    }

    #[test]
    fn another_objects_paint_does_not_leak() {
        let mut world = world_with_face_color();
        paint(&mut world, 4, 0, Color::rgb(1.0, 0.0, 0.0));
        let other = face_colors(&world, 5, 1);
        assert_eq!(other, vec![None], "object 5 has no override");
    }

    #[test]
    fn a_stale_face_index_is_ignored_instead_of_failing() {
        // The mesh was edited after the paint: face 9 no longer exists. Drawing must not fail.
        let mut world = world_with_face_color();
        paint(&mut world, 4, 9, Color::rgb(0.0, 1.0, 0.0));
        let colors = face_colors(&world, 4, 2);
        assert_eq!(colors, vec![None, None]);
    }
}
