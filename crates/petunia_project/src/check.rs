use std::path::{Path, PathBuf};

use petunia_math::CubicBezierSpline;
use petunia_world::{PersistentId, Value, World};

use crate::diagnostics::{Diagnostic, Report, Severity};
use crate::error::ProjectError;
use crate::io::{MAX_MANIFEST_BYTES, MAX_SCENE_BYTES, read_limited, resolve_inside};
use crate::manifest::{MANIFEST_FILE, ProjectManifest};
use crate::scene::{SCENE_FORMAT_VERSION, SceneFile, load_entities};
use crate::standard::{BRUSH_SHAPES, register_standard_tables};

/// Seed used for projects until the manifest carries one.
const PROJECT_SEED: i64 = 1;

/// A project loaded into a world, ready to run.
#[derive(Debug)]
pub struct LoadedProject {
    pub root: PathBuf,
    pub manifest: ProjectManifest,
    pub scene_path: PathBuf,
    pub world: World,
}

/// Result of loading: the project (when it could be built) and everything found on the way.
#[derive(Debug)]
pub struct LoadOutcome {
    pub project: Option<LoadedProject>,
    pub report: Report,
}

/// Validates a project without opening a window or running anything.
pub fn check_project(root: &Path) -> Report {
    load_project(root).report
}

/// Loads the manifest and the default scene. The project is returned even when only warnings
/// were found; callers decide whether errors in the report stop them.
pub fn load_project(root: &Path) -> LoadOutcome {
    let mut report = Report::default();

    let manifest = match read_manifest(root) {
        Ok(manifest) => manifest,
        Err(diagnostic) => {
            report.push(diagnostic);
            return LoadOutcome {
                project: None,
                report,
            };
        }
    };
    let manifest_problems = manifest.validate();
    let manifest_invalid = manifest_problems
        .iter()
        .any(|finding| finding.severity == Severity::Error);
    report.extend(manifest_problems);
    if manifest_invalid {
        return LoadOutcome {
            project: None,
            report,
        };
    }

    let scene_path = match resolve_inside(root, &manifest.default_scene) {
        Ok(path) => path,
        Err(error) => {
            report.push(Diagnostic::error(
                "project.manifest.scene_path_invalid",
                MANIFEST_FILE,
                &error.to_string(),
                "Corrija o campo `default_scene`.",
            ));
            return LoadOutcome {
                project: None,
                report,
            };
        }
    };

    let scene = match read_scene(&scene_path, &manifest.default_scene) {
        Ok(scene) => scene,
        Err(diagnostic) => {
            report.push(diagnostic);
            return LoadOutcome {
                project: None,
                report,
            };
        }
    };

    let mut world = World::new(PROJECT_SEED);
    if let Err(error) = register_standard_tables(&mut world) {
        report.push(Diagnostic::error(
            "engine.standard_tables",
            "engine",
            &format!("não foi possível registrar as tabelas padrão: {error}"),
            "Isso é um defeito da engine; registre o problema.",
        ));
        return LoadOutcome {
            project: None,
            report,
        };
    }
    load_entities(&mut world, &scene, &mut report);
    check_world(&world, &mut report);

    let project = LoadedProject {
        root: root.to_path_buf(),
        manifest,
        scene_path,
        world,
    };
    LoadOutcome {
        project: Some(project),
        report,
    }
}

fn read_manifest(root: &Path) -> Result<ProjectManifest, Diagnostic> {
    let path = root.join(MANIFEST_FILE);
    if !path.is_file() {
        return Err(Diagnostic::error(
            "project.manifest.missing",
            MANIFEST_FILE,
            &format!("não encontrei `{MANIFEST_FILE}` em {}", root.display()),
            "Confira a pasta ou crie um projeto com `petunia new <pasta>`.",
        ));
    }
    let bytes = read_limited(&path, MAX_MANIFEST_BYTES)
        .map_err(|error| io_diagnostic(MANIFEST_FILE, &error))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        Diagnostic::error(
            "project.manifest.invalid",
            MANIFEST_FILE,
            &format!("JSON inválido: {error}"),
            "Corrija o arquivo no ponto indicado (linha e coluna).",
        )
    })
}

