//! `.nlbl` design files: label size, margins and printer dpi.
//!
//! Fixtures are synthesised here (the format XML mirrors a ZebraDesigner 3
//! file) and encrypted with a throwaway test password, so the suite never
//! needs the vendor's archive password.
#![cfg(feature = "nlbl")]

use std::io::Write;

use labelize::parsers::nlbl::layout::parse_format_xml;
use labelize::{parse_nlbl, LabelMargins, LabelizeError};
use zip::write::SimpleFileOptions;

const TEST_PASSWORD: &str = "test-only-password";

/// Base64 DEVMODEW carrying `dpi` in dmPrintQuality and dmYResolution.
fn devmode_b64(dpi: u16) -> String {
    use base64::Engine as _;
    let mut dm = vec![0u8; 220];
    dm[72..76].copy_from_slice(&0x0000_2400u32.to_le_bytes()); // DM_PRINTQUALITY | DM_YRESOLUTION
    dm[90..92].copy_from_slice(&dpi.to_le_bytes());
    dm[96..98].copy_from_slice(&dpi.to_le_bytes());
    base64::engine::general_purpose::STANDARD.encode(dm)
}

/// Format XML in µm: `media` is the page (label + margins), `element` the
/// label at `offset` (left, top); `printer` is the `<Printer>` block body.
fn format_xml(media: (u32, u32), element: (u32, u32), offset: (u32, u32), printer: &str) -> String {
    format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>
<EuroPlus.NiceLabel Type=\"Format\">
  <Name>label</Name>
  <Media Type=\"FormatMedia\">
    <PaperFormat>256</PaperFormat>
    <Width>{}</Width>
    <Height>{}</Height>
    <MediaUnit>2</MediaUnit>
  </Media>
  <PageLayout Type=\"SingleFormatLayout\">
    <ElementWidth>{}</ElementWidth>
    <ElementHeight>{}</ElementHeight>
    <VerticalOffset>{}</VerticalOffset>
    <HorizontalOffset>{}</HorizontalOffset>
  </PageLayout>
  <PrintScenarioCollection>
    <PrintScenarioCollection Type=\"PrintScenario\">
      <Printer Type=\"Printer\">{printer}</Printer>
    </PrintScenarioCollection>
  </PrintScenarioCollection>
</EuroPlus.NiceLabel>",
        media.0, media.1, element.0, element.1, offset.1, offset.0
    )
}

fn zd621_300dpi() -> String {
    format!(
        "<DevModeBuffer>{}</DevModeBuffer>\
         <Name>ZDesigner ZD621-300dpi ZPL</Name>\
         <DriverName>ZDesigner ZD621-300dpi ZPL</DriverName>",
        devmode_b64(300)
    )
}

