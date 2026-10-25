use std::path::{Path, PathBuf};

use petunia_world::{Value, World};

use crate::diagnostics::{Diagnostic, Report};
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
    let manifest_invalid = !manifest_problems.is_empty();
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

    let has_start = world
        .entities_with_table("PlayerStart")
        .is_ok_and(|found| !found.is_empty());
    if !has_start {
        report.push(Diagnostic::warning(
            "scene.player_start.missing",
            "cena",
            "não há ponto de início do jogador; a câmera começa na origem",
            "Adicione uma entidade com as tabelas Transform e PlayerStart.",
        ));
    }
}
