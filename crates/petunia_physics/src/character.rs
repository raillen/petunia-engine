//! A character that walks: it moves by intent (a horizontal velocity and a jump request), slides
//! along walls, climbs steps and gentle slopes, stays on the ground going downhill, and forgives a
//! jump pressed just after leaving a ledge ("coyote time", an accessibility aid).
//!
//! The body is kinematic: the game decides where it goes, Rapier's character controller says how
//! far it can actually go. The solid collider of the owner is the character's shape.

use petunia_math::Vec3;
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::prelude as rapier;

use crate::{BodyKind, MAX_MAGNITUDE, PhysicsError, Simulation, check_range, check_vector, vector};

/// How one character behaves. Scripts and the editor change these through the `Character` table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterDescription {
    /// Persistent id of the object.
    pub entity: u64,
    /// Steepest slope it can walk up, in degrees (0 to 89).
    pub max_slope_degrees: f32,
    /// Tallest step it climbs without jumping, in meters.
    pub step_height: f32,
    /// Upward speed of a jump, in meters per second.
    pub jump_speed: f32,
    /// A jump still works this long after walking off a ledge, in seconds (0 turns it off).
    pub coyote_seconds: f32,
    /// 1 is normal gravity.
    pub gravity_scale: f32,
}

impl CharacterDescription {
    pub fn new(entity: u64) -> Self {
        Self {
            entity,
            max_slope_degrees: 50.0,
            step_height: 0.4,
            jump_speed: 6.5,
            coyote_seconds: 0.12,
            gravity_scale: 1.0,
        }
    }
}

/// What a character is doing right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CharacterState {
    pub(crate) description: CharacterDescription,
    /// Horizontal velocity asked for this step (meters per second, y ignored).
    pub(crate) intent: Vec3,
    pub(crate) jump_requested: bool,
    pub(crate) vertical_speed: f32,
    pub(crate) grounded: bool,
    /// Seconds since it last stood on something.
    pub(crate) air_seconds: f32,
    /// Slipping down a slope that is too steep.
    pub(crate) sliding: bool,
}

pub(crate) fn check_character(description: &CharacterDescription) -> Result<(), PhysicsError> {
    check_range(
        "inclinação máxima",
        description.max_slope_degrees,
        0.0,
        89.0,
    )?;
    check_range("altura do degrau", description.step_height, 0.0, 5.0)?;
    check_range("velocidade do pulo", description.jump_speed, 0.0, 100.0)?;
    check_range("tempo de coiote", description.coyote_seconds, 0.0, 2.0)?;
    check_range(
        "gravidade do personagem",
        description.gravity_scale,
        0.0,
        100.0,
    )
}

impl Simulation {
    /// Makes these objects characters. Each needs a kinematic body and a solid collider.
    pub fn add_characters(
        &mut self,
        characters: &[CharacterDescription],
    ) -> Result<(), PhysicsError> {
        for description in characters {
            check_character(description)?;
            let entity = description.entity;
            match self.kinds.get(&entity) {
                Some(BodyKind::Kinematic) => {}
                Some(_) => {
                    return Err(PhysicsError::Invalid(format!(
                        "o personagem #{entity} precisa de um corpo cinemático"
                    )));
                }
                None => return Err(PhysicsError::UnknownBody(entity)),
            }
            if self.character_collider(entity).is_none() {
                return Err(PhysicsError::Invalid(format!(
                    "o personagem #{entity} precisa de uma colisão sólida (não gatilho); uma cápsula é o melhor"
                )));
            }
            self.characters.insert(
                entity,
                CharacterState {
                    description: *description,
                    intent: Vec3::ZERO,
                    jump_requested: false,
                    vertical_speed: 0.0,
                    grounded: false,
                    air_seconds: 0.0,
                    sliding: false,
                },
            );
        }
        Ok(())
    }