/// An `.nlbl`-shaped archive; `password` = None leaves the entries plain.
fn archive(format: &str, password: Option<&'static str>) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let mut opts = SimpleFileOptions::default();
    if let Some(pw) = password {
        opts = opts.with_aes_encryption(zip::AesMode::Aes256, pw);
    }
    zip.start_file("Formats/label", opts).unwrap();
    zip.write_all(format.as_bytes()).unwrap();
    zip.start_file("label.slnx", opts).unwrap();
    zip.write_all(b"<EuroPlus.NiceLabel />").unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn reads_label_page_margins_and_dpi() {
    // The designer's "Label dimensions" dialog: 60 x 30 mm label with 3 mm
    // margins on every side, which the file stores as a 66 x 36 mm page.
    let xml = format_xml(
        (66000, 36000),
        (60000, 30000),
        (3000, 3000),
        &zd621_300dpi(),
    );
    let layout = parse_format_xml(&xml).expect("parse");
    assert_eq!(
        (layout.label_width_mm, layout.label_height_mm),
        (60.0, 30.0)
    );
    assert_eq!((layout.page_width_mm, layout.page_height_mm), (66.0, 36.0));
    assert_eq!(layout.margins, LabelMargins::uniform(3.0));
    assert_eq!(layout.dpi, Some(300));
    assert_eq!(layout.dpmm(), Some(12));
    assert_eq!(
        layout.printer.as_deref(),
        Some("ZDesigner ZD621-300dpi ZPL")
    );
}

#[test]
fn right_and_bottom_margins_are_what_remains() {
    let xml = format_xml((100000, 50000), (90000, 44000), (2000, 4000), "");
    let m = parse_format_xml(&xml).unwrap().margins;
    assert_eq!(
        (m.left_mm, m.top_mm, m.right_mm, m.bottom_mm),
        (2.0, 4.0, 8.0, 2.0)
    );
}

#[test]
fn dpi_falls_back_to_the_driver_name() {
    let printer = "<DriverName>ZDesigner ZT410-203dpi ZPL</DriverName>";
    let xml = format_xml((50000, 25000), (50000, 25000), (0, 0), printer);
    let layout = parse_format_xml(&xml).unwrap();
    assert_eq!(layout.dpi, Some(203));
    assert_eq!(layout.dpmm(), Some(8));
}

#[test]
fn devmode_wins_over_the_driver_name() {
    let printer = format!(
        "<DevModeBuffer>{}</DevModeBuffer><DriverName>X-203dpi</DriverName>",
        devmode_b64(600)
    );
    let xml = format_xml((50000, 25000), (50000, 25000), (0, 0), &printer);
    assert_eq!(parse_format_xml(&xml).unwrap().dpi, Some(600));
}

#[test]
fn unknown_printer_leaves_dpi_unset() {
    let xml = format_xml((50000, 25000), (50000, 25000), (0, 0), "");
    let layout = parse_format_xml(&xml).unwrap();
    assert_eq!(
        (layout.dpi, layout.dpmm(), layout.printer),
        (None, None, None)
    );
}

#[test]
fn without_page_layout_the_label_fills_the_page() {
    let xml = format_xml((50000, 25000), (0, 0), (0, 0), "")
        .replace(&xml_between("<PageLayout", "</PageLayout>"), "");
    let layout = parse_format_xml(&xml).unwrap();
    assert_eq!(
        (layout.label_width_mm, layout.label_height_mm),
        (50.0, 25.0)
    );
    assert_eq!(layout.margins, LabelMargins::default());
}

/// The literal `<PageLayout …</PageLayout>` block of [`format_xml`].
fn xml_between(start: &str, end: &str) -> String {
    let xml = format_xml((50000, 25000), (0, 0), (0, 0), "");
    let from = xml.find(start).unwrap();
    let to = xml.find(end).unwrap() + end.len();
    xml[from..to].to_string()
}

#[test]
fn malformed_layouts_are_parse_errors() {
    let no_media = "<EuroPlus.NiceLabel Type=\"Format\" />";
    let zero = format_xml((66000, 36000), (0, 30000), (0, 0), "");
    let bad = format_xml((66000, 36000), (60000, 30000), (3000, 3000), "")
        .replace("<Width>66000</Width>", "<Width>wide</Width>");
    let too_big = format_xml((66000, 36000), (64000, 30000), (3000, 3000), "");
    for xml in [
        no_media.to_string(),
        zero,
        bad,
        too_big,
        "not xml".to_string(),
    ] {
        let err = parse_format_xml(&xml).expect_err("must fail");
        assert!(matches!(err, LabelizeError::Parse(_)), "{err}");
    }
}

#[test]
fn encrypted_archive_round_trip() {
    let xml = format_xml(
        (66000, 36000),
        (60000, 30000),
        (3000, 3000),
        &zd621_300dpi(),
    );
    let bytes = archive(&xml, Some(TEST_PASSWORD));
    let layout = parse_nlbl(&bytes, TEST_PASSWORD.as_bytes()).expect("decrypt");
    assert_eq!(
        (layout.label_width_mm, layout.page_width_mm, layout.dpmm()),
        (60.0, 66.0, Some(12))
    );
}

#[test]
fn plain_archive_needs_no_password() {
    let xml = format_xml((66000, 36000), (60000, 30000), (3000, 3000), "");
    let layout = parse_nlbl(&archive(&xml, None), b"").expect("plain zip");
    assert_eq!(layout.label_width_mm, 60.0);
}

#[test]
fn password_errors_say_what_is_wrong() {
    let xml = format_xml((66000, 36000), (60000, 30000), (3000, 3000), "");
    let bytes = archive(&xml, Some(TEST_PASSWORD));

    let wrong = parse_nlbl(&bytes, b"nope").expect_err("wrong password");
    assert!(
        wrong.to_string().contains("wrong .nlbl password"),
        "{wrong}"
    );

    let none = parse_nlbl(&bytes, b"").expect_err("no password");
    assert!(none.to_string().contains("password is required"), "{none}");
}

#[test]
fn non_archives_and_missing_formats_are_rejected() {
    let err = parse_nlbl(b"^XA^XZ", b"").expect_err("not a zip");
    assert!(err.to_string().contains("not an .nlbl"), "{err}");

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("label.slnx", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"<x/>").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let err = parse_nlbl(&bytes, b"").expect_err("no format");
    assert!(err.to_string().contains("no Formats/ entry"), "{err}");
}

#[test]
fn render_sizes_land_on_the_printers_dots() {
    // 300 dpi is 11.81 dots/mm but renders at 12: the 66 x 36 mm page must
    // still be the designer's ^PW780 / ^LL425, and 3 mm margins 35 dots.
    let xml = format_xml(
        (66000, 36000),
        (60000, 30000),
        (3000, 3000),
        &zd621_300dpi(),
    );
    let layout = parse_format_xml(&xml).unwrap();
    let dpmm = layout.dpmm().unwrap() as f64;
    let (w, h) = layout.render_page_mm();
    assert_eq!(((w * dpmm).round(), (h * dpmm).round()), (780.0, 425.0));
    let m = layout.render_margins();
    assert_eq!((m.left_mm * dpmm).round(), 35.0);
    assert_eq!((m.bottom_mm * dpmm).round(), 35.0);

    // The rendered canvas is exactly that many dots.
    let out = labelize::render_png(
        b"^XA^PW780^LL425^FO100,100^GB10,10,10^FS^XZ",
        labelize::RenderOptions {
            width_mm: Some(w),
            height_mm: Some(h),
            dpmm: dpmm as i32,
            ..Default::default()
        },
    )
    .unwrap();
    let img = image::load_from_memory(&out.bytes).unwrap();
    assert_eq!((img.width(), img.height()), (780, 425));
}

#[test]
fn whole_dpmm_printers_keep_plain_millimetres() {
    let printer = format!("<DevModeBuffer>{}</DevModeBuffer>", devmode_b64(203));
    let xml = format_xml((66000, 36000), (60000, 30000), (3000, 3000), &printer);
    let layout = parse_format_xml(&xml).unwrap();
    // 203 dpi ≈ 7.99 dots/mm: 66 mm is 527 printer dots → 65.875 mm at 8.
    assert_eq!(layout.render_page_mm().0 * 8.0, 527.0);
    assert!((layout.render_margins().left_mm - 3.0).abs() < 0.01);

    let unknown = parse_format_xml(&format_xml(
        (66000, 36000),
        (60000, 30000),
        (3000, 3000),
        "",
    ))
    .unwrap();
    assert_eq!(unknown.render_page_mm(), (66.0, 36.0));
    assert_eq!(unknown.render_margins(), LabelMargins::uniform(3.0));
}

#[test]
fn every_printer_resolution_renders_on_its_own_dots() {
    // The same 60 x 30 mm label with 3 mm margins, exported for printers of
    // different resolutions. Whatever the dpi, the canvas must be exactly the
    // page's printer dots (the designer's ^PW/^LL), so a full-width ^PW is
    // never re-centred and the margin guide sits on the printer's dots.
    for (dpi, dpmm, page_dots, margin_dots) in [
        (203u16, 8, (527, 288), 24),
        (300, 12, (780, 425), 35),
        (500, 20, (1299, 709), 59),
        (600, 24, (1559, 850), 71),
    ] {
        let printer = format!("<DevModeBuffer>{}</DevModeBuffer>", devmode_b64(dpi));
        let xml = format_xml((66000, 36000), (60000, 30000), (3000, 3000), &printer);
        let layout = parse_format_xml(&xml).unwrap();
        assert_eq!(layout.dpmm(), Some(dpmm), "{dpi} dpi");

        let (w, h) = layout.render_page_mm();
        let zpl = format!(
            "^XA^PW{}^LL{}^FO0,0^GB{},{},2^FS^XZ",
            page_dots.0, page_dots.1, page_dots.0, page_dots.1
        );
        let out = labelize::render_png(
            zpl.as_bytes(),
            labelize::RenderOptions {
                width_mm: Some(w),
                height_mm: Some(h),
                dpmm,
                ..Default::default()
            },
        )
        .unwrap();
        let img = image::load_from_memory(&out.bytes).unwrap().to_luma8();
        assert_eq!((img.width(), img.height()), page_dots, "{dpi} dpi canvas");
        // A box drawn over the whole ^PW starts at x=0: nothing was shifted.
        assert_eq!(
            img.get_pixel(0, page_dots.1 / 2)[0],
            0,
            "{dpi} dpi left edge"
        );
        assert_eq!(
            img.get_pixel(page_dots.0 - 1, page_dots.1 / 2)[0],
            0,
            "{dpi} dpi right edge"
        );
        let left = (layout.render_margins().left_mm * dpmm as f64).round() as u32;
        assert_eq!(left, margin_dots, "{dpi} dpi margin");
    }
}
