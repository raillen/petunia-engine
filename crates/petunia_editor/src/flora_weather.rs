//! Rules, weather and precipitation in the editor (phases 4 and 5).
//!
//! The session keeps the **inputs** — the vegetation rules, the hour, the weather preset — and builds
//! the frame from them. Nothing here writes to the world: a forest is a view of the terrain, not a
//! thousand entities, so painting rules is cheap and undoing it is instant.
//!
//! What the rules produce is saved with the project (`Scene` table in the manifest folder is not
//! needed: the rules live in the `FloraRule` table), so reopening a map shows the same forest.

use petunia_flora::{FloraError, Rule};
use petunia_math::Vec3;
use petunia_render::particles::ParticleSystem;
use petunia_render::weather::{Precipitation, TimeOfDay, Weather, sky};
use petunia_world::{HeightChunk, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

/// Table with one row per vegetation rule, so a forest survives reopening.
pub const FLORA_RULE_TABLE: &str = "FloraRule";

impl EditorSession {
    /// `flora.add`: adds a rule for one species.
    pub fn flora_add(&mut self, species: &str, spacing: f32, seed: u64) -> Result<Json, Failure> {
        let name = species.trim();
        if name.is_empty() {
            return Err(Failure::new(
                "FLORA_NO_SPECIES",
                "a regra precisa de uma espécie",
                "Dê um nome à planta, como \"grama\" ou \"pinheiro\".",
            ));
        }
        let rule = Rule {
            species: name.to_string(),
            spacing,
            seed,
            ..Rule::empty(name)
        };
        // A rule that cannot produce anything is refused now, not at draw time.
        if let Err(error) = validate(&rule) {
            return Err(failure(error));
        }
        self.execute(&petunia_commands::CommandRecord::new(
            "world.create",
            json!({ "rows": { FLORA_RULE_TABLE: {
                "species": name,
                "spacing": spacing,
                "seed": seed as i64,
                "height_min": rule.height_range.0,
                "height_max": rule.height_range.1,
                "max_slope": rule.maximum_slope_degrees,
                "scale_min": rule.scale_range.0,
                "scale_max": rule.scale_range.1,
            } } }),
        ))?;
        Ok(json!({ "species": name, "rules": self.flora_rules().len() }))
    }

    /// `flora.list`: the rules of the project.
    pub fn flora_list(&self) -> Result<Json, Failure> {
        let rules = self.flora_rules();
        Ok(json!({
            "rules": rules.iter().map(|rule| json!({
                "species": rule.species,
                "spacing": rule.spacing,
                "seed": rule.seed,
                "max_slope": rule.maximum_slope_degrees,
            })).collect::<Vec<_>>(),
        }))
    }

    /// `flora.scatter`: how many plants the rules place right now, without building the mesh.
    ///
    /// The window uses it to warn before a rule that matches a whole island.
    pub fn flora_count(&self) -> Result<Json, Failure> {
        let rules = self.flora_rules();
        if rules.is_empty() {
            return Ok(json!({ "plants": 0 }));
        }
        let world = self.world()?;
        let chunks = terrain_chunks(world);
        let borrowed: Vec<&HeightChunk> = chunks.iter().collect();
        match petunia_flora::scatter_world(&borrowed, &rules) {
            Ok(plants) => Ok(json!({ "plants": plants.len() })),
            Err(FloraError::TooMany) => Ok(json!({
                "plants": petunia_flora::MAX_INSTANCES,
                "truncated": true,
            })),
            Err(error) => Err(failure(error)),
        }
    }

    /// `weather.set`: the hour and the preset the viewport draws with.
    pub fn weather_set(&mut self, hours: f32, preset: &str) -> Result<Json, Failure> {
        let weather = parse_weather(preset).ok_or_else(|| {
            Failure::new(
                "WEATHER_UNKNOWN",
                &format!("clima `{preset}` desconhecido"),
                "Use clear, cloudy, rain, fog ou snow.",
            )
        })?;
        self.time_of_day = TimeOfDay { hours }.clamped();
        self.weather = weather;
        self.precipitation = ParticleSystem::new(
            // A preset without falling water still keeps an empty system, so switching back is
            // instant and the pool is not rebuilt.
            petunia_render::particles::Kind::of(precipitation_of(weather))
                .unwrap_or(petunia_render::particles::Kind::Rain),
            4_800,
            14.0,
            7,
        );
        Ok(json!({
            "hours": self.time_of_day.hours,
            "weather": preset,
            "sky": {
                "sun": {
                    "x": sky(self.time_of_day, self.weather, false).sun_direction.x,
                    "y": sky(self.time_of_day, self.weather, false).sun_direction.y,
                    "z": sky(self.time_of_day, self.weather, false).sun_direction.z,
                },
                "overcast": sky(self.time_of_day, self.weather, false).overcast,
            }
        }))
    }

    /// `weather.get`: the current hour and preset.
    pub fn weather_get(&self) -> Json {
        if let Ok(world) = self.world() {
            let active = petunia_world::active_weathers_at(world, self.camera.camera.position);
            if let Some(first) = active.first() {
                return json!({
                    "hours": first.hours,
                    "weather": first.preset,
                });
            }
        }
        json!({
            "hours": self.time_of_day.hours,
            "weather": weather_name(self.weather),
        })
    }

    /// Fills the frame's sky, vegetation and precipitation from the session's inputs.
    ///
    /// Runs on the frame the renderer gets, so changing the hour or the rules never touches the
    /// world and never enters the undo history.
    pub(crate) fn apply_weather_and_flora(&self, frame: &mut petunia_render::RenderFrame) {
        let high_contrast = self.high_contrast;
        let world_sky = self.world().ok().and_then(|world| {
            petunia_render::weather::evaluate_world_sky(
                world,
                self.camera.camera.position,
                high_contrast,
            )
        });
        let computed =
            world_sky.unwrap_or_else(|| sky(self.time_of_day, self.weather, high_contrast));
        frame.fog = computed.fog;
        // The sky drives the light only when the scene has no light of its own; otherwise the
        // person's lights win, exactly as before.
        let has_lights = frame.lighting.custom;
        if !has_lights {
            frame.lighting.custom = true;
            frame.lighting.sun = Some((computed.sun_direction, computed.sun_color));
            frame.lighting.ambient = computed.ambient;
            frame.clear_color = computed.horizon;
        } else {
            frame.clear_color = computed.horizon;
        }
        frame.grade = if high_contrast {
            petunia_render::Grade::comfort()
        } else {
            petunia_render::Grade::default()
        };

        // Vegetation: only when there is a rule and a terrain to put plants on. The cache is keyed
        // by the rules and the camera's facing, because a scatter over a whole island on every
        // repaint is what froze the editor the first time this was written.
        frame.vegetation = self.cached_vegetation();

        // Precipitation: the particle mesh around the camera, when the preset asks for it. The pool
        // is stepped with the same clock as the animation (see `animation_advance`), so the drops
        // fall instead of hanging in the air.
        let precipitation_kind = world_sky
            .map(|sky_item| sky_item.precipitation)
            .unwrap_or_else(|| precipitation_of(self.weather));
        if precipitation_kind != Precipitation::None {
            frame.precipitation = Some(std::sync::Arc::new(self.precipitation.mesh()));
        }
    }

    /// Steps the precipitation around the camera. Called by the same clock as the animation, so a
    /// paused editor has still air and a playing one has rain.
    pub(crate) fn advance_precipitation(&mut self, seconds: f32) {
        let world_precipitation = self
            .world()
            .ok()
            .and_then(|world| {
                petunia_render::weather::evaluate_world_sky(
                    world,
                    self.camera.camera.position,
                    false,
                )
            })
            .map(|sky_item| sky_item.precipitation);
        let precipitation_kind =
            world_precipitation.unwrap_or_else(|| precipitation_of(self.weather));
        if precipitation_kind == Precipitation::None {
            return;
        }
        let centre = self.camera.camera.position;
        self.precipitation.step(seconds, centre);
    }

    /// The vegetation mesh, rebuilt only when the rules or the camera's facing changed.
    ///
    /// Turning the camera by a few degrees reuses the impostors: a quad facing 3 degrees off is not
    /// worth re-scattering 1 936 plants for.
    fn cached_vegetation(&self) -> Option<std::sync::Arc<petunia_render::MeshData>> {
        let rules = self.flora_rules();
        if rules.is_empty() {
            return None;
        }
        let facing = petunia_render::vegetation::facing_for(self.camera.camera.yaw_degrees);
        // Quantise the facing to the rebuild step, so a slow orbit does not rebuild constantly.
        let step = petunia_render::vegetation::REBUILD_ANGLE_DEGREES;
        let bucket = (facing / step).round() as i32;
        let key = (
            rules.len(),
            rules.iter().map(|rule| rule.seed).sum::<u64>(),
            bucket,
        );
        if let Ok(slot) = self.vegetation_cache.try_borrow()
            && let Some((cached_key, mesh)) = slot.as_ref()
            && *cached_key == key
        {
            return Some(mesh.clone());
        }
        let world = self.world().ok()?;
        let chunks = terrain_chunks(world);
        let borrowed: Vec<&HeightChunk> = chunks.iter().collect();
        let plants = petunia_flora::scatter_world(&borrowed, &rules).ok()?;
        let mesh = std::sync::Arc::new(petunia_render::vegetation::impostor_mesh(
            &plants,
            bucket as f32 * step,
            Vec3::new(0.5, 1.4, 0.5),
        ));
        // The cache is written through interior mutability: `apply_weather_and_flora` takes `&self`.
        if let Ok(mut slot) = self.vegetation_cache.try_borrow_mut() {
            *slot = Some((key, mesh.clone()));
        }
        Some(mesh)
    }

    fn flora_rules(&self) -> Vec<Rule> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        let Ok(handles) = world.entities_with_table(FLORA_RULE_TABLE) else {
            return Vec::new();
        };
        let mut rules = Vec::new();
        for handle in handles {
            let text = |column: &str| match world.get(handle, FLORA_RULE_TABLE, column) {
                Ok(Value::Text(value)) => value.clone(),
                _ => String::new(),
            };
            let number = |column: &str| match world.get(handle, FLORA_RULE_TABLE, column) {
                Ok(Value::Float(value)) => *value as f32,
                Ok(Value::Int(value)) => *value as f32,
                _ => 0.0,
            };
            let species = text("species");
            if species.is_empty() {
                continue;
            }
            let mut rule = Rule::empty(&species);
            rule.spacing = number("spacing");
            rule.height_range = (number("height_min"), number("height_max"));
            rule.maximum_slope_degrees = number("max_slope");
            rule.scale_range = (number("scale_min").max(0.01), number("scale_max").max(0.01));
            rule.seed = match world.get(handle, FLORA_RULE_TABLE, "seed") {
                Ok(Value::Int(value)) if *value >= 0 => *value as u64,
                _ => 0,
            };
            rules.push(rule);
        }
        rules
    }
}

