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
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    Scatter {
        points: Vec<[f64; 2]>,
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
}

impl Mark {
    /// Every finite number this mark draws from, for validation and scale checks.
    pub fn values(&self) -> Vec<f64> {
        match self {
            Mark::Line { points, .. } | Mark::Scatter { points, .. } => points
                .iter()
                .flat_map(|pair| pair.iter().copied())
                .collect(),
            Mark::Bar { values, .. } => values.clone(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Mark::Line { .. } => "line",
            Mark::Scatter { .. } => "scatter",
            Mark::Bar { .. } => "bar",
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
        serde_json::from_str(text).map_err(|error| at(where_, error.to_string()))
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