fn read_scene(path: &Path, label: &str) -> Result<SceneFile, Diagnostic> {
    if !path.is_file() {
        return Err(Diagnostic::error(
            "scene.file.missing",
            label,
            "o arquivo da cena padrão não existe",
            "Crie a cena ou corrija `default_scene` no manifesto.",
        ));
    }
    let bytes =
        read_limited(path, MAX_SCENE_BYTES).map_err(|error| io_diagnostic(label, &error))?;
    let scene: SceneFile = serde_json::from_slice(&bytes).map_err(|error| {
        Diagnostic::error(
            "scene.file.invalid",
            label,
            &format!("JSON inválido: {error}"),
            "Corrija o arquivo no ponto indicado (linha e coluna).",
        )
    })?;
    if scene.format_version != SCENE_FORMAT_VERSION {
        return Err(Diagnostic::error(
            "scene.file.version",
            label,
            &format!("format_version {} não é suportada", scene.format_version),
            "Atualize a engine ou migre a cena.",
        ));
    }
    Ok(scene)
}

fn io_diagnostic(target: &str, error: &ProjectError) -> Diagnostic {
    match error {
        ProjectError::TooLarge { max_bytes, .. } => Diagnostic::error(
            "scene.file.too_large",
            target,
            &format!("o arquivo passa do limite de {max_bytes} bytes"),
            "Divida o conteúdo em arquivos menores.",
        ),
        other => Diagnostic::error(
            "project.io",
            target,
            &other.to_string(),
            "Confira as permissões e se o arquivo existe.",
        ),
    }
}

/// Checks that make sense once the entities are loaded.
fn maximum_ascent(spline: &CubicBezierSpline) -> f32 {
    let segments = spline.segments();
    if segments == 0 {
        return 0.0;
    }
    let samples = 64;
    let mut maximum = 0.0f32;
    let mut previous = spline.evaluate_position(0.0).map_or(0.0, |point| point.y);
    for index in 1..=samples {
        let parameter = (index as f32 / samples as f32) * segments as f32;
        if let Some(position) = spline.evaluate_position(parameter) {
            maximum = maximum.max(position.y - previous);
            previous = position.y;
        }
    }
    maximum
}

fn check_terrain(world: &World, report: &mut Report) {
    for handle in world.entities_with_table("Terrain").unwrap_or_default() {
        let id = world.persistent_id(handle).map_or(0, |id| id.0);
        let target = format!("terreno #{id}");
        if let Ok(Value::Float(cell_size)) = world.get(handle, "Terrain", "cell_size")
            && *cell_size <= 0.0
        {
            report.push(Diagnostic::error(
                "scene.terrain.cell_size_invalid",
                &target,
                "o tamanho da célula do terreno precisa ser maior que zero",
                "Use um valor em metros, por exemplo 1,0.",
            ));
        }
    }
    let mut seen: std::collections::BTreeSet<(u64, i64, i64)> = std::collections::BTreeSet::new();
    for handle in world
        .entities_with_table("TerrainChunk")
        .unwrap_or_default()
    {
        let id = world.persistent_id(handle).map_or(0, |id| id.0);
        let target = format!("bloco de terreno #{id}");
        let owner = match world.get(handle, "TerrainChunk", "owner") {
            Ok(Value::Int(owner)) => *owner,
            _ => -1,
        };
        if owner <= 0
            || world
                .handle_of(PersistentId(owner as u64))
                .ok()
                .is_none_or(|owner_handle| !world.has_row(owner_handle, "Terrain"))
        {
            report.push(Diagnostic::warning(
                "scene.terrain_chunk.orphan",
                &target,
                "o bloco não aponta para um terreno existente e não será desenhado",
                "Corrija `owner` ou apague o bloco.",
            ));
        }
        let chunk_x = match world.get(handle, "TerrainChunk", "chunk_x") {
            Ok(Value::Int(value)) => *value,
            _ => 0,
        };
        let chunk_z = match world.get(handle, "TerrainChunk", "chunk_z") {
            Ok(Value::Int(value)) => *value,
            _ => 0,
        };
        if !seen.insert((owner.max(0) as u64, chunk_x, chunk_z)) {
            report.push(Diagnostic::warning(
                "scene.terrain_chunk.duplicate",
                &target,
                "dois blocos do mesmo terreno ocupam a mesma coordenada",
                "Apague um dos blocos duplicados.",
            ));
        }
        let size = match world.get(handle, "TerrainChunk", "size") {
            Ok(Value::Int(value)) => *value,
            _ => 0,
        };
        if !(2..=256).contains(&size) {
            report.push(Diagnostic::error(
                "scene.terrain_chunk.size_invalid",
                &target,
                "o bloco precisa ter entre 2 e 256 células por lado",
                "Corrija `size`.",
            ));
            continue;
        }
        if let Ok(Value::Floats(heights)) = world.get(handle, "TerrainChunk", "heights")
            && heights.len() != (size as usize) * (size as usize)
        {
            report.push(Diagnostic::error(
                "scene.terrain_chunk.heights_size",
                &target,
                &format!(
                    "o bloco tem {} alturas, mas precisa de {}",
                    heights.len(),
                    (size as usize) * (size as usize)
                ),
                "Reconstrua o bloco com a ferramenta de terreno.",
            ));
        }
    }
}

