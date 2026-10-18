//! Text is data: preserve names and refuse colors both backends cannot honor.

use figspec::{inspect, storyboard, to_png, to_svg, Figure};
use serde_json::json;

fn colored(kind: &str, color: &str) -> Figure {
    let mark = match kind {
        "line" => json!({"kind": kind, "points": [[0, 1], [1, 2]], "color": color}),
        "scatter" => json!({"kind": kind, "points": [[0, 1]], "color": color}),
        "bar" => json!({"kind": kind, "categories": ["a"], "values": [1], "color": color}),
        "grouped_bar" => {
            json!({"kind": kind, "categories": ["a"], "series": [{"values": [1], "color": color}]})
        }
        _ => unreachable!(),
    };
    Figure::from_json(
        &json!({"title": "Colors", "x_label": "x", "y_label": "y", "width": 240, "height": 200, "marks": [mark]}).to_string(),
        "color",
    ).unwrap()
}

#[test]
fn unsupported_colors_are_reported_and_refused_without_panicking() {
    // #éx and #aébcd used to slice inside UTF-8 in the backend and panic.
    // Other CSS colors were emitted to SVG but silently left unpainted in PNG.
    for kind in ["line", "scatter", "bar", "grouped_bar"] {
        for color in [
            "#éx",
            "#aébcd",
            "#abcdeg",
            "#1234",
            "#12345678",
            "rebeccapurple",
            "rgba(1,2,3,0.5)",
            "rgb(NaN,0,0)",
            "rgb(inf,0,0)",
        ] {
            let figure = colored(kind, color);
            let report = inspect(&figure);
            let found: Vec<_> = report
                .problems
                .iter()
                .filter(|p| p.code == "unsupported-color")
                .collect();
            assert_eq!(found.len(), 1, "{kind} {color}: {report:?}");
            assert_eq!(found[0].mark, Some(0));
            assert!(found[0].message.contains("color"));
            assert!(to_svg(&figure).is_err(), "{kind} {color}");
            assert!(to_png(&figure).is_err(), "{kind} {color}");
            assert!(storyboard::of(&figure, &report).is_err(), "{kind} {color}");
        }
    }
}

#[test]
fn shared_colors_still_draw() {
    for color in [
        "#abc",
        "#AbC123",
        "red",
        "steelblue",
        "rgb(12, 34, 56)",
        "rgb(12,\u{a0}34,56)",
        "none",
        "transparent",
    ] {
        let figure = colored("bar", color);
        assert_eq!(inspect(&figure).errors(), 0, "{color}");
        assert!(to_svg(&figure).is_ok(), "{color}");
        assert!(to_png(&figure).is_ok(), "{color}");
    }
}

#[test]
fn malformed_hex_is_not_a_rgb_triplet() {
    // Rust's integer parser accepts a leading '+', but CSS hex digits do not.
    for color in ["#+1+2+3", "#ff+000", "#+1FFFF"] {
        let figure = colored("bar", color);
        assert_eq!(inspect(&figure).errors(), 1, "{color}");
        assert!(to_svg(&figure).is_err(), "{color}");
    }
}

#[test]
fn engine_id_normalization_never_rewrites_declared_text() {
    // The old global string replacement rewrote a title to kuva-clip-1.
    let title = "kuva-clip-999 and url(#kuva-clip-999)";
    let mut figure = colored("bar", "red");
    figure.title = Some(title.into());
    figure.x_label = Some("kuva-clip-12".into());
    figure.y_label = Some("kuva-clip-123".into());
    if let figspec::Mark::Bar {
        categories, label, ..
    } = &mut figure.marks[0]
    {
        categories[0] = "kuva-clip-1234".into();
        *label = Some("kuva-clip-12345".into());
    }
    let svg = to_svg(&figure).unwrap();
    for text in [
        title,
        "kuva-clip-12",
        "kuva-clip-123",
        "kuva-clip-1234",
        "kuva-clip-12345",
    ] {
        assert!(svg.contains(&format!(">{text}</text>")), "lost {text}");
    }
    let drawn = storyboard::of(&figure, &inspect(&figure)).unwrap();
    assert_eq!(drawn.svg.matches(title).count(), 1);
    assert_eq!(svg, to_svg(&figure).unwrap());
    assert_eq!(drawn, storyboard::of(&figure, &inspect(&figure)).unwrap());
}

#[test]
fn clip_declarations_and_references_stay_consistent_across_renders() {
    let figure = colored("line", "red");
    let expected = to_svg(&figure).unwrap();
    // Exercise multi-digit process-global backend counters, not only 0 and 1.
    for _ in 0..25 {
        let svg = to_svg(&figure).unwrap();
        assert_eq!(svg, expected);
        assert!(svg.contains("<clipPath id=\"kuva-clip-0\">"));
        assert!(svg.contains("clip-path=\"url(#kuva-clip-0)\""));
    }
}

#[test]
fn heatmap_labels_are_independent_axis_declarations() {
    // Naming only one axis used to silently drop all its declared names.
    for field in ["row_labels", "col_labels"] {
        let mut mark = json!({"kind": "heatmap", "values": [[1, 2], [3, 4]]});
        mark[field] = json!(["FIRST-NAME", "SECOND-NAME"]);
        let figure = Figure::from_json(&json!({"marks": [mark]}).to_string(), "heatmap").unwrap();
        assert_eq!(inspect(&figure).errors(), 0);
        let svg = to_svg(&figure).unwrap();
        let drawn = storyboard::of(&figure, &inspect(&figure)).unwrap();
        for name in ["FIRST-NAME", "SECOND-NAME"] {
            assert_eq!(
                svg.matches(&format!(">{name}</text>")).count(),
                1,
                "{field}: {name}"
            );
            assert_eq!(drawn.svg.matches(&format!(">{name}</text>")).count(), 1);
        }
        assert!(to_png(&figure).is_ok());
    }
}
