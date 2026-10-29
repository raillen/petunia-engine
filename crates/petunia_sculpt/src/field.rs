//! The baked field of a sculpted volume and its chunked meshes.
//!
//! The field lives on a lattice anchored at the volume's origin (sample `i` sits at `i * cell`), so
//! it does not move when the volume grows. Values are clamped to `-T..=T` (`T` = three cells plus
//! the largest blend): far from the rock they carry no information, and the clamp is what lets a
//! stroke touch only the samples around it.
//!
//! Chunks own 16 × 16 × 16 cells. A chunk needs one extra sample on every side to mesh without
//! cracks, so it reads 18 × 18 × 18 samples and emits only the quads whose lower sample it owns:
//! every quad of the surface is emitted by exactly one chunk, and the vertices on a shared border
//! are computed from the same eight samples on both sides, so they match to the bit.

// Indexing here is on fixed 3-element arrays (`axis` runs over `0..3`) and on the 18-sample chunk
// buffers (loops run over `0..PADDED` / `0..CELLS`): every index is in range by construction, and
// `.get()` in the hot loops would only add a branch that can never fail.
#![allow(clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use petunia_math::Vec3;

use crate::stamp::{combine, reach, stamp_box, triangle_box, triangle_distance};
use crate::{
    Base, MAX_SAMPLES, MAX_SAMPLES_PER_AXIS, MAX_SMOOTH, MIN_CELL_SIZE, SculptError, Stamp,
    StampOperation, Stroke, StrokeMode, SurfaceMesh, Volume, apply_pointwise, box_distance,
    component_max, component_min, falloff,
};

/// Cells per chunk edge.
const CHUNK: i32 = 16;
/// Samples a chunk reads per axis, and cells whose vertex it computes per axis.
const PADDED: usize = CHUNK as usize + 2;
const CELLS: usize = PADDED - 1;
/// Cells of air kept around the rock so the outer faces close.
const MARGIN: i32 = 3;
/// Samples closer to the surface than this (meters) are treated as air.
const ON_SURFACE: f32 = 1.0e-3;
/// Replaying fewer strokes than this is not worth starting threads for.
const PARALLEL_REPLAY_STROKES: usize = 32;
/// A dab never starts more than this many meshing threads.
const MAX_MESH_THREADS: usize = 4;

static NEXT_MESH_ID: AtomicU64 = AtomicU64::new(1);

type Region = (Vec3, Vec3);

/// The surface inside one chunk, ready to upload. `id` is new every time the chunk is re-meshed,
/// so a renderer can cache GPU buffers by it.
#[derive(Clone, Debug)]
pub struct ChunkMesh {
    pub id: u64,
    pub min: Vec3,
    pub max: Vec3,
    pub mesh: Arc<SurfaceMesh>,
}

impl PartialEq for ChunkMesh {
    /// Two chunk meshes are the same when they are the same build: the id is new on every re-mesh.
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// What an [`Sculpture::update`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Update {
    /// The whole field was recomputed (first build, undo, or a change that is not an append).
    pub replayed: bool,
    pub chunks_meshed: usize,
    pub chunks_total: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    cell: f32,
    lo: [i32; 3],
    dims: [usize; 3],
    truncation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    base: Base,
    size_bits: [u32; 3],
    cell_bits: u32,
}

impl Settings {
    fn of(volume: &Volume, cell: f32) -> Self {
        Self {
            base: volume.base,
            size_bits: [
                volume.size.x.to_bits(),
                volume.size.y.to_bits(),
                volume.size.z.to_bits(),
            ],
            cell_bits: cell.to_bits(),
        }
    }
}

// ---- regions --------------------------------------------------------------------------------

fn capsule_box(start: Vec3, end: Vec3) -> Region {
    (component_min(start, end), component_max(start, end))
}

fn expand(region: Region, by: f32) -> Region {
    (region.0 - Vec3::splat(by), region.1 + Vec3::splat(by))
}

fn union(first: Region, second: Region) -> Region {
    (
        component_min(first.0, second.0),
        component_max(first.1, second.1),
    )
}

fn intersects(first: Region, second: Region) -> bool {
    first.0.x <= second.1.x
        && first.1.x >= second.0.x
        && first.0.y <= second.1.y
        && first.1.y >= second.0.y
        && first.0.z <= second.1.z
        && first.1.z >= second.0.z
}

/// Where a stroke can create rock (it decides how big the lattice must be).
fn growth_box(stroke: &Stroke) -> Option<Region> {
    match stroke.mode {
        StrokeMode::Add => Some(expand(
            capsule_box(stroke.start, stroke.end),
            stroke.radius + stroke.smooth,
        )),
        StrokeMode::Inflate => Some(expand(
            capsule_box(stroke.start, stroke.end),
            stroke.radius + stroke.strength.abs(),
        )),
        StrokeMode::Flatten => Some(expand((stroke.start, stroke.start), stroke.radius)),
        StrokeMode::Carve | StrokeMode::Smooth => None,
    }
}

