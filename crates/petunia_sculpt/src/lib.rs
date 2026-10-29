//! Sculpted volumes ("cavernas"): rock made of a base (a solid box, or empty space) and an
//! ordered list of strokes. The list is what gets saved; everything else is derived from it.
//!
//! A [`Sculpture`] turns a [`Volume`] into meshes. It keeps a baked signed-distance field (negative
//! inside rock) on a lattice anchored at the volume's origin, split into chunks of 16 cells. A new
//! stroke updates only the samples it touches and re-meshes only the chunks around it, so the cost
//! of a dab depends on the brush, not on the size of the cave or the number of earlier strokes.
//!
//! Meshing is surface nets: one vertex per cell the surface crosses, one quad per lattice edge it
//! crosses. Pure and deterministic: the same volume gives the same meshes on every machine.
//!
//! Coordinates are local to the volume: the base box is centered on the origin. The caller places
//! the meshes in the world with the volume's transform.

use std::fmt;

use petunia_math::Vec3;

mod field;
mod generate;
mod path;
mod simplify;
mod stamp;

pub use field::{ChunkMesh, Sculpture, Update};
pub use generate::{GenerateParams, generate_cave};
pub use path::tunnel_strokes;
pub use simplify::consolidate;

/// A volume keeps at most this many strokes.
pub const MAX_STROKES: usize = 4096;
/// Smallest lattice cell (meters). Smaller requests are raised to this.
pub const MIN_CELL_SIZE: f32 = 0.1;
/// The lattice never has more samples than this along one axis, nor in total: a request that would
/// exceed them is meshed with coarser cells instead.
pub const MAX_SAMPLES_PER_AXIS: usize = 192;
pub const MAX_SAMPLES: usize = 1_200_000;
/// Largest base box side and stroke radius (meters).
pub const MAX_SIZE: f32 = 512.0;
pub const MAX_RADIUS: f32 = 64.0;
/// Largest blend between a stroke and the rock around it (meters). It bounds how far a stroke can
/// reach beyond its radius, which is what keeps one dab cheap.
pub const MAX_SMOOTH: f32 = 2.0;
/// A volume keeps at most this many mesh stamps, each of at most this many triangles. Stamps are
/// re-read every frame, so they are for decoration and detail, not for scanned scenes.
pub const MAX_STAMPS: usize = 8;
pub const MAX_STAMP_TRIANGLES: usize = 4096;

/// What fills the volume before any stroke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    /// Nothing: strokes that add rock build it up (rocks, cliffs, overhangs).
    Empty,
    /// A box of rock of the volume's size: strokes that carve dig tunnels (caves).
    Solid,
}

impl Base {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "empty" => Some(Self::Empty),
            "solid" => Some(Self::Solid),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Solid => "solid",
        }
    }
}

/// What a stroke does to the rock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeMode {
    /// Adds rock inside a capsule from `start` to `end`.
    Add,
    /// Carves a capsule from `start` to `end` out of the rock.
    Carve,
    /// Pushes the surface near the capsule out (`strength` > 0, meters) or in (< 0).
    Inflate,
    /// Pulls the surface toward the plane through `start` whose normal points from `start` to
    /// `end`, within `radius` of `start`. `strength` is 0..1.
    Flatten,
    /// Blurs the field within `radius` of `end`. `strength` is 0..1.
    Smooth,
}

impl StrokeMode {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "add" => Some(Self::Add),
            "carve" => Some(Self::Carve),
            "inflate" => Some(Self::Inflate),
            "flatten" => Some(Self::Flatten),
            "smooth" => Some(Self::Smooth),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Carve => "carve",
            Self::Inflate => "inflate",
            Self::Flatten => "flatten",
            Self::Smooth => "smooth",
        }
    }
}

/// One stroke. See [`StrokeMode`] for how each mode reads the fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub mode: StrokeMode,
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32,
    /// Meters over which an add or carve blends with the rock around it (`0` is a sharp edge).
    pub smooth: f32,
    pub strength: f32,
}