    /// The first solid collider of the object, the character's shape.
    fn character_collider(&self, entity: u64) -> Option<rapier::ColliderHandle> {
        let body = self.world.bodies.get(*self.bodies.get(&entity)?)?;
        body.colliders().iter().copied().find(|handle| {
            self.world
                .colliders
                .get(*handle)
                .is_some_and(|collider| !collider.is_sensor())
        })
    }

    fn character_mut(
        &mut self,
        entity: u64,
    ) -> Result<&mut crate::character::CharacterState, PhysicsError> {
        self.characters
            .get_mut(&entity)
            .ok_or_else(|| PhysicsError::Invalid(format!("o objeto #{entity} não é um personagem")))
    }

    /// Where the character should walk: meters per second on the ground plane (y is ignored).
    pub fn set_character_intent(
        &mut self,
        entity: u64,
        velocity: Vec3,
    ) -> Result<(), PhysicsError> {
        check_vector("velocidade do personagem", velocity, MAX_MAGNITUDE)?;
        self.character_mut(entity)?.intent = Vec3::new(velocity.x, 0.0, velocity.z);
        Ok(())
    }

    /// Asks for a jump; it happens at the next step if the character is on the ground (or just left it).
    pub fn character_jump(&mut self, entity: u64) -> Result<(), PhysicsError> {
        self.character_mut(entity)?.jump_requested = true;
        Ok(())
    }

    /// Whether the object is a character (it moves itself, so its pose is written back).
    pub fn is_character(&self, entity: u64) -> bool {
        self.characters.contains_key(&entity)
    }

    pub fn character_grounded(&self, entity: u64) -> Result<bool, PhysicsError> {
        self.characters
            .get(&entity)
            .map(|state| state.grounded)
            .ok_or_else(|| PhysicsError::Invalid(format!("o objeto #{entity} não é um personagem")))
    }

    /// Moves every character for one step, before the world steps.
    pub(crate) fn drive_characters(&mut self) {
        let dt = self.settings.step_seconds;
        let gravity = -self.settings.gravity.y;
        let entities: Vec<u64> = self.characters.keys().copied().collect();
        for entity in entities {
            let Some(mut state) = self.characters.get(&entity).copied() else {
                continue;
            };
            let description = state.description;
            let mut vertical = state.vertical_speed;
            let coyote = state.grounded || state.air_seconds <= description.coyote_seconds;
            let position = self
                .bodies
                .get(&entity)
                .and_then(|handle| self.world.bodies.get(*handle))
                .map(|body| crate::from_vector(body.translation()));
            let water = position.and_then(|position| self.water_moment(position));
            let mut intent = state.intent;
            let jump_requested = state.jump_requested;
            state.jump_requested = false;
            if let Some(moment) = water {
                intent = intent + moment.flow;
                if intent.length() > moment.swim_force {
                    intent = intent.normalized_or_zero() * moment.swim_force;
                }
                if jump_requested {
                    vertical = description.jump_speed * 0.7;
                    state.air_seconds = description.coyote_seconds + 1.0;
                } else {
                    vertical = (vertical
                        + (moment.float_force - gravity * description.gravity_scale) * dt)
                        * 0.85;
                }
            } else if jump_requested && coyote {
                vertical = description.jump_speed;
                state.air_seconds = description.coyote_seconds + 1.0;
            } else if state.grounded && vertical < 0.0 {
                vertical = -1.0;
            } else {
                vertical -= gravity * description.gravity_scale * dt;
            }

            let desired = vector(Vec3::new(intent.x * dt, vertical * dt, intent.z * dt));
            let outcome = self.move_character_shape(entity, &description, desired, dt);
            if let Some((translation, grounded, sliding)) = outcome {
                if let Some(handle) = self.bodies.get(&entity).copied()
                    && let Some(body) = self.world.bodies.get_mut(handle)
                {
                    let mut pose = *body.position();
                    pose.translation += translation;
                    body.set_next_kinematic_position(pose);
                }
                // Hitting a ceiling stops the rise.
                if desired.y > 0.0 && translation.y < desired.y * 0.5 {
                    vertical = 0.0;
                }
                if grounded && !state.grounded {
                    vertical = 0.0;
                }
                state.grounded = grounded;
                state.sliding = sliding;
            }
            state.air_seconds = if state.grounded {
                0.0
            } else {
                state.air_seconds + dt
            };
            state.vertical_speed = vertical;
            self.characters.insert(entity, state);
        }
    }