fn check_paths(world: &World, report: &mut Report) {
    for handle in world.entities_with_table("River").unwrap_or_default() {
        let id = world.persistent_id(handle).map_or(0, |id| id.0);
        let target = format!("rio #{id}");
        if let Ok(Value::Spline(spline)) = world.get(handle, "River", "spline") {
            let ascent = maximum_ascent(spline);
            if ascent > 1.0e-4 {
                report.push(Diagnostic::warning(
                    "scene.river.ascends",
                    &target,
                    &format!("o rio sobe {ascent:.3} m em algum trecho"),
                    "Abra o rio e use \"Baixar o leito\", ou ajuste os pontos para descer.",
                ));
            }
        }
        if let Ok(Value::Float(speed)) = world.get(handle, "River", "flow_speed")
            && *speed < 0.0
        {
            report.push(Diagnostic::warning(
                "scene.river.flow_negative",
                &target,
                "a velocidade do fluxo é negativa",
                "Use zero ou um valor positivo.",
            ));
        }
    }
    for handle in world.entities_with_table("Road").unwrap_or_default() {
        let id = world.persistent_id(handle).map_or(0, |id| id.0);
        let target = format!("estrada #{id}");
        if let Ok(Value::Float(width)) = world.get(handle, "Road", "width")
            && *width <= 0.0
        {
            report.push(Diagnostic::error(
                "scene.road.width_invalid",
                &target,
                "a largura da estrada precisa ser maior que zero",
                "Aumente `width` (em metros).",
            ));
        }
        if let Ok(Value::Float(shoulder)) = world.get(handle, "Road", "shoulder")
            && *shoulder < 0.0
        {
            report.push(Diagnostic::warning(
                "scene.road.shoulder_negative",
                &target,
                "o acostamento é negativo",
                "Use zero ou uma fração positiva da meia largura.",
            ));
        }
    }
}

