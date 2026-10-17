//! The command line's grammar: what it reads, and what it refuses by name.
//!
//! Every case here was silent before:
//!
//! * `inspect a.json b.json` inspected `a.json` and never mentioned `b.json`;
//! * `inspect m.json --oops` — a typo'd `--json` — printed the human report, exit 0;
//! * `storyboard m.json -o out.png` wrote **SVG** to a path ending in `.png`, so the
//!   file lied about itself to every consumer of it;
//! * `render chart.svg -o out.svg` said "render needs a model file" about a file
//!   that was there, because the model and `-o`'s value were told apart by their
//!   *extensions*.
//!
//! A command that answers about something other than what it was given is worse
//! than one that fails, so each of these is now a refusal naming the argument.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use figspec::{Figure, Mark};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("figspec-cli-tests").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn model(path: &Path) {
    let figure = Figure::new()
        .with_title("a figure")
        .with_mark(Mark::Scatter {
            points: vec![[0.0, 1.0], [1.0, 2.0]],
            y_err: None,
            y_err_asymmetric: None,
            size: None,
            color: None,
            label: None,
        });
    figure.save(path).unwrap();
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_figspec"))
        .args(args)
        .output()
        .expect("the binary runs")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("the binary exits normally")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

#[test]
fn a_second_model_file_is_refused_rather_than_ignored() {
    let dir = scratch("second-model");
    model(&dir.join("a.json"));
    model(&dir.join("b.json"));
    let output = run(&[
        "inspect",
        dir.join("a.json").to_str().unwrap(),
        dir.join("b.json").to_str().unwrap(),
    ]);
    assert_eq!(code(&output), 2, "{}", stderr(&output));
    assert!(
        stderr(&output).contains("one model file"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_unknown_option_is_refused_rather_than_ignored() {
    let dir = scratch("unknown-option");
    let path = dir.join("a.json");
    model(&path);
    let output = run(&["inspect", path.to_str().unwrap(), "--jsonn"]);
    assert_eq!(code(&output), 2, "{}", stderr(&output));
    assert!(
        stderr(&output).contains("unknown option"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_flag_the_command_cannot_honour_is_refused_in_both_directions() {
    // `inspect` writes no file, and `render` prints no JSON report.
    let dir = scratch("flag-shape");
    let path = dir.join("a.json");
    model(&path);
    let inspect = run(&["inspect", path.to_str().unwrap(), "-o", "out.svg"]);
    assert_eq!(code(&inspect), 2, "{}", stderr(&inspect));
    assert!(
        stderr(&inspect).contains("-o is not an option"),
        "{}",
        stderr(&inspect)
    );
    let render = run(&["render", path.to_str().unwrap(), "--json"]);
    assert_eq!(code(&render), 2, "{}", stderr(&render));
    assert!(
        stderr(&render).contains("--json is for"),
        "{}",
        stderr(&render)
    );
}

#[test]
fn a_storyboard_refuses_a_path_that_would_lie_about_what_it_holds() {
    let dir = scratch("storyboard-extension");
    let path = dir.join("a.json");
    let out = dir.join("out.png");
    model(&path);
    // The mutation run writes this path too (its scratch tree shares the temporary
    // directory), so a leftover from someone else's run must not decide this test:
    // what is pinned is that *this* run did not write it.
    let _ = std::fs::remove_file(&out);
    let output = run(&[
        "storyboard",
        path.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code(&output), 2, "{}", stderr(&output));
    assert!(
        stderr(&output).contains("storyboard writes .svg"),
        "{}",
        stderr(&output)
    );
    assert!(!out.exists(), "the refused path was written anyway");
}

#[test]
fn a_model_file_whose_name_looks_like_output_is_still_a_model() {
    // The old grammar skipped arguments ending in `.svg`/`.png` to avoid mistaking
    // `-o`'s value for the model, which made a model named `chart.svg` invisible.
    let dir = scratch("model-named-svg");
    let path = dir.join("chart.svg");
    let out = dir.join("drawn.svg");
    model(&path);
    for command in ["render", "storyboard"] {
        let output = run(&[command, path.to_str().unwrap(), "-o", out.to_str().unwrap()]);
        assert_eq!(code(&output), 0, "{command}: {}", stderr(&output));
        assert!(out.exists(), "{command} wrote nothing");
    }
    let inspect = run(&["inspect", path.to_str().unwrap()]);
    assert_eq!(code(&inspect), 0, "{}", stderr(&inspect));
    let out = dir.join("drawn.png");
    let render = run(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code(&render), 0, "{}", stderr(&render));
}

#[test]
fn the_output_option_may_stand_before_the_model() {
    let dir = scratch("option-first");
    let path = dir.join("a.json");
    let out = dir.join("drawn.svg");
    model(&path);
    let output = run(&[
        "render",
        "-o",
        out.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    assert_eq!(code(&output), 0, "{}", stderr(&output));
    assert!(out.exists());
}

#[test]
fn a_command_line_that_cannot_be_honoured_exits_two() {
    let dir = scratch("usage");
    let path = dir.join("a.json");
    model(&path);
    let missing_model = run(&["inspect"]);
    assert_eq!(code(&missing_model), 2);
    assert!(
        stderr(&missing_model).contains("needs a model file"),
        "{}",
        stderr(&missing_model)
    );
    let missing_output = run(&["render", path.to_str().unwrap()]);
    assert_eq!(code(&missing_output), 2);
    assert!(
        stderr(&missing_output).contains("needs -o"),
        "{}",
        stderr(&missing_output)
    );
    let extension = run(&["render", path.to_str().unwrap(), "-o", "drawn"]);
    assert_eq!(code(&extension), 2);
    assert!(
        stderr(&extension).contains("not \"\""),
        "{}",
        stderr(&extension)
    );
    assert_eq!(code(&run(&["bogus"])), 2);
    assert_eq!(code(&run(&[])), 2);
}

#[test]
fn the_help_and_the_codebook_are_not_errors() {
    assert_eq!(code(&run(&["--help"])), 0);
    let codes = run(&["codes"]);
    assert_eq!(code(&codes), 0);
    // The codebook is the list a caller branches on, so the code this review added
    // has to be reachable from the CLI that prints it.
    let printed = String::from_utf8_lossy(&codes.stdout);
    assert!(printed.contains("negative-size"), "{printed}");
}
