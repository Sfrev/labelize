//! Synthetic `.nlbl` archives for tests: a ZebraDesigner-3-shaped format
//! XML, optionally AES-encrypted with a throwaway password (never the
//! vendor's). Included with `#[path]` by the tests that need it.

use std::io::Write;

use zip::write::SimpleFileOptions;

pub const TEST_PASSWORD: &str = "test-only-password";

/// Base64 DEVMODEW carrying `dpi` in dmPrintQuality and dmYResolution.
pub fn devmode_b64(dpi: u16) -> String {
    use base64::Engine as _;
    let mut dm = vec![0u8; 220];
    dm[72..76].copy_from_slice(&0x0000_2400u32.to_le_bytes()); // DM_PRINTQUALITY | DM_YRESOLUTION
    dm[90..92].copy_from_slice(&dpi.to_le_bytes());
    dm[96..98].copy_from_slice(&dpi.to_le_bytes());
    base64::engine::general_purpose::STANDARD.encode(dm)
}

/// Format XML in µm: `media` is the page (label + margins), `element` the
/// label at `offset` (left, top); `printer` is the `<Printer>` block body.
pub fn format_xml(
    media: (u32, u32),
    element: (u32, u32),
    offset: (u32, u32),
    printer: &str,
) -> String {
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

pub fn zd621_300dpi() -> String {
    format!(
        "<DevModeBuffer>{}</DevModeBuffer>\
         <Name>ZDesigner ZD621-300dpi ZPL</Name>\
         <DriverName>ZDesigner ZD621-300dpi ZPL</DriverName>",
        devmode_b64(300)
    )
}

/// An `.nlbl`-shaped archive; `password` = None leaves the entries plain.
pub fn archive(format: &str, password: Option<&'static str>) -> Vec<u8> {
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
