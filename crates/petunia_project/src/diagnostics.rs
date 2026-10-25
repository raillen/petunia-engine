use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// One finding: severity, stable code, where, what is wrong and what to do about it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub target: String,
    pub message: String,
    pub action: String,
}

impl Diagnostic {
    pub fn error(code: &str, target: &str, message: &str, action: &str) -> Self {
        Self::new(Severity::Error, code, target, message, action)
    }

    pub fn warning(code: &str, target: &str, message: &str, action: &str) -> Self {
        Self::new(Severity::Warning, code, target, message, action)
    }

    fn new(severity: Severity, code: &str, target: &str, message: &str, action: &str) -> Self {
        Self {
            severity,
            code: code.to_string(),
            target: target.to_string(),
            message: message.to_string(),
            action: action.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        self.diagnostics.extend(diagnostics);
    }

    pub fn has_errors(&self) -> bool {
        self.count(Severity::Error) > 0
    }

    pub fn count(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == severity)
            .count()
    }
}
