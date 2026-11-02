//! ZebraDesigner / NiceLabel `.nlbl` label design files.
//!
//! Unlike the printer-language frontends, this one reads no drawable
//! content: a design file only contributes the physical stock — label size,
//! margins and printer resolution — which printer languages do not carry.
//! Content still comes from the ZPL the designer exports.

pub mod container;
pub mod layout;

pub use layout::NlblLayout;

use crate::error::LabelizeError;

/// Environment variable the CLI reads the archive password from.
pub const PASSWORD_ENV: &str = "LABELIZE_NLBL_PASSWORD";

/// True when `err` comes from a missing or wrong archive password: a
/// configuration problem on the reading side rather than a bad file.
pub fn is_password_error(err: &LabelizeError) -> bool {
    matches!(err, LabelizeError::Parse(msg)
        if msg == container::WRONG_PASSWORD || msg == container::PASSWORD_REQUIRED)
}

/// Read the label layout from `.nlbl` bytes. `password` unlocks the
/// encrypted archive entries; pass an empty slice for unencrypted files.
pub fn parse_nlbl(bytes: &[u8], password: &[u8]) -> Result<NlblLayout, LabelizeError> {
    let entries = container::read_entries(bytes, password)?;
    let format = entries
        .iter()
        .find(|e| e.name.starts_with("Formats/"))
        .ok_or_else(|| LabelizeError::Parse(".nlbl has no Formats/ entry".to_string()))?;
    let xml = std::str::from_utf8(&format.data)
        .map_err(|e| LabelizeError::Parse(format!(".nlbl format is not UTF-8: {e}")))?;
    layout::parse_format_xml(xml)
}
