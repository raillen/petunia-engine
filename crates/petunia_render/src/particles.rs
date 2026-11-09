//! Particles for rain, snow and simple effects (phase 4 and 5).
//!
//! A particle system here is **deterministic and bounded**: a fixed pool, an explicit step, and no
//! dependency. That matters because precipitation is part of the scene the player sees, and a system
//! that drifts with frame rate would look different on every machine (`AGENTS.md` §2).
//!
//! The particles live in a box around the camera, so "rain everywhere on an island" costs the same
//! as "rain around me": when a particle falls out of the box it comes back at the top. That is the
//! standard trick and it is honest about what it does.

use crate::weather::Precipitation;
use petunia_math::Vec3;

/// Most particles one system may hold. A hostile setting cannot ask for millions.
pub const MAX_PARTICLES: usize = 20_000;

/// One particle: where it is and how fast it falls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec3,
    /// Fall speed in meters per second, with a small per-particle variation.
    pub speed: f32,
    /// Sideways drift, so rain is not a perfect grid.
    pub drift: Vec3,
    /// 0 to 1, how far through its life; a renderer may fade with it.
    pub life: f32,
}

/// What the system simulates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Rain,
    Snow,
}

impl Kind {
    /// The kind a weather preset asks for, or `None` when nothing falls.
    #[must_use]
    pub fn of(weather: Precipitation) -> Option<Self> {
        match weather {
            Precipitation::Rain => Some(Self::Rain),
            Precipitation::Snow => Some(Self::Snow),
            Precipitation::None => None,
        }
    }

    fn fall_speed(self) -> f32 {
        match self {
            // Rain falls fast, snow drifts.
            Self::Rain => 9.0,
            Self::Snow => 1.2,
        }
    }
}

/// A bounded pool of particles around a centre.
#[derive(Clone, Debug)]
pub struct ParticleSystem {
    kind: Kind,
    particles: Vec<Particle>,
    /// Half the side of the box the particles live in.
    radius: f32,
}

impl ParticleSystem {
    /// A system of `count` particles (clamped to [`MAX_PARTICLES`]) in a box of `radius` meters.
    #[must_use]
    pub fn new(kind: Kind, count: usize, radius: f32, seed: u64) -> Self {
        let count = count.min(MAX_PARTICLES);
        let radius = if radius.is_finite() && radius > 1.0 {
            radius.min(500.0)
        } else {
            20.0
        };
        let mut system = Self {
            kind,
            particles: Vec::with_capacity(count),
            radius,
        };
        for index in 0..count {
            let hash = hash_of(seed, index as u64);
            let position = system.random_position(hash);
            system.particles.push(Particle {
                position,
                speed: kind.fall_speed() * (0.85 + 0.3 * unit(hash >> 8)),
                drift: Vec3::new(
                    (unit(hash >> 16) - 0.5) * 0.8,
                    0.0,
                    (unit(hash >> 24) - 0.5) * 0.8,
                ),
                life: unit(hash >> 32),
            });
        }
        system
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.particles.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Steps the system by `seconds` around `centre`.
    ///
    /// A particle that leaves the box wraps to the other side: the pool never grows and never needs
    /// to spawn, so the cost per frame is constant.
    pub fn step(&mut self, seconds: f32, centre: Vec3) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        // A long frame (a breakpoint, a stalled disk) must not teleport every drop across the map.
        let seconds = seconds.min(0.1);
        let span = self.radius;
        for particle in &mut self.particles {
            particle.position.x += particle.drift.x * seconds;
            particle.position.y -= particle.speed * seconds;
            particle.position.z += particle.drift.z * seconds;
            particle.life = (particle.life + seconds * 0.25).min(1.0);
            // Wrap in the box around the centre.
            let offset = particle.position - centre;
            let wrap = |value: f32| -> f32 {
                if value > span {
                    value - 2.0 * span
                } else if value < -span {
                    value + 2.0 * span
                } else {
                    value
                }
            };
            particle.position = Vec3::new(
                centre.x + wrap(offset.x),
                centre.y + wrap(offset.y),
                centre.z + wrap(offset.z),
            );
        }
    }

    /// The skinned mesh of the particles, as tiny quads, for the renderer to draw.
    ///
    /// Rain is drawn as a thin vertical streak, snow as a small square; both are billboards, so they
    /// do not need to face anything in particular.
    #[must_use]
    pub fn mesh(&self) -> crate::mesh::MeshData {
        let mut mesh = crate::mesh::MeshData::default();
        let (half_width, half_height) = match self.kind {
            // A long thin streak reads as rain with fewer quads than a cloud of dots.
            Kind::Rain => (0.012, 0.55),
            Kind::Snow => (0.03, 0.03),
        };
        for particle in &self.particles {
            if particle.position.x.is_finite()
                && particle.position.y.is_finite()
                && particle.position.z.is_finite()
            {
                push_quad(&mut mesh, particle.position, half_width, half_height);
            }
        }
        mesh
    }

