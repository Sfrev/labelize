//! Physical label layout from a design file's `Formats/*` XML document.
//!
//! Field map, established on a ZebraDesigner 3 file (format version 50) and
//! cross-checked against the Windows DEVMODE stored beside it:
//!
//! - `Media/Width`, `Media/Height`: the page the driver prints, in µm — the
//!   label plus its margins (66000 = 66 mm for a 60 mm label with 3 mm
//!   margins; the DEVMODE paper size agrees in tenths of a millimetre).
//! - `PageLayout/ElementWidth`, `ElementHeight`: the label itself, as entered
//!   under "Label dimensions" in the designer.
//! - `PageLayout/HorizontalOffset`, `VerticalOffset`: left and top margins;
//!   right and bottom are what remains of the page around the label.
//! - `PrintScenario…/Printer/DevModeBuffer`: base64 DEVMODEW; its print
//!   quality / Y resolution give the printer dpi. The driver name
//!   (`ZDesigner ZD621-300dpi ZPL`) is the fallback.

use base64::Engine as _;
use roxmltree::{Document, Node};

use crate::elements::label_guides::LabelMargins;
use crate::error::LabelizeError;

/// Design-file lengths are stored in micrometres.
const UM_PER_MM: f64 = 1000.0;

/// What a design file says about the physical label.
///
/// The designer places the label on a page with margins around it:
/// `page = label + margins`. ZPL coordinates are relative to the page, so
/// that is the canvas to render at; the margins mark where the label sits.
#[derive(Clone, Debug, PartialEq)]
pub struct NlblLayout {
    /// Label width in mm (the designer's "Label dimensions").
    pub label_width_mm: f64,
    /// Label height in mm.
    pub label_height_mm: f64,
    /// Page width in mm: the label plus its left and right margins.
    pub page_width_mm: f64,
    /// Page height in mm: the label plus its top and bottom margins.
    pub page_height_mm: f64,
    /// Distance from each page edge to the label.
    pub margins: LabelMargins,
    /// Printer resolution in dots per inch, when the file records one.
    pub dpi: Option<u32>,
    /// Printer driver name, e.g. `ZDesigner ZD621-300dpi ZPL`.
    pub printer: Option<String>,
}

impl NlblLayout {
    /// The printer resolution in dots per millimetre (300 dpi → 12).
    pub fn dpmm(&self) -> Option<i32> {
        self.dpi.map(|dpi| (dpi as f64 / 25.4).round() as i32)
    }

    /// Page size in mm to render at [`NlblLayout::dpmm`] so the canvas has
    /// exactly the printer's dot count. The engine rasterises at whole dots
    /// per mm, but 300 dpi is 11.81: a 66 × 36 mm page is the 780 × 425 dots
    /// of the designer's `^PW780`/`^LL425`, not 792 × 432.
    pub fn render_page_mm(&self) -> (f64, f64) {
        match (self.dpi, self.dpmm()) {
            (Some(dpi), Some(dpmm)) if dpmm > 0 => {
                let dots = |mm: f64| (mm * dpi as f64 / 25.4).round() / dpmm as f64;
                (dots(self.page_width_mm), dots(self.page_height_mm))
            }
            _ => (self.page_width_mm, self.page_height_mm),
        }
    }

    /// Margins in mm to render at [`NlblLayout::dpmm`], on the same printer
    /// dots as [`NlblLayout::render_page_mm`].
    pub fn render_margins(&self) -> LabelMargins {
        let scale = match (self.dpi, self.dpmm()) {
            (Some(dpi), Some(dpmm)) if dpmm > 0 => dpi as f64 / 25.4 / dpmm as f64,
            _ => 1.0,
        };
        let m = self.margins;
        LabelMargins {
            left_mm: m.left_mm * scale,
            top_mm: m.top_mm * scale,
            right_mm: m.right_mm * scale,
            bottom_mm: m.bottom_mm * scale,
        }
    }
}

