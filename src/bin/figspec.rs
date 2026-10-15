//! `figspec inspect|render <model.json>` — the same two operations the Python
//! bindings expose, with the family's exit policy:
//!
//! * 0 — inspected, no `error`
//! * 1 — inspected, at least one `error`
//! * 2 — the input could not be read (missing, malformed, or an unknown field)

use std::path::PathBuf;
use std::process::ExitCode;

use figspec::{inspect, to_png, to_svg, Figure};

const USAGE: &str = "\
figspec — a figure as data

    figspec inspect <model.json> [--json]
    figspec render  <model.json> -o <out.svg|out.png>

The model is JSON: {\"marks\": [...], \"title\": ..., \"x_scale\": \"log\", ...}.
Exit: 0 no error, 1 at least one error, 2 the model could not be read.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("figspec: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<u8, String> {
    let Some(command) = args.first().map(String::as_str) else {
        println!("{USAGE}");
        return Ok(2);
    };
    match command {
        "inspect" => inspect_command(&args[1..]),
        "render" => render_command(&args[1..]),
        "codes" => {
            for (code, severity, meaning) in figspec::CODEBOOK {
                println!("{code:<24} {severity:<6} {meaning}");
            }
            Ok(0)
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(0)
        }
        other => Err(format!("unknown command {other:?}; try `figspec --help`")),
    }
}

fn inspect_command(args: &[String]) -> Result<u8, String> {
    let as_json = args.iter().any(|arg| arg == "--json");
    let path = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .ok_or("inspect needs a model file")?;
    let figure = Figure::from_path(path).map_err(|error| error.to_string())?;
    let report = inspect(&figure);
    if as_json {
        println!("{}", report.to_json());
    } else {
        println!("{path}: {} marks", figure.marks.len());
        if report.problems.is_empty() {
            println!("inspect: no problems");
        } else {
            println!(
                "inspect: {} problem{} ({} error{})",
                report.problems.len(),
                if report.problems.len() == 1 { "" } else { "s" },
                report.errors(),
                if report.errors() == 1 { "" } else { "s" }
            );
            for (index, problem) in report.problems.iter().enumerate() {
                let location = match problem.mark {
                    Some(mark) => format!("m[{mark}]"),
                    None => "-".to_string(),
                };
                println!(
                    "  [{}] {:<5} {:<8} {}: {}",
                    index + 1,
                    problem.severity,
                    location,
                    problem.code,
                    problem.message
                );
            }
        }
    }
    Ok(report.exit_code() as u8)
}

fn render_command(args: &[String]) -> Result<u8, String> {
    let path = args
        .iter()
        .find(|arg| !arg.starts_with('-') && !arg.ends_with(".svg") && !arg.ends_with(".png"))
        .ok_or("render needs a model file")?;
    let output = args
        .iter()
        .position(|arg| arg == "-o")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from)
        .ok_or("render needs -o <out.svg|out.png>")?;
    let figure = Figure::from_path(path).map_err(|error| error.to_string())?;
    let extension = output
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "svg" => {
            let svg = to_svg(&figure).map_err(|error| error.message)?;
            std::fs::write(&output, svg)
                .map_err(|error| format!("{}: {error}", output.display()))?;
        }
        "png" => {
            let png = to_png(&figure).map_err(|error| error.message)?;
            std::fs::write(&output, png)
                .map_err(|error| format!("{}: {error}", output.display()))?;
        }
        other => return Err(format!("render writes .svg or .png, not {other:?}")),
    }
    println!("{}", output.display());
    Ok(0)
}