impl Stroke {
    /// A capsule stroke (a sphere when both ends meet) with no extra strength.
    pub fn capsule(mode: StrokeMode, start: Vec3, end: Vec3, radius: f32, smooth: f32) -> Self {
        Self {
            mode,
            start,
            end,
            radius,
            smooth,
            strength: 0.0,
        }
    }
}

/// Which way a mesh stamp edits the rock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StampOperation {
    /// Rock behind the face: a relief, a plate, the outside of an imported model.
    Add,
    /// Air behind the face: a groove, a carved pattern, a piece of a model dug into the rock.
    Carve,
}

impl StampOperation {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "add" => Some(Self::Add),
            "carve" => Some(Self::Carve),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Carve => "carve",
        }
    }
}

/// A triangle surface imprinted on the rock, after the strokes.
#[derive(Clone, Debug, PartialEq)]
pub struct Stamp {
    pub operation: StampOperation,
    /// Triangles in the volume's own space (the editor bakes the source transform in).
    pub triangles: Vec<[Vec3; 3]>,
    /// Meters over which the stamp blends with the rock around it (`0` is a sharp edge).
    pub smooth: f32,
}

/// A whole sculpted volume. Strokes apply in order, then the stamps, in their own order.
#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    pub base: Base,
    /// Width, height and depth of the base box (meters). Used by the solid base only.
    pub size: Vec3,
    pub cell_size: f32,
    pub strokes: Vec<Stroke>,
    pub stamps: Vec<Stamp>,
}

/// Triangles ready for a renderer: positions and unit normals per vertex, three indices per
/// triangle, counter-clockwise when seen from the air.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SculptError {
    TooManyStrokes(usize),
    Invalid(&'static str),
}

