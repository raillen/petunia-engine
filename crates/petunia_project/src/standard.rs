use petunia_math::{Color, Vec3};
use petunia_world::{TableSpec, Value, ValueKind, World, WorldError};

/// Shapes a brush can have. The renderer builds the mesh from the shape and the size.
pub const BRUSH_SHAPES: [&str; 6] = ["box", "ramp", "cylinder", "cone", "sphere", "stairs"];

/// Registers the tables every project starts with.
///
/// - `Name`: a label for people (editor outline, debugging).
/// - `Transform`: position (meters), rotation (Euler degrees), scale, and `pivot`: the point the
///   object turns and scales around, in its own axes relative to its position (zero = center).
/// - `Brush`: solid shape for building levels. `size` is width, height, depth in meters
///   (for round shapes: x and z are the diameters, y the height). `detail` is the number of
///   segments around a round shape, or the number of steps of a staircase.
/// - `Script`: attaches a script to an object: `file` is the name of a file in the project's
///   `scripts/` folder, without the `.aipo` extension; `enabled` switches it off without removing it.
///   The Script Host calls its `on_start`, `on_update` and `on_event` handlers.
/// - `Floor`: a floor of a building ("andar") for the editor: `elevation` of its ground and
///   `height` to the next one, in meters. Saved with the scene like any other row.
/// - `Sculpt`: a sculpted volume (caves, rocks). `base` is `solid` (a box of rock of `size`,
///   centered on the position) or `empty`; `cell_size` is the meshing detail in meters; `faceted`
///   draws flat triangles (low poly) instead of smooth shading.
/// - `SculptStroke`: one stroke of a `Sculpt`: the persistent id of the `volume`, `mode` (`add`
///   or `carve`, plus `inflate`, `flatten` and `smooth`), a capsule from `start` to `end` (local to
///   the volume) with `radius`, blended over `smooth` meters (up to 2). `strength` is how far
///   inflate moves the surface (meters, negative deflates) or how hard flatten and smooth press
///   (0 to 1). Strokes apply in the order they were created.
/// - `PlayerStart`: where the player (and the fly camera) starts, facing `yaw_degrees`.
pub fn register_standard_tables(world: &mut World) -> Result<(), WorldError> {
    world.define_table(TableSpec::new("Name").column(
        "text",
        ValueKind::Text,
        Value::Text(String::new()),
    ))?;
    world.define_table(
        TableSpec::new("Transform")
            .column("position", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
            .column("rotation_degrees", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
            .column("scale", ValueKind::Vec3, Value::Vec3(Vec3::ONE))
            .column("pivot", ValueKind::Vec3, Value::Vec3(Vec3::ZERO)),
    )?;
    world.define_table(
        TableSpec::new("Brush")
            .column("shape", ValueKind::Text, Value::Text("box".to_string()))
            .column("size", ValueKind::Vec3, Value::Vec3(Vec3::ONE))
            .column("detail", ValueKind::Int, Value::Int(16))
            .column(
                "color",
                ValueKind::Color,
                Value::Color(Color::rgb(0.8, 0.8, 0.8)),
            ),
    )?;
    world.define_table(
        TableSpec::new("Script")
            .column("file", ValueKind::Text, Value::Text(String::new()))
            .column("enabled", ValueKind::Bool, Value::Bool(true)),
    )?;
    world.define_table(
        TableSpec::new("Floor")
            .column("elevation", ValueKind::Float, Value::Float(0.0))
            .column("height", ValueKind::Float, Value::Float(3.0)),
    )?;
    world.define_table(
        TableSpec::new("Sculpt")
            .column("base", ValueKind::Text, Value::Text("solid".to_string()))
            .column(
                "size",
                ValueKind::Vec3,
                Value::Vec3(Vec3::new(16.0, 6.0, 16.0)),
            )
            .column("cell_size", ValueKind::Float, Value::Float(0.5))
            .column(
                "color",
                ValueKind::Color,
                Value::Color(Color::rgb(0.46, 0.43, 0.40)),
            )
            .column("faceted", ValueKind::Bool, Value::Bool(false)),
    )?;
    world.define_table(
        TableSpec::new("SculptStroke")
            .column("volume", ValueKind::Int, Value::Int(0))
            .column("mode", ValueKind::Text, Value::Text("carve".to_string()))
            .column("start", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
            .column("end", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
            .column("radius", ValueKind::Float, Value::Float(1.5))
            .column("smooth", ValueKind::Float, Value::Float(0.5))
            .column("strength", ValueKind::Float, Value::Float(0.0)),
    )?;
    world.define_table(TableSpec::new("PlayerStart").column(
        "yaw_degrees",
        ValueKind::Float,
        Value::Float(0.0),
    ))?;
    Ok(())
}
