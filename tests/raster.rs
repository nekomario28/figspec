//! What the raster route can hold, and what it says when it cannot.
//!
//! Both cases here were panics before: `width`/`height` of 4 294 967 295 is a
//! `u32`, so the model accepted it, and the backend multiplied the canvas by the
//! density and allocated `w*h*4` bytes *in `u32`* — `attempt to multiply with
//! overflow` in debug, `capacity overflow` in release, exit 101 with a Rust
//! backtrace. A marker `size` of 1e9 did the same in debug and, in *release*,
//! wrapped the mask size and drew an image nobody declared.
//!
//! The SVG route has no pixel buffer, so it still draws both. That asymmetry is
//! intended and pinned here: a raster budget is a property of this route, not of
//! the figure, so `inspect` reports nothing about it.

use figspec::{to_png, to_svg, Figure, Mark};

fn scatter(size: Option<f64>) -> Mark {
    Mark::Scatter {
        points: vec![[0.0, 1.0], [1.0, 2.0]],
        y_err: None,
        y_err_asymmetric: None,
        size,
        color: None,
        label: None,
    }
}

/// A titled figure of the given size, so the refusal is never about a missing title.
fn sized(width: Option<u32>, height: Option<u32>) -> Figure {
    Figure {
        width,
        height,
        ..Figure::new()
    }
}

#[test]
fn a_canvas_the_raster_cannot_hold_is_refused_by_name() {
    let figure = sized(Some(u32::MAX), Some(u32::MAX))
        .with_title("huge")
        .with_mark(scatter(None));
    let error = to_png(&figure).expect_err("the raster route drew a 4-billion-pixel canvas");
    assert!(error.message.contains("raster limit"), "{}", error.message);
    assert!(
        error.message.contains("8589934592x8589934592"),
        "the refusal must name the canvas it computed, got: {}",
        error.message
    );
    to_svg(&figure).expect("an SVG has no pixel buffer and must still be drawn");
}

#[test]
fn a_marker_the_raster_cannot_hold_is_refused_by_name() {
    let figure = Figure::new()
        .with_title("big marker")
        .with_mark(scatter(Some(1e9)));
    let error = to_png(&figure).expect_err("the raster route drew a 1e9-point marker");
    assert!(
        error.message.contains("marks[0].size 1e9"),
        "{}",
        error.message
    );
    assert!(error.message.contains("raster limit"), "{}", error.message);
    to_svg(&figure).expect("an SVG has no pixel buffer and must still be drawn");

    // And a size so large that the mask side this route computes is infinite: the
    // refusal must still be about the declaration, not about `inf`.
    let infinite = Figure::new()
        .with_title("1e308")
        .with_mark(scatter(Some(1e308)));
    let error = to_png(&infinite).expect_err("the raster route drew a 1e308-point marker");
    assert!(
        error.message.contains("marks[0].size 1e308"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains("not a finite size"),
        "{}",
        error.message
    );
    to_svg(&infinite).expect("an SVG has no pixel buffer and must still be drawn");

    // The library route can set what JSON cannot: serde refuses `inf` outright
    // ("number out of range"), but a caller can hand this function one.
    let infinity = Figure::new()
        .with_title("infinity")
        .with_mark(scatter(Some(f64::INFINITY)));
    assert!(
        to_png(&infinity).is_err(),
        "an infinite marker size reached the raster backend"
    );
    // The SVG route builds no mask at all, so it may draw this — what it may not
    // do is panic, which is what this call would do if that ever changed.
    let _ = to_svg(&infinity);

    // `NaN` orders last under `total_cmp`, so it is "the largest size" and must be
    // refused rather than reaching the backend.
    let nan = Figure::new()
        .with_title("nan")
        .with_mark(scatter(Some(f64::NAN)));
    assert!(
        to_png(&nan).is_err(),
        "a NaN marker size reached the backend"
    );
    let _ = to_svg(&nan);
}

#[test]
fn the_pixel_budget_is_a_bound_and_not_a_cliff() {
    // At density 2, 2000x2000 is exactly 4000x4000 = 16 000 000 pixels, the budget
    // `shadergx` uses for the same reason; one pixel more is over it.
    let at = sized(Some(2000), Some(2000))
        .with_title("at the budget")
        .with_mark(scatter(None));
    to_png(&at).expect("2000x2000 is within the budget");
    let over = sized(Some(2001), Some(2000))
        .with_title("over the budget")
        .with_mark(scatter(None));
    assert!(
        to_png(&over).is_err(),
        "2001x2000 is 4002x4000 scaled, which is over the budget"
    );
}

#[test]
fn the_marker_budget_is_the_mask_the_backend_allocates() {
    // The backend builds an `(2*ceil(r+1.5)+1)`-sided mask per radius and allocates
    // its area in bytes, so the largest marker that fits the budget is the one
    // whose mask side is 4000: size 1997 (side 3999) draws, 1998 (side 4001) does
    // not. Pinned either side of the arithmetic, not of a round number.
    let draws = Figure::new()
        .with_title("1997")
        .with_mark(scatter(Some(1997.0)));
    to_png(&draws).expect("a 1997-point marker has a 3999-pixel mask");
    let refused = Figure::new()
        .with_title("1998")
        .with_mark(scatter(Some(1998.0)));
    assert!(
        to_png(&refused).is_err(),
        "a 1998-point marker has a 4001-pixel mask, whose area is over the budget"
    );
}

#[test]
fn an_ordinary_chart_is_untouched_by_the_budget() {
    // The control: the budget must not refuse the figures this library is for.
    let figure = Figure::new()
        .with_title("ordinary")
        .with_mark(scatter(Some(30.0)))
        .with_mark(scatter(None));
    let png = to_png(&figure).expect("an ordinary figure renders");
    assert!(png.starts_with(b"\x89PNG"), "not a PNG");
}
