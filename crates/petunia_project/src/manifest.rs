use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;
use crate::io::resolve_inside;

pub const MANIFEST_FILE: &str = "petunia.project.json";
pub const FORMAT_VERSION: u32 = 1;

fn default_language() -> String {
    "aipo".to_string()
}

/// The programming methods a project may pick (DR-40). One per project, start to finish.
pub const PROGRAMMING_METHODS: [&str; 3] = ["aipo", "luau", "nodes"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    pub format_version: u32,
    pub name: String,
    pub default_scene: String,
    /// How this project is programmed, one method per project: `aipo` (default), `luau` or `nodes`
    /// (pNodes, which compiles to Aipo). Reading a file written before DR-40 accepts the old
    /// `scripting_language` key through the alias, so no project stops opening.
    #[serde(alias = "scripting_language", default = "default_language")]
    pub programming_method: String,
}

impl ProjectManifest {
    pub fn new(name: &str, default_scene: &str) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.to_string(),
            default_scene: default_scene.to_string(),
            programming_method: default_language(),
        }
    }

    /// Whether this project programs with the node sheet (pNodes).
    pub fn uses_node_method(&self) -> bool {
        self.programming_method == "nodes"
    }

    /// The scripting language the project runs, which is Aipo under pNodes: the node sheet is an
    /// authoring view, not a fourth runtime (DR-40).
    pub fn scripting_language(&self) -> &str {
        if self.uses_node_method() {
            "aipo"
        } else {
            &self.programming_method
        }
    }

    /// Everything wrong with the manifest, as diagnostics (empty when valid).
    pub fn validate(&self) -> Vec<Diagnostic> {
        let target = MANIFEST_FILE;
        let mut found = Vec::new();
        if self.format_version != FORMAT_VERSION {
            found.push(Diagnostic::error(
                "project.manifest.version",
                target,
                &format!(
                    "format_version {} não é suportada (esta versão lê {FORMAT_VERSION})",
                    self.format_version
                ),
                "Atualize a engine ou migre o projeto.",
            ));
        }
        let name_length = self.name.chars().count();
        if name_length == 0 || name_length > 80 || self.name.chars().any(char::is_control) {
            found.push(Diagnostic::error(
                "project.manifest.name_invalid",
                target,
                "o nome do projeto precisa ter de 1 a 80 caracteres, sem caracteres de controle",
                "Edite o campo `name` do manifesto.",
            ));
        }
        if resolve_inside(Path::new("."), &self.default_scene).is_err()
            || !self.default_scene.ends_with(".scene.json")
        {
            found.push(Diagnostic::error(
                "project.manifest.scene_path_invalid",
                target,
                "`default_scene` precisa ser um caminho relativo dentro do projeto que termina em `.scene.json`",
                "Exemplo: \"scenes/room.scene.json\".",
            ));
        }
        if !PROGRAMMING_METHODS.contains(&self.programming_method.as_str()) {
            found.push(Diagnostic::error(
                "project.manifest.language_unknown",
                target,
                &format!("método de programação `{}` desconhecido", self.programming_method),
                "Use \"aipo\" (padrão), \"luau\" ou \"nodes\" (pNodes). Só um método fica ativo por projeto.",
            ));
        } else if self.programming_method == "luau" {
            found.push(Diagnostic::warning(
                "project.manifest.luau_pending",
                target,
                "a linguagem `luau` está planejada, mas o backend ainda não existe; só Aipo executa nesta versão",
                "Use \"aipo\" até o Luau entrar. O projeto abre e continua válido.",
            ));
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Severity;

    #[test]
    fn luau_is_known_but_warns_until_the_backend_exists() {
        let mut manifest = ProjectManifest::new("Teste", "scenes/room.scene.json");
        manifest.programming_method = "luau".to_string();
        let findings = manifest.validate();
        assert!(
            findings
                .iter()
                .any(|finding| finding.severity == Severity::Warning
                    && finding.code == "project.manifest.luau_pending"),
            "{findings:?}"
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding.severity == Severity::Error),
            "a planned language must not make the project invalid: {findings:?}"
        );
    }

    #[test]
    fn the_node_method_is_valid_and_runs_aipo() {
        let mut manifest = ProjectManifest::new("Teste", "scenes/room.scene.json");
        manifest.programming_method = "nodes".to_string();
        let findings = manifest.validate();
        assert!(
            !findings
                .iter()
                .any(|finding| finding.severity == Severity::Error),
            "pNodes is a supported method: {findings:?}"
        );
        assert!(manifest.uses_node_method());
        assert_eq!(
            manifest.scripting_language(),
            "aipo",
            "the node sheet is an authoring view over Aipo, not a fourth runtime"
        );
    }

    #[test]
    fn an_unknown_method_is_refused() {
        let mut manifest = ProjectManifest::new("Teste", "scenes/room.scene.json");
        manifest.programming_method = "javascript".to_string();
        let findings = manifest.validate();
        assert!(
            findings
                .iter()
                .any(|finding| finding.severity == Severity::Error
                    && finding.code == "project.manifest.language_unknown"),
            "{findings:?}"
        );
    }

    #[test]
    fn an_old_manifest_written_with_scripting_language_still_opens() {
        // Files saved before DR-40 carry the old key; they must load as the method it named.
        let old = r#"{
            "format_version": 1,
            "name": "Antigo",
            "default_scene": "scenes/room.scene.json",
            "scripting_language": "nodes"
        }"#;
        let manifest: ProjectManifest = serde_json::from_str(old).expect("old manifest parses");
        assert_eq!(manifest.programming_method, "nodes");
    }

    #[test]
    fn a_manifest_without_a_method_defaults_to_aipo() {
        let old = r#"{
            "format_version": 1,
            "name": "Antigo",
            "default_scene": "scenes/room.scene.json"
        }"#;
        let manifest: ProjectManifest = serde_json::from_str(old).expect("manifest parses");
        assert_eq!(manifest.programming_method, "aipo");
    }
}
