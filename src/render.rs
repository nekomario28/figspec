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
    let mut plots = Vec::with_capacity(figure.marks.len());
    for (index, mark) in figure.marks.iter().enumerate() {
        match mark {
            Mark::Line {
                points,
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
                if let Some(label) = label {
                    plot = plot.with_legend(label.clone());
                }
                plots.push(plot.into());
            }
            Mark::Scatter {
                points,
                color,
                label,
            } => {
                if points.is_empty() {
                    return Err(refuse(format!("mark {index}: a scatter needs points")));
                }
                let mut plot = ScatterPlot::new()
                    .with_data(points.iter().map(|pair| (pair[0], pair[1])))
                    .with_color(color.clone().unwrap_or_else(|| DEFAULT_COLOR.to_string()));
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
        }
    }
    if plots.is_empty() {
        return Err(refuse(
            "the figure declares no marks, so there is nothing to render",
        ));
    }
    Ok(plots)
}

fn non_positive_on_log(figure: &Figure) -> Option<(usize, f64)> {
    if !matches!(figure.x_scale, Scale::Log) && !matches!(figure.y_scale, Scale::Log) {
        return None;
    }
    figure.marks.iter().enumerate().find_map(|(index, mark)| {
        mark.values()
            .into_iter()
            .find(|value| *value <= 0.0)
            .map(|value| (index, value))
    })
}

fn layout_of(figure: &Figure, plots: &[Plot]) -> Result<Layout, RenderError> {
    if let Some((index, value)) = non_positive_on_log(figure) {
        return Err(refuse(format!(
            "mark {index}: a log axis cannot place {value} (it needs positive values)"
        )));
    }
    let mut layout = Layout::auto_from_plots(plots);
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
    Ok(layout)
}

/// Renumber the engine's clip ids so the same figure always renders to the same
/// bytes.
///
/// The engine's ids come from a process-global counter, so two renders of one
/// figure inside one process differ by `kuva-clip-3` versus `kuva-clip-6`. That
/// makes a rendered artifact impossible to compare, hash or cache — and comparing
/// two renders is exactly what an inspection workflow does. Renumbering by first
/// appearance is deterministic, and the ids are internal to the document.
fn normalized_svg(svg: String) -> String {
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

/// The figure as an SVG document.
pub fn to_svg(figure: &Figure) -> Result<String, RenderError> {
    let plots = plots_of(figure)?;
    let layout = layout_of(figure, &plots)?;
    Ok(normalized_svg(kuva::render_to_svg(plots, layout)))
}

/// The figure as a PNG.
pub fn to_png(figure: &Figure) -> Result<Vec<u8>, RenderError> {
    let plots = plots_of(figure)?;
    let layout = layout_of(figure, &plots)?;
    // 2.0 is the engine's retina density; it is a rendering choice, not a model
    // property, so it stays out of the model and out of the checks.
    kuva::render_to_png(plots, layout, 2.0).map_err(|error| refuse(format!("png: {error}")))
}