/// The terrain chunks of a world, in a stable order so a scatter is repeatable.
fn terrain_chunks(world: &petunia_world::World) -> Vec<HeightChunk> {
    let Ok(handles) = world.entities_with_table("TerrainChunk") else {
        return Vec::new();
    };
    let mut chunks = Vec::new();
    for handle in handles {
        let (Ok(Value::Int(chunk_x)), Ok(Value::Int(chunk_z))) = (
            world.get(handle, "TerrainChunk", "chunk_x"),
            world.get(handle, "TerrainChunk", "chunk_z"),
        ) else {
            continue;
        };
        let size = match world.get(handle, "TerrainChunk", "size") {
            Ok(Value::Int(size)) if (2..=256).contains(size) => *size as usize,
            _ => continue,
        };
        let cell_size = match world.get(handle, "TerrainChunk", "cell_size") {
            Ok(Value::Float(cell)) if cell.is_finite() && *cell > 0.0 => *cell as f32,
            _ => 1.0,
        };
        let Ok(Value::Floats(heights)) = world.get(handle, "TerrainChunk", "heights") else {
            continue;
        };
        if heights.len() != size * size {
            continue;
        }
        let mut chunk = HeightChunk::new([*chunk_x as i32, *chunk_z as i32], size, cell_size);
        chunk.heights.copy_from_slice(heights);
        chunks.push(chunk);
    }
    chunks.sort_by_key(|chunk| {
        (
            chunk.world_origin().x.to_bits(),
            chunk.world_origin().z.to_bits(),
        )
    });
    chunks
}