impl fmt::Display for SculptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyStrokes(count) => write!(
                formatter,
                "o volume tem {count} traços; o máximo é {MAX_STROKES}"
            ),
            Self::Invalid(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for SculptError {}

// ---- distance functions ---------------------------------------------------------------------

pub(crate) fn absolute(vector: Vec3) -> Vec3 {
    Vec3::new(vector.x.abs(), vector.y.abs(), vector.z.abs())
}

pub(crate) fn component_max(first: Vec3, second: Vec3) -> Vec3 {
    Vec3::new(
        first.x.max(second.x),
        first.y.max(second.y),
        first.z.max(second.z),
    )
}

pub(crate) fn component_min(first: Vec3, second: Vec3) -> Vec3 {
    Vec3::new(
        first.x.min(second.x),
        first.y.min(second.y),
        first.z.min(second.z),
    )
}

/// Signed distance to a box centered on the origin.
pub(crate) fn box_distance(point: Vec3, half_size: Vec3) -> f32 {
    let outside = absolute(point) - half_size;
    let positive = component_max(outside, Vec3::ZERO).length();
    let inside = outside.x.max(outside.y).max(outside.z).min(0.0);
    positive + inside
}

/// Distance from `point` to the axis of the stroke's capsule (a segment, or a point).
pub(crate) fn axis_distance(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let along = end - start;
    let from_start = point - start;
    let length_squared = along.length_squared();
    let parameter = if length_squared > 0.0 {
        (from_start.dot(along) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (from_start - along * parameter).length()
}

/// Polynomial smooth minimum: like `min`, rounded over `blend` meters.
pub(crate) fn smooth_min(first: f32, second: f32, blend: f32) -> f32 {
    if blend <= 0.0 {
        return first.min(second);
    }
    let weight = (blend - (first - second).abs()).max(0.0) / blend;
    first.min(second) - weight * weight * blend * 0.25
}

pub(crate) fn smooth_max(first: f32, second: f32, blend: f32) -> f32 {
    -smooth_min(-first, -second, blend)
}

/// Brush falloff: 1 at the center, 0 at the radius, smooth in between.
pub(crate) fn falloff(distance: f32, radius: f32) -> f32 {
    let inside = (1.0 - distance / radius).clamp(0.0, 1.0);
    inside * inside * (3.0 - 2.0 * inside)
}

/// The plane normal of a flatten stroke (up when the ends meet).
pub(crate) fn flatten_normal(stroke: &Stroke) -> Vec3 {
    let normal = (stroke.end - stroke.start).normalized_or_zero();
    if normal.length_squared() > 0.0 {
        normal
    } else {
        Vec3::UP
    }
}

/// What one stroke does to the distance `value` at `point`. Smooth needs the neighbors, so it is
/// handled on the lattice and leaves a lone value alone here.
pub(crate) fn apply_pointwise(value: f32, point: Vec3, stroke: &Stroke) -> f32 {
    match stroke.mode {
        StrokeMode::Add => {
            let distance = axis_distance(point, stroke.start, stroke.end) - stroke.radius;
            smooth_min(value, distance, stroke.smooth)
        }
        StrokeMode::Carve => {
            let distance = axis_distance(point, stroke.start, stroke.end) - stroke.radius;
            smooth_max(value, -distance, stroke.smooth)
        }
        StrokeMode::Inflate => {
            let weight = falloff(
                axis_distance(point, stroke.start, stroke.end),
                stroke.radius,
            );
            value - stroke.strength * weight
        }
        StrokeMode::Flatten => {
            let weight = falloff((point - stroke.start).length(), stroke.radius);
            let plane = (point - stroke.start).dot(flatten_normal(stroke));
            value + (plane - value) * weight * stroke.strength.clamp(0.0, 1.0)
        }
        StrokeMode::Smooth => value,
    }
}

impl Volume {
    /// Checks the numbers before any work: finite values, sizes and counts within limits.
    pub fn validate(&self) -> Result<(), SculptError> {
        if self.strokes.len() > MAX_STROKES {
            return Err(SculptError::TooManyStrokes(self.strokes.len()));
        }
        if self.stamps.len() > MAX_STAMPS {
            return Err(SculptError::Invalid("o volume passa de 8 carimbos"));
        }
        for stamp in &self.stamps {
            if stamp.triangles.len() > MAX_STAMP_TRIANGLES {
                return Err(SculptError::Invalid("um carimbo passa de 4096 triângulos"));
            }
            if !(stamp.smooth.is_finite() && (0.0..=MAX_SMOOTH).contains(&stamp.smooth)) {
                return Err(SculptError::Invalid(
                    "a suavização de um carimbo vai de 0 a 2 metros",
                ));
            }
            let far = MAX_SIZE * 2.0;
            for triangle in &stamp.triangles {
                for point in triangle {
                    let magnitude = absolute(*point);
                    if !magnitude.is_finite()
                        || magnitude.x > far
                        || magnitude.y > far
                        || magnitude.z > far
                    {
                        return Err(SculptError::Invalid(
                            "um carimbo tem pontos não finitos ou longe demais",
                        ));
                    }
                }
            }
        }
        if !(self.size.is_finite() && self.cell_size.is_finite()) {
            return Err(SculptError::Invalid("tamanho ou célula não finitos"));
        }
        if self.base == Base::Solid
            && !(self.size.x > 0.0 && self.size.y > 0.0 && self.size.z > 0.0)
        {
            return Err(SculptError::Invalid(
                "um volume sólido precisa de tamanho positivo",
            ));
        }
        if self.size.x > MAX_SIZE || self.size.y > MAX_SIZE || self.size.z > MAX_SIZE {
            return Err(SculptError::Invalid("o volume passa de 512 metros"));
        }
        let far = MAX_SIZE * 2.0;
        for stroke in &self.strokes {
            let finite = stroke.start.is_finite()
                && stroke.end.is_finite()
                && stroke.radius.is_finite()
                && stroke.smooth.is_finite()
                && stroke.strength.is_finite();
            if !finite {
                return Err(SculptError::Invalid("um traço tem valores não finitos"));
            }
            if !(stroke.radius > 0.0 && stroke.radius <= MAX_RADIUS) {
                return Err(SculptError::Invalid(
                    "o raio de um traço vai de 0 a 64 metros",
                ));
            }
            if !(0.0..=MAX_SMOOTH).contains(&stroke.smooth) {
                return Err(SculptError::Invalid(
                    "a suavização de um traço vai de 0 a 2 metros",
                ));
            }
            if stroke.strength.abs() > MAX_RADIUS {
                return Err(SculptError::Invalid("a força de um traço passa de 64"));
            }
            let outside = |point: Vec3| {
                let magnitude = absolute(point);
                magnitude.x > far || magnitude.y > far || magnitude.z > far
            };
            if outside(stroke.start) || outside(stroke.end) {
                return Err(SculptError::Invalid("um traço está longe demais do volume"));
            }
        }
        Ok(())
    }

    /// Exact signed distance at a local point (negative inside rock), by walking the strokes and
    /// then the stamps. Slow for many strokes and blind to Smooth strokes (they need the lattice);
    /// meshing uses the baked field instead.
    pub fn distance(&self, point: Vec3) -> f32 {
        let mut value = match self.base {
            Base::Solid => box_distance(point, self.size * 0.5),
            Base::Empty => f32::MAX,
        };
        for stroke in &self.strokes {
            value = apply_pointwise(value, point, stroke);
        }
        for stamp in &self.stamps {
            value = stamp::stamped_value(value, point, stamp, self.cell_size);
        }
        value
    }

    /// The whole surface as one welded mesh (vertices shared across chunks). Convenient for tests
    /// and export; renderers use the per-chunk meshes of a [`Sculpture`].
    pub fn mesh(&self) -> Result<SurfaceMesh, SculptError> {
        let mut sculpture = Sculpture::new();
        sculpture.update(self)?;
        Ok(sculpture.merged_mesh())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn sphere(mode: StrokeMode, center: Vec3, radius: f32) -> Stroke {
        Stroke::capsule(mode, center, center, radius, 0.0)
    }

    fn solid(size: Vec3, strokes: Vec<Stroke>) -> Volume {
        Volume {
            base: Base::Solid,
            size,
            cell_size: 0.5,
            strokes,
            stamps: Vec::new(),
        }
    }

    fn vertex(mesh: &SurfaceMesh, index: usize) -> (Vec3, Vec3) {
        let [x, y, z] = mesh.positions[index];
        let [a, b, c] = mesh.normals[index];
        (Vec3::new(x, y, z), Vec3::new(a, b, c))
    }

    /// Every edge of a closed, consistently wound mesh is used once in each direction.
    fn is_closed(mesh: &SurfaceMesh) -> bool {
        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for triangle in mesh.indices.as_chunks::<3>().0 {
            for (from, to) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                *edges.entry((from, to)).or_default() += 1;
            }
        }
        edges
            .iter()
            .all(|((from, to), count)| *count == 1 && edges.get(&(*to, *from)) == Some(&1))
    }

    #[test]
    fn an_added_sphere_is_a_closed_ball_facing_out() {
        let center = Vec3::new(1.0, 2.0, -1.0);
        let volume = Volume {
            base: Base::Empty,
            size: Vec3::ONE,
            cell_size: 0.25,
            strokes: vec![sphere(StrokeMode::Add, center, 2.0)],
            stamps: Vec::new(),
        };
        let mesh = volume.mesh().expect("mesh");
        assert!(mesh.indices.len() > 300);
        assert!(is_closed(&mesh));
        for index in 0..mesh.positions.len() {
            let (position, normal) = vertex(&mesh, index);
            let offset = position - center;
            assert!((offset.length() - 2.0).abs() < 0.25, "{offset:?}");
            assert!(normal.dot(offset.normalized_or_zero()) > 0.8);
        }
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let (a, normal) = vertex(&mesh, triangle[0] as usize);
            let (b, _) = vertex(&mesh, triangle[1] as usize);
            let (c, _) = vertex(&mesh, triangle[2] as usize);
            assert!((b - a).cross(c - a).dot(normal) > 0.0);
        }
    }

    #[test]
    fn carving_a_solid_block_leaves_a_hollow_facing_inward() {
        let volume = solid(
            Vec3::new(8.0, 8.0, 8.0),
            vec![sphere(StrokeMode::Carve, Vec3::ZERO, 2.0)],
        );
        assert!(volume.distance(Vec3::ZERO) > 0.0, "the center is air");
        assert!(
            volume.distance(Vec3::new(3.0, 0.0, 0.0)) < 0.0,
            "rock around it"
        );
        let mesh = volume.mesh().expect("mesh");
        assert!(is_closed(&mesh));
        let cavity: Vec<usize> = (0..mesh.positions.len())
            .filter(|index| vertex(&mesh, *index).0.length() < 3.0)
            .collect();
        assert!(cavity.len() > 20);
        for index in cavity {
            let (position, normal) = vertex(&mesh, index);
            assert!((position.length() - 2.0).abs() < 0.5);
            assert!(
                normal.dot(-position.normalized_or_zero()) > 0.7,
                "faces the air inside"
            );
        }
    }

    #[test]
    fn a_tunnel_through_the_block_opens_both_sides() {
        let volume = solid(
            Vec3::new(10.0, 6.0, 6.0),
            vec![Stroke::capsule(
                StrokeMode::Carve,
                Vec3::new(-6.0, 0.0, 0.0),
                Vec3::new(6.0, 0.0, 0.0),
                1.5,
                0.5,
            )],
        );
        assert!(volume.distance(Vec3::new(-5.0, 0.0, 0.0)) > 0.0);
        assert!(volume.distance(Vec3::new(5.0, 0.0, 0.0)) > 0.0);
        assert!(is_closed(&volume.mesh().expect("mesh")));
    }

    #[test]
    fn a_stamp_carves_the_side_its_face_looks_away_from() {
        let plate = Stamp {
            operation: StampOperation::Carve,
            triangles: vec![[
                Vec3::new(-3.0, 0.0, -3.0),
                Vec3::new(-3.0, 0.0, 3.0),
                Vec3::new(3.0, 0.0, 3.0),
            ]],
            smooth: 0.0,
        };
        let volume = Volume {
            stamps: vec![plate],
            ..solid(Vec3::new(16.0, 6.0, 16.0), Vec::new())
        };
        let below = volume.distance(Vec3::new(0.0, -0.5, 0.0));
        let above = volume.distance(Vec3::new(0.0, 0.5, 0.0));
        let outside = volume.distance(Vec3::new(6.0, -0.5, 0.0));
        assert!(below > 0.0, "air below the plate: {below}");
        assert!(above < 0.0, "rock above the plate: {above}");
        assert!(outside < 0.0, "rock outside the plate: {outside}");
        assert!(volume.mesh().expect("mesh").indices.len() > 6);
    }

    #[test]
    fn meshing_is_deterministic() {
        let volume = solid(
            Vec3::new(6.0, 4.0, 6.0),
            vec![sphere(StrokeMode::Carve, Vec3::new(1.0, 0.5, 0.0), 1.5)],
        );
        assert_eq!(volume.mesh(), volume.mesh());
    }

    #[test]
    fn bad_numbers_and_huge_requests_are_refused_or_coarsened() {
        let mut volume = solid(
            Vec3::new(4.0, 4.0, 4.0),
            vec![sphere(
                StrokeMode::Carve,
                Vec3::new(f32::NAN, 0.0, 0.0),
                1.0,
            )],
        );
        assert!(volume.mesh().is_err());
        volume.strokes = vec![sphere(StrokeMode::Carve, Vec3::ZERO, 1.0); MAX_STROKES + 1];
        assert_eq!(
            volume.mesh(),
            Err(SculptError::TooManyStrokes(MAX_STROKES + 1))
        );
        let mut blurry = solid(Vec3::ONE, vec![sphere(StrokeMode::Carve, Vec3::ZERO, 0.2)]);
        blurry.strokes[0].smooth = MAX_SMOOTH + 1.0;
        assert!(blurry.mesh().is_err());
        // 400 m at 0.1 m would be 64 billion samples: meshed with coarser cells instead.
        let huge = Volume {
            cell_size: 0.1,
            ..solid(Vec3::new(400.0, 400.0, 400.0), Vec::new())
        };
        assert!(!huge.mesh().expect("coarse mesh").indices.is_empty());
        let empty = Volume {
            base: Base::Empty,
            ..solid(Vec3::ONE, vec![sphere(StrokeMode::Carve, Vec3::ZERO, 1.0)])
        };
        assert!(empty.mesh().expect("mesh").indices.is_empty());
    }
}
