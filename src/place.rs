//! Where a mark lands in the picture.
//!
//! A finding names a mark, and a storyboard points at it. That needs the mark's
//! rectangle in pixels — and it has to be the rectangle the renderer actually
//! drew, or the picture disagrees with the report. That disagreement is the
//! failure mode every other rule here exists to prevent, so figspec does not
//! compute a layout at all.
//!
//! kuva computes it, and publishes the result: `ComputedLayout::from_layout`
//! gives the plot rectangle and the data ranges actually used, and
//! `render_multiple` draws that same layout. What follows reads kuva's numbers
//! instead of deriving them again, so there is one owner of where a mark goes.
//! `tests/storyboard.rs` holds this to kuva's own output — for every mark kind,
//! it recomputes each mark's position from these functions and requires it to
//! land on the primitive kuva drew — so a change in the engine's placement shows
//! up as a failing test rather than a box in the wrong place.

use kuva::render::layout::ComputedLayout;

use crate::model::{Figure, Mark};
use crate::render::RenderError;

/// How far a box is grown past the mark, in pixels: a mark with no extent (one
/// scatter point) still needs something to point at.
const PADDING: f64 = 4.0;

/// The plot rectangle, and the data ranges that map onto it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotArea {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub x_range: (f64, f64),
    pub y_range: (f64, f64),
    pub log_x: bool,
    pub log_y: bool,
    /// The canvas the rectangle is inside, so a label drawn on top of the picture
    /// can be kept within it. Read from the same layout as the rectangle: one
    /// source, one picture.
    pub width: f64,
    pub height: f64,
}

/// A mark's rectangle in the picture, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkBox {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl MarkBox {
    pub fn width(&self) -> f64 {
        self.right - self.left
    }

    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }
}

/// The data-space centre of category `index`.
///
/// kuva places categorical bars on a unit pitch whose first category is at 1, so
/// a bar chart's x axis runs over `(0.5, n + 0.5)`. Measured, not assumed: with
/// three bars kuva's axis range is `(0.5, 3.5)` and the drawn bar rectangles are
/// centred on 1, 2 and 3.
pub fn category_center(index: usize) -> f64 {
    index as f64 + 1.0
}

/// Half of that pitch: the slot one category occupies.
pub const CATEGORY_HALF_WIDTH: f64 = 0.5;

impl PlotArea {
    /// Read the plot rectangle and the ranges out of kuva's computed layout.
    pub fn of(computed: &ComputedLayout) -> Self {
        Self {
            left: computed.margin_left,
            top: computed.margin_top,
            right: computed.width - computed.margin_right,
            bottom: computed.height - computed.margin_bottom,
            width: computed.width,
            height: computed.height,
            x_range: computed.x_range,
            y_range: computed.y_range,
            log_x: computed.log_x,
            log_y: computed.log_y,
        }
    }

    /// Where `value` falls across `range`, as a fraction of the way along.
    fn fraction(value: f64, (low, high): (f64, f64), log: bool) -> f64 {
        // A range that is not a usable interval cannot be a mapping; every value
        // sits at one place. (A figure asking for one is refused, so this is a
        // total function rather than a case in practice.)
        if !(low.is_finite() && high.is_finite()) || high <= low {
            return 0.0;
        }
        let fraction = if log {
            // A non-positive value on a log axis is refused by the model; a
            // fraction is still owed, and zero is the honest edge.
            if value <= 0.0 || low <= 0.0 {
                return 0.0;
            }
            (value.ln() - low.ln()) / (high.ln() - low.ln())
        } else {
            (value - low) / (high - low)
        };
        fraction.clamp(0.0, 1.0)
    }

    pub fn x_pixel(&self, x: f64) -> f64 {
        self.left + Self::fraction(x, self.x_range, self.log_x) * (self.right - self.left)
    }

    pub fn y_pixel(&self, y: f64) -> f64 {
        self.bottom - Self::fraction(y, self.y_range, self.log_y) * (self.bottom - self.top)
    }

    /// The rectangle covering `mark`, or `None` when the mark has no extent to
    /// cover — a mark with nothing drawn has no location, and inventing one
    /// would put a box where the reader sees nothing.
    pub fn box_of(&self, figure: &Figure, mark_index: usize) -> Option<MarkBox> {
        let mark = figure.marks.get(mark_index)?;
        let (x_low, x_high, y_low, y_high) = self.extent(mark)?;
        // Clamp in data space first: a log axis cannot map a non-positive value,
        // and a bar's baseline is usually outside a truncated range.
        let (x_low, x_high) = clamp_pair(x_low, x_high, self.x_range);
        let (y_low, y_high) = clamp_pair(y_low, y_high, self.y_range);
        // A bar whose baseline and value map to the same pixel has no visible
        // height. Padding it would fabricate a location for an invisible mark.
        if matches!(mark, Mark::Bar { .. } | Mark::GroupedBar { .. })
            && self.y_pixel(y_low) == self.y_pixel(y_high)
        {
            return None;
        }
        let box_ = MarkBox {
            left: self.x_pixel(x_low),
            right: self.x_pixel(x_high),
            top: self.y_pixel(y_high),
            bottom: self.y_pixel(y_low),
        };
        Some(self.padded(box_))
    }

