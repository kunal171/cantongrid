//! CLI entrypoints for fixtures, source checks, model operations and authorized live indexing/APIs.
#![forbid(unsafe_code)]
mod commands;

use cantongrid_core::{config::Plan, fixture::Fixture};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    process::ExitCode,
};

const HELP: &str = "CantonGrid — local ingestion and storage reference

Usage:
  cantongrid ops prune-deliveries <runtime.json> --retain <1..100000> --limit <1..256>
  cantongrid inspect evaluate <models.json> <model-name> <payload.json>
  cantongrid inspect progress <runtime.json>
  cantongrid sync plan <old.dar> <new.dar> <models.json>
  cantongrid sync summary <old.dar> <new.dar> <models.json>
  cantongrid sync evaluate <old.dar> <new.dar> <models.json> <fixtures.json>
  cantongrid sync bundle <old.dar> <new.dar> <models.json> <output-directory>
  cantongrid metadata inspect <artifact.dar>
  cantongrid upgrade plan <old-runtime> <new-runtime> <dar> <models> <namespace>
  cantongrid upgrade start|step|status|activate|cancel <manifest> <old-runtime> <new-runtime> <dar> <models>
  cantongrid upgrade apply <manifest> <old-runtime> <new-runtime> <dar> <models>
  cantongrid index <runtime.json> [--once]
  cantongrid serve <runtime.json>
  cantongrid models typescript <models.json>
  cantongrid models graphql <models.json>
  cantongrid models validate <models.json>
  cantongrid models plan <models.json>
  cantongrid models preview <models.json> <model-name> <payload.json>
  cantongrid models register <models.json> --namespace <project> --database-env <CANTONGRID_VARIABLE>
  cantongrid models inspect <project> <model-name> <revision> --database-env <CANTONGRID_VARIABLE>
  cantongrid models rebuild start <runtime.json> <job> <project> <model-name> <revision> [--replace <previous-revision>]
  cantongrid models rebuild step <runtime.json> <job> [--limit <1..64>]
  cantongrid models rebuild status|activate|cancel <runtime.json> <job>
  cantongrid models bind <runtime.json> <project> <model-name> <revision> [--replace <previous-revision>]
  cantongrid models install <project> <model-name> <revision> --database-env <CANTONGRID_VARIABLE>
  cantongrid init <new-directory>
  cantongrid validate <project.json>
  cantongrid fixture run <project.json> [--party <synthetic-party>]...
  cantongrid storage init --database-env <CANTONGRID_VARIABLE>
  cantongrid fixture index <project.json> --database-env <CANTONGRID_VARIABLE>
  cantongrid fixture status <project.json> --database-env <CANTONGRID_VARIABLE>
  cantongrid fixture query <project.json> --database-env <CANTONGRID_VARIABLE> [--party <synthetic-party>]...
  cantongrid source inspect-pqs --database-env <CANTONGRID_VARIABLE>
  cantongrid source verify-grants <source-grants.json>
  cantongrid source verify-window --database-env <CANTONGRID_VARIABLE> --after <offset>

Fixture run uses process memory; index/status/query use an explicitly initialized local PostgreSQL destination.
Fixture caller grants are simulated. Source inspection is read-only metadata, not live ingestion.
No fixture parties means no visible records. Live index/serve require explicit runtime policy and credentials; dev remains planned.";
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;