fn check_world(world: &World, report: &mut Report) {
    for handle in world.entities_with_table("Brush").unwrap_or_default() {
        let id = world.persistent_id(handle).map_or(0, |id| id.0);
        let target = format!("entidade #{id}");

        if let Ok(Value::Text(shape)) = world.get(handle, "Brush", "shape")
            && !BRUSH_SHAPES.contains(&shape.as_str())
        {
            report.push(Diagnostic::error(
                "scene.brush.shape_unknown",
                &target,
                &format!("forma `{shape}` desconhecida"),
                &format!("Use uma destas: {}.", BRUSH_SHAPES.join(", ")),
            ));
        }
        if let Ok(Value::Vec3(size)) = world.get(handle, "Brush", "size")
            && (size.x <= 0.0 || size.y <= 0.0 || size.z <= 0.0)
        {
            report.push(Diagnostic::error(
                "scene.brush.size_invalid",
                &target,
                "o tamanho do brush precisa ser maior que zero nos três eixos",
                "Aumente `size` (em metros).",
            ));
        }
        if !world.has_row(handle, "Transform") {
            report.push(Diagnostic::warning(
                "scene.brush.no_transform",
                &target,
                "o brush não tem Transform, então ficará na origem",
                "Adicione a tabela Transform à entidade.",
            ));
        } else if let Ok(Value::Vec3(scale)) = world.get(handle, "Transform", "scale")
            && [scale.x, scale.y, scale.z]
                .iter()
                .any(|axis| axis.abs() < 1.0e-6)
        {
            report.push(Diagnostic::error(
                "scene.transform.scale_zero",
                &target,
                "a escala é zero em algum eixo e o objeto some",
                "Use uma escala diferente de zero.",
            ));
        }
    }

    for handle in world.entities_with_table("Mesh").unwrap_or_default() {
        let id = world.persistent_id(handle).map_or(0, |id| id.0);
        let target = format!("entidade #{id}");
        if let Ok(Value::Mesh(mesh)) = world.get(handle, "Mesh", "geometry")
            && mesh.faces.is_empty()
        {
            report.push(Diagnostic::warning(
                "scene.mesh.empty",
                &target,
                "a malha não tem faces, então não aparece",
                "Apague a entidade ou desfaça a operação que esvaziou a malha.",
            ));
        }
        if !world.has_row(handle, "Transform") {
            report.push(Diagnostic::warning(
                "scene.mesh.no_transform",
                &target,
                "a malha não tem Transform, então ficará na origem",
                "Adicione a tabela Transform à entidade.",
            ));
        }
        if world.has_row(handle, "Brush") {
            report.push(Diagnostic::warning(
                "scene.mesh.also_brush",
                &target,
                "a entidade tem Brush e Mesh; as duas formas são desenhadas uma sobre a outra",
                "Remova a tabela Brush (a malha já guarda a forma).",
            ));
        }
    }

    check_terrain(world, report);
    check_paths(world, report);

    let starts = world.entities_with_table("PlayerStart").unwrap_or_default();
    if starts.is_empty() {
        report.push(Diagnostic::warning(
            "scene.player_start.missing",
            "cena",
            "não há ponto de início do jogador; a câmera começa na origem",
            "Adicione uma entidade com as tabelas Transform e PlayerStart.",
        ));
    } else if starts.len() > 1 {
        for handle in starts {
            let drawn = world.has_row(handle, "Brush")
                || world.has_row(handle, "Mesh")
                || world.has_row(handle, "Sculpt");
            if !drawn {
                continue;
            }
            let id = world.persistent_id(handle).map_or(0, |id| id.0);
            report.push(Diagnostic::warning(
                "scene.player_start.on_drawn_object",
                &format!("entidade #{id}"),
                "há mais de um ponto de início e este está num objeto desenhado (como um piso); o motor prefere um marcador sem forma",
                "Remova PlayerStart deste objeto e deixe-o só no marcador de início.",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create_sample_project;

    #[test]
    fn a_manifest_warning_does_not_block_loading() {
        let root = std::env::temp_dir().join(format!(
            "petunia_project_luau_warning_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        create_sample_project(&root, "Luau pendente").expect("sample");
        let manifest_path = root.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&manifest_path).expect("manifest");
        std::fs::write(&manifest_path, text.replace("\"aipo\"", "\"luau\"")).expect("write");

        let outcome = load_project(&root);
        assert!(outcome.project.is_some(), "{:?}", outcome.report);
        assert!(
            outcome
                .report
                .diagnostics
                .iter()
                .any(|finding| finding.code == "project.manifest.luau_pending"),
            "{:?}",
            outcome.report
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