/// Every lattice sample this stroke can change.
fn influence_box(stroke: &Stroke, truncation: f32, cell: f32) -> Region {
    match stroke.mode {
        StrokeMode::Add | StrokeMode::Carve => expand(
            capsule_box(stroke.start, stroke.end),
            stroke.radius + stroke.smooth + truncation,
        ),
        StrokeMode::Inflate => expand(capsule_box(stroke.start, stroke.end), stroke.radius),
        StrokeMode::Flatten => expand((stroke.start, stroke.start), stroke.radius),
        StrokeMode::Smooth => expand((stroke.end, stroke.end), stroke.radius + cell),
    }
}

/// Every lattice sample a stamp can change: its box and the band around it.
fn stamp_surface_box(stamp: &Stamp, cell: f32) -> Region {
    match stamp_box(stamp) {
        Some(boxed) => expand(boxed, reach(stamp, cell) + 2.0 * cell),
        None => (Vec3::ZERO, Vec3::ZERO),
    }
}

/// Where the surface can change when this stroke is the newest one: what to re-mesh.
fn surface_box(stroke: &Stroke, cell: f32) -> Region {
    let margin = 2.0 * cell;
    match stroke.mode {
        StrokeMode::Add | StrokeMode::Carve => expand(
            capsule_box(stroke.start, stroke.end),
            stroke.radius + stroke.smooth + margin,
        ),
        StrokeMode::Inflate => expand(
            capsule_box(stroke.start, stroke.end),
            stroke.radius + stroke.strength.abs() + margin,
        ),
        StrokeMode::Flatten => expand((stroke.start, stroke.start), stroke.radius + margin),
        StrokeMode::Smooth => expand((stroke.end, stroke.end), stroke.radius + margin + cell),
    }
}

fn floor_index(value: f32, cell: f32) -> i32 {
    (value / cell).floor() as i32
}

fn ceil_index(value: f32, cell: f32) -> i32 {
    (value / cell).ceil() as i32
}

fn div_floor(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

impl Layout {
    /// The lattice for a volume: big enough for the base and every stroke that can create rock,
    /// coarsened (deterministically) until it fits the sample limits. `None` for a volume with no
    /// rock at all.
    fn of(volume: &Volume) -> Option<Self> {
        let mut region: Option<Region> = match volume.base {
            Base::Solid => Some((volume.size * -0.5, volume.size * 0.5)),
            Base::Empty => None,
        };
        for stroke in &volume.strokes {
            if let Some(grown) = growth_box(stroke) {
                region = Some(region.map_or(grown, |known| union(known, grown)));
            }
        }
        for stamp in &volume.stamps {
            if stamp.operation == StampOperation::Add
                && let Some(boxed) = stamp_box(stamp)
            {
                let grown = expand(boxed, stamp.smooth + 1.0);
                region = Some(region.map_or(grown, |known| union(known, grown)));
            }
        }
        let (low, high) = region?;
        let mut cell = volume.cell_size.max(MIN_CELL_SIZE);
        loop {
            let lo = [
                floor_index(low.x, cell) - MARGIN,
                floor_index(low.y, cell) - MARGIN,
                floor_index(low.z, cell) - MARGIN,
            ];
            let hi = [
                ceil_index(high.x, cell) + MARGIN,
                ceil_index(high.y, cell) + MARGIN,
                ceil_index(high.z, cell) + MARGIN,
            ];
            let dims = [
                (hi[0] - lo[0] + 1).max(1) as usize,
                (hi[1] - lo[1] + 1).max(1) as usize,
                (hi[2] - lo[2] + 1).max(1) as usize,
            ];
            let samples = dims.iter().product::<usize>();
            if dims.iter().all(|dim| *dim <= MAX_SAMPLES_PER_AXIS) && samples <= MAX_SAMPLES {
                return Some(Self {
                    cell,
                    lo,
                    dims,
                    truncation: 3.0 * cell + MAX_SMOOTH,
                });
            }
            cell *= 1.25;
        }
    }

    fn sample_count(&self) -> usize {
        self.dims.iter().product()
    }

    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.dims[0] * (j + self.dims[1] * k)
    }

    /// Position of the sample at lattice offset `(i, j, k)` from the first one.
    fn position(&self, i: usize, j: usize, k: usize) -> Vec3 {
        Vec3::new(
            (self.lo[0] + i as i32) as f32 * self.cell,
            (self.lo[1] + j as i32) as f32 * self.cell,
            (self.lo[2] + k as i32) as f32 * self.cell,
        )
    }

    /// Inclusive sample offsets covering `region`, or `None` when it misses the lattice.
    fn range(&self, region: Region) -> Option<[(usize, usize); 3]> {
        let low = [region.0.x, region.0.y, region.0.z];
        let high = [region.1.x, region.1.y, region.1.z];
        let mut out = [(0, 0); 3];
        for axis in 0..3 {
            let first = (floor_index(*low.get(axis)?, self.cell) - self.lo[axis]).max(0);
            let last = (ceil_index(*high.get(axis)?, self.cell) - self.lo[axis])
                .min(self.dims[axis] as i32 - 1);
            if first > last {
                return None;
            }
            *out.get_mut(axis)? = (first as usize, last as usize);
        }
        Some(out)
    }

    /// Chunk coordinates that can hold surface of this lattice.
    fn chunk_range(&self) -> ([i32; 3], [i32; 3]) {
        let mut first = [0; 3];
        let mut last = [0; 3];
        for axis in 0..3 {
            first[axis] = div_floor(self.lo[axis] - 1, CHUNK);
            last[axis] = div_floor(self.lo[axis] + self.dims[axis] as i32, CHUNK);
        }
        (first, last)
    }

    /// The field value at a global lattice index; air outside the lattice.
    fn sample(&self, field: &[f32], global: [i32; 3]) -> f32 {
        let mut local = [0usize; 3];
        for axis in 0..3 {
            let offset = global[axis] - self.lo[axis];
            if offset < 0 || offset >= self.dims[axis] as i32 {
                return self.truncation;
            }
            local[axis] = offset as usize;
        }
        field
            .get(self.index(local[0], local[1], local[2]))
            .copied()
            .unwrap_or(self.truncation)
    }
}

