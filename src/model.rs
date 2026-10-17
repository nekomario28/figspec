//! A figure is data.
//!
//! Nothing in this module needs a renderer, a Python runtime or matplotlib. The
//! model *is* the artifact: the renderer is a pure function of it, the checks read
//! it directly, and it survives a round trip through JSON. That is what makes a
//! chart inspectable without executing anything — and what lets a matplotlib
//! figure, an agent's output and a person's editor produce the same artifact.
//!
//! Unknown fields are refused rather than ignored (`deny_unknown_fields`). A
//! mistyped `y_min` that is silently dropped is a chart that renders differently
//! from what its author wrote, with nothing to show for it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How an axis maps value to position.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    #[default]
    Linear,
    Log,
}

/// One series drawn into the figure.
///
/// `points` are `[x, y]` pairs. A `Bar` mark is categorical on x, which is why it
/// carries labels instead of coordinates: the axis is the categories' order.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mark {
    Line {
        points: Vec<[f64; 2]>,
        /// Symmetric error magnitudes: one per point, drawn both ways.
        #[serde(default)]
        y_err: Option<Vec<f64>>,
        /// `[lower, upper]` per point, when the two sides differ.
        #[serde(default)]
        y_err_asymmetric: Option<Vec<[f64; 2]>>,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    Scatter {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        y_err: Option<Vec<f64>>,
        #[serde(default)]
        y_err_asymmetric: Option<Vec<[f64; 2]>>,
        /// Marker size in points. `None` lets the engine choose.
        #[serde(default)]
        size: Option<f64>,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    Bar {
        categories: Vec<String>,
        values: Vec<f64>,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    /// Several series side by side, one group per category.
    ///
    /// Separate from `Bar` rather than an optional field on it: one series and
    /// several are different shapes, and a mark that meant both depending on a
    /// field's presence would be two shapes pretending to be one.
    GroupedBar {
        categories: Vec<String>,
        series: Vec<Series>,
    },
    /// A matrix drawn as coloured cells: the value is the colour, and both axes
    /// are categorical.
    Heatmap {
        /// One inner vector per row. Row 0 is drawn at the **bottom**, because a
        /// y axis grows upward — measured: `[[north...], [west...]]` renders north
        /// below west, and `row_labels` is read in the same order.
        values: Vec<Vec<f64>>,
        /// Names for the rows and columns. Shown on the axes when declared, and
        /// checked against the matrix: a label array that does not match is a
        /// caption for something else.
        #[serde(default)]
        row_labels: Option<Vec<String>>,
        #[serde(default)]
        col_labels: Option<Vec<String>>,
        #[serde(default)]
        color_map: Option<ColorMap>,
        #[serde(default)]
        label: Option<String>,
    },
}

/// The colour ramp a heatmap is drawn with.
///
/// A closed set rather than a free string: an unknown name is refused when the
/// model is read (exit 2), so a typo cannot silently become the default ramp and
/// a picture that means something else.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColorMap {
    Viridis,
    Turbo,
    Inferno,
    Magma,
    Plasma,
    Cividis,
    Warm,
    Cool,
    Cubehelix,
}

/// One series of a grouped bar chart.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub values: Vec<f64>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

impl Mark {
    /// Every number this mark draws *from*, for validation and scale checks.
    ///
    /// Error magnitudes are deliberately excluded: they are distances, not values
    /// on the axis, so a log scale must not be asked to place them.
    /// The x coordinates a mark draws, when it has numeric ones.
    pub fn x_values(&self) -> Vec<f64> {
        match self {
            Mark::Line { points, .. } | Mark::Scatter { points, .. } => {
                points.iter().map(|pair| pair[0]).collect()
            }
            _ => vec![],
        }
    }

    /// The y values a mark draws: the second coordinate, or a bar's value.
    pub fn y_values(&self) -> Vec<f64> {
        match self {
            Mark::Line { points, .. } | Mark::Scatter { points, .. } => {
                points.iter().map(|pair| pair[1]).collect()
            }
            Mark::Bar { values, .. } => values.clone(),
            Mark::GroupedBar { series, .. } => series
                .iter()
                .flat_map(|one| one.values.iter().copied())
                .collect(),
            Mark::Heatmap { values, .. } => values.iter().flatten().copied().collect(),
        }
    }

    pub fn values(&self) -> Vec<f64> {
        match self {
            Mark::Line { points, .. } | Mark::Scatter { points, .. } => points
                .iter()
                .flat_map(|pair| pair.iter().copied())
                .collect(),
            Mark::Bar { values, .. } => values.clone(),
            Mark::GroupedBar { series, .. } => series
                .iter()
                .flat_map(|one| one.values.iter().copied())
                .collect(),
            Mark::Heatmap { values, .. } => values.iter().flatten().copied().collect(),
        }
    }

    /// Every error magnitude this mark draws, symmetric or asymmetric.
    pub fn errors(&self) -> Vec<f64> {
        let (symmetric, asymmetric) = match self {
            Mark::Line {
                y_err,
                y_err_asymmetric,
                ..
            }
            | Mark::Scatter {
                y_err,
                y_err_asymmetric,
                ..
            } => (y_err.as_deref(), y_err_asymmetric.as_deref()),
            _ => (None, None),
        };
        let mut out: Vec<f64> = symmetric.unwrap_or_default().to_vec();
        for pair in asymmetric.unwrap_or_default() {
            out.extend_from_slice(pair);
        }
        out
    }

    /// The error shape this mark declares: `(shape, points, magnitudes)`.
    ///
    /// `None` when the mark carries no error bars, or when it declares both
    /// shapes — that is its own finding (`two-error-shapes`), and reporting it
    /// twice would bury the reason.
    pub fn error_shape(&self) -> Option<(&'static str, usize, usize)> {
        match self {
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
            } => match (y_err, y_err_asymmetric) {
                (Some(values), None) => Some(("symmetric", points.len(), values.len())),
                (None, Some(pairs)) => Some(("asymmetric", points.len(), pairs.len())),
                _ => None,
            },
            _ => None,
        }
    }

    /// A mark carries symmetric or asymmetric bars, never both: two shapes with
    /// two meanings for one drawing is how a chart differs from its own data.
    pub fn declares_two_error_shapes(&self) -> bool {
        match self {
            Mark::Line {
                y_err,
                y_err_asymmetric,
                ..
            }
            | Mark::Scatter {
                y_err,
                y_err_asymmetric,
                ..
            } => y_err.is_some() && y_err_asymmetric.is_some(),
            _ => false,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Mark::Line { .. } => "line",
            Mark::Scatter { .. } => "scatter",
            Mark::Bar { .. } => "bar",
            Mark::GroupedBar { .. } => "grouped_bar",
            Mark::Heatmap { .. } => "heatmap",
        }
    }

    /// The category labels this mark draws, when it is categorical.
    pub fn categories(&self) -> Option<&[String]> {
        match self {
            Mark::Bar { categories, .. } | Mark::GroupedBar { categories, .. } => Some(categories),
            _ => None,
        }
    }

    /// How many bars the mark draws per category: one for `Bar`, one per series
    /// for `GroupedBar`.
    /// Whether the mark declares nothing to draw.
    ///
    /// The renderer consults this rather than deciding for itself: `refusals()` is
    /// the single list of what cannot be drawn, and a mark that looked empty only
    /// to the renderer would be reported as a clean figure and then refused.
    pub fn is_empty(&self) -> bool {
        match self {
            Mark::Line { points, .. } | Mark::Scatter { points, .. } => points.is_empty(),
            Mark::Bar { values, .. } => values.is_empty(),
            // No categories and series full of empty rows both draw nothing. Testing
            // only `series.is_empty()` let `{"categories": [], "series":
            // [{"values": []}]}` pass as a figure while the other route called it
            // empty, and `mismatched-lengths` agreed with it (0 == 0).
            Mark::GroupedBar {
                categories, series, ..
            } => {
                categories.is_empty()
                    || series.is_empty()
                    || series.iter().all(|one| one.values.is_empty())
            }
            Mark::Heatmap { values, .. } => values.is_empty() || values.iter().any(Vec::is_empty),
        }
    }

    pub fn values_per_category(&self) -> Vec<usize> {
        match self {
            Mark::Bar { values, .. } => vec![values.len()],
            Mark::GroupedBar { series, .. } => series.iter().map(|one| one.values.len()).collect(),
            _ => vec![],
        }
    }
}

