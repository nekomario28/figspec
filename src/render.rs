//! Rendering: a pure function of the model.
//!
//! The engine is `kuva` — a mature MIT-licensed Rust plotting library with 68
//! plot types and SVG/PNG/PDF backends. This module only maps the model onto it,
//! which keeps the interesting part (the model, and what can be checked in it) in
//! one place and the drawing in another.
//!
//! Two rules, both learned the hard way elsewhere:
//!
//! * **Refuse, do not approximate.** A log axis with a non-positive value, a bar
//!   mark whose labels and values disagree, a non-finite coordinate: each is an
//!   error naming the mark, never a picture that quietly differs from the data.
//! * **The same model renders to the same bytes.** Nothing here reads the clock,
//!   the environment or a random seed.

use kuva::prelude::*;
use kuva::render::layout::ComputedLayout;

use crate::model::{ColorMap, Figure, Mark, Scale};

/// Why a figure could not be rendered.
#[derive(Debug)]
pub struct RenderError {
    pub message: String,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RenderError {}

fn refuse(message: impl Into<String>) -> RenderError {
    RenderError {
        message: message.into(),
    }
}

const DEFAULT_COLOR: &str = "steelblue";

/// The largest pixel buffer the raster route will allocate.
///
/// The same 16 000 000 pixels `shadergx` renders a shader canvas at — one number
/// for the family, because it is the same kind of budget (4 bytes a pixel, held
/// while the picture is drawn).
const MAX_RASTER_PIXELS: f64 = 16_000_000.0;

/// The retina density a PNG is drawn at, as a multiplier on the canvas. A
/// rendering choice, not a model property, so it stays out of the model and out
/// of the checks.
const DENSITY: f32 = 2.0;

/// The side of the coverage mask the raster backend builds for a marker of
/// `size` points, in its own terms.
///
/// `make_circle_mask` takes `half = ceil(r + 1.5)` and allocates `(2*half+1)²`
/// bytes, where `r` is the radius in *scaled* pixels (`size/2 * density`).
/// Mirrored here so the refusal is about the arithmetic that actually allocates.
fn mask_side(size: f64) -> f64 {
    let radius = size / 2.0 * f64::from(DENSITY);
    2.0 * (radius + 1.5).ceil() + 1.0
}

/// Refuse a picture the raster backend cannot hold, before it panics trying.
///
/// Measured: `width`/`height` of 4 294 967 295 — a `u32`, so the model accepted
/// it — reached the backend, which multiplies the canvas by the density and
/// allocates `w*h*4` bytes **in `u32`**. Debug builds panicked at
/// `attempt to multiply with overflow`, release builds at `capacity overflow`,
/// both exit 101 with a Rust backtrace; the same figure rendered to SVG, which
/// has no pixel buffer. And a marker `size` of 1e9 panicked in debug while the
/// *release* build wrapped the mask size and drew an image nobody declared — the
/// silent disagreement this library exists to prevent. Both are the same budget,
/// so both are refused here, in `f64`, naming the declaration.
fn check_raster_budget(
    figure: &Figure,
    scene_width: f64,
    scene_height: f64,
) -> Result<(), RenderError> {
    // `width` and `height` are `u32`s rendered through `f32`, so they are finite;
    // the product is the thing that is not.
    let width = (scene_width as f32 * DENSITY).ceil() as f64;
    let height = (scene_height as f32 * DENSITY).ceil() as f64;
    if width * height > MAX_RASTER_PIXELS {
        return Err(refuse(format!(
            "a PNG is {width:.0}x{height:.0} pixels at density {DENSITY}, above the \
             {MAX_RASTER_PIXELS:.0}-pixel raster limit; render an SVG, which has no pixel \
             buffer, or declare a smaller width/height"
        )));
    }
    let largest = figure
        .marks
        .iter()
        .enumerate()
        .filter_map(|(index, mark)| match mark {
            Mark::Scatter {
                size: Some(size), ..
            } => Some((index, *size)),
            _ => None,
        })
        .max_by(|left, right| left.1.total_cmp(&right.1));
    if let Some((index, size)) = largest {
        let side = mask_side(size);
        // `size` is a `f64` a caller can set to infinity, and `side * side` can
        // overflow to it for a merely huge one, so both are checked — either way
        // there is no mask, and the backend would have wrapped or panicked.
        if !side.is_finite() || side * side > MAX_RASTER_PIXELS {
            // Two ways to have no mask, and the message says which: a side this
            // route computed and found too big, or a side that is not a number at
            // all (an infinite `size`, which JSON cannot carry but a caller can).
            let drawn = if side.is_finite() {
                format!(
                    "{side:.0}x{side:.0} pixels at density {DENSITY} is above the \
                     {MAX_RASTER_PIXELS:.0}-pixel raster limit"
                )
            } else {
                "its coverage mask is not a finite size".to_string()
            };
            return Err(refuse(format!(
                "marks[{index}].size {size:e} cannot be drawn: {drawn}; the SVG has no such buffer"
            )));
        }
    }
    Ok(())
}

/// Refuse a canvas that leaves no plot area to draw in.
///
/// The margins are absolute pixels — they grow with the tick labels — so a canvas
/// can be smaller than what its own axes need. A 1x1 canvas produced a negative
/// plot rectangle (width -70.65, height -60), so a positive canvas alone is not
/// enough. Require a positive plot area, not an arbitrary minimum canvas size.
///
/// The bound is in the route rather than the model, like the raster budget: how
/// much room the labels need is the layout engine's arithmetic, and `inspect`
/// still reads the model's own problems without a canvas.
fn check_plot_area(computed: &ComputedLayout) -> Result<(), RenderError> {
    // Finite model values can still overflow in layout arithmetic. The model
    // also has no inverted-axis mode: y_min must not reverse the computed range.
    for (axis, (lower, upper)) in [("x", computed.x_range), ("y", computed.y_range)] {
        let finite = lower.is_finite() && upper.is_finite() && (upper - lower).is_finite();
        if !finite || upper <= lower {
            return Err(refuse(format!(
                "the computed {axis} axis runs from {lower} to {upper}, not a finite increasing \
                 range with a finite span; reduce the data range and remove any y_min \
                 above the data's maximum"
            )));
        }
    }
    let across = computed.width - computed.margin_left - computed.margin_right;
    let down = computed.height - computed.margin_top - computed.margin_bottom;
    if across > 0.0 && down > 0.0 {
        return Ok(());
    }
    let missing = if across <= 0.0 && down <= 0.0 {
        "width and height"
    } else if across <= 0.0 {
        "width"
    } else {
        "height"
    };
    let need_width = (computed.margin_left + computed.margin_right).ceil();
    let need_height = (computed.margin_top + computed.margin_bottom).ceil();
    // Leave positive space beyond the axes, even when their extent is integral.
    // Regression tests render the suggested size rather than just matching text.
    Err(refuse(format!(
        "the canvas is {:.0}x{:.0} px and its own axes take {need_width:.0}x{need_height:.0} px \
         for these labels, so there is no {missing} left to draw in; declare a canvas of \
         at least {:.0}x{:.0} px",
        computed.width,
        computed.height,
        need_width + 1.0,
        need_height + 1.0
    )))
}

fn plots_of(figure: &Figure) -> Result<Vec<Plot>, RenderError> {
    // The same list the checks report, so a rule cannot hold in `inspect` and be
    // ignored here: a short error array used to render with the extra point's bar
    // silently missing.
    if let Some(refusal) = figure.refusals().first() {
        return Err(refuse(refusal.message.clone()));
    }
    let mut plots = Vec::with_capacity(figure.marks.len());
    for mark in &figure.marks {
        match mark {
            Mark::Line {
                points,
                y_err,
                y_err_asymmetric,
                color,
                label,
            } => {
                let mut plot = LinePlot::new()
                    .with_data(points.iter().map(|pair| (pair[0], pair[1])))
                    .with_color(color.clone().unwrap_or_else(|| DEFAULT_COLOR.to_string()));
                if let Some(errors) = y_err {
                    plot = plot.with_y_err(errors.iter().copied());
                } else if let Some(pairs) = y_err_asymmetric {
                    // Called after `with_data`: the errors index into the points.
                    plot = plot.with_y_err_asymmetric(pairs.iter().map(|pair| (pair[0], pair[1])));
                }
                if let Some(label) = label {
                    plot = plot.with_legend(label.clone());
                }
                plots.push(plot.into());
            }
            Mark::Scatter {
                points,
                y_err,
                y_err_asymmetric,
                size,
                color,
                label,
            } => {
                let mut plot = ScatterPlot::new()
                    .with_data(points.iter().map(|pair| (pair[0], pair[1])))
                    .with_color(color.clone().unwrap_or_else(|| DEFAULT_COLOR.to_string()));
                if let Some(errors) = y_err {
                    plot = plot.with_y_err(errors.iter().copied());
                } else if let Some(pairs) = y_err_asymmetric {
                    // Called after `with_data`: the errors index into the points.
                    plot = plot.with_y_err_asymmetric(pairs.iter().map(|pair| (pair[0], pair[1])));
                }
                if let Some(size) = size {
                    plot = plot.with_size(*size);
                }
                if let Some(label) = label {
                    plot = plot.with_legend(label.clone());
                }
                plots.push(plot.into());
            }
            Mark::Bar {
                categories,
                values,
                color,
                label,
            } => {
                let mut plot = BarPlot::new()
                    .with_bars(
                        categories
                            .iter()
                            .cloned()
                            .zip(values.iter().copied())
                            .collect::<Vec<_>>(),
                    )
                    .with_color(color.clone().unwrap_or_else(|| DEFAULT_COLOR.to_string()));
                if let Some(label) = label {
                    // A bar plot's legend names its series, and this model has one
                    // series per mark, so the label is the whole legend.
                    plot = plot.with_legend(vec![label.as_str()]);
                }
                plots.push(plot.into());
            }
            Mark::GroupedBar { categories, series } => {
                let colors: Vec<String> = series
                    .iter()
                    .map(|one| {
                        one.color
                            .clone()
                            .unwrap_or_else(|| DEFAULT_COLOR.to_string())
                    })
                    .collect();
                let mut plot = BarPlot::new();
                for (position, category) in categories.iter().enumerate() {
                    plot = plot.with_group(
                        category.clone(),
                        series
                            .iter()
                            .zip(colors.iter())
                            .map(|(one, color)| (one.values[position], color.clone())),
                    );
                }
                // One legend entry per series, named when the series is named.
                let names: Vec<String> = series
                    .iter()
                    .map(|one| one.label.clone().unwrap_or_default())
                    .collect();
                if names.iter().any(|name| !name.is_empty()) {
                    plot = plot.with_legend(names.iter().map(String::as_str).collect());
                }
                plots.push(plot.into());
            }
            Mark::Heatmap {
                values,
                color_map,
                label,
                ..
            } => {
                // Axis names belong to layout_of, independently on each axis.
                let mut plot = Heatmap::new().with_data(values.clone());
                if let Some(map) = color_map {
                    plot = plot.with_color_map(map_of(*map));
                }
                if let Some(label) = label {
                    plot = plot.with_legend(label.clone());
                }
                plots.push(plot.into());
            }
        }
    }
    if plots.is_empty() {
        return Err(refuse(
            "the figure declares no marks, so there is nothing to render",
        ));
    }
    Ok(plots)
}

/// The engine's ramp for a model's choice.
fn map_of(map: ColorMap) -> kuva::plot::ColorMap {
    use kuva::plot::ColorMap as K;
    match map {
        ColorMap::Viridis => K::Viridis,
        ColorMap::Turbo => K::Turbo,
        ColorMap::Inferno => K::Inferno,
        ColorMap::Magma => K::Magma,
        ColorMap::Plasma => K::Plasma,
        ColorMap::Cividis => K::Cividis,
        ColorMap::Warm => K::Warm,
        ColorMap::Cool => K::Cool,
        ColorMap::Cubehelix => K::Cubehelix,
    }
}

/// The smallest and largest y an error bar reaches, when a mark has any.
fn error_extent(figure: &Figure) -> Option<(f64, f64)> {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for mark in &figure.marks {
        let (points, symmetric, asymmetric) = match mark {
            Mark::Line {
                points,
                y_err,
                y_err_asymmetric,
                ..
            }
            | Mark::Scatter {
                points,
                y_err,
                y_err_asymmetric,
                ..
            } => (points, y_err.as_deref(), y_err_asymmetric.as_deref()),
            _ => continue,
        };
        for (index, point) in points.iter().enumerate() {
            let (lower, upper) = match (symmetric, asymmetric) {
                (Some(values), _) if index < values.len() => (values[index], values[index]),
                (_, Some(pairs)) if index < pairs.len() => (pairs[index][0], pairs[index][1]),
                _ => continue,
            };
            low = low.min(point[1] - lower);
            high = high.max(point[1] + upper);
        }
    }
    (low.is_finite() && high.is_finite()).then_some((low, high))
}

fn layout_of(figure: &Figure, plots: &[Plot]) -> Layout {
    let mut layout = Layout::auto_from_plots(plots);
    // The engine sizes the axes from the data, and an error bar is not data: a
    // range that stops at the value clips the bar, which is a picture that
    // disagrees with the numbers. Extend it to cover what is drawn.
    if let Some((low, high)) = error_extent(figure) {
        layout.y_range.0 = layout.y_range.0.min(low);
        layout.y_range.1 = layout.y_range.1.max(high);
        // The engine caps the padded range against the raw one, so extending only
        // the padded field is silently undone — measured: a bar reaching y=12.6
        // still drew against an axis that stopped at 12.0.
        if let Some((raw_low, raw_high)) = layout.data_y_range {
            layout.data_y_range = Some((raw_low.min(low), raw_high.max(high)));
        }
    }
    layout.log_x = matches!(figure.x_scale, Scale::Log);
    layout.log_y = matches!(figure.y_scale, Scale::Log);
    // A heatmap's names are axis tick labels, which live on the layout rather than
    // on the plot. Reading them from the mark keeps the declaration the only place
    // a name is written.
    for mark in &figure.marks {
        if let Mark::Heatmap {
            row_labels,
            col_labels,
            ..
        } = mark
        {
            if let Some(rows) = row_labels {
                layout = layout.with_y_categories(rows.clone());
            }
            if let Some(cols) = col_labels {
                layout = layout.with_x_categories(cols.clone());
            }
        }
    }
    layout.title = figure.title.clone();
    layout.x_label = figure.x_label.clone();
    layout.y_label = figure.y_label.clone();
    layout.width = figure.width.map(f64::from);
    layout.height = figure.height.map(f64::from);
    if let Some(min) = figure.y_min {
        if min.is_finite() {
            layout.y_range.0 = min;
        }
    }
    layout
}

/// Renumber only typed clip ids and their engine-generated definitions.
///
/// The backend's counter is process-global. Replacing strings in the completed
/// SVG also rewrote titles, categories and legends containing "kuva-clip-999".
/// Work on the scene instead: data-derived text is never an id to normalize.
pub(crate) fn svg_of(plots: Vec<Plot>, layout: Layout) -> String {
    use kuva::render::render::Primitive;
    let mut scene = kuva::render::render::render_multiple(plots, layout);
    for (index, id) in scene
        .elements
        .iter_mut()
        .filter_map(|primitive| match primitive {
            Primitive::ClipStart { id, .. } => Some(id),
            _ => None,
        })
        .enumerate()
    {
        let prefix = format!(r#"<clipPath id="{id}">"#);
        let normalized = format!("kuva-clip-{index}");
        for definition in &mut scene.defs {
            // These definitions are generated by the engine, not model strings.
            if definition.starts_with(&prefix) {
                *definition =
                    definition.replacen(&prefix, &format!(r#"<clipPath id="{normalized}">"#), 1);
            }
        }
        *id = normalized;
    }
    kuva::backend::svg::SvgBackend.render_scene(&scene)
}

/// The plots and the layout they are drawn against.
///
/// One function for all three consumers — the SVG, the PNG and the storyboard's
/// boxes — so a box cannot be computed against a different layout than the one
/// the picture was drawn with.
pub(crate) fn geometry(figure: &Figure) -> Result<(Vec<Plot>, Layout), RenderError> {
    let plots = plots_of(figure)?;
    let layout = layout_of(figure, &plots);
    check_plot_area(&ComputedLayout::from_layout(&layout))?;
    Ok((plots, layout))
}

/// The figure as an SVG document.
pub fn to_svg(figure: &Figure) -> Result<String, RenderError> {
    let (plots, layout) = geometry(figure)?;
    Ok(svg_of(plots, layout))
}

/// The figure as a PNG.
pub fn to_png(figure: &Figure) -> Result<Vec<u8>, RenderError> {
    let (plots, layout) = geometry(figure)?;
    // The scene is built here rather than through `render_to_png` so the size the
    // backend will multiply is the size that gets checked — one arithmetic, not a
    // second guess at what the layout's missing width defaults to.
    let scene = kuva::render::render::render_multiple(plots, layout);
    check_raster_budget(figure, scene.width, scene.height)?;
    kuva::backend::png::PngBackend::new()
        .with_scale(DENSITY)
        .render_scene(&scene)
        .map_err(|error| refuse(format!("png: {error}")))
}
