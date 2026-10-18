//! Drawn-output regressions: no fabricated locations or unreadable number plates.

use figspec::inspect::{Problem, Report};
use figspec::{inspect, storyboard, Figure, Mark};

fn bars(values: Vec<f64>) -> Figure {
    Figure::new().with_mark(Mark::Bar {
        categories: (0..values.len()).map(|i| i.to_string()).collect(),
        values,
        color: None,
        label: None,
    })
}

fn report(count: usize) -> Report {
    Report {
        problems: (0..count)
            .map(|_| Problem {
                code: "truncated-baseline".into(),
                severity: "error".into(),
                message: "a finding about this mark".into(),
                mark: Some(0),
            })
            .collect(),
    }
}

#[test]
fn an_unwritable_mark_field_is_reported_once() {
    let mut figure = bars(vec![1.0]);
    if let Mark::Bar { label, .. } = &mut figure.marks[0] {
        *label = Some("a\u{1}b".into());
    }
    let found: Vec<_> = inspect(&figure)
        .problems
        .into_iter()
        .filter(|p| p.code == "unwritable-character")
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].mark, Some(0));
}

#[test]
fn bars_with_no_visible_height_have_no_box() {
    // Measured SVG: zero bars at y=10, height=0; the storyboard fabricated a
    // box at y=456, height=4. An invisible rectangle has no location to point at.
    for values in [vec![0.0], vec![0.0, 0.0]] {
        let figure = bars(values);
        let drawn = storyboard::of(&figure, &report(1)).unwrap();
        assert!(drawn.boxes.is_empty(), "{:?}", drawn.boxes);
        assert_eq!(drawn.unboxable, vec![1]);
        assert!(drawn.unlocated.is_empty());
    }
    // Give the axis a valid upper bound while every bar is below its minimum.
    let mut below = bars(vec![0.25, 0.5]).with_mark(Mark::Scatter {
        points: vec![[1.0, 2.0]],
        y_err: None,
        y_err_asymmetric: None,
        size: None,
        color: None,
        label: None,
    });
    below.y_min = Some(1.0);
    assert_eq!(
        storyboard::of(&below, &report(1)).unwrap().unboxable,
        vec![1]
    );
    let mut visible = bars(vec![0.0, 2.0]);
    visible.y_min = Some(1.0);
    assert_eq!(storyboard::of(&visible, &report(1)).unwrap().boxes.len(), 1);
}

#[test]
fn a_number_plate_wider_than_the_canvas_is_refused() {
    // Clamping x cannot fit a label wider than the canvas. Keep all the finding
    // numbers readable or refuse, never silently clip or abbreviate them.
    let mut figure = bars(vec![1.0, 2.0]);
    figure.width = Some(100);
    figure.height = Some(400);
    let error = storyboard::of(&figure, &report(20)).unwrap_err();
    assert!(error.to_string().contains("number plate"), "{error}");
    assert!(error.to_string().contains("100"), "{error}");
    assert!(storyboard::of(&figure, &report(3)).is_ok());
}