/// A chart, as data.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Figure {
    pub marks: Vec<Mark>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub x_label: Option<String>,
    #[serde(default)]
    pub y_label: Option<String>,
    #[serde(default)]
    pub x_scale: Scale,
    #[serde(default)]
    pub y_scale: Scale,
    /// Where the y axis starts. `None` lets the renderer choose. A bar chart that
    /// starts above zero misleads, which is what `truncated-baseline` reports.
    #[serde(default)]
    pub y_min: Option<f64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

/// A fault that makes the figure impossible to draw as written.
///
/// One owner, two consumers: `inspect` reports these as `error`s and the renderer
/// refuses them, so a rule cannot hold in the checks and be ignored by the
/// renderer. (A figure that *can* be drawn while being misleading — a truncated
/// bar baseline — is a check, not a refusal: rendering it is how you see the
/// problem.)
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub code: &'static str,
    pub mark: Option<usize>,
    pub message: String,
}

/// Why a declaration could not be read.
#[derive(Debug)]
pub struct ModelError {
    pub where_: String,
    pub message: String,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.where_, self.message)
    }
}

impl std::error::Error for ModelError {}

fn at(where_: impl Into<String>, message: impl Into<String>) -> ModelError {
    ModelError {
        where_: where_.into(),
        message: message.into(),
    }
}

