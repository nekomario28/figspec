//! The checks and the renderer are one list.
//!
//! `docs`: a rule cannot hold in the checks and be ignored by the renderer.
//! That claim was false in three places — an empty scatter, a one-point line and
//! a grouped bar with no series were reported as a **clean figure** (exit 0) and
//! then refused by the renderer (exit 2). These tests are that defect's guard, in
//! both directions.

use figspec::inspect::{inspect, ERROR};
use figspec::model::{Figure, Mark, Scale};
use figspec::{to_svg, ERROR as ERROR_CODE};

use figspec::model::Series;

fn line(points: Vec<[f64; 2]>) -> Mark {
    Mark::Line {
        points,
        y_err: None,
        y_err_asymmetric: None,
        color: None,
        label: None,
    }
}

fn scatter(points: Vec<[f64; 2]>) -> Mark {
    Mark::Scatter {
        points,
        y_err: None,
        y_err_asymmetric: None,
        size: None,
        color: None,
        label: None,
    }
}

fn bar(categories: Vec<&str>, values: Vec<f64>) -> Mark {
    Mark::Bar {
        categories: categories.into_iter().map(String::from).collect(),
        values,
        color: None,
        label: None,
    }
}

fn heat(values: Vec<Vec<f64>>) -> Mark {
    Mark::Heatmap {
        values,
        row_labels: None,
        col_labels: None,
        color_map: None,
        label: None,
    }
}

/// A figure that cannot be drawn, and the code that says so.
fn broken() -> Vec<(&'static str, Figure, &'static str)> {
    let mut logged = Figure::new()
        .with_title("x")
        .with_labels("x", "y")
        .with_mark(bar(vec!["a"], vec![0.0]));
    logged.y_scale = Scale::Log;
    vec![
        ("no marks", Figure::new(), "empty-figure"),
        (
            "empty scatter",
            Figure::new().with_mark(scatter(Vec::new())),
            "empty-mark",
        ),
        (
            "empty bar",
            Figure::new().with_mark(bar(vec![], vec![])),
            "empty-mark",
        ),
        (
            "grouped bar with no series",
            Figure::new().with_mark(Mark::GroupedBar {
                categories: vec!["a".to_string()],
                series: Vec::new(),
            }),
            "empty-mark",
        ),
        (
            "empty matrix",
            Figure::new().with_mark(heat(Vec::new())),
            "empty-mark",
        ),
        (
            "one-point line",
            Figure::new().with_mark(line(vec![[1.0, 1.0]])),
            "short-line",
        ),
        ("zero on a log axis", logged, "non-positive-on-log-scale"),
        (
            "non-finite cell",
            Figure::new().with_mark(heat(vec![vec![1.0, f64::NAN]])),
            "non-finite-value",
        ),
        (
            "ragged matrix",
            Figure::new().with_mark(heat(vec![vec![1.0, 2.0], vec![3.0]])),
            "mismatched-lengths",
        ),
        (
            "bar lengths disagree",
            Figure::new().with_mark(bar(vec!["a", "b"], vec![1.0])),
            "mismatched-lengths",
        ),
    ]
}

#[test]
fn everything_that_cannot_be_drawn_is_reported_as_an_error() {
    for (name, figure, code) in broken() {
        let report = inspect(&figure);
        let found = report
            .problems
            .iter()
            .find(|problem| problem.code == code)
            .unwrap_or_else(|| panic!("{name}: no {code} in {:?}", codes(&figure)));
        assert_eq!(found.severity, ERROR, "{name}: {code} is not an error");
        assert_eq!(report.exit_code(), 1, "{name}: the build would pass");
    }
}

#[test]
fn everything_that_cannot_be_drawn_is_refused_by_the_renderer() {
    for (name, figure, code) in broken() {
        assert!(to_svg(&figure).is_err(), "{name} ({code}) rendered anyway");
    }
}

#[test]
fn the_renderer_has_no_refusals_of_its_own() {
    // The other direction, and the reason the defect existed: a figure the checks
    // call clean must render. Each mark kind gets one.
    let mut logged = Figure::new()
        .with_title("t")
        .with_labels("x", "y")
        .with_mark(line(vec![[1.0, 1.0], [2.0, 4.0]]));
    logged.y_scale = Scale::Log;
    let figures = vec![
        (
            "line",
            Figure::new()
                .with_title("t")
                .with_labels("x", "y")
                .with_mark(line(vec![[0.0, 0.0], [3.0, 1.0]])),
        ),
        (
            "scatter",
            Figure::new()
                .with_title("t")
                .with_labels("x", "y")
                .with_mark(scatter(vec![[0.0, 0.0]])),
        ),
        ("log line", logged),
        (
            "bar",
            Figure::new()
                .with_title("t")
                .with_labels("x", "y")
                .with_mark(bar(vec!["a", "b"], vec![1.0, 2.0])),
        ),
        (
            "grouped bar",
            Figure::new()
                .with_title("t")
                .with_labels("x", "y")
                .with_mark(Mark::GroupedBar {
                    categories: vec!["a".to_string()],
                    series: vec![
                        Series {
                            values: vec![1.0],
                            label: None,
                            color: None,
                        },
                        Series {
                            values: vec![2.0],
                            label: None,
                            color: None,
                        },
                    ],
                }),
        ),
        (
            "heatmap",
            Figure::new()
                .with_title("t")
                .with_labels("x", "y")
                .with_mark(heat(vec![vec![1.0, 2.0], vec![3.0, 4.0]])),
        ),
        (
            "heatmap one row",
            Figure::new()
                .with_title("t")
                .with_labels("x", "y")
                .with_mark(heat(vec![vec![1.0, 2.0, 3.0]])),
        ),
    ];
    for (name, figure) in figures {
        assert!(
            figure.refusals().is_empty(),
            "{name}: a clean figure has refusals: {:?}",
            figure.refusals()
        );
        assert!(
            to_svg(&figure).is_ok(),
            "{name}: a clean figure did not render"
        );
        assert_eq!(
            inspect(&figure).exit_code(),
            0,
            "{name}: {:?}",
            codes(&figure)
        );
    }
}

#[test]
fn the_new_codes_are_the_errors_a_caller_sees() {
    // Named here as well as in the codebook test: these three were added by this
    // change, and a caller branching on a code needs it to be an error.
    for code in ["empty-mark", "short-line", "ambiguous-labels"] {
        let (_, severity, _) = figspec::CODEBOOK
            .iter()
            .find(|(known, _, _)| *known == code)
            .unwrap_or_else(|| panic!("{code} is not in the codebook"));
        assert_eq!(*severity, ERROR_CODE, "{code}");
    }
}

fn codes(figure: &Figure) -> Vec<String> {
    inspect(figure)
        .problems
        .iter()
        .map(|problem| problem.code.clone())
        .collect()
}
