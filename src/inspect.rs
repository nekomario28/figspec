//! The checks. They read the model and never render it.
//!
//! Every problem carries a stable `code`, a severity, and enough context to act
//! on. `error` fails a build, `warn` and `info` never do — the same contract the
//! rest of this family uses, so a caller learns it once.
//!
//! The codes are deliberately few. A check that cannot say what is wrong, where,
//! and what to do instead is noise; these all can.

use serde::{Deserialize, Serialize};

use crate::model::{Figure, Mark, Scale};

/// `error` is the only severity that fails a build.
pub const ERROR: &str = "error";
pub const WARN: &str = "warn";
pub const INFO: &str = "info";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Problem {
    pub code: String,
    pub severity: String,
    pub message: String,
    /// Which mark (0-based) the problem is about, when it is about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub problems: Vec<Problem>,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.problems
            .iter()
            .filter(|problem| problem.severity == ERROR)
            .count()
    }

    /// 1 when anything would fail a build, 0 otherwise.
    pub fn exit_code(&self) -> i32 {
        i32::from(self.errors() > 0)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a report always serializes")
    }
}

fn problem(code: &str, severity: &str, message: String, mark: Option<usize>) -> Problem {
    Problem {
        code: code.to_string(),
        severity: severity.to_string(),
        message,
        mark,
    }
}

/// Inspect a figure.
pub fn inspect(figure: &Figure) -> Report {
    let mut problems = Vec::new();

    if figure.marks.is_empty() {
        problems.push(problem(
            "empty-figure",
            ERROR,
            "the figure declares no marks, so there is nothing to read".to_string(),
            None,
        ));
    }

    for (index, mark) in figure.marks.iter().enumerate() {
        let values = mark.values();
        if let Some(bad) = values.iter().find(|value| !value.is_finite()) {
            problems.push(problem(
                "non-finite-value",
                ERROR,
                format!("{} mark draws {bad}, which cannot be placed", mark.kind()),
                Some(index),
            ));
        }

        if let Mark::Bar {
            categories, values, ..
        } = mark
        {
            if categories.len() != values.len() {
                problems.push(problem(
                    "mismatched-lengths",
                    ERROR,
                    format!(
                        "bar mark has {} categories but {} values",
                        categories.len(),
                        values.len()
                    ),
                    Some(index),
                ));
            }
            match figure.y_min {
                // A bar's length is the value. Trimming the axis trims the length.
                Some(min) if min > 0.0 => problems.push(problem(
                    "truncated-baseline",
                    ERROR,
                    format!(
                        "bars start at y={min}, not 0, so their lengths no longer mean their values"
                    ),
                    Some(index),
                )),
                _ => {}
            }
        }

        if matches!(figure.x_scale, Scale::Log) || matches!(figure.y_scale, Scale::Log) {
            if let Some(bad) = values.iter().find(|value| **value <= 0.0) {
                problems.push(problem(
                    "non-positive-on-log-scale",
                    ERROR,
                    format!("a log axis cannot place {bad}"),
                    Some(index),
                ));
            }
        }
    }

    if figure.title.is_none() {
        problems.push(problem(
            "no-title",
            WARN,
            "the figure has no title, so a reader cannot tell what it shows".to_string(),
            None,
        ));
    }
    for (label, which) in [(&figure.x_label, "x"), (&figure.y_label, "y")] {
        if label.is_none() {
            problems.push(problem(
                "no-axis-label",
                INFO,
                format!("the {which} axis has no label"),
                None,
            ));
        }
    }

    Report { problems }
}
