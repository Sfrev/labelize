//! Preview overlay in the style of the label designer's canvas: the page
//! (label + margins) in pale yellow, the label as white stock with a solid
//! dark border, rounded when the die cut has a corner radius.
//!
//! Drawn over the finished, flattened label (after print-width centering,
//! inversion and mirroring), so the guides describe the physical stock in
//! output coordinates regardless of how the content was transformed.

use image::{GrayImage, Rgb, RgbImage};

use crate::elements::label_guides::LabelGuides;

/// Page colour outside the label (margins and cut-off corners), sampled from
/// ZebraDesigner's canvas. Multiplied in, so white paper takes exactly this
/// colour and ink stays dark.
const PAGE: Rgb<u8> = Rgb([253, 251, 194]);
/// Thin edge around the whole page.
const PAGE_EDGE: Rgb<u8> = Rgb([190, 186, 120]);
/// Solid border of the label (the area inside the margins).
const LABEL_EDGE: Rgb<u8> = Rgb([0, 0, 0]);

/// Colour the flattened label and draw `guides` over it. `dpmm` converts the
/// margins and corner radius to dots; the page is assumed to fill `content`
/// edge to edge.
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
    // The label as a half-open dot range, clamped to the canvas.
    let x0 = dots(m.left_mm).min(width);
    let y0 = dots(m.top_mm).min(height);
    let x1 = width.saturating_sub(dots(m.right_mm)).max(x0);
    let y1 = height.saturating_sub(dots(m.bottom_mm)).max(y0);
    // Corner radii in dots, clamped so opposite corners never overlap.
    let radius = |mm: f64, span: u32| (mm * dpmm as f64).max(0.0).min(span as f64 / 2.0);
    let label = RoundedBox {
        x0,
        y0,
        x1,
        y1,
        rx: radius(guides.radius.x_mm, x1 - x0),
        ry: radius(guides.radius.y_mm, y1 - y0),
    };

    // The page shows around the label: margins and the cut-off corners.
    for (x, y, px) in out.enumerate_pixels_mut() {
        if !label.contains(x as f64 + 0.5, y as f64 + 0.5) {
            *px = multiply(*px, PAGE);
        }
    }

    let stroke = (dpmm / 6).max(1) as u32;
    if x1 > x0 && y1 > y0 {
        border(&mut out, &label, stroke, LABEL_EDGE);
    }
    let page = RoundedBox {
        x0: 0,
        y0: 0,
        x1: width,
        y1: height,
        rx: 0.0,
        ry: 0.0,
    };
    border(&mut out, &page, 1, PAGE_EDGE);
    out
}

fn multiply(base: Rgb<u8>, tint: Rgb<u8>) -> Rgb<u8> {
    let mix = |b: u8, t: u8| ((b as u16 * t as u16 + 127) / 255) as u8;
    Rgb([
        mix(base[0], tint[0]),
        mix(base[1], tint[1]),
        mix(base[2], tint[2]),
    ])
}

/// Half-open dot box `[x0, x1) × [y0, y1)` whose corners are quarter
/// ellipses with radii `rx` × `ry` dots (0 = square).
struct RoundedBox {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    rx: f64,
    ry: f64,
}

impl RoundedBox {
    /// Whether the point `(px, py)`, in dots, lies inside the box shrunk by
    /// `inset_x` / `inset_y` dots on each side (the radii shrink with it).
    fn contains_inset(&self, px: f64, py: f64, inset_x: f64, inset_y: f64) -> bool {
        let (l, t) = (self.x0 as f64 + inset_x, self.y0 as f64 + inset_y);
        let (r, b) = (self.x1 as f64 - inset_x, self.y1 as f64 - inset_y);
        if px < l || px >= r || py < t || py >= b {
            return false;
        }
        let (rx, ry) = (self.rx - inset_x, self.ry - inset_y);
        if rx <= 0.0 || ry <= 0.0 {
            return true;
        }
        // Offset from the centre of the corner ellipse, when in a corner square.
        let dx = (l + rx - px).max(px - (r - rx));
        let dy = (t + ry - py).max(py - (b - ry));
        if dx <= 0.0 || dy <= 0.0 {
            return true;
        }
        (dx / rx).powi(2) + (dy / ry).powi(2) <= 1.0
    }

    fn contains(&self, px: f64, py: f64) -> bool {
        self.contains_inset(px, py, 0.0, 0.0)
    }
}

/// Solid border `stroke` dots thick, drawn inward from `shape`'s edge.
fn border(img: &mut RgbImage, shape: &RoundedBox, stroke: u32, color: Rgb<u8>) {
    let stroke_x = stroke.min(shape.x1 - shape.x0) as f64;
    let stroke_y = stroke.min(shape.y1 - shape.y0) as f64;
    for y in shape.y0..shape.y1 {
        for x in shape.x0..shape.x1 {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            if shape.contains(px, py) && !shape.contains_inset(px, py, stroke_x, stroke_y) {
                img.put_pixel(x, y, color);
            }
        }
    }
}