#[test]
fn colors_obey_the_same_xml_character_contract_as_names() {
    for kind in ["bar", "grouped_bar"] {
        let mark = if kind == "bar" {
            r#"{"kind":"bar","categories":["a"],"values":[1],"color":"red\u0001"}"#
        } else {
            r#"{"kind":"grouped_bar","categories":["a"],"series":[{"values":[1],"color":"red\u0001"}]}"#
        };
        let figure =
            Figure::from_json(&format!(r#"{{"marks":[{mark}]}}"#), "text-boundary").unwrap();
        let found: Vec<_> = inspect(&figure)
            .problems
            .into_iter()
            .filter(|p| p.code == "unwritable-character")
            .collect();
        assert_eq!(found.len(), 1, "{kind}: {found:?}");
        assert!(found[0].message.contains("color"), "{:?}", found[0]);
        assert!(figspec::to_svg(&figure).is_err());
    }
}

#[test]
fn an_inverted_computed_axis_is_refused_before_boxing() {
    let mut figure = bars(vec![-5.0]);
    // The engine draws a visible bar here, but the inverted range is not a
    // meaningful increasing y axis. Refuse it, not a fabricated or missing box.
    figure.y_min = Some(1.0);
    let error = figspec::to_svg(&figure).unwrap_err();
    assert!(error.to_string().contains("y axis"), "{error}");
    assert!(storyboard::of(&figure, &inspect(&figure)).is_err());
    figure.y_min = None;
    assert!(figspec::to_svg(&figure).is_ok());
    assert_eq!(storyboard::of(&figure, &report(1)).unwrap().boxes.len(), 1);
}

#[test]
fn a_nonfinite_computed_span_is_refused() {
    // Finite declarations can overflow only after layout: the old SVG path had
    // NaN coordinates while rendering exited 0.
    for (points, minimum, axis) in [
        (vec![[0.0, -1e308], [1.0, 1e308]], Some(-1e308), "y"),
        (vec![[0.0, -1e308], [1.0, 1e308]], None, "y"),
        (vec![[-1e308, 0.0], [1e308, 1.0]], None, "x"),
    ] {
        let mut figure = Figure::new().with_mark(Mark::Line {
            points,
            y_err: None,
            y_err_asymmetric: None,
            color: None,
            label: None,
        });
        figure.y_min = minimum;
        assert_eq!(inspect(&figure).errors(), 0);
        let error = figspec::to_svg(&figure).unwrap_err();
        assert!(
            error.to_string().contains(&format!("{axis} axis")),
            "{error}"
        );
        assert!(figspec::to_png(&figure).is_err());
        assert!(storyboard::of(&figure, &report(1)).is_err());
    }
}

fn overlap_count(figure: &Figure) -> usize {
    inspect(figure)
        .problems
        .iter()
        .filter(|p| p.code == "overlapping-bars")
        .count()
}

#[test]
fn only_filled_bar_intersections_are_overlaps() {
    for (a, b) in [
        (vec![1.0], vec![-1.0]),
        (vec![1.0, 0.0], vec![0.0, 2.0]),
        (vec![0.0], vec![2.0]),
    ] {
        let mut figure = bars(a);
        figure.marks.extend(bars(b).marks);
        assert_eq!(overlap_count(&figure), 0, "{figure:?}");
    }
    let mut negative = bars(vec![-1.0]);
    negative.marks.extend(bars(vec![-2.0]).marks);
    assert_eq!(overlap_count(&negative), 1);
    // An explicit minimum can clip all of one bar's filled area away.
    let mut clipped = bars(vec![1.0]);
    clipped.marks.extend(bars(vec![2.0]).marks);
    clipped.y_min = Some(1.0);
    assert_eq!(overlap_count(&clipped), 0);
    clipped.y_min = Some(0.5);
    assert_eq!(overlap_count(&clipped), 1);
    negative.y_min = Some(0.0);
    assert_eq!(overlap_count(&negative), 0);
}

fn grouped(values: &[f64]) -> Mark {
    Mark::GroupedBar {
        categories: vec!["a".into()],
        series: values
            .iter()
            .map(|v| figspec::model::Series {
                values: vec![*v],
                label: None,
                color: None,
            })
            .collect(),
    }
}

#[test]
fn grouped_and_mixed_bars_are_checked_in_their_own_subslots() {
    let overlapping = Figure::new()
        .with_mark(grouped(&[1.0]))
        .with_mark(grouped(&[2.0]));
    assert_eq!(overlap_count(&overlapping), 1);
    let mut mixed = bars(vec![1.0]);
    mixed.marks.push(grouped(&[2.0]));
    assert_eq!(overlap_count(&mixed), 1);
    // Disjoint subdivisions are not overlaps, despite positive values in both.
    let disjoint = Figure::new()
        .with_mark(grouped(&[1.0, 0.0]))
        .with_mark(grouped(&[0.0, 2.0]));
    assert_eq!(overlap_count(&disjoint), 0);
    // Different subdivision counts overlap across series indices: [0, 1/2]
    // intersects [1/3, 2/3], but [2/3, 1] does not intersect [0, 1/2].
    let crossed = Figure::new()
        .with_mark(grouped(&[1.0, 0.0]))
        .with_mark(grouped(&[0.0, 2.0, 0.0]));
    assert_eq!(overlap_count(&crossed), 1);
    let apart = Figure::new()
        .with_mark(grouped(&[1.0, 0.0]))
        .with_mark(grouped(&[0.0, 0.0, 2.0]));
    assert_eq!(overlap_count(&apart), 0);
}