/// Emit a redacted operator result or failure and choose the corresponding process exit status.
fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("CantonGrid: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Accept exact command shapes before dispatching fixture, source or operator actions.
fn run(args: Vec<String>) -> Result<(), String> {
    match args.as_slice() {
        [command, action, rest @ ..] if command == "ops" => commands::operations::run(action, rest),
        [command, action, rest @ ..] if command == "inspect" => {
            commands::inspector::run(action, rest)
        }
        [command, action, rest @ ..] if command == "upgrade" => {
            commands::selection_upgrade::run(action, rest)
        }
        [command, action, rest @ ..] if command == "sync" => commands::sync::run(action, rest),
        [command, action, path] if command == "metadata" && action == "inspect" => {
            let metadata =
                cantongrid_metadata::inspect_file(Path::new(path)).map_err(|e| e.to_string())?;
            println!("{}", metadata.to_json().map_err(|e| e.to_string())?);
            Ok(())
        }
        [command, subject, action, rest @ ..] if command == "models" && subject == "rebuild" => {
            commands::model_rebuild::run(action, rest)
        }
        [] => {
            println!("{HELP}");
            Ok(())
        }
        [command, action, path, namespace, name, revision, rest @ ..]
            if command == "models" && action == "bind" =>
        {
            commands::model_binding::bind(path, namespace, name, revision, rest)
        }
        [flag] if flag == "--version" || flag == "-V" => {
            println!("cantongrid {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{HELP}");
            Ok(())
        }
        [
            command,
            action,
            path,
            namespace_flag,
            namespace,
            database_flag,
            variable,
        ] if command == "models"
            && action == "register"
            && namespace_flag == "--namespace"
            && database_flag == "--database-env" =>
        {
            commands::model_registry::register(Path::new(path), namespace, variable)
        }
        [
            command,
            action,
            namespace,
            name,
            revision,
            database_flag,
            variable,
        ] if command == "models" && action == "inspect" && database_flag == "--database-env" => {
            commands::model_registry::inspect(namespace, name, revision, variable)
        }
        [
            command,
            action,
            namespace,
            name,
            revision,
            database_flag,
            variable,
        ] if command == "models" && action == "install" && database_flag == "--database-env" => {
            commands::model_installation::install(namespace, name, revision, variable)
        }
        [command, action, path, rest @ ..] if command == "models" => {
            commands::models::run(action, Path::new(path), rest)
        }
        [command, path, rest @ ..] if command == "index" || command == "serve" => {
            let once = rest == ["--once"];
            if !rest.is_empty() && !(command == "index" && once) {
                return Err("unsupported runtime arguments".into());
            }
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| "cannot initialize runtime")?;
            if command == "index" {
                runtime.block_on(cantongrid_runtime::index(Path::new(path), once))
            } else {
                runtime.block_on(cantongrid_runtime::serve(Path::new(path)))
            }
        }
        [command, directory] if command == "init" => init(Path::new(directory)),
        [command, path] if command == "validate" => {
            let plan = load_plan(Path::new(path))?;
            println!(
                "Valid fixture project: format/model/payload version 1; create/archive; complete history"
            );
            // Validation intentionally does not open the fixture or acquire credentials.
            let _ = plan;
            Ok(())
        }
        [command, action, flag, variable]
            if command == "storage" && action == "init" && flag == "--database-env" =>
        {
            commands::storage::initialize(variable)
        }
        [command, action, path] if command == "source" && action == "verify-grants" => {
            commands::source::verify_grants(path)
        }
        [command, action, flag, variable, after_flag, after]
            if command == "source"
                && action == "verify-window"
                && flag == "--database-env"
                && after_flag == "--after" =>
        {
            let after = after
                .parse::<i64>()
                .map_err(|_| "source boundary must be a nonnegative i64")?;
            let report =
                cantongrid_pqs::reconcile_env(variable, after, None).map_err(|e| e.to_string())?;
            print_json(
                &serde_json::json!({"operation":"retained_window_reconciliation","result":report}),
            )
        }
        [command, action, flag, variable]
            if command == "source" && action == "inspect-pqs" && flag == "--database-env" =>
        {
            commands::source::inspect(variable)
        }
        [command, action, path, flag, variable, rest @ ..]
            if command == "fixture"
                && ["index", "status", "query"].contains(&action.as_str())
                && flag == "--database-env" =>
        {
            commands::storage::fixture(action, Path::new(path), variable, rest)
        }
        [command, action, path, rest @ ..] if command == "fixture" && action == "run" => {
            let mut parties = vec![];
            let mut flags = rest.iter();
            while let Some(flag) = flags.next() {
                if flag != "--party" {
                    return Err("expected --party <synthetic-party>".into());
                }
                parties.push(flags.next().ok_or("missing fixture party")?.clone());
            }
            let path = Path::new(path);
            let plan = load_plan(path)?;
            let fixture_path = path
                .parent()
                .unwrap_or(Path::new("."))
                .join(plan.fixture_path());
            let fixture =
                Fixture::parse(&read_document(&fixture_path)?, &plan).map_err(|e| e.to_string())?;
            let summary = fixture.run(&plan, parties).map_err(|e| e.to_string())?;
            let output = serde_json::json!({"mode":"fixture","coverage":"complete_from_origin",
                "storage":"memory","authentication":"simulated_fixture_grants","result":summary});
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .map_err(|_| "cannot encode fixture summary")?
            );
            Ok(())
        }
        _ => Err("unsupported command or arguments; run cantongrid --help".into()),
    }
}

/// Load only the fixture schema; live worker/API commands use runtime configuration instead.
fn load_plan(path: &Path) -> Result<Plan, String> {
    Plan::parse(&read_document(path)?).map_err(|error| error.to_string())
}

/// Bound a regular input file before parsing and omit its path/content from failures.
fn read_document(path: &Path) -> Result<String, String> {
    // Reject directories/devices/FIFOs before opening (a FIFO open could otherwise block).
    if !fs::metadata(path)
        .map_err(|_| "cannot inspect input document")?
        .is_file()
    {
        return Err("input document must be a regular file".into());
    }
    let file = File::open(path).map_err(|_| "cannot open input document")?;
    if !file
        .metadata()
        .map_err(|_| "cannot inspect input document")?
        .is_file()
    {
        return Err("input document must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read input document")?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("input document exceeds 8 MiB limit".into());
    }
    String::from_utf8(bytes).map_err(|_| "input document must be UTF-8".into())
}

/// Scaffold a new fixture directory only; refuse overwriting existing local work.
fn init(directory: &Path) -> Result<(), String> {
    fs::create_dir(directory).map_err(
        |_| "init requires a new directory under an existing parent; no files were overwritten",
    )?;
    // Only synthetic, credential-free fixture content is scaffolded.
    for (name, content) in [
        (
            "cantongrid.json",
            include_str!("../assets/fixtures/visibility/cantongrid.json"),
        ),
        (
            "events.json",
            include_str!("../assets/fixtures/visibility/events.json"),
        ),
    ] {
        let result = File::create_new(directory.join(name)).and_then(|mut file| {
            use std::io::Write;
            file.write_all(content.as_bytes())
        });
        if result.is_err() {
            return Err("cannot finish fixture scaffold; inspect the newly created directory before retrying".into());
        }
    }
    println!(
        "Created synthetic fixture project; validate cantongrid.json, then fixture run with --party A or --party B"
    );
    Ok(())
}

/// Serialize bounded operator/fixture results without ever printing connection settings or payloads.
fn print_json(value: &serde_json::Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|_| "cannot encode command result")?
    );
    Ok(())
}
