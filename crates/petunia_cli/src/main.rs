//! The `petunia` command line.
//!
//! ```text
//! petunia new <folder> [--name <name>]   create a project with a sample room
//! petunia check <folder> [--json]        validate a project (no window)
//! petunia run <folder>                   open the project in a window with a fly camera
//! petunia schema [--json]                list the engine API (commands and queries)
//! ```
//!
//! Exit codes: 0 ok, 1 the check found errors, 2 wrong usage or an unrecoverable problem.

// The command line talks to the terminal; the print lints are for library crates.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use petunia_project::{Report, Severity};

const USAGE: &str = "\
Petunia Engine (fase 1, em construção)

Uso:
  petunia new <pasta> [--name <nome>]   cria um projeto com uma sala de exemplo
  petunia check <pasta> [--json]        valida o projeto, sem abrir janela
  petunia run <pasta>                   abre o projeto numa janela com câmera livre
  petunia schema [--json]               lista a API da engine (comandos e consultas)
  petunia version                       mostra a versão

Códigos de saída: 0 ok · 1 o check achou erros · 2 uso incorreto ou problema grave.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("erro: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("new") => command_new(rest),
        Some("check") => command_check(rest),
        Some("run") => command_run(rest),
        Some("schema") => command_schema(rest),
        Some("version") => {
            println!("petunia {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Some("help" | "--help" | "-h") | None => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        Some(other) => Err(format!(
            "comando desconhecido `{other}`. Rode `petunia help`."
        )),
    }
}

fn folder_argument(args: &[String]) -> Result<PathBuf, String> {
    args.iter()
        .find(|argument| !argument.starts_with("--"))
        .map(PathBuf::from)
        .ok_or_else(|| "informe a pasta do projeto. Exemplo: `petunia check meu_jogo`.".to_string())
}

fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let position = args.iter().position(|argument| argument == name)?;
    args.get(position + 1).map(String::as_str)
}

fn command_new(args: &[String]) -> Result<ExitCode, String> {
    let folder = folder_argument(args)?;
    let name = option_value(args, "--name")
        .map(str::to_string)
        .unwrap_or_else(|| {
            folder.file_name().map_or_else(
                || "Meu jogo".to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        });
    petunia_project::create_sample_project(&folder, &name).map_err(|error| error.to_string())?;
    println!("Projeto \"{name}\" criado em {}", folder.display());
    println!("Próximos passos:");
    println!("  petunia check {}", folder.display());
    println!("  petunia run {}", folder.display());
    Ok(ExitCode::SUCCESS)
}

fn command_check(args: &[String]) -> Result<ExitCode, String> {
    let folder = folder_argument(args)?;
    // The project's own findings, then the scripts' (a broken script is listed, never a reason the
    // project cannot be checked).
    let outcome = petunia_project::load_project(&folder);
    let mut report = outcome.report;
    if let Some(project) = &outcome.project {
        report.extend(petunia_script::project_diagnostics(
            &folder,
            &project.world,
            &project.manifest.programming_method,
        ));
    }
    if args.iter().any(|argument| argument == "--json") {
        let text = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
        println!("{text}");
    } else {
        print_report(&folder, &report);
    }
    Ok(if report.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn print_report(folder: &Path, report: &Report) {
    for diagnostic in &report.diagnostics {
        let label = match diagnostic.severity {
            Severity::Error => "erro",
            Severity::Warning => "aviso",
            Severity::Info => "info",
        };
        println!("[{label}] {} · {}", diagnostic.code, diagnostic.target);
        println!("  O que houve: {}", diagnostic.message);
        println!("  O que fazer: {}", diagnostic.action);
    }
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warning);
    if errors == 0 && warnings == 0 {
        println!("{}: tudo certo.", folder.display());
    } else {
        println!(
            "{}: {errors} erro(s), {warnings} aviso(s).",
            folder.display()
        );
    }
}

#[cfg(feature = "window")]
fn command_run(args: &[String]) -> Result<ExitCode, String> {
    let folder = folder_argument(args)?;
    let outcome = petunia_project::load_project(&folder);
    print_report(&folder, &outcome.report);
    let Some(project) = outcome.project.filter(|_| !outcome.report.has_errors()) else {
        return Ok(ExitCode::from(1));
    };
    println!(
        "Controles: clique na janela para capturar o mouse · WASD/setas movem · Espaço/E sobe · Q/Ctrl desce · Shift acelera · Esc solta o mouse."
    );
    let config = petunia_player::PlayerConfig {
        title: format!("{} — Petunia", project.manifest.name),
        width: 1280,
        height: 720,
        scripts: Some(folder.join(petunia_script::SCRIPTS_DIRECTORY)),
        programming_method: project.manifest.programming_method.clone(),
        project_root: Some(folder),
    };
    petunia_player::run(project.world, config).map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(feature = "window"))]
fn command_run(_args: &[String]) -> Result<ExitCode, String> {
    Err("esta build não inclui janela. Recompile com a feature `window` (o padrão).".to_string())
}

fn command_schema(args: &[String]) -> Result<ExitCode, String> {
    let schema = petunia_commands::builtin_schema();
    schema.validate().map_err(|error| error.to_string())?;
    if args.iter().any(|argument| argument == "--json") {
        let text = schema.to_json_pretty().map_err(|error| error.to_string())?;
        println!("{text}");
        return Ok(ExitCode::SUCCESS);
    }
    println!("API da engine (schema v{})", schema.version);
    println!("\nComandos (mudam algo, com desfazer):");
    for command in &schema.commands {
        println!("  {:<14} {}", command.id, command.summary);
    }
    println!("\nConsultas (só leem):");
    for query in &schema.queries {
        println!("  {:<14} {}", query.id, query.summary);
    }
    println!("\nUse `petunia schema --json` para o documento completo.");
    Ok(ExitCode::SUCCESS)
}
