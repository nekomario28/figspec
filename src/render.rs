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

use crate::model::{Figure, Mark, Scale};

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

fn plots_of(figure: &Figure) -> Result<Vec<Plot>, RenderError> {
    // The same list the checks report, so a rule cannot hold in `inspect` and be
    // ignored here: a short error array used to render with the extra point's bar
    // silently missing.
    if let Some(refusal) = figure.refusals().first() {
        return Err(refuse(refusal.message.clone()));
    }
    let mut plots = Vec::with_capacity(figure.marks.len());
    for (index, mark) in figure.marks.iter().enumerate() {
        match mark {
            Mark::Line {
                points,
                y_err,
                y_err_asymmetric,
                color,
                label,
            } => {
                if points.len() < 2 {
                    return Err(refuse(format!(
                        "mark {index}: a line needs at least two points, got {}",
                        points.len()
                    )));
                }
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
                if points.is_empty() {
                    return Err(refuse(format!("mark {index}: a scatter needs points")));
                }
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
                if categories.len() != values.len() {
                    return Err(refuse(format!(
                        "mark {index}: {} categories but {} values",
                        categories.len(),
                        values.len()
                    )));
                }
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
                for one in series {
                    if one.values.len() != categories.len() {
                        return Err(refuse(format!(
                            "mark {index}: a series has {} values for {} categories",
                            one.values.len(),
                            categories.len()
                        )));
                    }
                }
                if series.is_empty() {
                    return Err(refuse(format!(
                        "mark {index}: a grouped bar needs a series"
                    )));
                }
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
        }
    }
    if plots.is_empty() {
        return Err(refuse(
            "the figure declares no marks, so there is nothing to render",
        ));
    }
    Ok(plots)
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

/// Renumber the engine's clip ids so the same figure always renders to the same
/// bytes.
///
/// The engine's ids come from a process-global counter, so two renders of one
/// figure inside one process differ by `kuva-clip-3` versus `kuva-clip-6`. That
/// makes a rendered artifact impossible to compare, hash or cache — and comparing
/// two renders is exactly what an inspection workflow does. Renumbering by first
/// appearance is deterministic, and the ids are internal to the document.
pub(crate) fn normalized_svg(svg: String) -> String {
    const PREFIX: &str = "kuva-clip-";
    let mut seen: Vec<String> = Vec::new();
    let mut rest = svg.as_str();
    while let Some(start) = rest.find(PREFIX) {
        let after = &rest[start + PREFIX.len()..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            rest = after;
            continue;
        }
        let id = format!("{PREFIX}{digits}");
        if !seen.contains(&id) {
            seen.push(id);
        }
        rest = &after[digits.len()..];
    }
    let mut out = svg;
    for (index, id) in seen.iter().enumerate() {
        if id != &format!("{PREFIX}{index}") {
            out = out.replace(id, &format!("{PREFIX}{index}"));
        }
    }
    out
}

/// The plots and the layout they are drawn against.
///
/// One function for all three consumers — the SVG, the PNG and the storyboard's
/// boxes — so a box cannot be computed against a different layout than the one
/// the picture was drawn with.
pub(crate) fn geometry(figure: &Figure) -> Result<(Vec<Plot>, Layout), RenderError> {
    let plots = plots_of(figure)?;
    let layout = layout_of(figure, &plots);
    Ok((plots, layout))
}

/// The figure as an SVG document.
pub fn to_svg(figure: &Figure) -> Result<String, RenderError> {
    let (plots, layout) = geometry(figure)?;
    Ok(normalized_svg(kuva::render_to_svg(plots, layout)))
}

/// The figure as a PNG.
pub fn to_png(figure: &Figure) -> Result<Vec<u8>, RenderError> {
    let (plots, layout) = geometry(figure)?;
    // 2.0 is the engine's retina density; it is a rendering choice, not a model
    // property, so it stays out of the model and out of the checks.
    kuva::render_to_png(plots, layout, 2.0).map_err(|error| refuse(format!("png: {error}")))
}
