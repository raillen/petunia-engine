//! A procedural cave: rooms joined by winding tunnels, with a few dead-end passages and an
//! entrance. It does not mesh anything: it returns **strokes**, the same thing a person makes with
//! the brush, so a generated cave is saved, edited and undone like a hand-made one. The same seed
//! gives the same strokes on every machine (our own random numbers and noise, no dependency).
//!
//! The recipe follows the usual cave-generation literature (rooms as blobs, tunnels as lines
//! between rooms bent by smooth noise so they wind instead of running straight, worms that wander
//! from a room), kept small on purpose.

use std::f32::consts::PI;

use petunia_math::Vec3;

use crate::{MAX_SMOOTH, Stroke, StrokeMode};

/// A generated cave never has more strokes than this (the limit of a volume is far higher; this
/// keeps one click from filling it).
const MAX_GENERATED: usize = 600;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenerateParams {
    pub seed: u64,
    /// Size of the volume the cave goes in (meters); the cave stays inside it.
    pub size: Vec3,
    pub rooms: u32,
    /// Dead-end passages that wander out of a room.
    pub worms: u32,
    pub tunnel_radius: f32,
    /// A tunnel from the first room out through the +Z face.
    pub entrance: bool,
}

impl GenerateParams {
    /// Sensible numbers for a volume: more rooms in a bigger one.
    pub fn for_size(seed: u64, size: Vec3) -> Self {
        let area = (size.x * size.z).max(1.0);
        let rooms = ((area / 40.0).sqrt().round() as u32).clamp(2, 12);
        Self {
            seed,
            size,
            rooms,
            worms: rooms / 2,
            tunnel_radius: 1.4,
            entrance: true,
        }
    }
}

/// Splitmix64: tiny, fast and the same everywhere.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// In `0..1`.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    fn direction(&mut self) -> Vec3 {
        let angle = self.range(0.0, 2.0 * PI);
        let height = self.range(-0.5, 0.5);
        let flat = (1.0 - height * height).sqrt();
        Vec3::new(angle.cos() * flat, height, angle.sin() * flat)
    }
}

