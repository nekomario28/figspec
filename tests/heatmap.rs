//! A matrix as a mark: what it refuses, and that the names reach the axes.

use figspec::inspect::inspect;
use figspec::model::{ColorMap, Figure, Mark};
use figspec::to_svg;

fn heat(values: Vec<Vec<f64>>) -> Figure {
    Figure::new()
        .with_title("Sales by region")
        .with_labels("quarter", "region")
        .with_mark(Mark::Heatmap {
            values,
            row_labels: None,
            col_labels: None,
            color_map: None,
            label: None,
        })
}

fn codes(figure: &Figure) -> Vec<String> {
    inspect(figure)
        .problems
        .iter()
        .map(|problem| problem.code.clone())
        .collect()
}

#[test]
fn a_matrix_renders() {
    let svg = to_svg(&heat(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]])).expect("an svg");
    assert!(svg.contains("<svg"));
}

#[test]
fn the_labels_reach_the_axes() {
    let mut figure = heat(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    figure.marks[0] = Mark::Heatmap {
        values: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
        row_labels: Some(vec!["north".to_string(), "south".to_string()]),
        col_labels: Some(vec!["Q1".to_string(), "Q2".to_string()]),
        color_map: Some(ColorMap::Magma),
        label: Some("units".to_string()),
    };
    let svg = to_svg(&figure).expect("an svg");
    for name in ["north", "south", "Q1", "Q2"] {
        assert!(svg.contains(name), "{name} is missing from the axes");
    }
}

#[test]
fn a_ragged_matrix_is_an_error() {
    let figure = heat(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0]]);
    assert_eq!(codes(&figure), ["mismatched-lengths"]);
    assert!(to_svg(&figure).is_err(), "a ragged matrix must not render");
}

#[test]
fn label_counts_have_to_match_the_matrix() {
    let mut figure = heat(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    figure.marks[0] = Mark::Heatmap {
        values: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
        row_labels: Some(vec!["only".to_string()]),
        col_labels: Some(vec!["Q1".to_string(), "Q2".to_string()]),
        color_map: None,
        label: None,
    };
    assert_eq!(codes(&figure), ["mismatched-lengths"]);
    assert!(to_svg(&figure).is_err());
}

#[test]
fn an_empty_matrix_is_an_error_rather_than_an_empty_picture() {
    for values in [Vec::new(), vec![Vec::new()]] {
        let figure = heat(values);
        assert_eq!(codes(&figure), ["empty-mark"]);
        assert_eq!(inspect(&figure).exit_code(), 1);
        assert!(to_svg(&figure).is_err());
    }
}

#[test]
fn two_names_for_one_axis_are_refused_rather_than_picked_between() {
    let mut figure = heat(vec![vec![1.0, 2.0]]);
    // The first mark names its row; the second names *its* row something else.
    figure.marks[0] = Mark::Heatmap {
        values: vec![vec![1.0, 2.0]],
        row_labels: Some(vec!["north".to_string()]),
        col_labels: None,
        color_map: None,
        label: None,
    };
    figure.marks.push(Mark::Heatmap {
        values: vec![vec![3.0, 4.0]],
        row_labels: Some(vec!["somewhere else".to_string()]),
        col_labels: None,
        color_map: None,
        label: None,
    });
    assert!(codes(&figure).contains(&"ambiguous-labels".to_string()));
    assert!(to_svg(&figure).is_err());
}

#[test]
fn an_unknown_colour_map_is_refused_when_the_model_is_read() {
    let json = r#"{"marks":[{"kind":"heatmap","values":[[1.0]],"color_map":"rainbow"}]}"#;
    let error = Figure::from_json(json, "model").expect_err("rainbow is not a ramp we know");
    assert!(
        error.message.contains("rainbow"),
        "the refusal names the value: {error}"
    );
}

#[test]
fn a_heatmap_refuses_an_axis_setting_it_cannot_honour() {
    // A matrix is not a bar chart: colour is not a length, so a trimmed axis is
    // not `truncated-baseline`. But the axis start and a log scale are not *inert*
    // either — measured, they stretch and change the cells — so they are refused
    // with the reason rather than drawn as something the declaration did not ask
    // for.
    let mut trimmed = heat(vec![vec![1.0, 2.0, 3.0]]);
    trimmed.y_min = Some(1.0);
    assert_eq!(codes(&trimmed), ["inapplicable-axis-setting"]);
    assert!(
        !codes(&trimmed).contains(&"truncated-baseline".to_string()),
        "a matrix is not a bar"
    );
    assert!(
        to_svg(&trimmed).is_err(),
        "it must not draw a different picture"
    );

    let mut logged = heat(vec![vec![0.0, 2.0]]);
    logged.y_scale = figspec::model::Scale::Log;
    assert_eq!(codes(&logged), ["inapplicable-axis-setting"]);
}