// ---- the sculpture --------------------------------------------------------------------------

/// The baked field and chunk meshes of one volume. Keep one per volume and call
/// [`update`](Self::update) every time the volume may have changed: it works out what changed.
#[derive(Default)]
pub struct Sculpture {
    settings: Option<Settings>,
    layout: Option<Layout>,
    strokes: Vec<Stroke>,
    stamps: Vec<Stamp>,
    field: Vec<f32>,
    chunks: BTreeMap<[i32; 3], ChunkMesh>,
}

impl Sculpture {
    pub fn new() -> Self {
        Self::default()
    }

    /// Brings the field and the meshes up to date with `volume`.
    ///
    /// - New strokes at the end of the list are applied on top of the field, and only the chunks
    ///   around them are re-meshed (a brush dab).
    /// - Anything else (undo, an edited stroke, a different base) recomputes the field from the
    ///   strokes, and re-meshes only the chunks around what differs.
    pub fn update(&mut self, volume: &Volume) -> Result<Update, SculptError> {
        volume.validate()?;
        let Some(layout) = Layout::of(volume) else {
            *self = Self::new();
            return Ok(Update::default());
        };
        let settings = Settings::of(volume, layout.cell);
        let fresh = self.settings != Some(settings) || self.layout.is_none();
        let same_layout = !fresh && self.layout == Some(layout);
        let strokes_same =
            volume.strokes.get(..self.strokes.len()) == Some(self.strokes.as_slice());
        let stamps_same = volume.stamps.get(..self.stamps.len()) == Some(self.stamps.as_slice());
        let appended = same_layout && strokes_same && stamps_same;

        let mut dirty: Vec<Region> = Vec::new();
        if appended {
            for stroke in volume.strokes.iter().skip(self.strokes.len()) {
                self.apply(&layout, stroke);
                dirty.push(surface_box(stroke, layout.cell));
            }
            for stamp in volume.stamps.iter().skip(self.stamps.len()) {
                self.apply_stamp(&layout, stamp);
                dirty.push(stamp_surface_box(stamp, layout.cell));
            }
        } else {
            if fresh {
                self.chunks.clear();
                let far = layout.position(layout.dims[0], layout.dims[1], layout.dims[2]);
                dirty.push((layout.position(0, 0, 0), far));
            } else if self.stamps != volume.stamps {
                // A stamp reaches across its whole box: re-mesh everything rather than guess which
                // chunks it may have touched.
                let far = layout.position(layout.dims[0], layout.dims[1], layout.dims[2]);
                dirty.push((layout.position(0, 0, 0), far));
            } else {
                dirty = diff_dirty(&self.strokes, &volume.strokes, layout.cell);
            }
            self.rebuild(volume, layout);
        }
        self.strokes = volume.strokes.clone();
        self.stamps = volume.stamps.clone();
        self.settings = Some(settings);
        self.layout = Some(layout);

        // Chunks outside a smaller lattice are gone (an undone stroke had grown it).
        let (first, last) = layout.chunk_range();
        let inside = |chunk: &[i32; 3]| {
            (0..3).all(|axis| chunk[axis] >= first[axis] && chunk[axis] <= last[axis])
        };
        self.chunks.retain(|chunk, _| inside(chunk));

        let mut wanted: BTreeSet<[i32; 3]> = BTreeSet::new();
        for region in &dirty {
            let mut low = [0; 3];
            let mut high = [0; 3];
            let corners = [
                [region.0.x, region.0.y, region.0.z],
                [region.1.x, region.1.y, region.1.z],
            ];
            for axis in 0..3 {
                low[axis] = div_floor(floor_index(corners[0][axis], layout.cell) - 1, CHUNK)
                    .max(first[axis]);
                high[axis] =
                    div_floor(ceil_index(corners[1][axis], layout.cell) + 1, CHUNK).min(last[axis]);
            }
            for x in low[0]..=high[0] {
                for y in low[1]..=high[1] {
                    for z in low[2]..=high[2] {
                        wanted.insert([x, y, z]);
                    }
                }
            }
        }
        let coords: Vec<[i32; 3]> = wanted.into_iter().collect();
        let meshes = self.mesh_chunks(&layout, &coords);
        for (coord, mesh) in coords.iter().zip(meshes) {
            match mesh {
                Some(chunk) => {
                    self.chunks.insert(*coord, chunk);
                }
                None => {
                    self.chunks.remove(coord);
                }
            }
        }
        Ok(Update {
            replayed: !appended,
            chunks_meshed: coords.len(),
            chunks_total: self.chunks.len(),
        })
    }