    /// The movement Rapier allows: (translation, grounded, sliding down a steep slope).
    fn move_character_shape(
        &self,
        entity: u64,
        description: &CharacterDescription,
        desired: rapier::Vector,
        dt: f32,
    ) -> Option<(rapier::Vector, bool, bool)> {
        let collider_handle = self.character_collider(entity)?;
        let collider = self.world.colliders.get(collider_handle)?;
        let body_handle = *self.bodies.get(&entity)?;
        // Where the shape is now: the body's pose (a script may have just moved it, and Rapier
        // only updates the collider's own pose at the next step) with the collider's offset.
        let body = self.world.bodies.get(body_handle)?;
        let shape_pose = collider
            .position_wrt_parent()
            .map_or(*body.position(), |offset| *body.position() * *offset);
        let controller = KinematicCharacterController {
            up: rapier::Vector::Y,
            slide: true,
            autostep: (description.step_height > 0.0).then_some(CharacterAutostep {
                max_height: CharacterLength::Absolute(description.step_height),
                min_width: CharacterLength::Absolute(0.1),
                include_dynamic_bodies: false,
            }),
            max_slope_climb_angle: description.max_slope_degrees.to_radians(),
            min_slope_slide_angle: description.max_slope_degrees.to_radians(),
            snap_to_ground: Some(CharacterLength::Absolute(description.step_height.max(0.2))),
            ..KinematicCharacterController::default()
        };
        let filter = rapier::QueryFilter::default()
            .exclude_sensors()
            .exclude_rigid_body(body_handle)
            .groups(collider.collision_groups());
        let queries = self.world.query_pipeline_with_filter(filter);
        let movement =
            controller.move_shape(dt, &queries, collider.shape(), &shape_pose, desired, |_| {});
        Some((
            movement.translation,
            movement.grounded,
            movement.is_sliding_down_slope,
        ))
    }
}

#[cfg(test)]
mod tests {
    use petunia_math::Transform;

    use super::*;
    use crate::{
        BodyDescription, ColliderDescription, ColliderShape, DEFAULT_LAYER, DEFAULT_MASK,
        PhysicsSettings,
    };

    fn at(x: f32, y: f32, z: f32) -> Transform {
        Transform {
            position: Vec3::new(x, y, z),
            ..Transform::IDENTITY
        }
    }

