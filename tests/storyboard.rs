//! The storyboard's promise: the box is where the renderer put the mark.
//!
//! Every test here builds the picture *again*, with kuva directly, and requires
//! figspec's pixel arithmetic to land on the primitive kuva drew. That is the
//! whole claim — a box in the wrong place is a picture that disagrees with the
//! report — so it is checked against the engine rather than against a number
//! copied from a previous run.

use figspec::inspect::inspect;
use figspec::model::{Figure, Mark, Scale, Series};
use figspec::place::{boxes, category_center, PlotArea};
use figspec::{storyboard_svg, to_svg};

use kuva::plot::{BarPlot, Heatmap, LinePlot, ScatterPlot};
use kuva::render::layout::{ComputedLayout, Layout};
use kuva::render::plots::Plot;
use kuva::render::render::{render_multiple, Primitive};

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

fn bars(values: Vec<f64>) -> Mark {
    Mark::Bar {
        categories: (0..values.len()).map(|index| format!("c{index}")).collect(),
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

fn grouped(categories: Vec<&str>, series: Vec<Vec<f64>>) -> Mark {
    Mark::GroupedBar {
        categories: categories.into_iter().map(String::from).collect(),
        series: series
            .into_iter()
            .enumerate()
            .map(|(index, values)| Series {
                values,
                label: Some(format!("s{index}")),
                color: None,
            })
            .collect(),
    }
}

/// The plot area the engine computed for `plots`, which is what figspec reads.
fn area_of(plots: &[Plot]) -> PlotArea {
    let layout = Layout::auto_from_plots(plots);
    PlotArea::of(&ComputedLayout::from_layout(&layout))
}

fn near(left: f64, right: f64) {
    assert!(
        (left - right).abs() < 1e-9,
        "{left} is not the engine's {right}"
    );
}

/// The circles kuva drew for a scatter mark, in draw order.
fn drawn_circles(plots: Vec<Plot>) -> Vec<(f64, f64)> {
    let layout = Layout::auto_from_plots(&plots);
    let scene = render_multiple(plots, layout);
    scene
        .elements
        .iter()
        .find_map(|element| match element {
            Primitive::CircleBatch { cx, cy, .. } => {
                Some(cx.iter().copied().zip(cy.iter().copied()).collect())
            }
            _ => None,
        })
        .expect("a scatter draws circles")
}

/// The rectangles kuva drew for a bar mark: `(left, top, width, height)`.
fn drawn_rects(plots: Vec<Plot>) -> Vec<(f64, f64, f64, f64)> {
    let layout = Layout::auto_from_plots(&plots);
    let scene = render_multiple(plots, layout);
    scene
        .elements
        .iter()
        .filter_map(|element| match element {
            Primitive::Rect {
                x,
                y,
                width,
                height,
                ..
            } => Some((*x, *y, *width, *height)),
            _ => None,
        })
        .collect()
}

fn drawn_polyline(plots: Vec<Plot>) -> Vec<(f64, f64)> {
    let layout = Layout::auto_from_plots(&plots);
    let scene = render_multiple(plots, layout);
    scene
        .elements
        .iter()
        .find_map(|element| match element {
            Primitive::PolyLine { points, .. } => Some(points.clone()),
            _ => None,
        })
        .expect("a line draws a polyline")
}

#[test]
fn a_line_maps_onto_the_polyline_the_engine_draws() {
    let points = [[2.0, 20.0], [8.0, 80.0]];
    let plots: Vec<Plot> = vec![LinePlot::new()
        .with_data(points.iter().map(|p| (p[0], p[1])))
        .into()];
    let area = area_of(&plots);
    let drawn = drawn_polyline(vec![LinePlot::new()
        .with_data(points.iter().map(|p| (p[0], p[1])))
        .into()]);

    for (point, pixel) in points.iter().zip(drawn.iter()) {
        near(area.x_pixel(point[0]), pixel.0);
        near(area.y_pixel(point[1]), pixel.1);
    }
}

#[test]
fn a_scatter_point_maps_onto_the_circle_the_engine_draws() {
    let points = [[1.0, 1.0], [2.0, 4.0], [3.0, 9.0]];
    let plots: Vec<Plot> = vec![ScatterPlot::new()
        .with_data(points.iter().map(|p| (p[0], p[1])))
        .into()];
    let area = area_of(&plots);
    let drawn = drawn_circles(vec![ScatterPlot::new()
        .with_data(points.iter().map(|p| (p[0], p[1])))
        .into()]);

    assert_eq!(drawn.len(), points.len());
    for (point, pixel) in points.iter().zip(drawn.iter()) {
        near(area.x_pixel(point[0]), pixel.0);
        near(area.y_pixel(point[1]), pixel.1);
    }
}

#[test]
fn a_bar_maps_onto_the_centre_of_the_rectangle_the_engine_draws() {
    let values = [5.0, 10.0, 15.0];
    let plots: Vec<Plot> = vec![BarPlot::new()
        .with_bars(
            values
                .iter()
                .enumerate()
                .map(|(index, value)| (format!("c{index}"), *value))
                .collect::<Vec<_>>(),
        )
        .into()];
    let area = area_of(&plots);
    let drawn = drawn_rects(vec![BarPlot::new()
        .with_bars(
            values
                .iter()
                .enumerate()
                .map(|(index, value)| (format!("c{index}"), *value))
                .collect::<Vec<_>>(),
        )
        .into()]);

    assert_eq!(drawn.len(), values.len());
    for (index, (left, _top, width, _height)) in drawn.iter().enumerate() {
        // The engine centres a category on the rectangle it draws.
        near(area.x_pixel(category_center(index)), left + width / 2.0);
    }
    for (value, (_left, top, _width, height)) in values.iter().zip(drawn.iter()) {
        // And the bar's far end is the value, its other end the baseline.
        near(area.y_pixel(*value), *top);
        near(area.y_pixel(0.0), top + height);
    }
}

#[test]
fn a_bar_box_covers_every_bar_the_engine_draws() {
    let figure = Figure::new().with_mark(bars(vec![5.0, 10.0, 15.0]));
    let boxed =
        boxes(&figure).expect("a bar chart has boxes")[0].expect("a bar mark has an extent");
    let drawn = drawn_rects(vec![BarPlot::new()
        .with_bars(
            vec![("c0", 5.0), ("c1", 10.0), ("c2", 15.0)]
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect::<Vec<_>>(),
        )
        .into()]);
    for (left, top, width, height) in drawn {
        assert!(boxed.left <= left + 1e-9, "{} > {left}", boxed.left);
        assert!(
            boxed.right >= left + width - 1e-9,
            "{} < {}",
            boxed.right,
            left + width
        );
        assert!(boxed.top <= top + 1e-9, "{} > {top}", boxed.top);
        assert!(
            boxed.bottom >= top + height - 1e-9,
            "{} < {}",
            boxed.bottom,
            top + height
        );
    }
}

#[test]
fn a_grouped_bar_box_covers_every_sub_bar() {
    let figure = Figure::new().with_mark(grouped(
        vec!["a", "b"],
        vec![vec![1.0, 3.0], vec![2.0, 4.0]],
    ));
    let boxed = boxes(&figure).expect("boxes")[0].expect("an extent");
    let mut plot = BarPlot::new();
    plot = plot.with_group(
        "a".to_string(),
        vec![(1.0, "red".to_string()), (2.0, "blue".to_string())],
    );
    plot = plot.with_group(
        "b".to_string(),
        vec![(3.0, "red".to_string()), (4.0, "blue".to_string())],
    );
    for (left, top, width, height) in drawn_rects(vec![plot.into()]) {
        assert!(boxed.left <= left + 1e-9);
        assert!(boxed.right >= left + width - 1e-9);
        assert!(boxed.top <= top + 1e-9);
        assert!(boxed.bottom >= top + height - 1e-9);
    }
}

#[test]
fn a_log_axis_maps_as_the_engine_does() {
    let points = vec![[1.0, 1.0], [2.0, 100.0]];
    let plot = || -> Plot {
        ScatterPlot::new()
            .with_data(points.iter().map(|p| (p[0], p[1])))
            .into()
    };
    let log_layout = || {
        let mut layout = Layout::auto_from_plots(&[plot()]);
        layout.log_y = true;
        layout
    };
    let area = PlotArea::of(&ComputedLayout::from_layout(&log_layout()));
    let scene = render_multiple(vec![plot()], log_layout());
    let drawn: Vec<(f64, f64)> = scene
        .elements
        .iter()
        .find_map(|element| match element {
            Primitive::CircleBatch { cx, cy, .. } => {
                Some(cx.iter().copied().zip(cy.iter().copied()).collect())
            }
            _ => None,
        })
        .expect("a scatter draws circles");

    // The engine draws its own log axis; the mapping has to be that same one.
    for (point, pixel) in points.iter().zip(drawn.iter()) {
        near(area.x_pixel(point[0]), pixel.0);
        near(area.y_pixel(point[1]), pixel.1);
    }

    let mut figure = Figure::new().with_mark(scatter(points.clone()));
    figure.y_scale = Scale::Log;
    let boxed = boxes(&figure).expect("boxes")[0].expect("an extent");
    for point in &points {
        let (x, y) = (area.x_pixel(point[0]), area.y_pixel(point[1]));
        assert!(
            boxed.left <= x && x <= boxed.right,
            "x {x} outside {boxed:?}"
        );
        assert!(
            boxed.top <= y && y <= boxed.bottom,
            "y {y} outside {boxed:?}"
        );
    }
}

#[test]
fn a_heatmap_box_covers_every_cell_the_engine_draws() {
    let figure = Figure::new().with_mark(heat(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]));
    let boxed = boxes(&figure).expect("boxes")[0].expect("an extent");
    let plot = || -> Plot {
        Heatmap::new()
            .with_data(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]])
            .into()
    };
    let layout = Layout::auto_from_plots(&[plot()]);
    let area = PlotArea::of(&ComputedLayout::from_layout(&layout));
    let scene = render_multiple(vec![plot()], layout);
    let mut cells = 0;
    for element in &scene.elements {
        if let Primitive::RectBatch { x, y, w, h, .. } = element {
            for index in 0..x.len() {
                cells += 1;
                assert!(boxed.left <= x[index] + 1e-9, "{boxed:?} {element:?}");
                assert!(boxed.right >= x[index] + w[index] - 1e-9);
                assert!(boxed.top <= y[index] + 1e-9);
                assert!(boxed.bottom >= y[index] + h[index] - 1e-9);
                assert!(area.left <= x[index] + 1e-9);
                assert!(area.right >= x[index] + w[index] - 1e-9);
            }
        }
    }
    // Every cell of a 2x3 matrix, drawn by the engine, inside figspec's box.
    assert_eq!(cells, 6);
}