    /// The mark's extent in data space: `(x_low, x_high, y_low, y_high)`.
    fn extent(&self, mark: &Mark) -> Option<(f64, f64, f64, f64)> {
        match mark {
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
            } => {
                if points.is_empty() {
                    return None;
                }
                let mut x_low = f64::INFINITY;
                let mut x_high = f64::NEG_INFINITY;
                let mut y_low = f64::INFINITY;
                let mut y_high = f64::NEG_INFINITY;
                for (index, point) in points.iter().enumerate() {
                    x_low = x_low.min(point[0]);
                    x_high = x_high.max(point[0]);
                    // An error bar is drawn, so it is part of where the mark is:
                    // a box around the markers alone would cut the bar off.
                    let (below, above) = match (y_err, y_err_asymmetric) {
                        (Some(errors), _) if index < errors.len() => (errors[index], errors[index]),
                        (_, Some(pairs)) if index < pairs.len() => {
                            (pairs[index][0], pairs[index][1])
                        }
                        _ => (0.0, 0.0),
                    };
                    if !below.is_finite() || !above.is_finite() {
                        return None;
                    }
                    y_low = y_low.min(point[1] - below);
                    y_high = y_high.max(point[1] + above);
                }
                Some((x_low, x_high, y_low, y_high))
            }
            Mark::Bar {
                categories, values, ..
            } => {
                let (x_low, x_high) = self.category_span(categories.len())?;
                let (y_low, y_high) = value_span(values)?;
                Some((x_low, x_high, y_low, y_high))
            }
            Mark::GroupedBar { categories, series } => {
                let (x_low, x_high) = self.category_span(categories.len())?;
                let mut all: Vec<f64> = Vec::new();
                for one in series {
                    all.extend_from_slice(&one.values);
                }
                let (y_low, y_high) = value_span(&all)?;
                Some((x_low, x_high, y_low, y_high))
            }
            Mark::Heatmap { values, .. } => {
                // Categorical on both axes: a cell index is a category, and the
                // engine places the first at 1 exactly as it does for bars
                // (measured: a 2x3 heatmap computes x_range (0.5, 3.5) and
                // y_range (0.5, 2.5), and draws its cells across the whole plot).
                let columns = values.first().map_or(0, Vec::len);
                let (x_low, x_high) = self.category_span(columns)?;
                let (y_low, y_high) = self.category_span(values.len())?;
                Some((x_low, x_high, y_low, y_high))
            }
        }
    }

    /// The span of `count` categories, in data space.
    fn category_span(&self, count: usize) -> Option<(f64, f64)> {
        if count == 0 {
            return None;
        }
        let first = category_center(0) - CATEGORY_HALF_WIDTH;
        let last = category_center(count - 1) + CATEGORY_HALF_WIDTH;
        Some((first, last))
    }

    /// Grow a box so a mark that is a single point is still visible, and keep it
    /// inside the plot: a box drawn over the axis belongs to no mark.
    fn padded(&self, box_: MarkBox) -> MarkBox {
        let (left, right) = padded_pair(box_.left, box_.right, PADDING, self.left, self.right);
        let (top, bottom) = padded_pair(box_.top, box_.bottom, PADDING, self.top, self.bottom);
        MarkBox {
            left,
            top,
            right,
            bottom,
        }
    }
}

/// Values a bar can reach: a bar is drawn from the baseline, so zero is always
/// part of its extent even when the axis starts above it.
fn value_span(values: &[f64]) -> Option<(f64, f64)> {
    let mut low = 0.0f64;
    let mut high = 0.0f64;
    let mut any = false;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        low = low.min(*value);
        high = high.max(*value);
        any = true;
    }
    any.then_some((low, high))
}

fn clamp_pair(low: f64, high: f64, (range_low, range_high): (f64, f64)) -> (f64, f64) {
    (low.max(range_low), high.min(range_high))
}

/// Pad a pair to at least twice `padding` wide, then keep it within `limit`.
fn padded_pair(low: f64, high: f64, padding: f64, limit_low: f64, limit_high: f64) -> (f64, f64) {
    let (low, high) = if high - low < 2.0 * padding {
        let middle = (low + high) / 2.0;
        (middle - padding, middle + padding)
    } else {
        (low - padding, high + padding)
    };
    (low.max(limit_low), high.min(limit_high))
}

/// Each mark's box in the figure's own picture, in mark order.
///
/// `None` for a mark with nothing drawn: it has no location, and a box somewhere
/// plausible would be worse than no box.
pub fn boxes(figure: &Figure) -> Result<Vec<Option<MarkBox>>, RenderError> {
    let (_plots, layout) = crate::render::geometry(figure)?;
    let area = PlotArea::of(&ComputedLayout::from_layout(&layout));
    Ok((0..figure.marks.len())
        .map(|index| area.box_of(figure, index))
        .collect())
}
