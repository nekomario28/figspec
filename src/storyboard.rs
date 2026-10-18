//! The figure with the findings pointed at.
//!
//! `inspect` says *what* is wrong and, when it is about a mark, *which* mark. A
//! reader still has to find that mark in the picture. So the storyboard draws the
//! figure, boxes the marks the findings name, and numbers each box with the
//! finding's position in the report — the same numbers `figspec inspect` prints,
//! so the two can be read against each other without a lookup table.
//!
//! Two rules, from the rest of this family:
//!
//! * **The box is the mark's real rectangle.** It comes from kuva's own computed
//!   layout (`crate::place`), not from a redeclared guess.
//! * **A finding with no mark gets no box.** It is listed in the report, and the
//!   storyboard says nothing about it rather than drawing a box somewhere
//!   plausible. Silence is never a verdict, but neither is a fabricated pointer.

use kuva::render::layout::ComputedLayout;

use crate::inspect::{Report, ERROR};
use crate::model::Figure;
use crate::place::{MarkBox, PlotArea};
use crate::render::{geometry, normalized_svg, RenderError};

/// A box drawn on the storyboard, and the findings it points at.
#[derive(Debug, Clone, PartialEq)]
pub struct Boxed {
    pub mark: usize,
    /// Positions in `inspect`'s report (1-based), in report order.
    pub numbers: Vec<usize>,
    /// At least one of `numbers` is an `error`; the box is coloured for the worst.
    pub is_error: bool,
    pub rect: MarkBox,
}

/// A drawn figure, and what was pointed at in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Storyboard {
    pub svg: String,
    pub boxes: Vec<Boxed>,
    /// Findings the report carries that are about no particular mark, by report
    /// position. They are not boxed, deliberately.
    pub unlocated: Vec<usize>,
    /// Findings that named a mark this figure has no box for, by report position:
    /// the figure has no such mark, or the mark is one it draws nothing for.
    ///
    /// Caller-supplied reports can refer to a different figure. Naming a missing
    /// mark is not the same as naming no mark.
    pub unboxable: Vec<usize>,
}

const ERROR_COLOR: &str = "#c0392b";
const OTHER_COLOR: &str = "#2c6fbb";
/// The plate a number is drawn on, in pixels.
const PLATE_HEIGHT: f64 = 16.0;

/// Draw `figure`, boxing every mark `report` names.
///
/// The report is an argument rather than something this calls, so a storyboard is
/// a pure function of a figure and a report: the same two inputs, the same bytes.
/// It also means a report from somewhere else — a different inspector, a review,
/// a baseline run — can be drawn over the figure it is about.
pub fn of(figure: &Figure, report: &Report) -> Result<Storyboard, RenderError> {
    let (plots, layout) = geometry(figure)?;
    let computed = ComputedLayout::from_layout(&layout);
    let area = PlotArea::of(&computed);
    let svg = normalized_svg(
        kuva::backend::svg::SvgBackend
            .render_scene(&kuva::render::render::render_multiple(plots, layout)),
    );

    // Group by mark, in report order, so one mark's findings share one box: two
    // identical rectangles on top of each other is a worse picture, not more
    // information.
    let mut boxes: Vec<Boxed> = Vec::new();
    let mut unlocated: Vec<usize> = Vec::new();
    let mut unboxable: Vec<usize> = Vec::new();
    for (index, problem) in report.problems.iter().enumerate() {
        let number = index + 1;
        let Some(mark) = problem.mark else {
            unlocated.push(number);
            continue;
        };
        let Some(rect) = area.box_of(figure, mark) else {
            // Do not invent a location for an invisible or missing mark.
            unboxable.push(number);
            continue;
        };
        match boxes.iter_mut().find(|boxed| boxed.mark == mark) {
            Some(boxed) => {
                boxed.numbers.push(number);
                boxed.is_error |= problem.severity == ERROR;
            }
            None => boxes.push(Boxed {
                mark,
                numbers: vec![number],
                is_error: problem.severity == ERROR,
                rect,
            }),
        }
    }

    let mut svg = svg;
    let overlay = overlay(&boxes, &area)?;
    if let Some(end) = svg.rfind("</svg>") {
        svg.insert_str(end, &overlay);
    }
    Ok(Storyboard {
        svg,
        boxes,
        unlocated,
        unboxable,
    })
}

/// The boxes and their numbers, as SVG to place on top of the figure.
fn overlay(boxes: &[Boxed], area: &PlotArea) -> Result<String, RenderError> {
    let mut out = String::from("<g id=\"figspec-storyboard\">\n");
    for boxed in boxes {
        let color = if boxed.is_error {
            ERROR_COLOR
        } else {
            OTHER_COLOR
        };
        out.push_str(&format!(
            "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" \
             fill=\"none\" stroke=\"{color}\" stroke-width=\"2\"/>\n",
            boxed.rect.left,
            boxed.rect.top,
            boxed.rect.width(),
            boxed.rect.height()
        ));
        let label: String = boxed
            .numbers
            .iter()
            .map(|number| number.to_string())
            .collect::<Vec<_>>()
            .join(",");
        // Move the label, not the measured box. A 16 px plate above a plot with
        // a 10 px top margin lands at y=-6 unless clamped. Several findings on
        // one mark can also make the plate wider than the right margin.
        let plate = 8.0 + 7.0 * label.len() as f64;
        if plate > area.width || PLATE_HEIGHT > area.height {
            return Err(RenderError {
                message: format!(
                    "mark {}'s number plate needs {plate:.0}x{PLATE_HEIGHT:.0} px, \
                     larger than the {:.0}x{:.0} px canvas; declare a larger canvas",
                    boxed.mark, area.width, area.height
                ),
            });
        }
        let x = boxed.rect.left.clamp(0.0, (area.width - plate).max(0.0));
        let y = (boxed.rect.top - PLATE_HEIGHT).clamp(0.0, (area.height - PLATE_HEIGHT).max(0.0));
        out.push_str(&format!(
            "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{plate:.2}\" height=\"{PLATE_HEIGHT:.0}\" \
             fill=\"{color}\"/>\n"
        ));
        out.push_str(&format!(
            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"sans-serif\" font-size=\"11\" \
             fill=\"white\" text-anchor=\"middle\">{label}</text>\n",
            x + plate / 2.0,
            y + 11.5
        ));
    }
    out.push_str("</g>\n");
    Ok(out)
}
