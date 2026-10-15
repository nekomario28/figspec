//! figspec — a figure as data.
//!
//! ```text
//!   model (JSON)  ──inspect──▶  problems, with stable codes
//!        │
//!        └──render──▶  SVG / PNG, by kuva (MIT)
//! ```
//!
//! The model is plain data, so it can be written by a person, emitted by an
//! agent, or converted from an existing matplotlib figure — and inspected
//! without rendering it and without executing anything.
//!
//! The string-in/string-out entry points ([`render_svg`], [`inspect_json`]) are
//! the boundary the Python bindings and the CLI share, so both see exactly the
//! same behaviour.

pub mod inspect;
pub mod model;
pub mod place;
pub mod render;
pub mod storyboard;

pub use inspect::{inspect, Report, ERROR, INFO, WARN};
pub use model::{Figure, Mark, ModelError, Scale};
pub use place::{MarkBox, PlotArea};
pub use render::{to_png, to_svg, RenderError};
pub use storyboard::Storyboard;

/// Render a figure given as JSON to SVG.
pub fn render_svg(model_json: &str) -> Result<String, String> {
    let figure = Figure::from_json(model_json, "(model)").map_err(|error| error.to_string())?;
    to_svg(&figure).map_err(|error| error.to_string())
}

/// Render a figure given as JSON to PNG.
pub fn render_png(model_json: &str) -> Result<Vec<u8>, String> {
    let figure = Figure::from_json(model_json, "(model)").map_err(|error| error.to_string())?;
    to_png(&figure).map_err(|error| error.to_string())
}

/// The figure as an SVG with a numbered box on every mark a finding names, given
/// as JSON. The numbers are the findings' positions in `inspect`'s report.
pub fn storyboard_svg(model_json: &str) -> Result<String, String> {
    let figure = Figure::from_json(model_json, "(model)").map_err(|error| error.to_string())?;
    let report = inspect(&figure);
    storyboard::of(&figure, &report)
        .map(|drawn| drawn.svg)
        .map_err(|error| error.to_string())
}

/// Inspect a figure given as JSON, returning the report as JSON.
pub fn inspect_json(model_json: &str) -> Result<String, String> {
    let figure = Figure::from_json(model_json, "(model)").map_err(|error| error.to_string())?;
    Ok(inspect(&figure).to_json())
}

/// Every problem code, its severity and what it means: the codebook a caller
/// branches on. Kept here so the documentation cannot drift from the checks.
pub const CODEBOOK: &[(&str, &str, &str)] = &[
    (
        "empty-figure",
        ERROR,
        "the figure declares no marks, so there is nothing to read",
    ),
    (
        "empty-mark",
        ERROR,
        "a mark declares nothing to draw, so it would render as nothing",
    ),
    (
        "short-line",
        ERROR,
        "a line has fewer than two points, so there is no line to draw",
    ),
    (
        "inapplicable-axis-setting",
        ERROR,
        "a heatmap's rows are its y axis, so y_min and a log scale cannot apply to it",
    ),
    (
        "ambiguous-labels",
        ERROR,
        "two marks declare different names for the same axis, so one would caption the other",
    ),
    (
        "non-finite-value",
        ERROR,
        "a coordinate is NaN or infinite and cannot be placed",
    ),
    (
        "mismatched-lengths",
        ERROR,
        "a collection disagrees in number with what it annotates: a group's series, an error array and its points, or a heatmap's rows and labels",
    ),
    (
        "truncated-baseline",
        ERROR,
        "bars start above zero, so their lengths no longer mean their values",
    ),
    (
        "non-positive-on-log-scale",
        ERROR,
        "a log axis is asked to place zero or a negative value",
    ),
    (
        "two-error-shapes",
        ERROR,
        "a mark declares symmetric and asymmetric error bars at once",
    ),
    (
        "negative-error",
        ERROR,
        "an error magnitude is negative, which is not a distance",
    ),
    (
        "no-title",
        WARN,
        "the figure has no title, so a reader cannot tell what it shows",
    ),
    ("no-axis-label", INFO, "an axis has no label"),
];

#[cfg(feature = "python")]
mod python {
    use pyo3::exceptions::PyValueError;
    use pyo3::prelude::*;

    #[pyfunction]
    fn render_svg(model_json: &str) -> PyResult<String> {
        super::render_svg(model_json).map_err(PyValueError::new_err)
    }

    #[pyfunction]
    fn render_png(model_json: &str) -> PyResult<Vec<u8>> {
        super::render_png(model_json).map_err(PyValueError::new_err)
    }

    #[pyfunction]
    fn inspect_json(model_json: &str) -> PyResult<String> {
        super::inspect_json(model_json).map_err(PyValueError::new_err)
    }

    #[pyfunction]
    fn storyboard_svg(model_json: &str) -> PyResult<String> {
        super::storyboard_svg(model_json).map_err(PyValueError::new_err)
    }

    /// figspec's compiled half. The Python package in `python/figspec` is the
    /// public surface; this module only carries the model across the boundary as
    /// JSON, so both languages see the same checks and the same rendering.
    #[pymodule]
    fn _figspec(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add_function(wrap_pyfunction!(render_svg, module)?)?;
        module.add_function(wrap_pyfunction!(render_png, module)?)?;
        module.add_function(wrap_pyfunction!(inspect_json, module)?)?;
        module.add_function(wrap_pyfunction!(storyboard_svg, module)?)?;
        module.add("__doc__", "figspec's compiled extension")?;
        Ok(())
    }
}
