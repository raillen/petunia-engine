use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;
use crate::io::resolve_inside;

pub const MANIFEST_FILE: &str = "petunia.project.json";
pub const FORMAT_VERSION: u32 = 1;

fn default_language() -> String {
    "aipo".to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    pub format_version: u32,
    pub name: String,
    pub default_scene: String,
    /// One active scripting language per project: `aipo` (default) or `luau`.
    #[serde(default = "default_language")]
    pub scripting_language: String,
}

impl ProjectManifest {
    pub fn new(name: &str, default_scene: &str) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.to_string(),
            default_scene: default_scene.to_string(),
            scripting_language: default_language(),
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
        if !matches!(self.scripting_language.as_str(), "aipo" | "luau") {
            found.push(Diagnostic::error(
                "project.manifest.language_unknown",
                target,
                &format!("linguagem `{}` desconhecida", self.scripting_language),
                "Use \"aipo\" (padrão) ou \"luau\". Só uma linguagem fica ativa por projeto.",
            ));
        } else if self.scripting_language == "luau" {
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
        manifest.scripting_language = "luau".to_string();
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
}