impl Figure {
    /// Read a figure from JSON, refusing anything the model does not name.
    pub fn from_json(text: &str, where_: &str) -> Result<Self, ModelError> {
        let figure: Self =
            serde_json::from_str(text).map_err(|error| at(where_, error.to_string()))?;
        // A canvas with no extent is not a canvas. `u32` accepts zero, so this was
        // read and drawn as a 0-pixel-wide artifact; the other route refuses the
        // file, and refusing it here (rather than reporting it as a check) keeps the
        // two readers agreeing on *how* a malformed figure is refused.
        for (field, size) in [("width", figure.width), ("height", figure.height)] {
            if size == Some(0) {
                return Err(at(
                    where_,
                    format!("{field} is 0, expected a positive integer"),
                ));
            }
        }
        Ok(figure)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ModelError> {
        let path = path.as_ref();
        let display = path.display().to_string();
        let text = std::fs::read_to_string(path)
            .map_err(|error| at(&display, format!("cannot read: {error}")))?;
        Self::from_json(&text, &display)
    }

    /// Canonical JSON: stable key order, so two equal figures are byte-identical.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a figure always serializes")
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ModelError> {
        let path = path.as_ref();
        std::fs::write(path, format!("{}\n", self.to_json()))
            .map_err(|error| at(path.display().to_string(), format!("cannot write: {error}")))
    }

    /// Everything that makes this figure impossible to draw as written.
    pub fn refusals(&self) -> Vec<Refusal> {
        let mut refusals = Vec::new();
        if self.marks.is_empty() {
            refusals.push(Refusal {
                code: "empty-figure",
                mark: None,
                message: "the figure declares no marks, so there is nothing to read".to_string(),
            });
        }
        for (index, mark) in self.marks.iter().enumerate() {
            let values = mark.values();
            if let Some(bad) = values.iter().find(|value| !value.is_finite()) {
                refusals.push(Refusal {
                    code: "non-finite-value",
                    mark: Some(index),
                    message: format!("{} mark draws {bad}, which cannot be placed", mark.kind()),
                });
            }
            // A log axis is *one* axis. Concatenating x and y and testing the lot
            // reported a non-positive x against a log y (measured: a line at x=0
            // with `y_scale: "log"` was refused although its x axis is linear, and
            // `render` refused with it).
            //
            // A heatmap's cells are not values on the axis either: the colour is
            // the value, so a non-positive cell is not a log-axis problem. (A log
            // *scale* on a heatmap figure is refused above, because the engine
            // would apply it to the cells' positions.)
            if !matches!(mark, Mark::Heatmap { .. }) {
                // A log axis is *one* axis. Concatenating x and y and testing the lot
                // reported a non-positive x against a log y (measured: a line at x=0
                // with `y_scale: "log"` was refused although its x axis is linear).
                // A heatmap is skipped entirely: its cells are not values on the
                // axis, so a non-positive cell is not a log-axis problem.
                let on_a_log_axis = match (self.x_scale, self.y_scale) {
                    (Scale::Log, Scale::Log) => mark.values(),
                    (Scale::Log, Scale::Linear) => mark.x_values(),
                    (Scale::Linear, Scale::Log) => mark.y_values(),
                    (Scale::Linear, Scale::Linear) => Vec::new(),
                };
                if let Some(bad) = on_a_log_axis.iter().find(|value| **value <= 0.0) {
                    refusals.push(Refusal {
                        code: "non-positive-on-log-scale",
                        mark: Some(index),
                        message: format!("a log axis cannot place {bad}"),
                    });
                }
            }
            if let Some((shape, points, declared)) = mark.error_shape() {
                if points != declared {
                    refusals.push(Refusal {
                        code: "mismatched-lengths",
                        mark: Some(index),
                        message: format!(
                            "{} mark declares {declared} {shape} error magnitudes for {points} points",
                            mark.kind()
                        ),
                    });
                }
            }
            if mark.declares_two_error_shapes() {
                refusals.push(Refusal {
                    code: "two-error-shapes",
                    mark: Some(index),
                    message: format!(
                        "{} mark declares symmetric and asymmetric error bars; one drawing cannot mean both",
                        mark.kind()
                    ),
                });
            }
            if mark.is_empty() {
                refusals.push(Refusal {
                    code: "empty-mark",
                    mark: Some(index),
                    message: format!("{} mark declares nothing to draw", mark.kind()),
                });
            }
            if let Mark::Line { points, .. } = mark {
                if points.len() == 1 {
                    refusals.push(Refusal {
                        code: "short-line",
                        mark: Some(index),
                        message: "a line needs at least two points, and this one has 1".to_string(),
                    });
                }
            }
            if let Mark::Heatmap {
                values,
                row_labels,
                col_labels,
                ..
            } = mark
            {
                // A row that is not the width of the first is not a matrix, and
                // drawing it would silently drop or stretch cells.
                if let Some(width) = values.first().map(Vec::len) {
                    for (row, cells) in values.iter().enumerate() {
                        if cells.len() != width {
                            refusals.push(Refusal {
                                code: "mismatched-lengths",
                                mark: Some(index),
                                message: format!(
                                    "heatmap row {row} has {} cells for {width} columns",
                                    cells.len()
                                ),
                            });
                        }
                    }
                }
                if let Some(labels) = row_labels {
                    if labels.len() != values.len() {
                        refusals.push(Refusal {
                            code: "mismatched-lengths",
                            mark: Some(index),
                            message: format!(
                                "heatmap declares {} row labels for {} rows",
                                labels.len(),
                                values.len()
                            ),
                        });
                    }
                }
                if let Some(labels) = col_labels {
                    if labels.len() != values.first().map_or(0, Vec::len) {
                        refusals.push(Refusal {
                            code: "mismatched-lengths",
                            mark: Some(index),
                            message: format!(
                                "heatmap declares {} column labels for {} columns",
                                labels.len(),
                                values.first().map_or(0, Vec::len)
                            ),
                        });
                    }
                }
            }
            if mark.errors().iter().any(|value| *value < 0.0) {
                refusals.push(Refusal {
                    code: "negative-error",
                    mark: Some(index),
                    message: format!("{} mark has a negative error magnitude", mark.kind()),
                });
            }
            // A marker size is a *length*, and the same rule a negative error
            // magnitude is refused by applies to it: measured, `size: -5` was
            // accepted, inspected clean and drawn — a declaration with no meaning,
            // and the renderer chose one for it.
            if let Mark::Scatter {
                size: Some(size), ..
            } = mark
            {
                if *size < 0.0 {
                    refusals.push(Refusal {
                        code: "negative-size",
                        mark: Some(index),
                        message: format!(
                            "scatter mark has a marker size of {size}, which is not a length"
                        ),
                    });
                }
            }
            if let Some(categories) = mark.categories() {
                for (series_index, length) in mark.values_per_category().iter().enumerate() {
                    if *length != categories.len() {
                        refusals.push(Refusal {
                            code: "mismatched-lengths",
                            mark: Some(index),
                            message: format!(
                                "{} mark series {series_index} has {length} values for {} categories",
                                mark.kind(),
                                categories.len()
                            ),
                        });
                    }
                }
            }
        }
        // A heatmap's rows *are* its y axis, so where the axis starts and whether
        // it is logarithmic do not mean anything for it — but they are not inert
        // either: measured, `y_min` stretches the cells (3 rows: 148.5 px each at
        // `y_min` 0.5, 178.2 at 1.0, 445.5 at 2.5, pushing the last row past the
        // canvas) and a log scale changes them (106.3 px). Drawing it anyway would
        // be a picture that follows an instruction it cannot honour, so it is
        // refused with the reason.
        if self.y_min.is_some()
            || matches!(self.y_scale, Scale::Log)
            || matches!(self.x_scale, Scale::Log)
        {
            for (index, mark) in self.marks.iter().enumerate() {
                if matches!(mark, Mark::Heatmap { .. }) {
                    refusals.push(Refusal {
                        code: "inapplicable-axis-setting",
                        mark: Some(index),
                        message: "a heatmap's rows are its y axis, so `y_min` and a log \
                                  scale cannot apply to it"
                            .to_string(),
                    });
                    break;
                }
            }
        }
        // One pair of axes carries one set of labels. Two marks declaring
        // different names for the same axis cannot both be shown, and picking one
        // would caption the other's data with the wrong names. Both axes are
        // checked: the row-only version let two heatmaps name the *columns*
        // differently and drew the second set of names over the first's data.
        for (which, axis) in [("row", 0usize), ("column", 1usize)] {
            let declared: Vec<&Vec<String>> = self
                .marks
                .iter()
                .filter_map(|mark| match mark {
                    Mark::Heatmap {
                        row_labels,
                        col_labels,
                        ..
                    } => match axis {
                        0 => row_labels.as_ref(),
                        _ => col_labels.as_ref(),
                    },
                    _ => None,
                })
                .collect();
            if let Some(first) = declared.first() {
                if declared.iter().any(|labels| *labels != *first) {
                    refusals.push(Refusal {
                        code: "ambiguous-labels",
                        mark: None,
                        message: format!(
                            "two marks declare different {which} labels for the same axis"
                        ),
                    });
                }
            }
        }
        refusals
    }

    /// A figure with no marks, for a caller building one up.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_labels(mut self, x: impl Into<String>, y: impl Into<String>) -> Self {
        self.x_label = Some(x.into());
        self.y_label = Some(y.into());
        self
    }

    pub fn with_mark(mut self, mark: Mark) -> Self {
        self.marks.push(mark);
        self
    }

    /// A convenient path type for callers that hold a `PathBuf`.
    pub fn path_of(path: impl Into<PathBuf>) -> PathBuf {
        path.into()
    }
}