fn validate(rule: &Rule) -> Result<(), FloraError> {
    if !rule.spacing.is_finite() || rule.spacing <= 0.05 {
        return Err(FloraError::BadRule(format!(
            "o espaçamento de `{}` precisa ser maior que 5 cm",
            rule.species
        )));
    }
    Ok(())
}

fn failure(error: FloraError) -> Failure {
    Failure::new(
        "FLORA_RULE",
        &error.to_string(),
        "Ajuste o espaçamento, a altura ou a inclinação da regra.",
    )
}

fn parse_weather(name: &str) -> Option<Weather> {
    match name {
        "clear" => Some(Weather::Clear),
        "cloudy" => Some(Weather::Cloudy),
        "rain" => Some(Weather::Rain),
        "fog" => Some(Weather::Fog),
        "snow" => Some(Weather::Snow),
        _ => None,
    }
}

fn weather_name(weather: Weather) -> &'static str {
    match weather {
        Weather::Clear => "clear",
        Weather::Cloudy => "cloudy",
        Weather::Rain => "rain",
        Weather::Fog => "fog",
        Weather::Snow => "snow",
    }
}

fn precipitation_of(weather: Weather) -> Precipitation {
    match weather {
        Weather::Rain => Precipitation::Rain,
        Weather::Snow => Precipitation::Snow,
        _ => Precipitation::None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value as Json, json};

    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn opened(label: &str) -> (EditorSession, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_flora_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": label }),
        );
        (session, root)
    }

    #[test]
    fn a_rule_is_added_listed_and_refused_when_it_cannot_work() {
        let (mut session, root) = opened("rules");
        call(
            &mut session,
            json!({ "op": "flora.add", "species": "grama", "spacing": 1.5, "seed": 11 }),
        );
        let listed = call(&mut session, json!({ "op": "flora.list" }));
        assert_eq!(listed["rules"][0]["species"], "grama", "{listed}");

        // A spacing under 5 cm cannot produce a forest, only a wall: refused by name.
        let refused = session.handle(&json!({
            "op": "flora.add", "species": "musgo", "spacing": 0.01
        }));
        assert_eq!(refused["ok"], false, "{refused}");
        assert_eq!(refused["error"]["code"], "FLORA_RULE");

        let no_name = session.handle(&json!({ "op": "flora.add", "species": "  " }));
        assert_eq!(no_name["ok"], false);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_weather_changes_the_sky_and_never_the_scene() {
        let (mut session, root) = opened("sky");
        let before = call(&mut session, json!({ "op": "weather.get" }));
        assert_eq!(before["weather"], "clear");

        let rainy = call(
            &mut session,
            json!({ "op": "weather.set", "hours": 21.0, "weather": "rain" }),
        );
        assert_eq!(rainy["weather"], "rain", "{rainy}");
        assert!((rainy["hours"].as_f64().unwrap_or(0.0) - 21.0).abs() < 1e-6);

        // The frame carries the fog and the falling water.
        let frame = session.frame().expect("frame");
        assert!(frame.fog.is_some(), "rain brings fog");
        assert!(frame.precipitation.is_some(), "rain falls");
        assert!(frame.lighting.custom, "the sky drives the light");

        // Nothing became undoable: the weather is a view.
        let snapshot = call(&mut session, json!({ "op": "editor.snapshot" }));
        assert_eq!(snapshot["can_undo"], false, "{snapshot}");

        let unknown = session.handle(&json!({
            "op": "weather.set", "hours": 10.0, "weather": "tsunami"
        }));
        assert_eq!(unknown["ok"], false);
        assert_eq!(unknown["error"]["code"], "WEATHER_UNKNOWN");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clear_weather_leaves_no_fog_and_no_rain() {
        let (mut session, root) = opened("clear");
        call(
            &mut session,
            json!({ "op": "weather.set", "hours": 12.0, "weather": "clear" }),
        );
        let frame = session.frame().expect("frame");
        assert!(frame.fog.is_none());
        assert!(frame.precipitation.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_plant_count_answers_without_building_the_mesh() {
        let (mut session, root) = opened("count");
        // No rules, no plants.
        let empty = call(&mut session, json!({ "op": "flora.count" }));
        assert_eq!(empty["plants"], 0, "{empty}");

        call(
            &mut session,
            json!({ "op": "flora.add", "species": "grama", "spacing": 1.0 }),
        );
        // The sample project has no terrain, so the count stays zero: a floor and walls are not
        // ground a rule can plant on.
        let counted = call(&mut session, json!({ "op": "flora.count" }));
        assert_eq!(counted["plants"], 0, "{counted}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_project_without_rules_draws_no_vegetation() {
        let (session, root) = opened("none");
        let frame = session.frame().expect("frame");
        assert!(frame.vegetation.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
