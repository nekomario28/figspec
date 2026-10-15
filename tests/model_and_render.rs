//! The properties this crate promises, each one that a real mistake would break.

use figspec::inspect::{inspect, ERROR, INFO, WARN};
use figspec::model::{Figure, Mark, Scale};
use figspec::{to_png, to_svg};

fn bars(values: Vec<f64>) -> Mark {
    Mark::Bar {
        categories: (0..values.len()).map(|index| format!("c{index}")).collect(),
        values,
        color: None,
        label: None,
    }
}

fn line(points: Vec<[f64; 2]>) -> Mark {
    Mark::Line {
        points,
        color: None,
        label: None,
    }
}

fn honest() -> Figure {
    Figure::new()
        .with_title("Revenue")
        .with_labels("quarter", "revenue (M JPY)")
        .with_mark(bars(vec![31.5, 34.2, 29.8, 41.0]))
        .with_mark(line(vec![[0.0, 31.5], [3.0, 41.0]]))
}

#[test]
fn the_same_model_renders_to_the_same_bytes() {
    // A renderer that reads a clock, a hash seed or an environment variable would
    // make a rendered artifact impossible to compare. Nothing here does.
    let figure = honest();
    assert_eq!(to_svg(&figure).unwrap(), to_svg(&figure).unwrap());
    assert_eq!(to_png(&figure).unwrap(), to_png(&figure).unwrap());
}

#[test]
fn a_figure_survives_a_json_round_trip() {
    let figure = honest();
    let again = Figure::from_json(&figure.to_json(), "(test)").expect("round trip");
    assert_eq!(figure, again);
    assert_eq!(to_svg(&figure).unwrap(), to_svg(&again).unwrap());
}

#[test]
fn an_unknown_field_is_refused_rather_than_ignored() {
    // A mistyped `y_min` that is silently dropped renders a different chart from
    // the one that was written, with nothing to show for it.
    let error = Figure::from_json(r#"{"marks": [], "titlee": "x"}"#, "model.json")
        .expect_err("an unknown field must not be ignored");
    assert!(error.to_string().contains("titlee"), "{error}");
}

#[test]
fn the_honest_figure_has_nothing_to_report() {
    let report = inspect(&honest());
    assert_eq!(report.problems, vec![], "{}", report.to_json());
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn a_bar_chart_that_starts_above_zero_is_an_error() {
    let figure = honest();
    let mut trimmed = figure.clone();
    trimmed.y_min = Some(25.0);
    let report = inspect(&trimmed);
    let codes: Vec<&str> = report
        .problems
        .iter()
        .map(|problem| problem.code.as_str())
        .collect();
    assert!(codes.contains(&"truncated-baseline"), "{:?}", codes);
    assert_eq!(report.exit_code(), 1);
    // The finding names the mark it is about.
    let problem = report
        .problems
        .iter()
        .find(|problem| problem.code == "truncated-baseline")
        .unwrap();
    assert_eq!(problem.mark, Some(0));
    assert_eq!(problem.severity, ERROR);
}

#[test]
fn warnings_and_information_never_fail_a_build() {
    let figure = Figure::new().with_mark(bars(vec![1.0, 2.0]));
    let report = inspect(&figure);
    assert!(report
        .problems
        .iter()
        .any(|p| p.code == "no-title" && p.severity == WARN));
    assert!(report
        .problems
        .iter()
        .any(|p| p.code == "no-axis-label" && p.severity == INFO));
    assert_eq!(report.errors(), 0);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn a_figure_with_no_marks_says_so() {
    let report = inspect(&Figure::new());
    assert_eq!(report.problems[0].code, "empty-figure");
    assert_eq!(report.problems[0].severity, ERROR);
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn disagreeing_bar_lengths_are_an_error() {
    let figure = Figure::new().with_mark(Mark::Bar {
        categories: vec!["a".into(), "b".into()],
        values: vec![1.0],
        color: None,
        label: None,
    });
    let report = inspect(&figure);
    assert!(report
        .problems
        .iter()
        .any(|p| p.code == "mismatched-lengths"));
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn a_log_axis_cannot_place_a_non_positive_value() {
    let figure = Figure::new()
        .with_title("log")
        .with_mark(bars(vec![0.0, 1.0]))
        .with_mark(line(vec![[0.0, 1.0], [1.0, 2.0]]));
    let mut logged = figure.clone();
    logged.y_scale = Scale::Log;
    let report = inspect(&logged);
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.code == "non-positive-on-log-scale"),
        "{}",
        report.to_json()
    );
}

#[test]
fn rendering_refuses_what_it_cannot_draw_instead_of_approximating() {
    let mut bad = Figure::new()
        .with_title("x")
        .with_mark(bars(vec![0.0, 1.0]));
    bad.y_scale = Scale::Log;
    assert!(
        to_svg(&bad).is_err(),
        "a log axis with a zero must not render"
    );

    let empty = Figure::new();
    assert!(
        to_svg(&empty).is_err(),
        "an empty figure has nothing to draw"
    );

    let one_point = Figure::new()
        .with_title("x")
        .with_mark(line(vec![[0.0, 0.0]]));
    assert!(to_svg(&one_point).is_err(), "a line needs two points");
}

#[test]
fn the_codebook_names_every_code_the_checks_emit() {
    // The codebook is what a caller branches on, so it cannot drift from the
    // checks. Every code below is constructed above.
    let emitted = [
        "empty-figure",
        "non-finite-value",
        "mismatched-lengths",
        "truncated-baseline",
        "non-positive-on-log-scale",
        "no-title",
        "no-axis-label",
    ];
    for code in emitted {
        assert!(
            figspec::CODEBOOK.iter().any(|(known, _, _)| *known == code),
            "{code} is emitted but not in the codebook"
        );
    }
    for (code, severity, _) in figspec::CODEBOOK {
        assert!(
            emitted.contains(code),
            "{code} is in the codebook but nothing emits it"
        );
        assert!(
            matches!(*severity, ERROR | WARN | INFO),
            "{code}: {severity}"
        );
    }
}

#[test]
fn the_string_boundary_matches_the_typed_api() {
    // The Python bindings and the CLI go through these, so they must agree with
    // the typed API exactly.
    let figure = honest();
    let json = figure.to_json();
    assert_eq!(
        figspec::render_svg(&json).unwrap(),
        to_svg(&figure).unwrap()
    );
    assert_eq!(
        figspec::render_png(&json).unwrap(),
        to_png(&figure).unwrap()
    );
    assert_eq!(
        figspec::inspect_json(&json).unwrap(),
        inspect(&figure).to_json()
    );
    assert!(figspec::render_svg("not json").is_err());
}

#[test]
fn the_command_line_keeps_the_exit_policy() {
    // 0 nothing to report, 1 an error, 2 an input that could not be read.
    let dir = std::env::temp_dir().join("figspec-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let ok = dir.join("ok.json");
    let bad = dir.join("bad.json");
    let broken = dir.join("broken.json");
    honest().save(&ok).unwrap();
    let mut trimmed = honest();
    trimmed.y_min = Some(25.0);
    trimmed.save(&bad).unwrap();
    std::fs::write(&broken, "{ not json").unwrap();

    let run = |path: &std::path::Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_figspec"))
            .args(["inspect"])
            .arg(path)
            .output()
            .expect("the binary runs")
    };
    assert!(run(&ok).status.success());
    assert_eq!(run(&bad).status.code(), Some(1));
    assert_eq!(run(&broken).status.code(), Some(2));
}
