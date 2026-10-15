//! `figspec inspect|render|storyboard <model.json>` — the same operations the
//! Python bindings expose, with the family's exit policy:
//!
//! * 0 — inspected, no `error`
//! * 1 — inspected, at least one `error`
//! * 2 — the input could not be read (missing, malformed, or an unknown field)

use std::path::PathBuf;
use std::process::ExitCode;

use figspec::{inspect, to_png, to_svg, Figure};

const USAGE: &str = "\
figspec — a figure as data

    figspec inspect    <model.json> [--json]
    figspec render     <model.json> -o <out.svg|out.png>
    figspec storyboard <model.json> -o <out.svg>

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
        "storyboard" => storyboard_command(&args[1..]),
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
        print_report(path, &figure, &report);
    }
    Ok(report.exit_code() as u8)
}

/// The report, as a person reads it: the findings numbered, and each one's mark
/// when it has one.
fn print_report(path: &str, figure: &Figure, report: &figspec::Report) {
    println!("{path}: {} marks", figure.marks.len());
    if report.problems.is_empty() {
        println!("inspect: no problems");
        return;
    }
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

/// The report, with a drawn figure whose boxes carry the report's numbers.
fn storyboard_command(args: &[String]) -> Result<u8, String> {
    let path = args
        .iter()
        .find(|arg| !arg.starts_with('-') && !arg.ends_with(".svg"))
        .ok_or("storyboard needs a model file")?;
    let output = args
        .iter()
        .position(|arg| arg == "-o")
        .and_then(|index| args.get(index + 1))
        .map(PathBuf::from)
        .ok_or("storyboard needs -o <out.svg>")?;
    let figure = Figure::from_path(path).map_err(|error| error.to_string())?;
    let report = inspect(&figure);
    let drawn = figspec::storyboard::of(&figure, &report).map_err(|error| error.message)?;
    std::fs::write(&output, &drawn.svg)
        .map_err(|error| format!("{}: {error}", output.display()))?;
    print_report(path, &figure, &report);
    if drawn.boxes.is_empty() {
        println!("storyboard: no finding names a mark, so no box was drawn");
    } else {
        println!(
            "storyboard: {} box(es) in {}",
            drawn.boxes.len(),
            output.display()
        );
        for boxed in &drawn.boxes {
            let numbers: Vec<String> = boxed.numbers.iter().map(usize::to_string).collect();
            println!("  [{}] mark {}", numbers.join(","), boxed.mark);
        }
    }
    if !drawn.unlocated.is_empty() {
        let numbers: Vec<String> = drawn.unlocated.iter().map(usize::to_string).collect();
        println!(
            "  finding(s) {} are about no mark and are not boxed",
            numbers.join(",")
        );
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
