//! Preview overlay: label outline, printable area and margin bands.
//!
//! Drawn over the finished, flattened label (after print-width centering,
//! inversion and mirroring), so the guides describe the physical stock in
//! output coordinates regardless of how the content was transformed.

use image::{GrayImage, Rgb, RgbImage};

use crate::elements::label_guides::LabelGuides;

/// Solid outline on the label edge.
const OUTLINE: Rgb<u8> = Rgb([220, 0, 0]);
/// Dashed border of the printable area.
const PRINTABLE: Rgb<u8> = Rgb([0, 102, 255]);
/// Colour blended over the margin bands, and how strongly (0-255).
const MARGIN_TINT: Rgb<u8> = Rgb([255, 64, 64]);
const MARGIN_TINT_ALPHA: u16 = 64;
/// Dash and gap length of the printable-area border, in millimetres.
const DASH_MM: f64 = 1.0;

/// Colour the flattened label and draw `guides` over it. `dpmm` converts the
/// margins to dots; the label is assumed to fill `content` edge to edge.
pub fn overlay_guides(content: &GrayImage, guides: &LabelGuides, dpmm: i32) -> RgbImage {
    let (width, height) = content.dimensions();
    let mut out = RgbImage::from_fn(width, height, |x, y| {
        let v = content.get_pixel(x, y)[0];
        Rgb([v, v, v])
    });
    if width == 0 || height == 0 {
        return out;
    }

    let dots = |mm: f64| (mm * dpmm as f64).round().max(0.0) as u32;
    let m = guides.margins;
    // Printable area as a half-open dot range, clamped to the canvas.
    let x0 = dots(m.left_mm).min(width);
    let y0 = dots(m.top_mm).min(height);
    let x1 = width.saturating_sub(dots(m.right_mm)).max(x0);
    let y1 = height.saturating_sub(dots(m.bottom_mm)).max(y0);

    for (x, y, px) in out.enumerate_pixels_mut() {
        if x < x0 || x >= x1 || y < y0 || y >= y1 {
            *px = blend(*px, MARGIN_TINT, MARGIN_TINT_ALPHA);
        }
    }

    let stroke = (dpmm / 8).max(1) as u32;
    let dash = dots(DASH_MM).max(1);
    if x1 > x0 && y1 > y0 {
        dashed_rect(&mut out, x0, y0, x1, y1, stroke, dash, PRINTABLE);
    }
    dashed_rect(&mut out, 0, 0, width, height, stroke, u32::MAX, OUTLINE);
    out
}

fn blend(base: Rgb<u8>, tint: Rgb<u8>, alpha: u16) -> Rgb<u8> {
    let mix = |b: u8, t: u8| ((b as u16 * (255 - alpha) + t as u16 * alpha) / 255) as u8;
    Rgb([
        mix(base[0], tint[0]),
        mix(base[1], tint[1]),
        mix(base[2], tint[2]),
    ])
}

/// Rectangle border `stroke` dots thick, drawn inward from the half-open box
/// `[x0, x1) × [y0, y1)`. `dash` is the on/off run length; `u32::MAX` is solid.
#[allow(clippy::too_many_arguments)]
fn dashed_rect(
    img: &mut RgbImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    stroke: u32,
    dash: u32,
    color: Rgb<u8>,
) {
    let on = |i: u32| dash == u32::MAX || (i / dash).is_multiple_of(2);
    let stroke_x = stroke.min(x1 - x0);
    let stroke_y = stroke.min(y1 - y0);
    for x in x0..x1 {
        if on(x - x0) {
            for t in 0..stroke_y {
                img.put_pixel(x, y0 + t, color);
                img.put_pixel(x, y1 - 1 - t, color);
            }
        }
    }
    for y in y0..y1 {
        if on(y - y0) {
            for t in 0..stroke_x {
                img.put_pixel(x0 + t, y, color);
                img.put_pixel(x1 - 1 - t, y, color);
            }
        }
    }
}