    fn solid(id: u64, owner: u64, name: &str, shape: ColliderShape) -> ColliderDescription {
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

    fn block(entity: u64, center: Vec3, size: Vec3) -> (BodyDescription, ColliderDescription) {
        (
            BodyDescription::fixed(entity, at(center.x, center.y, center.z)),
            solid(entity * 10, entity, "bloco", ColliderShape::Box { size }),
        )
    }

    /// A 40 m floor with its top at y = 0, the given extra blocks, and a 1.8 m capsule character
    /// (entity 100) standing at `start`.
    fn world_with(
        extra: &[(BodyDescription, ColliderDescription)],
        start: Vec3,
        description: CharacterDescription,
    ) -> Simulation {
        let mut bodies = vec![BodyDescription::fixed(1, at(0.0, -0.5, 0.0))];
        let mut colliders = vec![solid(
            10,
            1,
            "chão",
            ColliderShape::Box {
                size: Vec3::new(40.0, 1.0, 40.0),
            },
        )];
        for (body, collider) in extra {
            bodies.push(body.clone());
            colliders.push(collider.clone());
        }
        let mut hero = BodyDescription::fixed(100, at(start.x, start.y, start.z));
        hero.kind = BodyKind::Kinematic;
        bodies.push(hero);
        colliders.push(solid(
            1000,
            100,
            "corpo",
            ColliderShape::Capsule {
                radius: 0.3,
                height: 1.8,
            },
        ));
        let mut simulation = Simulation::build(PhysicsSettings::default(), &bodies, &colliders)
            .expect("valid scene");
        simulation
            .add_characters(&[description])
            .expect("a character");
        simulation
    }

    fn hero(simulation: &Simulation) -> Vec3 {
        simulation
            .poses()
            .into_iter()
            .find(|pose| pose.entity == 100)
            .expect("the character moves")
            .position
    }

    fn run(simulation: &mut Simulation, steps: usize) {
        for _ in 0..steps {
            simulation.step();
        }
    }

    #[test]
    fn a_character_walking_into_a_static_trigger_or_wall_reports_the_contact() {
        // A trigger (a sensor, not solid) 4 m east of the start, and a solid wall further on.
        let (sensor_body, mut sensor) =
            block(2, Vec3::new(4.0, 0.9, 0.0), Vec3::new(1.0, 2.0, 2.0));
        sensor.trigger = true;
        let (wall_body, wall) = block(3, Vec3::new(8.0, 1.0, 0.0), Vec3::new(1.0, 2.0, 4.0));
        let mut simulation = world_with(
            &[(sensor_body, sensor), (wall_body, wall)],
            Vec3::new(0.0, 0.9, 0.0),
            CharacterDescription::new(100),
        );
        run(&mut simulation, 30);
        simulation
            .set_character_intent(100, Vec3::new(4.0, 0.0, 0.0))
            .expect("character");
        let mut touched: Vec<u64> = Vec::new();
        for _ in 0..240 {
            for event in simulation.step() {
                if event.phase == crate::ContactPhase::Started {
                    touched.push(event.first.owner);
                    touched.push(event.second.owner);
                }
            }
        }
        assert!(
            touched.contains(&2),
            "the trigger reported the character: {touched:?}"
        );
        assert!(
            touched.contains(&3),
            "the wall reported the character: {touched:?}"
        );
    }

    #[test]
    fn it_falls_lands_and_walks_where_it_is_told() {
        let mut simulation = world_with(
            &[],
            Vec3::new(0.0, 3.0, 0.0),
            CharacterDescription::new(100),
        );
        run(&mut simulation, 120);
        assert!(
            simulation.character_grounded(100).expect("character"),
            "landed"
        );
        assert!(
            (hero(&simulation).y - 0.9).abs() < 0.05,
            "capsule center 0.9 m above the floor"
        );
        simulation
            .set_character_intent(100, Vec3::new(3.0, 0.0, 0.0))
            .expect("character");
        run(&mut simulation, 60);
        let walked = hero(&simulation);
        assert!(
            (walked.x - 3.0).abs() < 0.2,
            "one second at 3 m/s: {walked:?}"
        );
        assert!((walked.y - 0.9).abs() < 0.05, "stays on the ground");
    }

    #[test]
    fn it_climbs_low_steps_and_stops_at_walls() {
        let step = block(2, Vec3::new(2.0, 0.15, 0.0), Vec3::new(1.0, 0.3, 4.0));
        let wall = block(3, Vec3::new(6.0, 1.0, 0.0), Vec3::new(1.0, 2.0, 4.0));
        let mut simulation = world_with(
            &[step, wall],
            Vec3::new(0.0, 0.9, 0.0),
            CharacterDescription::new(100),
        );
        run(&mut simulation, 10);
        simulation
            .set_character_intent(100, Vec3::new(4.0, 0.0, 0.0))
            .expect("character");
        run(&mut simulation, 90);
        let position = hero(&simulation);
        assert!(
            position.x > 4.0 && position.x < 5.6,
            "climbed the step, stopped at the wall: {position:?}"
        );
        assert!(position.x < 5.5 + 1e-3, "did not enter the wall");
    }

    #[test]
    fn a_step_taller_than_allowed_blocks_it() {
        let ledge = block(2, Vec3::new(2.0, 0.6, 0.0), Vec3::new(1.0, 1.2, 4.0));
        let mut simulation = world_with(
            &[ledge],
            Vec3::new(0.0, 0.9, 0.0),
            CharacterDescription::new(100),
        );
        run(&mut simulation, 10);
        simulation
            .set_character_intent(100, Vec3::new(4.0, 0.0, 0.0))
            .expect("character");
        run(&mut simulation, 90);
        assert!(
            hero(&simulation).x < 1.6,
            "1.2 m is a wall for a 0.4 m step: {:?}",
            hero(&simulation)
        );
    }

    #[test]
    fn it_jumps_and_forgives_a_late_jump_after_leaving_a_ledge() {
        let mut simulation = world_with(
            &[],
            Vec3::new(0.0, 0.9, 0.0),
            CharacterDescription::new(100),
        );
        run(&mut simulation, 20);
        simulation.character_jump(100).expect("character");
        run(&mut simulation, 15);
        assert!(
            hero(&simulation).y > 1.5,
            "in the air: {:?}",
            hero(&simulation)
        );
        assert!(!simulation.character_grounded(100).expect("character"));
        run(&mut simulation, 120);
        assert!(
            simulation.character_grounded(100).expect("character"),
            "landed again"
        );

        // A platform edge at x = 3: walk off it and jump 0.08 s later (inside the 0.12 s window).
        let platform = block(2, Vec3::new(0.0, 1.0, 0.0), Vec3::new(6.0, 2.0, 6.0));
        let mut simulation = world_with(
            &[platform],
            Vec3::new(0.0, 2.9, 0.0),
            CharacterDescription::new(100),
        );
        run(&mut simulation, 20);
        simulation
            .set_character_intent(100, Vec3::new(4.0, 0.0, 0.0))
            .expect("character");
        let mut left_at = None;
        for index in 0..120 {
            simulation.step();
            if left_at.is_none() && !simulation.character_grounded(100).expect("character") {
                left_at = Some(index);
                break;
            }
        }
        assert!(left_at.is_some(), "walked off the platform");
        run(&mut simulation, 3);
        let before = hero(&simulation).y;
        simulation.character_jump(100).expect("character");
        run(&mut simulation, 6);
        assert!(
            hero(&simulation).y > before,
            "the late jump went up instead of falling"
        );

        // With coyote time off, the same late jump does nothing.
        let platform = block(2, Vec3::new(0.0, 1.0, 0.0), Vec3::new(6.0, 2.0, 6.0));
        let mut strict = CharacterDescription::new(100);
        strict.coyote_seconds = 0.0;
        let mut simulation = world_with(&[platform], Vec3::new(0.0, 2.9, 0.0), strict);
        run(&mut simulation, 20);
        simulation
            .set_character_intent(100, Vec3::new(4.0, 0.0, 0.0))
            .expect("character");
        for _ in 0..120 {
            simulation.step();
            if !simulation.character_grounded(100).expect("character") {
                break;
            }
        }
        run(&mut simulation, 3);
        let before = hero(&simulation).y;
        simulation.character_jump(100).expect("character");
        run(&mut simulation, 6);
        assert!(
            hero(&simulation).y < before,
            "without coyote time it keeps falling"
        );
    }

    #[test]
    fn it_refuses_bad_setups_and_bad_numbers() {
        let mut bodies = vec![BodyDescription::fixed(100, at(0.0, 1.0, 0.0))];
        let colliders = [solid(
            1000,
            100,
            "corpo",
            ColliderShape::Capsule {
                radius: 0.3,
                height: 1.8,
            },
        )];
        let mut simulation =
            Simulation::build(PhysicsSettings::default(), &bodies, &colliders).expect("valid");
        assert!(
            simulation
                .add_characters(&[CharacterDescription::new(100)])
                .is_err(),
            "static body"
        );
        assert!(
            simulation
                .add_characters(&[CharacterDescription::new(7)])
                .is_err(),
            "unknown object"
        );
        bodies[0].kind = BodyKind::Kinematic;
        let mut simulation =
            Simulation::build(PhysicsSettings::default(), &bodies, &[]).expect("valid");
        assert!(
            simulation
                .add_characters(&[CharacterDescription::new(100)])
                .is_err(),
            "no collider"
        );
        let mut simulation =
            Simulation::build(PhysicsSettings::default(), &bodies, &colliders).expect("valid");
        let mut wild = CharacterDescription::new(100);
        wild.step_height = f32::NAN;
        assert!(simulation.add_characters(&[wild]).is_err());
        wild.step_height = 0.4;
        wild.max_slope_degrees = 120.0;
        assert!(simulation.add_characters(&[wild]).is_err());
        assert!(
            simulation
                .set_character_intent(100, Vec3::new(1.0, 0.0, 0.0))
                .is_err(),
            "not a character yet"
        );
        simulation
            .add_characters(&[CharacterDescription::new(100)])
            .expect("valid now");
        assert!(
            simulation
                .set_character_intent(100, Vec3::new(f32::NAN, 0.0, 0.0))
                .is_err()
        );
    }

    #[test]
    fn layers_and_masks_decide_who_touches_whom() {
        let mut ghost = CharacterDescription::new(100);
        ghost.step_height = 0.0;
        let bodies = [BodyDescription::fixed(1, at(0.0, -0.5, 0.0)), {
            let mut hero = BodyDescription::fixed(100, at(0.0, 2.0, 0.0));
            hero.kind = BodyKind::Kinematic;
            hero
        }];
        let floor = solid(
            10,
            1,
            "chão",
            ColliderShape::Box {
                size: Vec3::new(40.0, 1.0, 40.0),
            },
        );
        let mut body = solid(
            1000,
            100,
            "corpo",
            ColliderShape::Capsule {
                radius: 0.3,
                height: 1.8,
            },
        );
        // The floor is on layer 2; the character only touches layer 1: it falls through.
        let mut other_floor = floor.clone();
        other_floor.layer = 2;
        body.mask = 1;
        let mut simulation = Simulation::build(
            PhysicsSettings::default(),
            &bodies,
            &[other_floor, body.clone()],
        )
        .expect("valid");
        simulation.add_characters(&[ghost]).expect("a character");
        run(&mut simulation, 120);
        assert!(
            hero(&simulation).y < -1.0,
            "fell through the floor on another layer: {:?}",
            hero(&simulation)
        );

        // Same floor on layer 1: the character lands.
        let mut simulation =
            Simulation::build(PhysicsSettings::default(), &bodies, &[floor, body]).expect("valid");
        simulation.add_characters(&[ghost]).expect("a character");
        run(&mut simulation, 120);
        assert!(
            (hero(&simulation).y - 0.9).abs() < 0.05,
            "{:?}",
            hero(&simulation)
        );
    }

    #[test]
    fn the_same_walk_gives_the_same_path() {
        let run_once = || {
            let step = block(2, Vec3::new(2.0, 0.15, 0.0), Vec3::new(1.0, 0.3, 4.0));
            let mut simulation = world_with(
                &[step],
                Vec3::new(0.0, 1.5, 0.0),
                CharacterDescription::new(100),
            );
            simulation
                .set_character_intent(100, Vec3::new(2.5, 0.0, 0.4))
                .expect("character");
            for index in 0..200 {
                if index == 40 {
                    simulation.character_jump(100).expect("character");
                }
                simulation.step();
            }
            let position = hero(&simulation);
            [position.x, position.y, position.z].map(f32::to_bits)
        };
        assert_eq!(run_once(), run_once());
    }
}
