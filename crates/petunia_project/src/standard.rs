use petunia_math::{Color, PolygonMesh, Vec3};
use petunia_world::{TableSpec, Value, ValueKind, World, WorldError};

/// Shapes a brush can have. The renderer builds the mesh from the shape and the size.
pub const BRUSH_SHAPES: [&str; 6] = ["box", "ramp", "cylinder", "cone", "sphere", "stairs"];

/// Shapes a collider can have (`convex` wraps the owner's mesh).
pub const COLLIDER_SHAPES: [&str; 4] = ["box", "sphere", "capsule", "convex"];

/// Kinds of light.
pub const LIGHT_KINDS: [&str; 2] = ["sun", "point"];

/// How a rigid body moves.
pub const BODY_KINDS: [&str; 3] = ["static", "dynamic", "kinematic"];

/// Registers the tables every project starts with.
///
/// - `Name`: a label for people (editor outline, debugging).
/// - `Transform`: position (meters), rotation (Euler degrees), scale, and `pivot`: the point the
///   object turns and scales around, in its own axes relative to its position (zero = center).
/// - `Brush`: solid shape for building levels. `size` is width, height, depth in meters
///   (for round shapes: x and z are the diameters, y the height). `detail` is the number of
///   segments around a round shape, or the number of steps of a staircase.
/// - `Mesh`: an editable polygon mesh (DR-22, option A): `geometry` holds the vertices and the
///   faces themselves, in the object's own coordinates (meters, before `Transform`), and `color`
///   paints every face. A brush becomes a mesh with "Tornar editável"; booleans, inset, loop cut,
///   bevel and the other modeling operations replace `geometry`, one undo step each.
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
/// - `Collider`: one named collision shape of an object (DR-27). Each collider is its own entity,
///   so an object can have many: `owner` is the object's persistent id and `name` is what scripts
///   use ("pés", "corpo"). `shape` is `box` (`size`), `sphere` (`radius`), `capsule` (`radius`,
///   total `height`) or `convex` (the hull of the owner's mesh). `center` and `rotation_degrees`
///   place it in the owner's own axes; the owner's scale stretches it. A `trigger` reports contacts
///   but nothing bumps into it. `friction` and `restitution` (0 to 1, bounce) are for contacts.
///   `layer` and `mask` are bit sets (a whole number): the collider is on the layers whose bits
///   are set in `layer` and only touches colliders whose `layer` shares a bit with its `mask`
///   (layer 1 and a full mask is "everything touches everything").
/// - `Character`: makes an object walk (DR-28). It needs a solid collider (a capsule is best) and
///   gets a kinematic body. `speed` is meters per second, `jump_speed` the upward speed of a jump,
///   `step_height` the tallest step it climbs, `max_slope_degrees` the steepest slope it walks up,
///   `coyote_seconds` how long a jump still works after leaving a ledge (accessibility aid) and
///   `gravity_scale` its gravity. `player` makes the keyboard drive it in Play mode.
/// - `Light` (DR-29): a light. `kind` is `sun` (parallel rays: only the **rotation** of the object
///   matters, it shines along its forward direction) or `point` (a bulb: the **position** matters,
///   it lights up to `range` meters). `color` and `intensity` (0 to 16) set how strong; `ambient`
///   is the soft light everywhere (the first light's value is used). A scene with no `Light` row
///   uses the built-in sun and ambient light; with at least one, only its lights are used.
/// - `RigidBody`: makes an object move with physics. `kind` is `static`, `dynamic` (gravity and
///   contacts) or `kinematic` (moved by the game, pushes others). `mass` in kilograms,
///   `gravity_scale` (1 normal, 0 floats), damping, and `lock_rotation` to keep it upright.
///   Objects with colliders and no `RigidBody` are static.
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
        TableSpec::new("Mesh")
            .column(
                "geometry",
                ValueKind::Mesh,
                Value::Mesh(PolygonMesh::default()),
            )
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
    world.define_table(
        TableSpec::new("Collider")
            .column("owner", ValueKind::Int, Value::Int(0))
            .column("name", ValueKind::Text, Value::Text("colisão".to_string()))
            .column("shape", ValueKind::Text, Value::Text("box".to_string()))
            .column("center", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
            .column("rotation_degrees", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
            .column("size", ValueKind::Vec3, Value::Vec3(Vec3::ONE))
            .column("radius", ValueKind::Float, Value::Float(0.5))
            .column("height", ValueKind::Float, Value::Float(2.0))
            .column("trigger", ValueKind::Bool, Value::Bool(false))
            .column("friction", ValueKind::Float, Value::Float(0.5))
            .column("restitution", ValueKind::Float, Value::Float(0.0))
            .column("layer", ValueKind::Int, Value::Int(1))
            .column("mask", ValueKind::Int, Value::Int(4_294_967_295)),
    )?;
    world.define_table(
        TableSpec::new("Light")
            .column("kind", ValueKind::Text, Value::Text("point".to_string()))
            .column("color", ValueKind::Color, Value::Color(Color::WHITE))
            .column("intensity", ValueKind::Float, Value::Float(1.0))
            .column("range", ValueKind::Float, Value::Float(10.0))
            .column("ambient", ValueKind::Float, Value::Float(0.25)),
    )?;
    world.define_table(
        TableSpec::new("Character")
            .column("speed", ValueKind::Float, Value::Float(5.0))
            .column("jump_speed", ValueKind::Float, Value::Float(6.5))
            .column("step_height", ValueKind::Float, Value::Float(0.4))
            .column("max_slope_degrees", ValueKind::Float, Value::Float(50.0))
            .column("coyote_seconds", ValueKind::Float, Value::Float(0.12))
            .column("gravity_scale", ValueKind::Float, Value::Float(1.0))
            .column("player", ValueKind::Bool, Value::Bool(false)),
    )?;
    world.define_table(
        TableSpec::new("RigidBody")
            .column("kind", ValueKind::Text, Value::Text("dynamic".to_string()))
            .column("mass", ValueKind::Float, Value::Float(1.0))
            .column("gravity_scale", ValueKind::Float, Value::Float(1.0))
            .column("linear_damping", ValueKind::Float, Value::Float(0.0))
            .column("angular_damping", ValueKind::Float, Value::Float(0.05))
            .column("lock_rotation", ValueKind::Bool, Value::Bool(false)),
    )?;
    Ok(())
}
