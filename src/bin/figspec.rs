//! `figspec inspect|render|storyboard|codes <model.json>` — the same operations
//! the Python bindings expose, with the family's exit policy:
//!
//! * 0 — inspected, no `error`
//! * 1 — inspected, at least one `error`
//! * 2 — the input could not be read (missing, malformed, or an unknown field),
//!   the command line could not be honoured, or the figure cannot be drawn as
//!   written

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use figspec::{inspect, to_png, to_svg, Figure};

const USAGE: &str = "\
figspec — a figure as data

    figspec inspect    <model.json> [--json]
    figspec render     <model.json> -o <out.svg|out.png>
    figspec storyboard <model.json> -o <out.svg>
    figspec codes

The model is JSON: {\"marks\": [...], \"title\": ..., \"x_scale\": \"log\", ...}.
Exit: 0 no error, 1 at least one error, 2 the model could not be read, the command
line could not be honoured, or the figure cannot be drawn as written.";

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
        // Usage for a command line that was not given is an error, and errors are
        // written where errors go.
        eprintln!("{USAGE}");
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

/// The model file and the flags every command shares.
///
/// One grammar, walked once. `-o`'s value used to be told apart from the model by
/// its *extension* (`find(|arg| !arg.ends_with(".svg"))`), so a model named
/// `chart.svg` was invisible to `render` and `storyboard` ("render needs a model
/// file", for a file that was there) while `inspect` read it — and a second model
/// file, or an unknown flag, was **silently ignored**, so the answer was about
/// something other than what the command was given.
struct Flags {
    model: Option<String>,
    output: Option<PathBuf>,
    as_json: bool,
}

fn scan(command: &str, args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags {
        model: None,
        output: None,
        as_json: false,
    };
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "-o" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{command} needs a value after -o"))?;
                flags.output = Some(PathBuf::from(value));
                index += 2;
            }
            "--json" => {
                flags.as_json = true;
                index += 1;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other:?}; try `figspec --help`"));
            }
            other => {
                if flags.model.is_some() {
                    return Err(format!(
                        "{command} takes one model file, and {other:?} is a second"
                    ));
                }
                flags.model = Some(other.to_string());
                index += 1;
            }
        }
    }
    Ok(flags)
}

/// The model and the `--json` choice `inspect` can honour.
fn report_flags(command: &str, args: &[String]) -> Result<(String, bool), String> {
    let flags = scan(command, args)?;
    if flags.output.is_some() {
        return Err(format!(
            "{command} reports and writes no file, so -o is not an option here"
        ));
    }
    let model = flags
        .model
        .ok_or_else(|| format!("{command} needs a model file"))?;
    Ok((model, flags.as_json))
}

/// The model and the output path `render`/`storyboard` require.
fn writer_flags(command: &str, args: &[String], usage: &str) -> Result<(String, PathBuf), String> {
    let flags = scan(command, args)?;
    if flags.as_json {
        return Err(format!(
            "{command} prints its findings as a report; --json is for `inspect`"
        ));
    }
    let output = flags
        .output
        .ok_or_else(|| format!("{command} needs -o <{usage}>"))?;
    let model = flags
        .model
        .ok_or_else(|| format!("{command} needs a model file"))?;
    Ok((model, output))
}

/// The extension a path claims, lowercased, or `""`.
fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn inspect_command(args: &[String]) -> Result<u8, String> {
    let (model, as_json) = report_flags("inspect", args)?;
    let figure = Figure::from_path(&model).map_err(|error| error.to_string())?;
    let report = inspect(&figure);
    if as_json {
        println!("{}", report.to_json());
    } else {
        print_report(&model, &figure, &report);
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
    let (model, output) = writer_flags("storyboard", args, "out.svg")?;
    // A storyboard is an SVG and only an SVG. It used to be written to whatever
    // path was given, so `-o out.png` produced a file named `.png` whose contents
    // are SVG — a picture that lies about itself to every consumer of it.
    let extension = extension_of(&output);
    if !matches!(extension.as_str(), "" | "svg") {
        return Err(format!("storyboard writes .svg, not {extension:?}"));
    }
    let figure = Figure::from_path(&model).map_err(|error| error.to_string())?;
    let report = inspect(&figure);
    let drawn = figspec::storyboard::of(&figure, &report).map_err(|error| error.message)?;
    std::fs::write(&output, &drawn.svg)
        .map_err(|error| format!("{}: {error}", output.display()))?;
    print_report(&model, &figure, &report);
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
    let (model, output) = writer_flags("render", args, "out.svg|out.png")?;
    let figure = Figure::from_path(&model).map_err(|error| error.to_string())?;
    let extension = extension_of(&output);
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