    /// The chunk meshes, in a stable order.
    pub fn chunks(&self) -> impl Iterator<Item = &ChunkMesh> {
        self.chunks.values()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Interpolated distance from the baked field (negative inside rock). `None` outside the
    /// lattice or before the first update. Near the rock it matches [`Volume::distance`] to
    /// within a cell; far from it the value is clamped.
    pub fn distance_at(&self, point: Vec3) -> Option<f32> {
        let layout = self.layout?;
        let scaled = point * (1.0 / layout.cell);
        let base = [
            scaled.x.floor() as i32,
            scaled.y.floor() as i32,
            scaled.z.floor() as i32,
        ];
        let fraction = [
            scaled.x - base[0] as f32,
            scaled.y - base[1] as f32,
            scaled.z - base[2] as f32,
        ];
        let inside = (0..3).all(|axis| {
            let offset = base[axis] - layout.lo[axis];
            offset >= 0 && offset + 1 < layout.dims[axis] as i32
        });
        if !inside {
            return None;
        }
        let mut value = 0.0;
        for corner in 0..8usize {
            let (dx, dy, dz) = (corner & 1, (corner >> 1) & 1, (corner >> 2) & 1);
            let weight = [dx, dy, dz]
                .iter()
                .zip(fraction)
                .map(|(bit, fraction)| if *bit == 1 { fraction } else { 1.0 - fraction })
                .product::<f32>();
            let sample = layout.sample(
                &self.field,
                [
                    base[0] + dx as i32,
                    base[1] + dy as i32,
                    base[2] + dz as i32,
                ],
            );
            value += sample * weight;
        }
        Some(value)
    }

    /// The whole surface as one mesh with the vertices on chunk borders welded together.
    pub fn merged_mesh(&self) -> SurfaceMesh {
        let mut merged = SurfaceMesh::default();
        let mut known: HashMap<[u32; 3], u32> = HashMap::new();
        for chunk in self.chunks.values() {
            let mut remap = Vec::with_capacity(chunk.mesh.positions.len());
            for (position, normal) in chunk.mesh.positions.iter().zip(&chunk.mesh.normals) {
                let key = position.map(f32::to_bits);
                let index = *known.entry(key).or_insert_with(|| {
                    merged.positions.push(*position);
                    merged.normals.push(*normal);
                    (merged.positions.len() - 1) as u32
                });
                remap.push(index);
            }
            for index in &chunk.mesh.indices {
                if let Some(mapped) = remap.get(*index as usize) {
                    merged.indices.push(*mapped);
                }
            }
        }
        merged
    }

    // ---- field ------------------------------------------------------------------------------

    fn rebuild(&mut self, volume: &Volume, layout: Layout) {
        let half = volume.size * 0.5;
        let truncation = layout.truncation;
        let mut field = Vec::with_capacity(layout.sample_count());
        for k in 0..layout.dims[2] {
            for j in 0..layout.dims[1] {
                for i in 0..layout.dims[0] {
                    field.push(match volume.base {
                        Base::Solid => box_distance(layout.position(i, j, k), half)
                            .clamp(-truncation, truncation),
                        Base::Empty => truncation,
                    });
                }
            }
        }
        self.field = field;
        self.layout = Some(layout);
        let workers = std::thread::available_parallelism()
            .map_or(1, |count| count.get())
            .min(MAX_MESH_THREADS);
        let pointwise_only = volume
            .strokes
            .iter()
            .all(|stroke| stroke.mode != StrokeMode::Smooth);
        if workers > 1 && pointwise_only && volume.strokes.len() >= PARALLEL_REPLAY_STROKES {
            self.replay_in_slabs(&layout, &volume.strokes, workers);
        } else {
            for stroke in &volume.strokes {
                self.apply(&layout, stroke);
            }
        }
        for stamp in &volume.stamps {
            self.apply_stamp(&layout, stamp);
        }
    }

    /// Replays the strokes with the field cut into slabs of layers, one per thread. A stroke changes
    /// each sample from that sample's own value alone, so every slab can run the whole list in order
    /// by itself and the result is the same, bit for bit, as running them one after another.
    fn replay_in_slabs(&mut self, layout: &Layout, strokes: &[Stroke], workers: usize) {
        let layer = layout.dims[0] * layout.dims[1];
        let layers_per_slab = layout.dims[2].div_ceil(workers).max(1);
        std::thread::scope(|scope| {
            for (index, slab) in self.field.chunks_mut(layer * layers_per_slab).enumerate() {
                scope.spawn(move || {
                    for stroke in strokes {
                        apply_pointwise_slab(layout, slab, index * layers_per_slab, stroke);
                    }
                });
            }
        });
    }

    /// Imprints one stamp: every sample within the band of each triangle takes the signed distance
    /// to that triangle, combined into the field. Applying triangle by triangle gives the same
    /// result as walking the whole mesh per sample, for the cost of the band instead of the volume.
    fn apply_stamp(&mut self, layout: &Layout, stamp: &Stamp) {
        let band = reach(stamp, layout.cell);
        for triangle in &stamp.triangles {
            let Some((low, high)) = triangle_box(triangle) else {
                continue;
            };
            let Some([(i0, i1), (j0, j1), (k0, k1)]) = layout.range(expand((low, high), band))
            else {
                continue;
            };
            for k in k0..=k1 {
                for j in j0..=j1 {
                    for i in i0..=i1 {
                        let point = layout.position(i, j, k);
                        let distance = triangle_distance(point, triangle);
                        if !distance.is_finite() || distance.abs() > band {
                            continue;
                        }
                        if let Some(slot) = self.field.get_mut(layout.index(i, j, k)) {
                            *slot = combine(*slot, distance, stamp)
                                .clamp(-layout.truncation, layout.truncation);
                        }
                    }
                }
            }
        }
    }

    /// Applies one stroke to the samples it can reach.
    fn apply(&mut self, layout: &Layout, stroke: &Stroke) {
        let region = influence_box(stroke, layout.truncation, layout.cell);
        let Some(range) = layout.range(region) else {
            return;
        };
        if stroke.mode == StrokeMode::Smooth {
            self.apply_smooth(layout, stroke, range);
            return;
        }
        apply_pointwise_slab(layout, &mut self.field, 0, stroke);
    }

    /// Smooth blurs the field with its 3 × 3 × 3 neighbors, weighted by the brush falloff. It
    /// reads a snapshot, so the order the samples are visited in does not matter.
    fn apply_smooth(&mut self, layout: &Layout, stroke: &Stroke, range: [(usize, usize); 3]) {
        let [(i0, i1), (j0, j1), (k0, k1)] = range;
        let last = [layout.dims[0] - 1, layout.dims[1] - 1, layout.dims[2] - 1];
        let from = [
            i0.saturating_sub(1),
            j0.saturating_sub(1),
            k0.saturating_sub(1),
        ];
        let to = [
            (i1 + 1).min(last[0]),
            (j1 + 1).min(last[1]),
            (k1 + 1).min(last[2]),
        ];
        let size = [
            to[0] - from[0] + 1,
            to[1] - from[1] + 1,
            to[2] - from[2] + 1,
        ];
        let mut snapshot = Vec::with_capacity(size.iter().product());
        for k in from[2]..=to[2] {
            for j in from[1]..=to[1] {
                for i in from[0]..=to[0] {
                    snapshot.push(
                        self.field
                            .get(layout.index(i, j, k))
                            .copied()
                            .unwrap_or(layout.truncation),
                    );
                }
            }
        }
        let strength = stroke.strength.clamp(0.0, 1.0);
        for k in k0..=k1 {
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let weight = falloff(
                        (layout.position(i, j, k) - stroke.end).length(),
                        stroke.radius,
                    ) * strength;
                    if weight <= 0.0 {
                        continue;
                    }
                    let mut sum = 0.0;
                    let mut count = 0.0;
                    for nk in k.saturating_sub(1).max(from[2])..=(k + 1).min(to[2]) {
                        for nj in j.saturating_sub(1).max(from[1])..=(j + 1).min(to[1]) {
                            for ni in i.saturating_sub(1).max(from[0])..=(i + 1).min(to[0]) {
                                let at = (ni - from[0])
                                    + size[0] * ((nj - from[1]) + size[1] * (nk - from[2]));
                                sum += snapshot.get(at).copied().unwrap_or(layout.truncation);
                                count += 1.0;
                            }
                        }
                    }
                    if let Some(slot) = self.field.get_mut(layout.index(i, j, k)) {
                        *slot = (*slot + (sum / count - *slot) * weight)
                            .clamp(-layout.truncation, layout.truncation);
                    }
                }
            }
        }
    }

    // ---- meshing ----------------------------------------------------------------------------

    /// Meshes the chunks, spreading them over a few threads when there are enough of them.
    fn mesh_chunks(&self, layout: &Layout, coords: &[[i32; 3]]) -> Vec<Option<ChunkMesh>> {
        let field = self.field.as_slice();
        let workers = std::thread::available_parallelism()
            .map_or(1, |count| count.get())
            .min(MAX_MESH_THREADS)
            .min(coords.len());
        if workers <= 1 || coords.len() < 4 {
            return coords
                .iter()
                .map(|chunk| mesh_chunk(field, layout, *chunk))
                .collect();
        }
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|worker| {
                    scope.spawn(move || {
                        coords
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| index % workers == worker)
                            .map(|(index, chunk)| (index, mesh_chunk(field, layout, *chunk)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            let mut meshes: Vec<Option<ChunkMesh>> = vec![None; coords.len()];
            for handle in handles {
                // A worker that panicked leaves its chunks empty rather than taking the editor down.
                if let Ok(results) = handle.join() {
                    for (index, mesh) in results {
                        if let Some(slot) = meshes.get_mut(index) {
                            *slot = mesh;
                        }
                    }
                }
            }
            meshes
        })
    }
}

/// Applies a pointwise stroke to the samples of `slab`, a run of whole layers of the field that
/// starts at layer `first_layer` (the whole field is the slab that starts at layer 0).
fn apply_pointwise_slab(layout: &Layout, slab: &mut [f32], first_layer: usize, stroke: &Stroke) {
    let region = influence_box(stroke, layout.truncation, layout.cell);
    let Some([(i0, i1), (j0, j1), (k0, k1)]) = layout.range(region) else {
        return;
    };
    let layer = layout.dims[0] * layout.dims[1];
    let layers = slab.len() / layer.max(1);
    let (first, last) = (
        k0.max(first_layer),
        k1.min(first_layer + layers.saturating_sub(1)),
    );
    if layers == 0 || first > last {
        return;
    }
    let truncation = layout.truncation;
    for k in first..=last {
        for j in j0..=j1 {
            for i in i0..=i1 {
                let point = layout.position(i, j, k);
                let at = i + layout.dims[0] * j + layer * (k - first_layer);
                if let Some(slot) = slab.get_mut(at) {
                    *slot = apply_pointwise(*slot, point, stroke).clamp(-truncation, truncation);
                }
            }
        }
    }
}

/// What to re-mesh when the strokes changed in a way that is not an append: the neighborhood of
/// every stroke that was added, removed or edited (plus the largest blend, because later strokes
/// blend with what it left), and of any smooth stroke that reads that neighborhood.
fn diff_dirty(old: &[Stroke], new: &[Stroke], cell: f32) -> Vec<Region> {
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(first, second)| first == second)
        .count();
    let longest = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(longest)
        .take_while(|(first, second)| first == second)
        .count();
    let changed_old = old.get(prefix..old.len() - suffix).unwrap_or(&[]);
    let changed_new = new.get(prefix..new.len() - suffix).unwrap_or(&[]);
    let mut dirty: Vec<Region> = changed_old
        .iter()
        .chain(changed_new)
        .map(|stroke| expand(surface_box(stroke, cell), MAX_SMOOTH))
        .collect();
    for stroke in new.iter().skip(prefix) {
        if stroke.mode == StrokeMode::Smooth {
            let reach = surface_box(stroke, cell);
            if dirty.iter().any(|region| intersects(*region, reach)) {
                dirty.push(reach);
            }
        }
    }
    dirty
}

