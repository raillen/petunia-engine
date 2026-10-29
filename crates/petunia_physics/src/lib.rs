//! Rigid bodies and collision for Petunia, behind an adapter around Rapier (DR-08, DR-27).
//!
//! Everything public speaks Petunia's own types (`Vec3`, `Transform`, entity ids). Rapier types
//! stay inside this crate, so the engine can change the library without touching the world, the
//! files or the script API.
//!
//! A [`Simulation`] is built once from plain descriptions (bodies and named colliders), then
//! stepped at a fixed rate. Each step returns the contacts that started or stopped; queries answer
//! "what does this ray hit?" and "what touches this collider?".

use std::collections::BTreeMap;
use std::sync::Mutex;

use petunia_math::{Transform, Vec3};
use rapier3d::prelude as rapier;

mod character;
mod scene;

pub use character::CharacterDescription;
pub use petunia_math::SolidShape;
pub use scene::{AUTO_COLLIDER_FLAG, SceneDescription, describe};

/// Most bodies and colliders one simulation accepts; bigger scenes fail with a clear error.
pub const MAX_BODIES: usize = 20_000;
pub const MAX_COLLIDERS: usize = 50_000;
/// Most points a convex collider is built from (the hull keeps fewer).
pub const MAX_HULL_POINTS: usize = 4_096;
/// Most points an exact surface keeps.
pub const MAX_TRIMESH_POINTS: usize = 400_000;
/// Largest size, distance or mass accepted, in meters or kilograms.
pub const MAX_MAGNITUDE: f32 = 100_000.0;
/// Smallest size: thinner shapes make the solver unstable.
pub const MIN_SIZE: f32 = 0.001;

/// How a body moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Never moves (walls, floors). Objects with colliders and no body are static.
    Static,
    /// Moved by gravity and contacts.
    Dynamic,
    /// Moved only by the game (scripts, animation); pushes dynamic bodies out of its way.
    Kinematic,
}

impl BodyKind {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "static" => Some(Self::Static),
            "dynamic" => Some(Self::Dynamic),
            "kinematic" => Some(Self::Kinematic),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dynamic => "dynamic",
            Self::Kinematic => "kinematic",
        }
    }
}

/// One body: an object whose colliders move together.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyDescription {
    /// Persistent id of the object.
    pub entity: u64,
    pub kind: BodyKind,
    /// Where the object is. The scale stretches its colliders; it is not part of the body.
    pub transform: Transform,
    /// Kilograms, spread over the body's solid colliders.
    pub mass: f32,
    /// 1 is normal gravity, 0 floats.
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    /// Keeps the body upright (characters).
    pub lock_rotation: bool,
}

impl BodyDescription {
    /// A body that never moves, for objects that have colliders but no `RigidBody` table.
    pub fn fixed(entity: u64, transform: Transform) -> Self {
        Self {
            entity,
            kind: BodyKind::Static,
            transform,
            mass: 1.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            lock_rotation: false,
        }
    }
}

/// The form of a collider, in the owner's own (unscaled) coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum ColliderShape {
    /// Full size along x, y and z.
    Box {
        size: Vec3,
    },
    Sphere {
        radius: f32,
    },
    /// Upright capsule; `height` is the total height, caps included.
    Capsule {
        radius: f32,
        height: f32,
    },
    /// The convex hull of these points (usually the owner's mesh).
    Convex {
        points: Vec<Vec3>,
    },
    /// An exact surface (stairs, caves, editable meshes): for things that do not move.
    TriMesh {
        points: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
    },
    /// A chunk of heightmap. `rows` and `columns` are the sample counts (at least 2), `heights`
    /// is row-major with `rows * columns` entries (row = z, column = x) and `scale` is the
    /// total size in meters. Far cheaper than a triangle mesh for terrain.
    Heightfield {
        heights: Vec<f32>,
        rows: usize,
        columns: usize,
        scale: Vec3,
    },
}

/// One named collider of an object.
#[derive(Clone, Debug, PartialEq)]
pub struct ColliderDescription {
    /// Persistent id of the collider's own entity.
    pub collider: u64,
    /// Persistent id of the object it belongs to.
    pub owner: u64,
    /// The name scripts use ("pés", "corpo").
    pub name: String,
    pub shape: ColliderShape,
    /// Offset and turn from the owner's origin, in the owner's coordinates.
    pub center: Vec3,
    pub rotation_degrees: Vec3,
    /// A trigger reports contacts but nothing bumps into it.
    pub trigger: bool,
    pub friction: f32,
    /// 0 does not bounce, 1 bounces back fully.
    pub restitution: f32,
    /// Bits of the layers this collider is on (bit 0 is layer 1).
    pub layer: u32,
    /// It only touches colliders whose `layer` shares a bit with this mask.
    pub mask: u32,
}

/// Layer bits and mask every collider starts with: layer 1, touching everything.
pub const DEFAULT_LAYER: u32 = 1;
pub const DEFAULT_MASK: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsSettings {
    /// Meters per second squared.
    pub gravity: Vec3,
    /// Seconds per step. The game steps as many times as needed to keep up with real time.
    pub step_seconds: f32,
}

impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            step_seconds: 1.0 / 60.0,
        }
    }
}

/// A collider as scripts know it: its entity, its owner and its name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColliderRef {
    pub collider: u64,
    pub owner: u64,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactPhase {
    Started,
    Stopped,
}

