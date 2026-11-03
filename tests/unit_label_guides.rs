//! Label outline / margin guide overlay (`DrawerOptions::guides`).

use labelize::{
    render_pdf, render_png, CornerRadius, LabelGuides, LabelMargins, LabelizeError, RenderOptions,
};

/// 50 × 30 mm at 8 dpmm = 400 × 240 dots; a 2 mm margin is 16 dots.
fn options(margins: Option<LabelMargins>) -> RenderOptions {
    RenderOptions {
        width_mm: Some(50.0),
        height_mm: Some(30.0),
        guides: margins.map(|margins| LabelGuides {
            margins,
            ..LabelGuides::default()
        }),
        ..RenderOptions::default()
    }
}

/// A near-empty label: one dot at (200,200), away from every probed pixel
/// (a format with no fields emits no label at all).
const BLANK: &[u8] = b"^XA^FO200,200^GB1,1,1^FS^XZ";

/// A solid 40-dot box straddling the 16-dot left margin.
const BOX_IN_MARGIN: &[u8] = b"^XA^FO0,100^GB40,40,40^FS^XZ";

fn rgb(png: &[u8]) -> image::RgbImage {
    image::load_from_memory(png).expect("decode png").to_rgb8()
}

#[test]
fn without_guides_output_stays_greyscale() {
    let out = render_png(BOX_IN_MARGIN, options(None)).expect("render");
    let img = image::load_from_memory(&out.bytes).unwrap();
    assert_eq!(img.color(), image::ColorType::L8);
}

#[test]
fn guides_draw_page_label_border_and_margins() {
    let out = render_png(BOX_IN_MARGIN, options(Some(LabelMargins::uniform(2.0)))).expect("render");
    let img = image::load_from_memory(&out.bytes).unwrap();
    assert_eq!(img.color(), image::ColorType::Rgb8, "guides need colour");
    let img = img.to_rgb8();
    assert_eq!(
        img.dimensions(),
        (400, 240),
        "guides never resize the label"
    );

    // Thin page edge all round.
    for (x, y) in [(0, 0), (399, 0), (0, 239), (399, 239), (200, 0), (0, 50)] {
        assert_eq!(
            img.get_pixel(x, y).0,
            [190, 186, 120],
            "page edge at ({x},{y})"
        );
    }
    // Solid label border 16 dots in (no dashes), page colour just outside it.
    assert_eq!(img.get_pixel(16, 20).0, [0, 0, 0], "left border");
    assert_eq!(img.get_pixel(16, 28).0, [0, 0, 0], "border is solid");
    assert_eq!(
        img.get_pixel(15, 28).0,
        [253, 251, 194],
        "margin side of the border"
    );
    assert_eq!(img.get_pixel(383, 20).0, [0, 0, 0], "right border");
    // Paper on the label stays white; margin paper takes the page yellow.
    assert_eq!(img.get_pixel(200, 60).0, [255, 255, 255]);
    assert_eq!(img.get_pixel(8, 60).0, [253, 251, 194]);
    // Ink stays black on the label and in the margin.
    assert_eq!(img.get_pixel(30, 120).0, [0, 0, 0]);
    assert_eq!(img.get_pixel(8, 120).0, [0, 0, 0]);
}

#[test]
fn asymmetric_margins_land_on_their_own_sides() {
    // top,right,bottom,left = 0,0,0,10 → only a left band of 80 dots.
    let margins: LabelMargins = "0,0,0,10".parse().unwrap();
    let img = rgb(&render_png(BLANK, options(Some(margins))).unwrap().bytes);
    assert_eq!(img.get_pixel(79, 60).0, [253, 251, 194], "inside left band");
    assert_eq!(img.get_pixel(80, 4).0, [0, 0, 0], "left border at 10 mm");
    assert_eq!(img.get_pixel(81, 60).0, [255, 255, 255], "printable paper");
    assert_eq!(img.get_pixel(390, 60).0, [255, 255, 255], "no right band");
}

#[test]
fn zero_margins_draw_only_the_outline() {
    let img = rgb(&render_png(BLANK, options(Some(LabelMargins::default())))
        .unwrap()
        .bytes);
    assert_eq!(img.get_pixel(0, 0).0, [190, 186, 120]);
    assert_eq!(img.get_pixel(1, 1).0, [255, 255, 255]);
}

#[test]
fn invalid_margins_are_rejected_as_input_errors() {
    for margins in [
        LabelMargins::uniform(-1.0),
        LabelMargins::uniform(f64::NAN),
        LabelMargins::uniform(25.0), // 25 + 25 = full 50 mm width
    ] {
        let err = render_png(BLANK, options(Some(margins))).expect_err("must fail");
        assert!(matches!(err, LabelizeError::Parse(_)), "{margins:?}: {err}");
    }
}