const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// Meshes one chunk from the field.
fn mesh_chunk(field: &[f32], layout: &Layout, chunk: [i32; 3]) -> Option<ChunkMesh> {
    let base = [
        chunk[0] * CHUNK - 1,
        chunk[1] * CHUNK - 1,
        chunk[2] * CHUNK - 1,
    ];
    let mut samples = vec![0.0f32; PADDED * PADDED * PADDED];
    let (mut negative, mut positive) = (false, false);
    for k in 0..PADDED {
        for j in 0..PADDED {
            for i in 0..PADDED {
                let value = layout.sample(
                    field,
                    [base[0] + i as i32, base[1] + j as i32, base[2] + k as i32],
                );
                // A sample exactly on the surface would put the vertices of every cell around it
                // on the same point (zero-area triangles). Pushing values within a millimeter of
                // zero to the air side keeps each vertex strictly inside its own cell.
                let value = if value.abs() < ON_SURFACE {
                    ON_SURFACE
                } else {
                    value
                };
                samples[i + PADDED * (j + PADDED * k)] = value;
                if value < 0.0 {
                    negative = true;
                } else {
                    positive = true;
                }
            }
        }
    }
    if !(negative && positive) {
        return None;
    }
    let at = |i: usize, j: usize, k: usize| samples[i + PADDED * (j + PADDED * k)];

    // One vertex per cell the surface crosses, at the mean of its edge crossings.
    let mut vertex_of = vec![u32::MAX; CELLS * CELLS * CELLS];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    for c in 0..CELLS {
        for b in 0..CELLS {
            for a in 0..CELLS {
                let mut corner = [0.0f32; 8];
                for (bits, slot) in corner.iter_mut().enumerate() {
                    *slot = at(a + (bits & 1), b + ((bits >> 1) & 1), c + ((bits >> 2) & 1));
                }
                let below = corner.iter().filter(|value| **value < 0.0).count();
                if below == 0 || below == 8 {
                    continue;
                }
                let corner_position = |bits: usize| {
                    Vec3::new(
                        (base[0] + (a + (bits & 1)) as i32) as f32 * layout.cell,
                        (base[1] + (b + ((bits >> 1) & 1)) as i32) as f32 * layout.cell,
                        (base[2] + (c + ((bits >> 2) & 1)) as i32) as f32 * layout.cell,
                    )
                };
                let mut sum = Vec3::ZERO;
                let mut crossings = 0.0f32;
                for (first, second) in EDGES {
                    let (first_value, second_value) = (corner[first], corner[second]);
                    if (first_value < 0.0) != (second_value < 0.0) {
                        let along = first_value / (first_value - second_value);
                        let from = corner_position(first);
                        sum = sum + from + (corner_position(second) - from) * along;
                        crossings += 1.0;
                    }
                }
                let vertex = sum * (1.0 / crossings);
                let gradient = Vec3::new(
                    (corner[1] + corner[3] + corner[5] + corner[7])
                        - (corner[0] + corner[2] + corner[4] + corner[6]),
                    (corner[2] + corner[3] + corner[6] + corner[7])
                        - (corner[0] + corner[1] + corner[4] + corner[5]),
                    (corner[4] + corner[5] + corner[6] + corner[7])
                        - (corner[0] + corner[1] + corner[2] + corner[3]),
                )
                .normalized_or_zero();
                let normal = if gradient.length_squared() > 0.0 {
                    gradient
                } else {
                    Vec3::UP
                };
                vertex_of[a + CELLS * (b + CELLS * c)] = positions.len() as u32;
                positions.push([vertex.x, vertex.y, vertex.z]);
                normals.push([normal.x, normal.y, normal.z]);
            }
        }
    }

    // One quad per lattice edge the surface crosses, joining the four cells around it. Which way
    // the quad faces depends only on which end of the edge is rock, so folded quads at sharp
    // corners stay consistent and the surface stays closed. This chunk emits the edges whose
    // lower sample it owns (local 1..=16).
    let cell_vertex = |i: usize, j: usize, k: usize| vertex_of[i + CELLS * (j + CELLS * k)];
    let mut indices: Vec<u32> = Vec::new();
    let mut emit = |cells: [(usize, usize, usize); 4], reverse: bool| {
        let [a, b, c, d] = cells.map(|(i, j, k)| cell_vertex(i, j, k));
        if [a, b, c, d].contains(&u32::MAX) {
            return;
        }
        if reverse {
            indices.extend_from_slice(&[a, c, b, a, d, c]);
        } else {
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    };
    for k in 1..=CHUNK as usize {
        for j in 1..=CHUNK as usize {
            for i in 1..=CHUNK as usize {
                let rock = at(i, j, k) < 0.0;
                if rock != (at(i + 1, j, k) < 0.0) {
                    emit(
                        [(i, j - 1, k - 1), (i, j, k - 1), (i, j, k), (i, j - 1, k)],
                        !rock,
                    );
                }
                if rock != (at(i, j + 1, k) < 0.0) {
                    emit(
                        [(i - 1, j, k - 1), (i, j, k - 1), (i, j, k), (i - 1, j, k)],
                        rock,
                    );
                }
                if rock != (at(i, j, k + 1) < 0.0) {
                    emit(
                        [(i - 1, j - 1, k), (i, j - 1, k), (i, j, k), (i - 1, j, k)],
                        !rock,
                    );
                }
            }
        }
    }
    if indices.is_empty() {
        return None;
    }

    // Keep only the vertices some quad uses (the padding layer computes many that no quad of
    // this chunk touches).
    let mut remap = vec![u32::MAX; positions.len()];
    let mut mesh = SurfaceMesh::default();
    for index in &mut indices {
        let slot = &mut remap[*index as usize];
        if *slot == u32::MAX {
            *slot = mesh.positions.len() as u32;
            mesh.positions.push(positions[*index as usize]);
            mesh.normals.push(normals[*index as usize]);
        }
        *index = *slot;
    }
    mesh.indices = indices;
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for [x, y, z] in &mesh.positions {
        min = component_min(min, Vec3::new(*x, *y, *z));
        max = component_max(max, Vec3::new(*x, *y, *z));
    }
    Some(ChunkMesh {
        id: NEXT_MESH_ID.fetch_add(1, Ordering::Relaxed),
        min,
        max,
        mesh: Arc::new(mesh),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(mode: StrokeMode, center: Vec3, radius: f32) -> Stroke {
        Stroke::capsule(mode, center, center, radius, 0.0)
    }

    fn block(size: Vec3, strokes: Vec<Stroke>) -> Volume {
        Volume {
            base: Base::Solid,
            size,
            cell_size: 0.5,
            strokes,
            stamps: Vec::new(),
        }
    }

    #[test]
    fn appending_strokes_gives_the_same_surface_as_building_from_scratch() {
        let mut volume = block(Vec3::new(40.0, 10.0, 40.0), Vec::new());
        let mut live = Sculpture::new();
        live.update(&volume).expect("first");
        for index in 0..12 {
            let center = Vec3::new(
                index as f32 * 2.0 - 12.0,
                0.5,
                (index % 3) as f32 * 3.0 - 3.0,
            );
            volume.strokes.push(Stroke::capsule(
                if index % 4 == 3 {
                    StrokeMode::Add
                } else {
                    StrokeMode::Carve
                },
                center,
                center + Vec3::new(1.5, 0.3, 0.8),
                1.6,
                0.6,
            ));
            let update = live.update(&volume).expect("dab");
            assert!(!update.replayed, "an append must not replay");
        }
        let mut fresh = Sculpture::new();
        fresh.update(&volume).expect("fresh");
        assert_eq!(live.merged_mesh(), fresh.merged_mesh());
    }

    #[test]
    fn replaying_many_strokes_in_parallel_gives_the_same_surface_as_one_by_one() {
        let mut volume = block(Vec3::new(48.0, 12.0, 48.0), Vec::new());
        for index in 0..70 {
            let t = index as f32 * 0.7;
            let start = Vec3::new(t.cos() * 15.0, (t * 0.5).sin() * 2.0, t.sin() * 15.0);
            volume.strokes.push(Stroke::capsule(
                if index % 5 == 4 {
                    StrokeMode::Add
                } else {
                    StrokeMode::Carve
                },
                start,
                start + Vec3::new(1.2, 0.4, -0.8),
                1.4,
                0.5,
            ));
        }
        // Built from scratch it replays in slabs; built by appending one stroke at a time it never does.
        let mut parallel = Sculpture::new();
        assert!(parallel.update(&volume).expect("fresh").replayed);
        let mut serial = Sculpture::new();
        let mut growing = Volume {
            strokes: Vec::new(),
            ..volume.clone()
        };
        serial.update(&growing).expect("empty");
        for stroke in &volume.strokes {
            growing.strokes.push(*stroke);
            serial.update(&growing).expect("append");
        }
        assert_eq!(parallel.merged_mesh(), serial.merged_mesh());
    }

    #[test]
    fn a_dab_re_meshes_only_the_chunks_around_it() {
        let mut volume = block(Vec3::new(64.0, 16.0, 64.0), Vec::new());
        let mut sculpture = Sculpture::new();
        let first = sculpture.update(&volume).expect("first");
        assert!(first.chunks_total >= 16, "a big block spans many chunks");
        volume
            .strokes
            .push(sphere(StrokeMode::Carve, Vec3::new(0.0, 3.0, 0.0), 1.5));
        let dab = sculpture.update(&volume).expect("dab");
        assert!(!dab.replayed);
        assert!(
            dab.chunks_meshed * 4 <= first.chunks_total,
            "{} of {} chunks",
            dab.chunks_meshed,
            first.chunks_total
        );
    }

    #[test]
    fn undo_goes_back_to_the_earlier_surface() {
        let mut volume = block(Vec3::new(30.0, 8.0, 30.0), Vec::new());
        volume
            .strokes
            .push(sphere(StrokeMode::Carve, Vec3::new(-6.0, 0.0, 0.0), 2.0));
        let mut sculpture = Sculpture::new();
        sculpture.update(&volume).expect("one");
        let before = sculpture.merged_mesh();
        volume
            .strokes
            .push(sphere(StrokeMode::Add, Vec3::new(8.0, 4.0, 0.0), 3.0));
        sculpture.update(&volume).expect("two");
        assert_ne!(sculpture.merged_mesh(), before);
        volume.strokes.pop();
        let undo = sculpture.update(&volume).expect("undo");
        assert!(undo.replayed);
        assert_eq!(sculpture.merged_mesh(), before);
    }

    #[test]
    fn inflate_grows_the_surface_and_flatten_levels_it() {
        let top = 3.0;
        let mut volume = block(Vec3::new(12.0, 2.0 * top, 12.0), Vec::new());
        let mut sculpture = Sculpture::new();
        sculpture.update(&volume).expect("block");
        let at_top = |sculpture: &Sculpture, height: f32| {
            sculpture
                .distance_at(Vec3::new(0.0, height, 0.0))
                .expect("inside lattice")
        };
        assert!(at_top(&sculpture, top + 0.4) > 0.0, "air above the top");
        volume.strokes.push(Stroke {
            strength: 0.8,
            ..Stroke::capsule(
                StrokeMode::Inflate,
                Vec3::new(0.0, top, 0.0),
                Vec3::new(0.0, top, 0.0),
                3.0,
                0.0,
            )
        });
        sculpture.update(&volume).expect("inflate");
        assert!(
            at_top(&sculpture, top + 0.4) < 0.0,
            "rock grew past the old top"
        );

        // Flatten toward a plane one meter lower: the center ends on that plane.
        volume.strokes.clear();
        volume.strokes.push(Stroke {
            strength: 1.0,
            ..Stroke::capsule(
                StrokeMode::Flatten,
                Vec3::new(0.0, top - 1.0, 0.0),
                Vec3::new(0.0, top, 0.0),
                4.0,
                0.0,
            )
        });
        sculpture.update(&volume).expect("flatten");
        assert!(
            at_top(&sculpture, top - 1.0).abs() < 0.3,
            "the plane is the surface"
        );
        assert!(
            at_top(&sculpture, top - 0.5) > 0.0,
            "the rock above it is gone"
        );
    }

    #[test]
    fn smoothing_keeps_the_surface_closed_and_changes_it() {
        let mut volume = block(
            Vec3::new(10.0, 6.0, 10.0),
            vec![sphere(StrokeMode::Carve, Vec3::new(0.0, 3.0, 0.0), 2.0)],
        );
        let mut sculpture = Sculpture::new();
        sculpture.update(&volume).expect("carved");
        let before = sculpture.merged_mesh();
        volume.strokes.push(Stroke {
            strength: 1.0,
            ..Stroke::capsule(
                StrokeMode::Smooth,
                Vec3::new(0.0, 3.0, 0.0),
                Vec3::new(0.0, 3.0, 0.0),
                3.5,
                0.0,
            )
        });
        sculpture.update(&volume).expect("smooth");
        let after = sculpture.merged_mesh();
        assert_ne!(after, before);
        assert!(!after.indices.is_empty());
    }
}