#[test]
fn a_lone_point_still_has_a_box_to_draw() {
    let figure = Figure::new().with_mark(scatter(vec![[2.0, 2.0]]));
    let boxed = boxes(&figure).expect("boxes")[0].expect("an extent");
    assert!(boxed.width() > 0.0 && boxed.height() > 0.0, "{boxed:?}");
}

#[test]
fn a_box_never_leaves_the_plot() {
    // A bar chart whose axis starts above the baseline: the bar is drawn down to
    // the axis, so the box can only be the visible part.
    let mut figure = Figure::new().with_mark(bars(vec![5.0, 10.0, 15.0]));
    figure.y_min = Some(5.0);
    let boxed = boxes(&figure).expect("boxes")[0].expect("an extent");
    let mut layout = Layout::auto_from_plots(&[BarPlot::new()
        .with_bars(
            vec![("c0", 5.0), ("c1", 10.0), ("c2", 15.0)]
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect::<Vec<_>>(),
        )
        .into()]);
    // The same axis the figure asks for: that is what moves the y labels and so
    // the margins the plot is drawn inside.
    layout.y_range.0 = 5.0;
    let area = PlotArea::of(&ComputedLayout::from_layout(&layout));
    assert!(boxed.left >= area.left - 1e-9, "{boxed:?} {area:?}");
    assert!(boxed.right <= area.right + 1e-9, "{boxed:?} {area:?}");
    assert!(boxed.top >= area.top - 1e-9, "{boxed:?} {area:?}");
    assert!(boxed.bottom <= area.bottom + 1e-9, "{boxed:?} {area:?}");
    // The bars are drawn from the baseline, which this axis starts above, so the
    // box reaches the bottom of the plot instead of a baseline off-screen.
    near(boxed.bottom, area.bottom);
}

