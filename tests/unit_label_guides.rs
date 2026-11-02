//! Label outline / margin guide overlay (`DrawerOptions::guides`).

use labelize::{render_pdf, render_png, LabelGuides, LabelMargins, LabelizeError, RenderOptions};

/// 50 × 30 mm at 8 dpmm = 400 × 240 dots; a 2 mm margin is 16 dots.
fn options(margins: Option<LabelMargins>) -> RenderOptions {
    RenderOptions {
        width_mm: Some(50.0),
        height_mm: Some(30.0),
        guides: margins.map(|margins| LabelGuides { margins }),
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
fn guides_draw_outline_printable_area_and_margin_tint() {
    let out = render_png(BOX_IN_MARGIN, options(Some(LabelMargins::uniform(2.0)))).expect("render");
    let img = image::load_from_memory(&out.bytes).unwrap();
    assert_eq!(img.color(), image::ColorType::Rgb8, "guides need colour");
    let img = img.to_rgb8();
    assert_eq!(
        img.dimensions(),
        (400, 240),
        "guides never resize the label"
    );

    // Solid outline on every edge.
    for (x, y) in [(0, 0), (399, 0), (0, 239), (399, 239), (200, 0), (0, 50)] {
        assert_eq!(img.get_pixel(x, y).0, [220, 0, 0], "outline at ({x},{y})");
    }
    // Printable-area border starts 16 dots in; dashes are 8 dots (1 mm) long.
    assert_eq!(img.get_pixel(16, 20).0, [0, 102, 255], "dash on");
    assert_eq!(img.get_pixel(16, 28).0, [255, 255, 255], "dash gap");
    assert_eq!(
        img.get_pixel(15, 28).0,
        [255, 207, 207],
        "margin side of the border"
    );
    assert_eq!(img.get_pixel(383, 20).0, [0, 102, 255], "right border");
    // Paper inside the printable area is untouched, margin paper is tinted.
    assert_eq!(img.get_pixel(200, 60).0, [255, 255, 255]);
    assert_eq!(img.get_pixel(8, 60).0, [255, 207, 207]);
    // Ink keeps its colour inside, and turns dark red where it hits the margin.
    assert_eq!(img.get_pixel(30, 120).0, [0, 0, 0]);
    assert_eq!(img.get_pixel(8, 120).0, [64, 16, 16]);
}

#[test]
fn asymmetric_margins_land_on_their_own_sides() {
    // top,right,bottom,left = 0,0,0,10 → only a left band of 80 dots.
    let margins: LabelMargins = "0,0,0,10".parse().unwrap();
    let img = rgb(&render_png(BLANK, options(Some(margins))).unwrap().bytes);
    assert_eq!(img.get_pixel(79, 60).0, [255, 207, 207], "inside left band");
    assert_eq!(
        img.get_pixel(80, 4).0,
        [0, 102, 255],
        "left border at 10 mm"
    );
    assert_eq!(img.get_pixel(81, 60).0, [255, 255, 255], "printable paper");
    assert_eq!(img.get_pixel(390, 60).0, [255, 255, 255], "no right band");
}

#[test]
fn zero_margins_draw_only_the_outline() {
    let img = rgb(&render_png(BLANK, options(Some(LabelMargins::default())))
        .unwrap()
        .bytes);
    assert_eq!(img.get_pixel(0, 0).0, [220, 0, 0]);
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
