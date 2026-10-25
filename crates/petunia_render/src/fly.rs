use petunia_math::Vec3;

use crate::camera::Camera;

/// Free-flying camera for exploring a scene. It is a view tool, not simulation, so it uses the
/// real elapsed time of each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyCamera {
    pub camera: Camera,
    /// Meters per second.
    pub move_speed: f32,
    /// Speed multiplier while the boost action is held.
    pub boost_multiplier: f32,
    pub look_degrees_per_pixel: f32,
}

impl FlyCamera {
    pub fn new(camera: Camera) -> Self {
        Self {
            camera,
            move_speed: 6.0,
            boost_multiplier: 3.0,
            look_degrees_per_pixel: 0.1,
        }
    }

    /// Turns the camera by a mouse movement in pixels. Non-finite input is ignored.
    pub fn look(&mut self, delta_x_pixels: f32, delta_y_pixels: f32) {
        if !(delta_x_pixels.is_finite() && delta_y_pixels.is_finite()) {
            return;
        }
        let yaw = self.camera.yaw_degrees + delta_x_pixels * self.look_degrees_per_pixel;
        self.camera.yaw_degrees = (yaw + 180.0).rem_euclid(360.0) - 180.0;
        let pitch = self.camera.pitch_degrees - delta_y_pixels * self.look_degrees_per_pixel;
        self.camera.pitch_degrees = pitch.clamp(-89.0, 89.0);
    }

    /// Moves along the camera's right, the world's up and the camera's forward. Each axis is in
    /// `-1..=1`. The elapsed time is clamped so a long frame cannot teleport the camera.
    pub fn advance(
        &mut self,
        right: f32,
        up: f32,
        forward: f32,
        boost: bool,
        elapsed_seconds: f32,
    ) {
        if !(right.is_finite()
            && up.is_finite()
            && forward.is_finite()
            && elapsed_seconds.is_finite())
        {
            return;
        }
        let seconds = elapsed_seconds.clamp(0.0, 0.1);
        let speed = if boost {
            self.move_speed * self.boost_multiplier
        } else {
            self.move_speed
        };
        let direction =
            self.camera.right() * right + Vec3::UP * up + self.camera.forward() * forward;
        self.camera.position = self.camera.position + direction * (speed * seconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_is_clamped_and_yaw_wraps() {
        let mut fly = FlyCamera::new(Camera::default());
        fly.look(0.0, -10_000.0);
        assert_eq!(fly.camera.pitch_degrees, 89.0);
        fly.look(1_900.0, 0.0);
        assert!(fly.camera.yaw_degrees > -180.0 && fly.camera.yaw_degrees <= 180.0);
    }

    #[test]
    fn long_frames_do_not_teleport() {
        let mut fly = FlyCamera::new(Camera::default());
        fly.advance(0.0, 0.0, 1.0, false, 10.0);
        let moved = (fly.camera.position - Vec3::ZERO).length();
        assert!(moved <= fly.move_speed * 0.1 + 1.0e-4);
    }
}
