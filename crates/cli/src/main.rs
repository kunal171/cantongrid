//! Local, credential-free fixture CLI. Live ingestion and API commands are not implemented.
#![forbid(unsafe_code)]
use cantongrid_core::{config::Plan, fixture::Fixture};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    process::ExitCode,
};

const HELP: &str = "CantonGrid — Rust fixture milestone

Usage:
  cantongrid init <new-directory>
  cantongrid validate <project.json>
  cantongrid fixture run <project.json> [--party <synthetic-party>]...

Fixture commands use process memory and simulated caller grants, with no network access.
No parties means no visible records. index, serve and dev remain planned.";
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("CantonGrid: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.as_slice() {
        [] => {
            println!("{HELP}");
            Ok(())
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{HELP}");
            Ok(())
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

fn load_plan(path: &Path) -> Result<Plan, String> {
    Plan::parse(&read_document(path)?).map_err(|error| error.to_string())
}

fn read_document(path: &Path) -> Result<String, String> {
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

fn init(directory: &Path) -> Result<(), String> {
    fs::create_dir(directory).map_err(
        |_| "init requires a new directory under an existing parent; no files were overwritten",
    )?;
    // Only synthetic, credential-free fixture content is scaffolded.
    for (name, content) in [
        (
            "cantongrid.json",
            include_str!("../../../examples/fixtures/visibility/cantongrid.json"),
        ),
        (
            "events.json",
            include_str!("../../../examples/fixtures/visibility/events.json"),
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
