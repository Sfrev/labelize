//! Preview overlay: label outline, printable area (optionally with rounded
//! corners) and margin bands.
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
/// margins and corner radius to dots; the label is assumed to fill `content`
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
    // Printable area as a half-open dot range, clamped to the canvas.
    let x0 = dots(m.left_mm).min(width);
    let y0 = dots(m.top_mm).min(height);
    let x1 = width.saturating_sub(dots(m.right_mm)).max(x0);
    let y1 = height.saturating_sub(dots(m.bottom_mm)).max(y0);
    // Corner radii in dots, clamped so opposite corners never overlap.
    let radius = |mm: f64, span: u32| (mm * dpmm as f64).max(0.0).min(span as f64 / 2.0);
    let printable = RoundedBox {
        x0,
        y0,
        x1,
        y1,
        rx: radius(guides.radius.x_mm, x1 - x0),
        ry: radius(guides.radius.y_mm, y1 - y0),
    };

    // Margin bands, plus the corners a rounded die cut leaves off the label.
    for (x, y, px) in out.enumerate_pixels_mut() {
        if !printable.contains(x as f64 + 0.5, y as f64 + 0.5) {
            *px = blend(*px, MARGIN_TINT, MARGIN_TINT_ALPHA);
        }
    }

    let stroke = (dpmm / 8).max(1) as u32;
    let dash = dots(DASH_MM).max(1);
    if x1 > x0 && y1 > y0 {
        dashed_border(&mut out, &printable, stroke, dash, PRINTABLE);
    }
    let canvas = RoundedBox {
        x0: 0,
        y0: 0,
        x1: width,
        y1: height,
        rx: 0.0,
        ry: 0.0,
    };
    dashed_border(&mut out, &canvas, stroke, u32::MAX, OUTLINE);
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
        match self.corner_offset(px, py, l, t, r, b, rx, ry) {
            Some((dx, dy)) => (dx / rx).powi(2) + (dy / ry).powi(2) <= 1.0,
            None => true,
        }
    }

    fn contains(&self, px: f64, py: f64) -> bool {
        self.contains_inset(px, py, 0.0, 0.0)
    }

    /// Offset of the point from the centre of the corner ellipse it falls
    /// under, or `None` when it is outside every corner square.
    #[allow(clippy::too_many_arguments)]
    fn corner_offset(
        &self,
        px: f64,
        py: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        rx: f64,
        ry: f64,
    ) -> Option<(f64, f64)> {
        let dx = (l + rx - px).max(px - (r - rx));
        let dy = (t + ry - py).max(py - (b - ry));
        (dx > 0.0 && dy > 0.0).then_some((dx, dy))
    }
}

/// Border `stroke` dots thick, drawn inward from `shape`'s edge. `dash` is
/// the on/off run length; `u32::MAX` is solid. Straight runs are measured
/// from the box's top-left corner, corner arcs by their arc length.
fn dashed_border(img: &mut RgbImage, shape: &RoundedBox, stroke: u32, dash: u32, color: Rgb<u8>) {
    let on = |i: f64| dash == u32::MAX || ((i.max(0.0) as u32) / dash).is_multiple_of(2);
    let stroke_x = stroke.min(shape.x1 - shape.x0) as f64;
    let stroke_y = stroke.min(shape.y1 - shape.y0) as f64;
    let (l, t) = (shape.x0 as f64, shape.y0 as f64);
    let (r, b) = (shape.x1 as f64, shape.y1 as f64);
    let rounded = shape.rx > 0.0 && shape.ry > 0.0;
    for y in shape.y0..shape.y1 {
        for x in shape.x0..shape.x1 {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            if !shape.contains(px, py) || shape.contains_inset(px, py, stroke_x, stroke_y) {
                continue;
            }
            let arc = rounded
                .then(|| shape.corner_offset(px, py, l, t, r, b, shape.rx, shape.ry))
                .flatten();
            let lit = match arc {
                // Arc length from the angle around the corner's centre.
                Some((dx, dy)) => {
                    let angle = (dy / shape.ry).atan2(dx / shape.rx);
                    on(angle * (shape.rx + shape.ry) / 2.0)
                }
                None => {
                    let horizontal = py < t + stroke_y || py >= b - stroke_y;
                    let vertical = px < l + stroke_x || px >= r - stroke_x;
                    (horizontal && on((x - shape.x0) as f64))
                        || (vertical && on((y - shape.y0) as f64))
                }
            };
            if lit {
                img.put_pixel(x, y, color);
            }
        }
    }
}