/// Read the layout from the format XML (the decrypted `Formats/*` entry).
pub fn parse_format_xml(xml: &str) -> Result<NlblLayout, LabelizeError> {
    let xml = xml.trim_start_matches('\u{feff}');
    let doc =
        Document::parse(xml).map_err(|e| LabelizeError::Parse(format!(".nlbl format XML: {e}")))?;
    let root = doc.root_element();

    let media = child(root, "Media").ok_or_else(|| missing("Media"))?;
    let page_width_mm = length_mm(media, "Width")?;
    let page_height_mm = length_mm(media, "Height")?;

    // Without a PageLayout the label fills the page.
    let (label_width_mm, label_height_mm, margins) = match child(root, "PageLayout") {
        Some(layout) => {
            let left = length_mm(layout, "HorizontalOffset")?;
            let top = length_mm(layout, "VerticalOffset")?;
            let label_w = length_mm(layout, "ElementWidth")?;
            let label_h = length_mm(layout, "ElementHeight")?;
            let margins = LabelMargins {
                left_mm: left,
                top_mm: top,
                right_mm: (page_width_mm - label_w - left).max(0.0),
                bottom_mm: (page_height_mm - label_h - top).max(0.0),
            };
            (label_w, label_h, margins)
        }
        None => (page_width_mm, page_height_mm, LabelMargins::default()),
    };
    if label_width_mm <= 0.0 || label_height_mm <= 0.0 {
        return Err(LabelizeError::Parse(format!(
            ".nlbl label size must be positive, got {label_width_mm}x{label_height_mm} mm"
        )));
    }
    if label_width_mm + margins.left_mm > page_width_mm
        || label_height_mm + margins.top_mm > page_height_mm
    {
        return Err(LabelizeError::Parse(format!(
            ".nlbl label {label_width_mm}x{label_height_mm} mm at {margins:?} does not fit \
             its {page_width_mm}x{page_height_mm} mm page"
        )));
    }

    let printer_node = root
        .descendants()
        .find(|n| n.has_tag_name("Printer") && n.attribute("Type") == Some("Printer"));
    let printer = printer_node
        .and_then(|p| text(p, "DriverName").or_else(|| text(p, "Name")))
        .map(str::to_string);
    let dpi = printer_node
        .and_then(|p| text(p, "DevModeBuffer"))
        .and_then(devmode_dpi)
        .or_else(|| printer.as_deref().and_then(dpi_from_name));

    Ok(NlblLayout {
        label_width_mm,
        label_height_mm,
        page_width_mm,
        page_height_mm,
        margins,
        dpi,
        printer,
    })
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|n| n.has_tag_name(name))
}

fn text<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    child(node, name)
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn missing(what: &str) -> LabelizeError {
    LabelizeError::Parse(format!(".nlbl format XML has no <{what}>"))
}

fn length_mm(node: Node, name: &str) -> Result<f64, LabelizeError> {
    let raw = text(node, name).ok_or_else(|| missing(name))?;
    let um: f64 = raw
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| LabelizeError::Parse(format!(".nlbl <{name}> is not a length: '{raw}'")))?;
    Ok(um / UM_PER_MM)
}

/// Printer dpi from a base64 Windows DEVMODEW: `dmPrintQuality` (positive
/// values are dpi; negative ones are draft/high presets) else `dmYResolution`.
fn devmode_dpi(b64: &str) -> Option<u32> {
    // DEVMODEW: dmDeviceName is 32 UTF-16 units, then the fixed fields.
    const DM_FIELDS: usize = 72;
    const DM_PRINT_QUALITY: usize = 90;
    const DM_Y_RESOLUTION: usize = 96;
    const DM_PRINTQUALITY_BIT: u32 = 0x0000_0400;
    const DM_YRESOLUTION_BIT: u32 = 0x0000_2000;

    let dm = base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .ok()?;
    let u16_at = |off: usize| {
        dm.get(off..off + 2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
    };
    let fields = dm
        .get(DM_FIELDS..DM_FIELDS + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))?;

    let positive = |v: Option<i16>| v.filter(|v| *v > 0).map(|v| v as u32);
    let quality = (fields & DM_PRINTQUALITY_BIT != 0)
        .then(|| positive(u16_at(DM_PRINT_QUALITY)))
        .flatten();
    let y_res = (fields & DM_YRESOLUTION_BIT != 0)
        .then(|| positive(u16_at(DM_Y_RESOLUTION)))
        .flatten();
    quality.or(y_res)
}

/// `ZDesigner ZD621-300dpi ZPL` → 300.
fn dpi_from_name(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let end = lower.find("dpi")?;
    let digits: String = lower[..end]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok().filter(|dpi| *dpi > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_from_driver_names() {
        assert_eq!(dpi_from_name("ZDesigner ZD621-300dpi ZPL"), Some(300));
        assert_eq!(dpi_from_name("ZDesigner GK420t 203 DPI"), None);
        assert_eq!(dpi_from_name("ZDesigner ZT410-203dpi ZPL"), Some(203));
        assert_eq!(dpi_from_name("Generic text"), None);
    }

    #[test]
    fn dpmm_rounds_to_the_engine_classes() {
        let with = |dpi| NlblLayout {
            label_width_mm: 1.0,
            label_height_mm: 1.0,
            page_width_mm: 1.0,
            page_height_mm: 1.0,
            margins: LabelMargins::default(),
            dpi: Some(dpi),
            printer: None,
        };
        assert_eq!(with(152).dpmm(), Some(6));
        assert_eq!(with(203).dpmm(), Some(8));
        assert_eq!(with(300).dpmm(), Some(12));
        assert_eq!(with(600).dpmm(), Some(24));
    }
}