/// Value noise in `-1..1`, smooth between lattice points.
fn noise(seed: u64, point: Vec3) -> f32 {
    let lattice = |x: i32, y: i32, z: i32| {
        let mut hash = seed
            ^ (x as u64).wrapping_mul(0x9E37_79B1_85EB_CA87)
            ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
            ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
        hash = (hash ^ (hash >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        hash = (hash ^ (hash >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
        ((hash >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    };
    let (x0, y0, z0) = (point.x.floor(), point.y.floor(), point.z.floor());
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let (fx, fy, fz) = (
        smooth(point.x - x0),
        smooth(point.y - y0),
        smooth(point.z - z0),
    );
    let (x0, y0, z0) = (x0 as i32, y0 as i32, z0 as i32);
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let along_x = |y: i32, z: i32| mix(lattice(x0, y, z), lattice(x0 + 1, y, z), fx);
    let along_y = |z: i32| mix(along_x(y0, z), along_x(y0 + 1, z), fy);
    mix(along_y(z0), along_y(z0 + 1), fz)
}

fn noise_vector(seed: u64, point: Vec3) -> Vec3 {
    Vec3::new(
        noise(seed, point),
        noise(seed.wrapping_add(1), point + Vec3::new(31.4, 0.0, 0.0)),
        noise(seed.wrapping_add(2), point + Vec3::new(0.0, 0.0, 59.2)),
    )
}

fn clamp_inside(point: Vec3, limit: Vec3) -> Vec3 {
    Vec3::new(
        point.x.clamp(-limit.x, limit.x),
        point.y.clamp(-limit.y, limit.y),
        point.z.clamp(-limit.z, limit.z),
    )
}

fn carve(start: Vec3, end: Vec3, radius: f32) -> Stroke {
    Stroke::capsule(
        StrokeMode::Carve,
        start,
        end,
        radius,
        (radius * 0.4).min(MAX_SMOOTH),
    )
}

/// The strokes of a cave for `params`. Empty only when the volume is too small to hold a room.
pub fn generate_cave(params: &GenerateParams) -> Vec<Stroke> {
    let mut random = Random(params.seed);
    let noise_seed = random.next();
    let half = params.size * 0.5;
    if half.x < 2.0 || half.y < 1.5 || half.z < 2.0 {
        return Vec::new();
    }
    let tunnel = params.tunnel_radius.clamp(0.5, 6.0).min(half.y * 0.8);
    // Tunnel centers keep a wall's thickness from the faces, except the entrance.
    let inner = Vec3::new(
        (half.x - tunnel - 0.5).max(0.0),
        (half.y - tunnel - 0.3).max(0.0),
        (half.z - tunnel - 0.5).max(0.0),
    );
    let mut strokes: Vec<Stroke> = Vec::new();
    let mut rooms: Vec<(Vec3, f32)> = Vec::new();

    for _ in 0..params.rooms.clamp(1, 24) {
        let radius = random
            .range(tunnel * 1.5, tunnel * 3.0)
            .min(half.x.min(half.z) * 0.55)
            .min(half.y * 0.9)
            .max(0.6);
        let reach = Vec3::new(
            (half.x - radius - 0.5).max(0.0),
            (half.y - radius - 0.3).max(0.0),
            (half.z - radius - 0.5).max(0.0),
        );
        let center = Vec3::new(
            random.range(-1.0, 1.0) * reach.x,
            random.range(-0.4, 0.4) * reach.y,
            random.range(-1.0, 1.0) * reach.z,
        );
        let smooth = (radius * 0.4).min(MAX_SMOOTH);
        strokes.push(Stroke::capsule(
            StrokeMode::Carve,
            center,
            center,
            radius,
            smooth,
        ));
        // Two smaller blobs make the room irregular instead of a perfect ball.
        for _ in 0..2 {
            let blob = clamp_inside(
                center + random.direction() * (radius * 0.6),
                reach + Vec3::splat(radius * 0.3),
            );
            strokes.push(Stroke::capsule(
                StrokeMode::Carve,
                blob,
                blob,
                radius * 0.65,
                smooth,
            ));
        }
        rooms.push((center, radius));
    }

    // Every room joins the nearest earlier one, so the cave is one connected space.
    for index in 1..rooms.len() {
        let Some(&(from, _)) = rooms.get(index) else {
            continue;
        };
        let nearest =
            rooms
                .iter()
                .take(index)
                .map(|(center, _)| *center)
                .min_by(|first, second| {
                    (*first - from)
                        .length_squared()
                        .total_cmp(&(*second - from).length_squared())
                });
        if let Some(to) = nearest {
            wander(
                &mut strokes,
                &mut random,
                noise_seed,
                from,
                to,
                tunnel,
                inner,
            );
        }
    }
    // A loop, so there is more than one way through.
    if rooms.len() >= 4
        && let (Some(&(last, _)), Some(&(other, _))) = (
            rooms.last(),
            rooms.get((random.next() % (rooms.len() as u64 - 2)) as usize),
        )
    {
        wander(
            &mut strokes,
            &mut random,
            noise_seed,
            last,
            other,
            tunnel,
            inner,
        );
    }

    // Dead ends: random walks that start in a room and turn gently.
    for _ in 0..params.worms.min(12) {
        let Some(&(start, _)) = rooms.get((random.next() % rooms.len().max(1) as u64) as usize)
        else {
            break;
        };
        let mut position = start;
        let mut heading = random.direction();
        let mut radius = tunnel;
        for _ in 0..random.range(5.0, 10.0) as usize {
            heading = (heading
                + noise_vector(noise_seed, position * 0.3 + Vec3::splat(7.0)) * 0.9
                + (Vec3::ZERO - position).normalized_or_zero() * 0.15)
                .normalized_or_zero();
            let next = clamp_inside(position + heading * (tunnel * 2.5), inner);
            radius = (radius * 0.93).max(tunnel * 0.6);
            strokes.push(carve(position, next, radius));
            position = next;
        }
    }

    if params.entrance
        && let Some(&(first, _)) = rooms.first()
    {
        let outside = Vec3::new(first.x, first.y, half.z + 2.0);
        let limit = Vec3::new(inner.x, inner.y, half.z + 3.0);
        wander(
            &mut strokes,
            &mut random,
            noise_seed,
            first,
            outside,
            tunnel,
            limit,
        );
    }

    strokes.truncate(MAX_GENERATED);
    strokes
}

/// A tunnel from `from` to `to`, bent by noise (zero at both ends) so it winds.
fn wander(
    strokes: &mut Vec<Stroke>,
    random: &mut Random,
    noise_seed: u64,
    from: Vec3,
    to: Vec3,
    radius: f32,
    limit: Vec3,
) {
    let spacing = (radius * 2.5).max(1.5);
    let steps = (((to - from).length() / spacing).ceil() as usize).clamp(1, 64);
    let mut previous = from;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let mut point = from + (to - from) * t;
        if step < steps {
            let bend = radius * 1.6 * (t * PI).sin();
            point = clamp_inside(point + noise_vector(noise_seed, point * 0.35) * bend, limit);
        }
        let tube = radius * random.range(0.9, 1.1);
        strokes.push(carve(previous, point, tube));
        previous = point;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Base, Volume};

    fn volume(strokes: Vec<Stroke>, size: Vec3) -> Volume {
        Volume {
            base: Base::Solid,
            size,
            cell_size: 0.5,
            strokes,
            stamps: Vec::new(),
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_cave_and_another_seed_a_different_one() {
        let size = Vec3::new(32.0, 8.0, 32.0);
        let first = generate_cave(&GenerateParams::for_size(7, size));
        assert!(first.len() > 10);
        assert_eq!(first, generate_cave(&GenerateParams::for_size(7, size)));
        assert_ne!(first, generate_cave(&GenerateParams::for_size(8, size)));
    }

    #[test]
    fn a_generated_cave_is_valid_hollow_and_connected_to_the_outside() {
        let size = Vec3::new(32.0, 8.0, 32.0);
        let params = GenerateParams::for_size(3, size);
        let strokes = generate_cave(&params);
        let cave = volume(strokes.clone(), size);
        cave.validate().expect("valid strokes");
        let center = strokes.first().expect("a room").start;
        assert!(cave.distance(center) > 0.0, "the first room is air");
        let mesh = cave.mesh().expect("mesh");
        assert!(mesh.indices.len() > 3000, "a real cave has a real surface");
        // The entrance reaches the +Z face: its last stroke ends outside the block.
        let end = strokes.last().expect("entrance").end;
        assert!(end.z > size.z * 0.5, "{end:?}");
        assert!(cave.distance(Vec3::new(end.x, end.y, size.z * 0.5 - 0.5)) > 0.0);
    }

    #[test]
    fn a_volume_too_small_for_a_room_gives_nothing() {
        assert!(generate_cave(&GenerateParams::for_size(1, Vec3::new(2.0, 2.0, 2.0))).is_empty());
    }
}