    fn random_position(&self, hash: u64) -> Vec3 {
        Vec3::new(
            (unit(hash) * 2.0 - 1.0) * self.radius,
            (unit(hash >> 16) * 2.0 - 1.0) * self.radius,
            (unit(hash >> 32) * 2.0 - 1.0) * self.radius,
        )
    }
}

fn push_quad(mesh: &mut crate::mesh::MeshData, centre: Vec3, half_width: f32, half_height: f32) {
    let first = u32::try_from(mesh.positions.len()).unwrap_or(0);
    for (dx, dy) in [
        (-half_width, -half_height),
        (half_width, -half_height),
        (half_width, half_height),
        (-half_width, half_height),
    ] {
        mesh.positions
            .push([centre.x + dx, centre.y + dy, centre.z]);
        mesh.normals.push([0.0, 1.0, 0.0]);
        mesh.uvs.push([0.0, 0.0]);
        mesh.flows.push([0.0, 0.0, 0.0]);
        mesh.margins.push(0.0);
    }
    mesh.indices
        .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
}

/// A number in 0..1 from a hash, the only randomness in the system.
fn unit(hash: u64) -> f32 {
    (hash & 0xFFFF) as f32 / 65_535.0
}

fn hash_of(seed: u64, index: u64) -> u64 {
    let mut value = seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_never_exceeds_the_pool() {
        let system = ParticleSystem::new(Kind::Rain, MAX_PARTICLES * 4, 20.0, 7);
        assert_eq!(system.len(), MAX_PARTICLES);
        assert!(system.mesh().positions.len() >= MAX_PARTICLES * 4 / 2);
    }

    #[test]
    fn the_same_seed_gives_the_same_drops() {
        let first = ParticleSystem::new(Kind::Rain, 500, 20.0, 42);
        let second = ParticleSystem::new(Kind::Rain, 500, 20.0, 42);
        assert_eq!(first.particles(), second.particles());
        let other = ParticleSystem::new(Kind::Rain, 500, 20.0, 43);
        assert_ne!(
            first.particles(),
            other.particles(),
            "another seed, another rain"
        );
    }

    #[test]
    fn rain_falls_and_stays_inside_the_box() {
        let mut system = ParticleSystem::new(Kind::Rain, 200, 10.0, 3);
        let centre = Vec3::ZERO;
        let before: Vec<f32> = system.particles().iter().map(|p| p.position.y).collect();
        for _ in 0..120 {
            system.step(0.016, centre);
        }
        let mut fell = 0;
        for (index, particle) in system.particles().iter().enumerate() {
            // The pool is a box: a particle that leaves it comes back at the other side, which is a
            // jump of 2 * radius and is correct. What must never happen is escaping the box.
            assert!(
                particle.position.y.abs() <= 10.0 + 1e-3,
                "wrapped, not escaped: {}",
                particle.position.y
            );
            let start = before.get(index).copied().unwrap_or(0.0);
            if particle.position.y < start {
                fell += 1;
            }
        }
        assert!(fell > 0, "at least some drops fell rather than wrapping");
        assert!(fell < 200, "and some wrapped: the box does its job");
    }

    #[test]
    fn snow_falls_slower_than_rain() {
        let rain = ParticleSystem::new(Kind::Rain, 1, 10.0, 1);
        let snow = ParticleSystem::new(Kind::Snow, 1, 10.0, 1);
        let rain_speed = rain.particles().first().map(|p| p.speed).unwrap_or(0.0);
        let snow_speed = snow.particles().first().map(|p| p.speed).unwrap_or(0.0);
        assert!(rain_speed > snow_speed, "{rain_speed} vs {snow_speed}");
    }

    #[test]
    fn a_long_frame_does_not_teleport_the_drops() {
        let mut system = ParticleSystem::new(Kind::Rain, 10, 10.0, 5);
        let before = system.particles().to_vec();
        // A minute-long frame is clamped to a tenth of a second, so the fall is small; a particle
        // near the floor wraps to the top, which is a jump of 2 * radius and is correct.
        system.step(60.0, Vec3::ZERO);
        let radius = 10.0f32;
        for (index, particle) in system.particles().iter().enumerate() {
            let start = before.get(index).copied().expect("particle");
            let mut expected = start.position.y - start.speed * 0.1;
            if expected < -radius {
                expected += 2.0 * radius;
            }
            assert!(
                (particle.position.y - expected).abs() < 1e-3,
                "fell a tenth of a second, not a minute: {} expected {}",
                particle.position.y,
                expected
            );
            assert!(particle.position.y.abs() <= radius + 1e-3);
        }
    }

    #[test]
    fn a_broken_step_is_ignored() {
        let mut system = ParticleSystem::new(Kind::Rain, 5, 10.0, 9);
        let before = system.particles().to_vec();
        system.step(f32::NAN, Vec3::ZERO);
        system.step(-1.0, Vec3::ZERO);
        assert_eq!(system.particles(), before.as_slice());
    }

    #[test]
    fn a_weather_preset_picks_the_right_kind() {
        assert_eq!(Kind::of(Precipitation::Rain), Some(Kind::Rain));
        assert_eq!(Kind::of(Precipitation::Snow), Some(Kind::Snow));
        assert_eq!(Kind::of(Precipitation::None), None);
    }
}