/// Two colliders started or stopped touching (or a body entered or left a trigger).
#[derive(Clone, Debug, PartialEq)]
pub struct ContactEvent {
    pub phase: ContactPhase,
    pub first: ColliderRef,
    pub second: ColliderRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RayHit {
    pub collider: ColliderRef,
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

/// Where a moving body is after a step, to write back into its `Transform`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPose {
    pub entity: u64,
    pub position: Vec3,
    pub rotation_degrees: Vec3,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PhysicsError {
    #[error("{0}")]
    Invalid(String),
    #[error("a colisão #{collider} pertence ao objeto #{owner}, que não está na simulação")]
    MissingOwner { collider: u64, owner: u64 },
    #[error("há {count} {what}; o limite é {limit}")]
    TooMany {
        what: &'static str,
        count: usize,
        limit: usize,
    },
    #[error("o objeto #{0} não tem corpo na simulação")]
    UnknownBody(u64),
    #[error("a colisão #{0} não existe na simulação")]
    UnknownCollider(u64),
}

/// Collects contacts during a step (Rapier calls it from inside the step).
#[derive(Default)]
struct ContactCollector {
    events: Mutex<Vec<(ContactPhase, rapier::ColliderHandle, rapier::ColliderHandle)>>,
}

impl rapier::EventHandler for ContactCollector {
    fn handle_collision_event(
        &self,
        _bodies: &rapier::RigidBodySet,
        _colliders: &rapier::ColliderSet,
        event: rapier::CollisionEvent,
        _contact_pair: Option<&rapier::ContactPair>,
    ) {
        let entry = match event {
            rapier::CollisionEvent::Started(first, second, _) => {
                (ContactPhase::Started, first, second)
            }
            rapier::CollisionEvent::Stopped(first, second, _) => {
                (ContactPhase::Stopped, first, second)
            }
        };
        if let Ok(mut events) = self.events.lock() {
            events.push(entry);
        }
    }

    fn handle_contact_force_event(
        &self,
        _dt: rapier::Real,
        _bodies: &rapier::RigidBodySet,
        _colliders: &rapier::ColliderSet,
        _contact_pair: &rapier::ContactPair,
        _total_force_magnitude: rapier::Real,
    ) {
    }

    fn handle_soft_body_tear_event(
        &self,
        _soft_bodies: &rapier::SoftBodySet,
        _event: &rapier::SoftBodyTearEvent,
    ) {
    }
}

/// A running physics world.
pub struct Simulation {
    world: rapier::PhysicsWorld,
    settings: PhysicsSettings,
    bodies: BTreeMap<u64, rapier::RigidBodyHandle>,
    kinds: BTreeMap<u64, BodyKind>,
    colliders: BTreeMap<u64, rapier::ColliderHandle>,
    /// Indexed by the `user_data` of each Rapier collider.
    references: Vec<ColliderRef>,
    collector: ContactCollector,
    characters: BTreeMap<u64, character::CharacterState>,
}

impl Simulation {
    /// Builds the world. Every collider's owner must be among `bodies` (callers add a
    /// [`BodyDescription::fixed`] for objects without a `RigidBody`). Input is validated:
    /// finite numbers, positive sizes, and the limits above.
    pub fn build(
        settings: PhysicsSettings,
        bodies: &[BodyDescription],
        colliders: &[ColliderDescription],
    ) -> Result<Self, PhysicsError> {
        check_settings(&settings)?;
        if bodies.len() > MAX_BODIES {
            return Err(PhysicsError::TooMany {
                what: "corpos",
                count: bodies.len(),
                limit: MAX_BODIES,
            });
        }
        if colliders.len() > MAX_COLLIDERS {
            return Err(PhysicsError::TooMany {
                what: "colisões",
                count: colliders.len(),
                limit: MAX_COLLIDERS,
            });
        }
        // Sorted by id: the same scene always builds the same world, in the same order.
        let mut sorted_bodies: Vec<&BodyDescription> = bodies.iter().collect();
        sorted_bodies.sort_by_key(|body| body.entity);
        let mut sorted_colliders: Vec<&ColliderDescription> = colliders.iter().collect();
        sorted_colliders.sort_by_key(|collider| collider.collider);

        let mut world = rapier::PhysicsWorld::new();
        world.gravity = vector(settings.gravity);
        world.integration_parameters.dt = settings.step_seconds;

        let mut solid_count: BTreeMap<u64, usize> = BTreeMap::new();
        for collider in &sorted_colliders {
            if !collider.trigger {
                *solid_count.entry(collider.owner).or_default() += 1;
            }
        }

        let mut handles = BTreeMap::new();
        let mut kinds = BTreeMap::new();
        let mut scales = BTreeMap::new();
        for body in sorted_bodies {
            check_body(body)?;
            if handles.contains_key(&body.entity) {
                return Err(PhysicsError::Invalid(format!(
                    "o objeto #{} aparece duas vezes",
                    body.entity
                )));
            }
            let pose = pose_of(body.transform.position, body.transform.rotation_degrees);
            let mut builder = match body.kind {
                BodyKind::Static => rapier::RigidBodyBuilder::fixed(),
                BodyKind::Dynamic => rapier::RigidBodyBuilder::dynamic(),
                BodyKind::Kinematic => rapier::RigidBodyBuilder::kinematic_position_based(),
            }
            .pose(pose)
            .gravity_scale(body.gravity_scale)
            .linear_damping(body.linear_damping)
            .angular_damping(body.angular_damping)
            .user_data(u128::from(body.entity));
            if body.lock_rotation {
                builder = builder.lock_rotations();
            }
            if body.kind == BodyKind::Dynamic
                && solid_count.get(&body.entity).copied().unwrap_or(0) == 0
            {
                // Nothing solid to take the mass from: give it directly.
                builder = builder.additional_mass(body.mass);
            }
            handles.insert(body.entity, world.insert_body(builder));
            kinds.insert(body.entity, body.kind);
            scales.insert(body.entity, (body.transform.scale, body.mass, body.kind));
        }

        let mut collider_handles = BTreeMap::new();
        let mut references = Vec::with_capacity(sorted_colliders.len());
        for collider in sorted_colliders {
            check_collider(collider)?;
            let Some(&parent) = handles.get(&collider.owner) else {
                return Err(PhysicsError::MissingOwner {
                    collider: collider.collider,
                    owner: collider.owner,
                });
            };
            if collider_handles.contains_key(&collider.collider) {
                return Err(PhysicsError::Invalid(format!(
                    "a colisão #{} aparece duas vezes",
                    collider.collider
                )));
            }
            let (scale, mass, kind) =
                scales
                    .get(&collider.owner)
                    .copied()
                    .unwrap_or((Vec3::ONE, 1.0, BodyKind::Static));
            let mut builder = shape_builder(collider, scale)?
                .position(pose_of(
                    multiply(collider.center, scale),
                    collider.rotation_degrees,
                ))
                .sensor(collider.trigger)
                .friction(collider.friction)
                .restitution(collider.restitution)
                .collision_groups(rapier::InteractionGroups::new(
                    rapier::Group::from_bits_truncate(collider.layer),
                    rapier::Group::from_bits_truncate(collider.mask),
                    rapier::InteractionTestMode::And,
                ))
                .active_events(rapier::ActiveEvents::COLLISION_EVENTS)
                // A character (kinematic) walking into a static trigger or wall must report it:
                // by default Rapier skips kinematic against fixed pairs.
                .active_collision_types(
                    rapier::ActiveCollisionTypes::default()
                        | rapier::ActiveCollisionTypes::KINEMATIC_KINEMATIC
                        | rapier::ActiveCollisionTypes::KINEMATIC_FIXED,
                )
                .user_data(references.len() as u128);
            if collider.trigger {
                builder = builder.density(0.0);
            } else if kind == BodyKind::Dynamic {
                let share = solid_count
                    .get(&collider.owner)
                    .copied()
                    .unwrap_or(1)
                    .max(1) as f32;
                builder = builder.mass(mass / share);
            }
            let handle = world.insert_collider(builder, Some(parent));
            collider_handles.insert(collider.collider, handle);
            references.push(ColliderRef {
                collider: collider.collider,
                owner: collider.owner,
                name: collider.name.clone(),
            });
        }

        let collector = ContactCollector::default();
        // Fill the broad phase now, so queries work before the first step. Detection takes the
        // "just added" mark off the bodies, and a body without it never joins the simulation's
        // active set (it would hang in the air): mark every body as changed again.
        world.detect_collisions(&(), &collector);
        for handle in handles.values() {
            if let Some(body) = world.bodies.get_mut(*handle) {
                let pose = *body.position();
                body.set_position(pose, true);
            }
        }
        if let Ok(mut events) = collector.events.lock() {
            events.clear();
        }
        Ok(Self {
            world,
            settings,
            bodies: handles,
            kinds,
            colliders: collider_handles,
            references,
            collector,
            characters: BTreeMap::new(),
        })
    }

    pub fn settings(&self) -> PhysicsSettings {
        self.settings
    }

    /// Advances one fixed step and returns the contacts that started or stopped in it, sorted so
    /// the order never depends on the library's internals.
    pub fn step(&mut self) -> Vec<ContactEvent> {
        self.drive_characters();
        self.world.step_with_events(&(), &self.collector);
        let raw = match self.collector.events.lock() {
            Ok(mut events) => std::mem::take(&mut *events),
            Err(_) => Vec::new(),
        };
        let mut events: Vec<ContactEvent> = raw
            .into_iter()
            .filter_map(|(phase, first, second)| {
                let first = self.reference(first)?;
                let second = self.reference(second)?;
                // The pair in a fixed order: lower collider id first.
                let (first, second) = if first <= second {
                    (first, second)
                } else {
                    (second, first)
                };
                Some(ContactEvent {
                    phase,
                    first,
                    second,
                })
            })
            .collect();
        events.sort_by(|a, b| {
            (a.phase == ContactPhase::Stopped, &a.first, &a.second).cmp(&(
                b.phase == ContactPhase::Stopped,
                &b.first,
                &b.second,
            ))
        });
        events
    }

    /// Where every dynamic and kinematic body is now (static ones never move).
    pub fn poses(&self) -> Vec<BodyPose> {
        self.bodies
            .iter()
            .filter(|(entity, _)| self.kinds.get(entity) != Some(&BodyKind::Static))
            .filter_map(|(entity, handle)| {
                let body = self.world.bodies.get(*handle)?;
                Some(BodyPose {
                    entity: *entity,
                    position: from_vector(body.translation()),
                    rotation_degrees: degrees_from_rotation(*body.rotation()),
                })
            })
            .collect()
    }

    /// Moves a kinematic body to `transform` during the next step (it pushes what is in the way).
    /// For a dynamic body this teleports it and keeps its velocity.
    pub fn move_body(&mut self, entity: u64, transform: &Transform) -> Result<(), PhysicsError> {
        check_vector("posição", transform.position, MAX_MAGNITUDE)?;
        check_vector("rotação", transform.rotation_degrees, 1.0e6)?;
        let handle = *self
            .bodies
            .get(&entity)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        let kind = self.kinds.get(&entity).copied().unwrap_or(BodyKind::Static);
        let pose = pose_of(transform.position, transform.rotation_degrees);
        let body = self
            .world
            .bodies
            .get_mut(handle)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        match kind {
            // A character moves itself every step; a move from outside is a teleport.
            BodyKind::Kinematic if self.characters.contains_key(&entity) => {
                body.set_position(pose, true);
            }
            BodyKind::Kinematic => body.set_next_kinematic_position(pose),
            BodyKind::Dynamic => body.set_position(pose, true),
            BodyKind::Static => {
                return Err(PhysicsError::Invalid(format!(
                    "o objeto #{entity} é estático; mude o corpo para cinemático para movê-lo com a física"
                )));
            }
        }
        Ok(())
    }

    pub fn velocity(&self, entity: u64) -> Result<Vec3, PhysicsError> {
        let handle = self
            .bodies
            .get(&entity)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        let body = self
            .world
            .bodies
            .get(*handle)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        Ok(from_vector(body.linvel()))
    }

    pub fn set_velocity(&mut self, entity: u64, velocity: Vec3) -> Result<(), PhysicsError> {
        check_vector("velocidade", velocity, MAX_MAGNITUDE)?;
        let handle = *self
            .bodies
            .get(&entity)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        let body = self
            .world
            .bodies
            .get_mut(handle)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        body.set_linvel(vector(velocity), true);
        Ok(())
    }

    /// A push: changes the velocity by `impulse / mass`.
    pub fn apply_impulse(&mut self, entity: u64, impulse: Vec3) -> Result<(), PhysicsError> {
        check_vector("impulso", impulse, MAX_MAGNITUDE)?;
        let handle = *self
            .bodies
            .get(&entity)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        let body = self
            .world
            .bodies
            .get_mut(handle)
            .ok_or(PhysicsError::UnknownBody(entity))?;
        body.apply_impulse(vector(impulse), true);
        Ok(())
    }

    /// The first solid collider along a ray (triggers are ignored), up to `max_distance`.
    pub fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Result<Option<RayHit>, PhysicsError> {
        self.raycast_ignoring(origin, direction, max_distance, None)
    }

    /// Like [`Simulation::raycast`], skipping every collider of `ignored` (a ray that starts
    /// inside an object would otherwise hit the object itself).
    pub fn raycast_ignoring(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        ignored: Option<u64>,
    ) -> Result<Option<RayHit>, PhysicsError> {
        check_vector("origem", origin, MAX_MAGNITUDE)?;
        check_vector("direção", direction, MAX_MAGNITUDE)?;
        if !max_distance.is_finite() || max_distance <= 0.0 || max_distance > MAX_MAGNITUDE {
            return Err(PhysicsError::Invalid(format!(
                "a distância vai de 0 a {MAX_MAGNITUDE}"
            )));
        }
        let unit = direction.normalized_or_zero();
        if unit.length_squared() == 0.0 {
            return Err(PhysicsError::Invalid(
                "a direção do raio não pode ser zero".to_string(),
            ));
        }
        let ray = rapier::Ray::new(vector(origin), vector(unit));
        let mut filter = rapier::QueryFilter::default().exclude_sensors();
        if let Some(entity) = ignored {
            let handle = *self
                .bodies
                .get(&entity)
                .ok_or(PhysicsError::UnknownBody(entity))?;
            filter = filter.exclude_rigid_body(handle);
        }
        let Some((handle, hit)) =
            self.world
                .cast_ray_and_get_normal(&ray, max_distance, true, filter)
        else {
            return Ok(None);
        };
        let Some(collider) = self.reference(handle) else {
            return Ok(None);
        };
        Ok(Some(RayHit {
            collider,
            point: origin + unit * hit.time_of_impact,
            normal: from_vector(hit.normal),
            distance: hit.time_of_impact,
        }))
    }

    /// Every collider that overlaps the given one right now (triggers included), sorted.
    pub fn overlapping(&self, collider: u64) -> Result<Vec<ColliderRef>, PhysicsError> {
        let handle = *self
            .colliders
            .get(&collider)
            .ok_or(PhysicsError::UnknownCollider(collider))?;
        let own = self
            .world
            .colliders
            .get(handle)
            .ok_or(PhysicsError::UnknownCollider(collider))?;
        let mut found: Vec<ColliderRef> = self
            .world
            .intersect_shape(
                *own.position(),
                own.shape(),
                rapier::QueryFilter::default().exclude_collider(handle),
            )
            .filter_map(|(other, _)| self.reference(other))
            .collect();
        found.sort();
        found.dedup();
        Ok(found)
    }

    fn reference(&self, handle: rapier::ColliderHandle) -> Option<ColliderRef> {
        let collider = self.world.colliders.get(handle)?;
        let index = usize::try_from(collider.user_data).ok()?;
        self.references.get(index).cloned()
    }
}

// ---- validation -------------------------------------------------------------------------------

fn check_settings(settings: &PhysicsSettings) -> Result<(), PhysicsError> {
    check_vector("gravidade", settings.gravity, 1_000.0)?;
    if !settings.step_seconds.is_finite() || !(0.001..=0.1).contains(&settings.step_seconds) {
        return Err(PhysicsError::Invalid(
            "o passo da física vai de 0,001 a 0,1 segundo".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn check_vector(what: &str, value: Vec3, limit: f32) -> Result<(), PhysicsError> {
    if !value.is_finite() || value.x.abs() > limit || value.y.abs() > limit || value.z.abs() > limit
    {
        return Err(PhysicsError::Invalid(format!(
            "{what}: só números finitos, até {limit}"
        )));
    }
    Ok(())
}

pub(crate) fn check_range(what: &str, value: f32, low: f32, high: f32) -> Result<(), PhysicsError> {
    if !value.is_finite() || value < low || value > high {
        return Err(PhysicsError::Invalid(format!(
            "{what} vai de {low} a {high}"
        )));
    }
    Ok(())
}

pub(crate) fn check_body(body: &BodyDescription) -> Result<(), PhysicsError> {
    check_vector("posição", body.transform.position, MAX_MAGNITUDE)?;
    check_vector("rotação", body.transform.rotation_degrees, 1.0e6)?;
    check_vector("escala", body.transform.scale, MAX_MAGNITUDE)?;
    check_range("massa", body.mass, MIN_SIZE, MAX_MAGNITUDE)?;
    check_range("gravidade do corpo", body.gravity_scale, -100.0, 100.0)?;
    check_range("amortecimento", body.linear_damping, 0.0, 1_000.0)?;
    check_range("amortecimento de giro", body.angular_damping, 0.0, 1_000.0)
}

pub(crate) fn check_collider(collider: &ColliderDescription) -> Result<(), PhysicsError> {
    if collider.name.chars().count() > 64 {
        return Err(PhysicsError::Invalid(
            "o nome da colisão tem até 64 letras".to_string(),
        ));
    }
    check_vector("centro da colisão", collider.center, MAX_MAGNITUDE)?;
    check_vector("rotação da colisão", collider.rotation_degrees, 1.0e6)?;
    check_range("atrito", collider.friction, 0.0, 100.0)?;
    check_range("quique", collider.restitution, 0.0, 1.0)?;
    match &collider.shape {
        ColliderShape::Box { size } => {
            for (axis, value) in [("x", size.x), ("y", size.y), ("z", size.z)] {
                check_range(
                    &format!("tamanho {axis} da caixa"),
                    value,
                    MIN_SIZE,
                    MAX_MAGNITUDE,
                )?;
            }
        }
        ColliderShape::Sphere { radius } => {
            check_range("raio da esfera", *radius, MIN_SIZE, MAX_MAGNITUDE)?
        }
        ColliderShape::Capsule { radius, height } => {
            check_range("raio da cápsula", *radius, MIN_SIZE, MAX_MAGNITUDE)?;
            check_range("altura da cápsula", *height, MIN_SIZE, MAX_MAGNITUDE)?;
        }
        ColliderShape::Convex { points } => {
            if points.len() < 4 || points.len() > MAX_HULL_POINTS {
                return Err(PhysicsError::Invalid(format!(
                    "a colisão convexa precisa de 4 a {MAX_HULL_POINTS} pontos (tem {})",
                    points.len()
                )));
            }
            for point in points {
                check_vector("ponto da colisão convexa", *point, MAX_MAGNITUDE)?;
            }
            if !spans_volume(points) {
                return Err(PhysicsError::Invalid(format!(
                    "a colisão #{} não forma um volume (pontos todos num plano ou numa linha)",
                    collider.collider
                )));
            }
        }
        ColliderShape::Heightfield {
            heights,
            rows,
            columns,
            scale,
        } => {
            if *rows < 2 || *columns < 2 {
                return Err(PhysicsError::Invalid(
                    "o heightfield precisa de pelo menos 2 linhas e 2 colunas".to_string(),
                ));
            }
            let expected = rows
                .checked_mul(*columns)
                .ok_or_else(|| PhysicsError::Invalid("heightfield grande demais".to_string()))?;
            if heights.len() != expected {
                return Err(PhysicsError::Invalid(format!(
                    "o heightfield tem {} alturas; esperava {expected}",
                    heights.len()
                )));
            }
            if heights.iter().any(|height| !height.is_finite()) {
                return Err(PhysicsError::Invalid(
                    "o heightfield tem uma altura que não é um número finito".to_string(),
                ));
            }
            for (axis, value) in [("x", scale.x), ("y", scale.y), ("z", scale.z)] {
                check_range(
                    &format!("escala {axis} do heightfield"),
                    value,
                    MIN_SIZE,
                    MAX_MAGNITUDE,
                )?;
            }
        }
        ColliderShape::TriMesh { points, triangles } => {
            if points.len() < 3 || points.len() > MAX_TRIMESH_POINTS {
                return Err(PhysicsError::Invalid(format!(
                    "a superfície de colisão precisa de 3 a {MAX_TRIMESH_POINTS} pontos (tem {})",
                    points.len()
                )));
            }
            if triangles.is_empty() || triangles.len() > petunia_math::MAX_SOLID_TRIANGLES {
                return Err(PhysicsError::Invalid(format!(
                    "a superfície de colisão precisa de 1 a {} triângulos (tem {})",
                    petunia_math::MAX_SOLID_TRIANGLES,
                    triangles.len()
                )));
            }
            for point in points {
                check_vector("ponto da superfície de colisão", *point, MAX_MAGNITUDE)?;
            }
            let count = points.len() as u32;
            if triangles
                .iter()
                .any(|corner| corner.iter().any(|index| *index >= count))
            {
                return Err(PhysicsError::Invalid(
                    "a superfície de colisão tem um triângulo que aponta para fora da lista de pontos"
                        .to_string(),
                ));
            }
        }
    }
    Ok(())
}

/// True when the points are not all on one plane: the farthest point from the first, then the
/// farthest from that line, then the farthest from that plane, must stand out of it.
fn spans_volume(points: &[Vec3]) -> bool {
    let Some(&first) = points.first() else {
        return false;
    };
    let farthest = |score: &dyn Fn(Vec3) -> f32| {
        points
            .iter()
            .copied()
            .map(|point| (score(point), point))
            .fold(
                (0.0f32, first),
                |best, next| if next.0 > best.0 { next } else { best },
            )
    };
    let (length, second) = farthest(&|point| (point - first).length());
    if length < MIN_SIZE {
        return false;
    }
    let line = (second - first) * (1.0 / length);
    let (area, third) = farthest(&|point| (point - first).cross(line).length());
    if area < MIN_SIZE {
        return false;
    }
    let normal = (third - first).cross(line).normalized_or_zero();
    let (height, _) = farthest(&|point| (point - first).dot(normal).abs());
    height >= MIN_SIZE
}

// ---- conversions ------------------------------------------------------------------------------

pub(crate) fn vector(value: Vec3) -> rapier::Vector {
    rapier::Vector::new(value.x, value.y, value.z)
}

fn from_vector(value: rapier::Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

fn multiply(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x * b.x, a.y * b.y, a.z * b.z)
}

/// The engine's rotation: roll (z), then pitch (x), then yaw (y), from Euler degrees.
fn rotation_from_degrees(degrees: Vec3) -> rapier::Rotation {
    rapier::Rotation::from_rotation_y(degrees.y.to_radians())
        * rapier::Rotation::from_rotation_x(degrees.x.to_radians())
        * rapier::Rotation::from_rotation_z(degrees.z.to_radians())
}

/// Back to the engine's Euler degrees. With R = Ry · Rx · Rz: pitch = asin(−R₁₂),
/// yaw = atan2(R₀₂, R₂₂), roll = atan2(R₁₀, R₁₁); at ±90° of pitch the roll is folded into yaw.
fn degrees_from_rotation(rotation: rapier::Rotation) -> Vec3 {
    let column_x = rotation * rapier::Vector::X;
    let column_y = rotation * rapier::Vector::Y;
    let column_z = rotation * rapier::Vector::Z;
    let sine_pitch = (-column_z.y).clamp(-1.0, 1.0);
    let pitch = sine_pitch.asin();
    let (yaw, roll) = if sine_pitch.abs() < 0.9999 {
        (column_z.x.atan2(column_z.z), column_x.y.atan2(column_y.y))
    } else {
        ((-column_x.z).atan2(column_x.x), 0.0)
    };
    Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
}

fn pose_of(position: Vec3, rotation_degrees: Vec3) -> rapier::Pose {
    rapier::Pose::from_parts(vector(position), rotation_from_degrees(rotation_degrees))
}

/// The collider's shape with the owner's scale applied (a scale stretches the shape, it is not
/// part of the body). Spheres take the largest scale; capsules the largest across x and z.
fn shape_builder(
    collider: &ColliderDescription,
    scale: Vec3,
) -> Result<rapier::ColliderBuilder, PhysicsError> {
    let scale = Vec3::new(scale.x.abs(), scale.y.abs(), scale.z.abs());
    if scale.x < MIN_SIZE || scale.y < MIN_SIZE || scale.z < MIN_SIZE {
        return Err(PhysicsError::Invalid(format!(
            "o objeto #{} tem escala zero; a colisão #{} não teria tamanho",
            collider.owner, collider.collider
        )));
    }
    Ok(match &collider.shape {
        ColliderShape::Box { size } => {
            let half = multiply(*size, scale) * 0.5;
            rapier::ColliderBuilder::cuboid(half.x, half.y, half.z)
        }
        ColliderShape::Sphere { radius } => {
            rapier::ColliderBuilder::ball(radius * scale.x.max(scale.y).max(scale.z))
        }
        ColliderShape::Capsule { radius, height } => {
            let radius = radius * scale.x.max(scale.z);
            let half_cylinder = (height * scale.y * 0.5 - radius).max(0.0);
            rapier::ColliderBuilder::capsule_y(half_cylinder, radius)
        }
        ColliderShape::Convex { points } => {
            let scaled: Vec<rapier::Vector> = points
                .iter()
                .map(|point| vector(multiply(*point, scale)))
                .collect();
            rapier::ColliderBuilder::convex_hull(&scaled).ok_or_else(|| {
                PhysicsError::Invalid(format!(
                    "a colisão #{} não forma um volume (pontos todos num plano ou numa linha)",
                    collider.collider
                ))
            })?
        }
        ColliderShape::TriMesh { points, triangles } => {
            let scaled: Vec<rapier::Vector> = points
                .iter()
                .map(|point| vector(multiply(*point, scale)))
                .collect();
            rapier::ColliderBuilder::trimesh(scaled, triangles.clone()).map_err(|error| {
                PhysicsError::Invalid(format!(
                    "a superfície de colisão #{} não serve: {error}",
                    collider.collider
                ))
            })?
        }
        ColliderShape::Heightfield {
            heights,
            rows,
            columns,
            scale: size,
        } => {
            let size = multiply(*size, scale);
            let grid = rapier3d::parry::utils::Array2::new(*rows, *columns, heights.clone());
            rapier::ColliderBuilder::new(rapier::SharedShape::heightfield(
                grid,
                rapier::Vector::new(size.x, size.y, size.z),
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32, z: f32) -> Transform {
        Transform {
            position: Vec3::new(x, y, z),
            ..Transform::IDENTITY
        }
    }

    fn collider(id: u64, owner: u64, name: &str, shape: ColliderShape) -> ColliderDescription {
        ColliderDescription {
            collider: id,
            owner,
            name: name.to_string(),
            shape,
            center: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            trigger: false,
            friction: 0.5,
            restitution: 0.0,
            layer: DEFAULT_LAYER,
            mask: DEFAULT_MASK,
        }
    }

    fn ball_body(entity: u64, height: f32) -> BodyDescription {
        BodyDescription {
            kind: BodyKind::Dynamic,
            ..BodyDescription::fixed(entity, at(0.0, height, 0.0))
        }
    }

    /// A 20 m floor (top at y = 0) and a ball of radius 0.5 dropped from `height`.
    fn floor_and_ball(height: f32) -> Simulation {
        let bodies = [
            BodyDescription::fixed(1, at(0.0, -0.5, 0.0)),
            ball_body(2, height),
        ];
        let colliders = [
            collider(
                10,
                1,
                "chão",
                ColliderShape::Box {
                    size: Vec3::new(20.0, 1.0, 20.0),
                },
            ),
            collider(20, 2, "bola", ColliderShape::Sphere { radius: 0.5 }),
        ];
        Simulation::build(PhysicsSettings::default(), &bodies, &colliders).expect("valid scene")
    }

    #[test]
    fn euler_degrees_survive_the_round_trip() {
        for degrees in [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(30.0, 45.0, -20.0),
            Vec3::new(-60.0, 170.0, 10.0),
            Vec3::new(10.0, -90.0, 80.0),
        ] {
            let back = degrees_from_rotation(rotation_from_degrees(degrees));
            let again = rotation_from_degrees(back);
            let original = rotation_from_degrees(degrees);
            // The same rotation (a quaternion and its negative are the same turn).
            assert!(
                again.dot(original).abs() > 0.9999,
                "{degrees:?} -> {back:?}"
            );
        }
        // Same convention as petunia_math: the forward vector agrees.
        let transform = Transform {
            rotation_degrees: Vec3::new(20.0, 35.0, -15.0),
            ..Transform::IDENTITY
        };
        let ours = from_vector(
            rotation_from_degrees(transform.rotation_degrees) * rapier::Vector::new(0.0, 0.0, -1.0),
        );
        assert!((ours - transform.forward()).length() < 1.0e-5);
    }

    #[test]
    fn a_ball_falls_and_rests_on_the_floor() {
        let mut simulation = floor_and_ball(3.0);
        for _ in 0..240 {
            simulation.step();
        }
        let ball = simulation
            .poses()
            .into_iter()
            .find(|pose| pose.entity == 2)
            .expect("ball moves");
        assert!(
            (ball.position.y - 0.5).abs() < 0.05,
            "rests on the floor: {:?}",
            ball.position
        );
        assert!(
            simulation.poses().iter().all(|pose| pose.entity != 1),
            "the floor is static"
        );
    }

    #[test]
    fn contacts_and_triggers_report_names() {
        let mut bodies = vec![
            BodyDescription::fixed(1, at(0.0, -0.5, 0.0)),
            ball_body(2, 4.0),
        ];
        bodies.push(BodyDescription::fixed(3, at(0.0, 2.0, 0.0)));
        let mut sensor = collider(
            30,
            3,
            "zona",
            ColliderShape::Box {
                size: Vec3::new(2.0, 0.5, 2.0),
            },
        );
        sensor.trigger = true;
        let colliders = [
            collider(
                10,
                1,
                "chão",
                ColliderShape::Box {
                    size: Vec3::new(20.0, 1.0, 20.0),
                },
            ),
            collider(20, 2, "bola", ColliderShape::Sphere { radius: 0.5 }),
            sensor,
        ];
        let mut simulation =
            Simulation::build(PhysicsSettings::default(), &bodies, &colliders).expect("valid");
        let mut seen = Vec::new();
        for _ in 0..240 {
            for event in simulation.step() {
                seen.push((
                    event.phase,
                    event.first.name.clone(),
                    event.second.name.clone(),
                ));
            }
        }
        let zone_started = seen.iter().position(|e| {
            *e == (
                ContactPhase::Started,
                "bola".to_string(),
                "zona".to_string(),
            )
        });
        let zone_stopped = seen.iter().position(|e| {
            *e == (
                ContactPhase::Stopped,
                "bola".to_string(),
                "zona".to_string(),
            )
        });
        assert!(
            zone_started.is_some() && zone_stopped > zone_started,
            "entered then left the trigger: {seen:?}"
        );
        assert!(
            seen.contains(&(
                ContactPhase::Started,
                "chão".to_string(),
                "bola".to_string()
            )),
            "touched the floor: {seen:?}"
        );
    }

    #[test]
    fn rays_and_overlaps_name_what_they_find() {
        let simulation = floor_and_ball(3.0);
        let hit = simulation
            .raycast(Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, -2.0, 0.0), 50.0)
            .expect("valid ray")
            .expect("hits the ball");
        assert_eq!(hit.collider.name, "bola");
        assert!(
            (hit.distance - 6.5).abs() < 1.0e-3,
            "ball top at 3.5: {}",
            hit.distance
        );
        assert!((hit.normal - Vec3::new(0.0, 1.0, 0.0)).length() < 1.0e-3);
        assert!(
            simulation
                .raycast(Vec3::new(0.0, 0.2, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.1)
                .expect("valid")
                .is_none(),
            "nothing within 10 cm above the floor"
        );

        // From inside the ball: itself first, unless it is ignored.
        let down = Vec3::new(0.0, -1.0, 0.0);
        let inside = simulation
            .raycast(Vec3::new(0.0, 3.0, 0.0), down, 50.0)
            .expect("ok")
            .expect("hit");
        assert_eq!(inside.collider.name, "bola");
        let past = simulation
            .raycast_ignoring(Vec3::new(0.0, 3.0, 0.0), down, 50.0, Some(2))
            .expect("ok")
            .expect("hit");
        assert_eq!(past.collider.name, "chão");
        assert!((past.distance - 3.0).abs() < 1.0e-3);

        let resting = floor_and_ball(0.4);
        let touching = resting.overlapping(20).expect("known collider");
        assert_eq!(
            touching.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["chão"]
        );
        assert!(resting.overlapping(99).is_err());
    }

    #[test]
    fn convex_and_capsule_colliders_work_and_scale() {
        let cube: Vec<Vec3> = [-0.5f32, 0.5]
            .iter()
            .flat_map(|&x| [-0.5f32, 0.5].map(move |y| (x, y)))
            .flat_map(|(x, y)| [-0.5f32, 0.5].map(move |z| Vec3::new(x, y, z)))
            .collect();
        let mut wide = BodyDescription::fixed(1, at(0.0, 0.0, 0.0));
        wide.transform.scale = Vec3::new(4.0, 1.0, 4.0);
        let bodies = [wide, BodyDescription::fixed(2, at(10.0, 0.0, 0.0))];
        let colliders = [
            collider(10, 1, "malha", ColliderShape::Convex { points: cube }),
            collider(
                20,
                2,
                "corpo",
                ColliderShape::Capsule {
                    radius: 0.5,
                    height: 2.0,
                },
            ),
        ];
        let simulation =
            Simulation::build(PhysicsSettings::default(), &bodies, &colliders).expect("valid");
        // The scaled hull spans x from -2 to 2.
        let side = simulation
            .raycast(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 20.0)
            .expect("ok")
            .expect("hit");
        assert_eq!(side.collider.name, "malha");
        assert!((side.distance - 3.0).abs() < 1.0e-3);
        // The capsule top is at y = 1.
        let top = simulation
            .raycast(Vec3::new(10.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0), 20.0)
            .expect("ok")
            .expect("hit");
        assert!((top.distance - 4.0).abs() < 1.0e-3, "{}", top.distance);
    }

    #[test]
    fn hostile_input_is_refused() {
        let floor = || BodyDescription::fixed(1, at(0.0, 0.0, 0.0));
        let refuse = |shape: ColliderShape| {
            Simulation::build(
                PhysicsSettings::default(),
                &[floor()],
                &[collider(10, 1, "x", shape)],
            )
            .is_err()
        };
        assert!(refuse(ColliderShape::Sphere { radius: f32::NAN }));
        assert!(refuse(ColliderShape::Sphere { radius: -1.0 }));
        assert!(refuse(ColliderShape::Box {
            size: Vec3::new(1.0, 0.0, 1.0)
        }));
        assert!(refuse(ColliderShape::Capsule {
            radius: 0.5,
            height: f32::INFINITY
        }));
        assert!(refuse(ColliderShape::Convex {
            points: vec![Vec3::ZERO; 3]
        }));
        // Points on a plane do not make a volume.
        let flat = (0..8)
            .map(|index| Vec3::new(index as f32, (index % 3) as f32, 0.0))
            .collect();
        assert!(refuse(ColliderShape::Convex { points: flat }));
        assert!(refuse(ColliderShape::Convex {
            points: vec![Vec3::ONE; MAX_HULL_POINTS + 1]
        }));

        let orphan = Simulation::build(
            PhysicsSettings::default(),
            &[],
            &[collider(10, 7, "x", ColliderShape::Sphere { radius: 1.0 })],
        );
        assert!(matches!(
            orphan,
            Err(PhysicsError::MissingOwner { owner: 7, .. })
        ));
        let mut heavy = ball_body(2, 1.0);
        heavy.mass = f32::NAN;
        assert!(Simulation::build(PhysicsSettings::default(), &[heavy], &[]).is_err());
        let slow = PhysicsSettings {
            step_seconds: 2.0,
            ..PhysicsSettings::default()
        };
        assert!(Simulation::build(slow, &[], &[]).is_err());

        let mut simulation = floor_and_ball(3.0);
        assert!(simulation.raycast(Vec3::ZERO, Vec3::ZERO, 10.0).is_err());
        assert!(
            simulation
                .apply_impulse(2, Vec3::new(f32::NAN, 0.0, 0.0))
                .is_err()
        );
        assert!(simulation.set_velocity(99, Vec3::ZERO).is_err());
        assert!(
            simulation.move_body(1, &at(0.0, 5.0, 0.0)).is_err(),
            "static bodies do not move"
        );
    }

    #[test]
    fn the_same_scene_gives_the_same_steps() {
        let run = || {
            let mut simulation = floor_and_ball(5.0);
            simulation
                .apply_impulse(2, Vec3::new(0.7, 0.0, -0.3))
                .expect("ball exists");
            for _ in 0..180 {
                simulation.step();
            }
            simulation.poses()
        };
        let first = run();
        let second = run();
        let bits = |poses: &[BodyPose]| {
            poses
                .iter()
                .flat_map(|pose| {
                    [
                        pose.position.x,
                        pose.position.y,
                        pose.position.z,
                        pose.rotation_degrees.y,
                    ]
                    .map(f32::to_bits)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&first), bits(&second), "bit for bit");
    }

    #[test]
    fn kinematic_bodies_follow_the_game_and_push() {
        let mut lift = BodyDescription::fixed(3, at(0.0, 0.0, 0.0));
        lift.kind = BodyKind::Kinematic;
        let bodies = [lift];
        let colliders = [collider(
            30,
            3,
            "elevador",
            ColliderShape::Box {
                size: Vec3::new(2.0, 0.2, 2.0),
            },
        )];
        let mut simulation =
            Simulation::build(PhysicsSettings::default(), &bodies, &colliders).expect("valid");
        simulation
            .move_body(3, &at(0.0, 1.0, 0.0))
            .expect("kinematic moves");
        simulation.step();
        let pose = simulation
            .poses()
            .into_iter()
            .find(|pose| pose.entity == 3)
            .expect("listed");
        assert!((pose.position.y - 1.0).abs() < 1.0e-4);
    }
}
