//! The checks. They read the model and never render it.
//!
//! Every problem carries a stable `code`, a severity, and enough context to act
//! on. `error` fails a build, `warn` and `info` never do — the same contract the
//! rest of this family uses, so a caller learns it once.
//!
//! The codes are deliberately few. A check that cannot say what is wrong, where,
//! and what to do instead is noise; these all can.

use serde::{Deserialize, Serialize};

use crate::model::Figure;

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
///
/// Two kinds of finding, and the difference matters:
///
/// * the model's own **refusals** — shapes that cannot be drawn as written, which
///   the renderer refuses from the same list;
/// * **judgement** — a figure that draws fine while being misleading, such as bars
///   that start above zero. Rendering it is how a reader sees the problem.
pub fn inspect(figure: &Figure) -> Report {
    let mut problems: Vec<Problem> = figure
        .refusals()
        .into_iter()
        .map(|refusal| problem(refusal.code, ERROR, refusal.message, refusal.mark))
        .collect();

    if let Some(min) = figure.y_min {
        // A bar's length is the value. Trimming the axis trims the length.
        if min > 0.0 {
            // One finding per categorical mark, each pointing at its own mark: a
            // figure with two bar marks fails in two places, and replacing one
            // would otherwise read as unchanged.
            for (index, mark) in figure.marks.iter().enumerate() {
                if mark.categories().is_some() {
                    problems.push(problem(
                        "truncated-baseline",
                        ERROR,
                        format!(
                            "the y axis starts at {min}, not 0, so the bars' lengths no \
                             longer mean their values"
                        ),
                        Some(index),
                    ));
                }
            }
        }
    }

    // An empty string is not a title. `is_none()` let `"title": ""` count as
    // present, so a page with a blank header passed while the other route warned.
    if figure.title.as_deref().is_none_or(str::is_empty) {
        problems.push(problem(
            "no-title",
            WARN,
            "the figure has no title, so a reader cannot tell what it shows".to_string(),
            None,
        ));
    }
    for (label, which) in [(&figure.x_label, "x"), (&figure.y_label, "y")] {
        if label.as_deref().is_none_or(str::is_empty) {
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
