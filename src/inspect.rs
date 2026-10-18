//! The checks. They read the model and never render it.
//!
//! Every problem carries a stable `code`, a severity, and enough context to act
//! on. `error` fails a build, `warn` and `info` never do — the same contract the
//! rest of this family uses, so a caller learns it once.
//!
//! The codes are deliberately few. A check that cannot say what is wrong, where,
//! and what to do instead is noise; these all can.

use serde::{Deserialize, Serialize};

use crate::model::{Figure, Mark};

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

fn bar_series(mark: &Mark) -> usize {
    match mark {
        Mark::Bar { .. } => 1,
        Mark::GroupedBar { series, .. } => series.len(),
        _ => 0,
    }
}

fn bar_value(mark: &Mark, category: usize, series_index: usize) -> Option<f64> {
    match mark {
        Mark::Bar { values, .. } => values.get(category).copied(),
        Mark::GroupedBar { series, .. } => series.get(series_index)?.values.get(category).copied(),
        _ => None,
    }
}

/// Filled bars overlap only in a shared category, intersecting subslots and on
/// the same side of zero. Sweep the two partitions rather than comparing every
/// pair of series. Separate marks use the same 0.8-wide category slot in kuva.
fn bars_overlap(figure: &Figure, earlier: usize, later: usize) -> bool {
    let (a, b) = (&figure.marks[earlier], &figure.marks[later]);
    let minimum = figure.y_min.unwrap_or(f64::NEG_INFINITY);
    let (na, nb) = (bar_series(a), bar_series(b));
    let categories = a
        .categories()
        .map_or(0, <[_]>::len)
        .min(b.categories().map_or(0, <[_]>::len));
    for category in 0..categories {
        let (mut i, mut j) = (0, 0);
        while i < na && j < nb {
            if let (Some(x), Some(y)) = (bar_value(a, category, i), bar_value(b, category, j)) {
                let low = x.min(0.0).max(y.min(0.0)).max(minimum);
                let high = x.max(0.0).min(y.max(0.0));
                if x.is_finite() && y.is_finite() && low < high {
                    return true;
                }
            }
            let end_a = (i + 1) as f64 / na as f64;
            let end_b = (j + 1) as f64 / nb as f64;
            if end_a <= end_b {
                i += 1;
            }
            if end_b <= end_a {
                j += 1;
            }
        }
    }
    false
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

    // Separate bar marks use the same categorical slots, not adjacent series.
    // This is a finding rather than a refusal: rendering shows the overplotting.
    let bars: Vec<usize> = figure
        .marks
        .iter()
        .enumerate()
        .filter(|(_, mark)| mark.categories().is_some())
        .map(|(index, _)| index)
        .collect();
    for (position, later) in bars.iter().enumerate() {
        for earlier in &bars[..position] {
            if !bars_overlap(figure, *earlier, *later) {
                continue;
            }
            problems.push(problem(
                "overlapping-bars",
                ERROR,
                format!(
                    "mark {later} has filled bars overlapping mark {earlier}'s in the same \
                     category slots; later marks are drawn over earlier ones; declare one \
                     `grouped_bar` with a series per set of values"
                ),
                Some(*later),
            ));
        }
    }

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