#[test]
fn a_figure_that_cannot_be_drawn_has_no_storyboard() {
    let figure = Figure::new().with_mark(scatter(Vec::new()));
    assert!(boxes(&figure).is_err());
    let report = inspect(&figure);
    assert!(figspec::storyboard::of(&figure, &report).is_err());
}

#[test]
fn the_storyboard_is_the_rendered_figure_with_boxes_on_top() {
    let mut figure = Figure::new()
        .with_title("Revenue")
        .with_labels("quarter", "revenue")
        .with_mark(bars(vec![31.5, 34.2, 29.8, 41.0]));
    figure.y_min = Some(5.0);
    let report = inspect(&figure);
    let drawn = figspec::storyboard::of(&figure, &report).expect("a storyboard");

    let plain = to_svg(&figure).expect("an svg");
    let start = drawn
        .svg
        .find("<g id=\"figspec-storyboard\">")
        .expect("the boxes are a group");
    let end = drawn.svg.rfind("</svg>").expect("an svg document");
    assert!(start < end);
    // Everything the engine drew is untouched: the storyboard only adds.
    assert_eq!(
        drawn.svg[..start].trim_end(),
        plain.trim_end().trim_end_matches("</svg>").trim_end()
    );
    assert!(drawn.svg[end..].starts_with("</svg>"));
    assert!(drawn.svg[start..end].contains("<rect"));
}

