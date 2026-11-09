//! Weather and time of day (phase 5): the light a scene is under.
//!
//! A scene has one **time of day**, in hours, and one **weather** preset. Both are data: the renderer
//! reads the resulting sun direction, sun colour, ambient colour and fog, so nothing in the shader
//! knows what "rain" means.
//!
//! Time of day is a pure function of the hour, so a screenshot at 06:30 today looks exactly like one
//! tomorrow — the visual-regression rule from `AGENTS.md` §6.
//!
//! **Accessibility:** the profile of light changes is bounded. A person who asked for reduced motion
//! or high contrast gets a flatter, brighter sky and no rapid darkening; the comfort rule in `DR-13`
//! applies here before looks.

use petunia_math::{Color, Vec3};
use serde::{Deserialize, Serialize};

/// A time of day, in hours from 0 to 24.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeOfDay {
    pub hours: f32,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self { hours: 13.0 }
    }
}

/// Weather presets. Values are dials the renderer reads, not branches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weather {
    #[default]
    Clear,
    Cloudy,
    Rain,
    Fog,
    Snow,
}

/// How the scene is lit and how far you can see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// Direction **toward** the sun, normalised.
    pub sun_direction: Vec3,
    /// Sun colour times intensity.
    pub sun_color: Vec3,
    /// Ambient light on everything.
    pub ambient: Vec3,
    /// Sky colour at the horizon (what the background clears to).
    pub horizon: Color,
    /// Colour of the fog and how far it reaches, in meters. `None` when the air is clear.
    pub fog: Option<(Color, f32)>,
    /// How cloudy the sky is, 0 to 1. A renderer may use it to dim the sun.
    pub overcast: f32,
    /// Whether it is raining or snowing, for the particle system to read.
    pub precipitation: Precipitation,
}

/// What falls from the sky.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Precipitation {
    #[default]
    None,
    Rain,
    Snow,
}

impl TimeOfDay {
    /// Clamps the hour into 0..24, so a hostile file cannot ask for hour 900.
    #[must_use]
    pub fn clamped(self) -> Self {
        let hours = if self.hours.is_finite() {
            self.hours.rem_euclid(24.0)
        } else {
            Self::default().hours
        };
        Self { hours }
    }

    /// The sun's elevation in degrees: -90 at midnight, 90 at noon.
    #[must_use]
    pub fn sun_elevation_degrees(self) -> f32 {
        // The sun rises at 6 and sets at 18: a cosine through the day.
        let hours = self.clamped().hours;
        let phase = (hours - 6.0) / 24.0 * std::f32::consts::TAU;
        phase.sin() * 90.0
    }

    /// How high the sun is, from 0 (night) to 1 (noon).
    #[must_use]
    pub fn daylight(self) -> f32 {
        (self.sun_elevation_degrees() / 90.0).clamp(0.0, 1.0)
    }
}