#[test]
fn margins_parse_from_one_or_four_values() {
    assert_eq!("2".parse::<LabelMargins>(), Ok(LabelMargins::uniform(2.0)));
    assert_eq!(
        " 1, 2 ,3,4".parse::<LabelMargins>(),
        Ok(LabelMargins {
            top_mm: 1.0,
            right_mm: 2.0,
            bottom_mm: 3.0,
            left_mm: 4.0,
        })
    );
    assert!("1,2".parse::<LabelMargins>().is_err());
    assert!("x".parse::<LabelMargins>().is_err());
    assert!("".parse::<LabelMargins>().is_err());
}

#[test]
fn pdf_with_guides_embeds_colour() {
    let plain = render_pdf(BOX_IN_MARGIN, options(None)).unwrap().bytes;
    let guided = render_pdf(BOX_IN_MARGIN, options(Some(LabelMargins::uniform(2.0))))
        .unwrap()
        .bytes;
    let has = |pdf: &[u8], needle: &[u8]| pdf.windows(needle.len()).any(|w| w == needle);
    assert!(has(&plain, b"DeviceGray") && !has(&plain, b"DeviceRGB"));
    assert!(has(&guided, b"DeviceRGB"));
}

/// 2 mm margins plus a corner radius on the 50 × 30 mm label.
fn rounded(radius: CornerRadius) -> RenderOptions {
    RenderOptions {
        guides: Some(LabelGuides {
            margins: LabelMargins::uniform(2.0),
            radius,
        }),
        ..options(None)
    }
}

#[test]
fn rounded_corners_tint_the_cut_off_corner() {
    // 2 mm radius = 16 dots: the label's corner box is (16..32, 16..32).
    let img = rgb(&render_png(BLANK, rounded(CornerRadius::uniform(2.0)))
        .unwrap()
        .bytes);
    // The corner tip is off the die-cut label, so it shows the page colour.
    assert_eq!(img.get_pixel(17, 17).0, [253, 251, 194], "cut-off corner");
    assert_eq!(
        img.get_pixel(382, 222).0,
        [253, 251, 194],
        "opposite corner"
    );
    // Just inside the arc is paper.
    assert_eq!(img.get_pixel(26, 26).0, [255, 255, 255], "inside the arc");
    // The border follows the arc through the corner square, and never
    // touches the square's outer tip.
    let arc_dots = (16..32)
        .flat_map(|y| (16..32).map(move |x| (x, y)))
        .filter(|&(x, y)| img.get_pixel(x, y).0 == [0, 0, 0])
        .count();
    assert!(arc_dots > 0, "border follows the arc");
    assert_ne!(img.get_pixel(16, 16).0, [0, 0, 0], "no square corner");
    // Straight edges are unchanged: the top border still starts 16 dots down.
    assert_eq!(img.get_pixel(48, 16).0, [0, 0, 0], "straight edge");
}

#[test]
fn zero_radius_matches_square_guides() {
    let square = render_png(BLANK, options(Some(LabelMargins::uniform(2.0)))).unwrap();
    let zero = render_png(BLANK, rounded(CornerRadius::default())).unwrap();
    assert_eq!(square.bytes, zero.bytes);
}

#[test]
fn oversized_radius_is_clamped_to_half_the_label() {
    // 100 mm far exceeds the 46 × 26 mm printable area: the corners become
    // half-ellipses rather than overlapping, and the centre stays paper.
    let img = rgb(&render_png(BLANK, rounded(CornerRadius::uniform(100.0)))
        .unwrap()
        .bytes);
    assert_eq!(img.get_pixel(200, 120).0, [255, 255, 255], "centre");
    assert_eq!(img.get_pixel(20, 20).0, [253, 251, 194], "corner");
}

#[test]
fn invalid_radius_is_an_input_error() {
    for radius in [CornerRadius::uniform(-1.0), CornerRadius::uniform(f64::NAN)] {
        let err = render_png(BLANK, rounded(radius)).expect_err("must fail");
        assert!(matches!(err, LabelizeError::Parse(_)), "{radius:?}: {err}");
    }
}

#[test]
fn radius_parses_from_one_or_two_values() {
    assert_eq!("1".parse::<CornerRadius>(), Ok(CornerRadius::uniform(1.0)));
    assert_eq!(
        "1, 2".parse::<CornerRadius>(),
        Ok(CornerRadius {
            x_mm: 1.0,
            y_mm: 2.0
        })
    );
    assert!("1,2,3".parse::<CornerRadius>().is_err());
    assert!("x".parse::<CornerRadius>().is_err());
}