#[test]
fn the_numbers_are_the_report_positions() {
    // Two truncated bar marks: two errors, on two marks, and the warnings after
    // them keep their own numbers.
    let mut figure = Figure::new()
        .with_mark(bars(vec![31.5, 34.2]))
        .with_mark(bars(vec![10.0, 11.0]));
    figure.y_min = Some(5.0);
    let report = inspect(&figure);
    let drawn = figspec::storyboard::of(&figure, &report).expect("a storyboard");
    assert_eq!(drawn.boxes.len(), 2, "{:?}", drawn.boxes);
    assert_eq!(drawn.boxes[0].mark, 0);
    assert_eq!(drawn.boxes[1].mark, 1);
    assert_eq!(drawn.boxes[0].numbers, vec![1]);
    assert_eq!(drawn.boxes[1].numbers, vec![2]);
    assert!(drawn.boxes.iter().all(|boxed| boxed.is_error));
    assert!(drawn.svg.contains(">1</text>"));
    assert!(drawn.svg.contains(">2</text>"));
}

#[test]
fn a_finding_about_no_mark_is_reported_and_not_boxed() {
    let figure = Figure::new().with_mark(bars(vec![1.0, 2.0]));
    let report = inspect(&figure);
    assert!(report.problems.iter().any(|problem| problem.mark.is_none()));
    let drawn = figspec::storyboard::of(&figure, &report).expect("a storyboard");
    assert!(drawn.boxes.is_empty(), "{:?}", drawn.boxes);
    let boxed: Vec<usize> = drawn
        .boxes
        .iter()
        .flat_map(|boxed| boxed.numbers.clone())
        .collect();
    let all: Vec<usize> = (1..=report.problems.len()).collect();
    let expected: Vec<usize> = all.into_iter().filter(|n| !boxed.contains(n)).collect();
    assert_eq!(drawn.unlocated, expected);
}

#[test]
fn two_findings_on_one_mark_share_one_box() {
    // A report can name the same mark twice — a baseline plus a review, say. One
    // box with two numbers beats two boxes drawn on top of each other.
    let mut figure = Figure::new().with_mark(bars(vec![1.0, 2.0]));
    figure.y_min = Some(1.0);
    let mut report = inspect(&figure);
    let mut second = report.problems[0].clone();
    assert_eq!(
        second.mark,
        Some(0),
        "the truncated baseline is about mark 0"
    );
    second.code = "truncated-baseline".to_string();
    report.problems.insert(0, second);
    let drawn = figspec::storyboard::of(&figure, &report).expect("a storyboard");
    assert_eq!(drawn.boxes.len(), 1, "{:?}", drawn.boxes);
    assert_eq!(drawn.boxes[0].numbers, vec![1, 2]);
    assert!(drawn.svg.contains(">1,2</text>"));
}

#[test]
fn the_same_figure_and_report_draw_the_same_bytes() {
    let figure = Figure::new()
        .with_title("t")
        .with_labels("x", "y")
        .with_mark(line(vec![[0.0, 0.0], [1.0, 1.0]]));
    let report = inspect(&figure);
    let first = figspec::storyboard::of(&figure, &report).expect("a storyboard");
    let second = figspec::storyboard::of(&figure, &report).expect("a storyboard");
    assert_eq!(first.svg, second.svg);
    let json = figure.to_json();
    assert_eq!(
        storyboard_svg(&json).expect("svg"),
        storyboard_svg(&json).expect("svg")
    );
}