/// The sky for a time and a weather preset.
///
/// `high_contrast` flattens the light and keeps the scene readable for someone who asked for it; it
/// never turns the scene into a different one, only into a clearer one.
#[must_use]
pub fn sky(time: TimeOfDay, weather: Weather, high_contrast: bool) -> Sky {
    let time = time.clamped();
    let elevation = time.sun_elevation_degrees();
    // The sun goes from east to west as the day passes.
    let (azimuth_sin, azimuth_cos) = {
        let phase = (time.hours - 6.0) / 12.0 * std::f32::consts::PI;
        (phase.sin(), phase.cos())
    };
    let elevation_radians = elevation.to_radians();
    let sun_direction = Vec3::new(
        azimuth_cos * elevation_radians.cos(),
        elevation_radians.sin(),
        azimuth_sin * elevation_radians.cos(),
    )
    .normalized_or_zero();
    let daylight = time.daylight();

    // Night is dark and blue, day is bright and warm; the mix is smooth so dusk does not snap.
    let night = Vec3::new(0.02, 0.03, 0.07);
    let day = Vec3::new(1.0, 0.97, 0.9);
    let sun_strength = daylight.max(0.0);
    let mut sun_color = Vec3::new(
        night.x + (day.x - night.x) * sun_strength,
        night.y + (day.y - night.y) * sun_strength,
        night.z + (day.z - night.z) * sun_strength,
    );
    let mut ambient = Vec3::new(
        0.10 + 0.45 * sun_strength,
        0.11 + 0.46 * sun_strength,
        0.14 + 0.44 * sun_strength,
    );
    let mut horizon = Color::new(
        0.05 + 0.55 * sun_strength,
        0.07 + 0.58 * sun_strength,
        0.12 + 0.60 * sun_strength,
        1.0,
    );
    let mut overcast = 0.0f32;
    let mut fog = None;
    let mut precipitation = Precipitation::None;

    match weather {
        Weather::Clear => {}
        Weather::Cloudy => {
            overcast = 0.6;
            // A cloudy sky is grey: the sun is still there, just spread out.
            sun_color = Vec3::new(sun_color.x * 0.5, sun_color.y * 0.5, sun_color.z * 0.55);
            ambient = Vec3::new(ambient.x * 1.15, ambient.y * 1.15, ambient.z * 1.2);
            horizon = Color::new(horizon.r * 0.85, horizon.g * 0.87, horizon.b * 0.9, 1.0);
        }
        Weather::Rain => {
            overcast = 0.85;
            sun_color = Vec3::new(sun_color.x * 0.35, sun_color.y * 0.36, sun_color.z * 0.4);
            ambient = Vec3::new(ambient.x * 0.9, ambient.y * 0.95, ambient.z * 1.0);
            horizon = Color::new(0.35, 0.39, 0.44, 1.0);
            fog = Some((Color::new(0.42, 0.46, 0.5, 1.0), 140.0));
            precipitation = Precipitation::Rain;
        }
        Weather::Fog => {
            overcast = 0.4;
            sun_color = Vec3::new(sun_color.x * 0.6, sun_color.y * 0.6, sun_color.z * 0.62);
            horizon = Color::new(0.62, 0.64, 0.66, 1.0);
            fog = Some((Color::new(0.66, 0.68, 0.70, 1.0), 60.0));
        }
        Weather::Snow => {
            overcast = 0.7;
            sun_color = Vec3::new(sun_color.x * 0.55, sun_color.y * 0.57, sun_color.z * 0.62);
            ambient = Vec3::new(ambient.x * 1.25, ambient.y * 1.25, ambient.z * 1.3);
            horizon = Color::new(0.82, 0.85, 0.9, 1.0);
            fog = Some((Color::new(0.85, 0.88, 0.92, 1.0), 90.0));
            precipitation = Precipitation::Snow;
        }
    }

    if high_contrast {
        // Flatten: a brighter floor, a weaker sun, so shapes read by silhouette, not by shading.
        ambient = Vec3::new(
            ambient.x.max(0.35),
            ambient.y.max(0.36),
            ambient.z.max(0.38),
        );
        sun_color = Vec3::new(sun_color.x * 0.7, sun_color.y * 0.7, sun_color.z * 0.7);
        // Fog is thinner: a wall of grey hurts more than it helps.
        fog = fog.map(|(color, reach)| (color, reach * 1.6));
    }

    Sky {
        sun_direction,
        sun_color,
        ambient,
        horizon,
        fog,
        overcast,
        precipitation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_the_highest_sun_and_midnight_the_lowest() {
        assert!((TimeOfDay { hours: 12.0 }.sun_elevation_degrees() - 90.0).abs() < 1e-3);
        assert!(TimeOfDay { hours: 0.0 }.sun_elevation_degrees() < -80.0);
        assert!(
            (TimeOfDay { hours: 6.0 }.sun_elevation_degrees()).abs() < 1e-3,
            "sunrise"
        );
        assert!(
            (TimeOfDay { hours: 18.0 }.sun_elevation_degrees()).abs() < 1e-3,
            "sunset"
        );
    }

    #[test]
    fn the_same_hour_and_weather_always_give_the_same_sky() {
        let time = TimeOfDay { hours: 6.5 };
        let first = sky(time, Weather::Rain, false);
        for _ in 0..6 {
            assert_eq!(sky(time, Weather::Rain, false), first);
        }
    }

    #[test]
    fn night_is_darker_and_has_no_daylight() {
        let noon = sky(TimeOfDay { hours: 12.0 }, Weather::Clear, false);
        let night = sky(TimeOfDay { hours: 1.0 }, Weather::Clear, false);
        assert!(noon.ambient.x > night.ambient.x, "noon is brighter");
        assert!(noon.sun_color.x > night.sun_color.x);
        assert!((TimeOfDay { hours: 1.0 }.daylight()).abs() < 1e-4);
        assert!((TimeOfDay { hours: 12.0 }.daylight() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn rain_brings_fog_and_precipitation_and_fog_brings_only_fog() {
        let rain = sky(TimeOfDay::default(), Weather::Rain, false);
        assert!(rain.fog.is_some());
        assert_eq!(rain.precipitation, Precipitation::Rain);
        let fog = sky(TimeOfDay::default(), Weather::Fog, false);
        assert!(fog.fog.is_some());
        assert_eq!(fog.precipitation, Precipitation::None);
    }

    #[test]
    fn high_contrast_raises_the_floor_and_thins_the_fog() {
        let normal = sky(TimeOfDay { hours: 21.0 }, Weather::Fog, false);
        let contrast = sky(TimeOfDay { hours: 21.0 }, Weather::Fog, true);
        assert!(
            contrast.ambient.x > normal.ambient.x,
            "the floor is raised: {} vs {}",
            normal.ambient.x,
            contrast.ambient.x
        );
        let (_, normal_reach) = normal.fog.expect("fog");
        let (_, contrast_reach) = contrast.fog.expect("fog");
        assert!(contrast_reach > normal_reach, "the fog reaches further");
    }

    #[test]
    fn the_sun_direction_is_always_a_unit_vector() {
        for hour in [0.0f32, 3.0, 6.0, 9.0, 12.0, 15.0, 18.0, 21.0, 23.9] {
            let sky = sky(TimeOfDay { hours: hour }, Weather::Clear, false);
            let length = sky.sun_direction.length();
            assert!(
                (length - 1.0).abs() < 1e-3 || length.abs() < 1e-6,
                "hour {hour}: length {length}"
            );
        }
    }

    #[test]
    fn a_hostile_hour_is_wrapped_instead_of_breaking_the_sky() {
        for hours in [f32::NAN, f32::INFINITY, -5.0, 900.0] {
            let time = TimeOfDay { hours }.clamped();
            assert!(time.hours.is_finite());
            assert!(
                (0.0..24.0).contains(&time.hours),
                "wrapped to {}",
                time.hours
            );
        }
    }
}
