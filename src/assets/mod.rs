/// Font 0 substitute: Roboto Condensed Bold, subset to the font-0 character
/// set plus the Latin Extended-A glyphs Labelary renders (see issue #65).
/// Apache-2.0; see licenses/Roboto-Condensed-Apache2.txt.
pub static FONT_ZERO_SUBSTITUTE: &[u8] = include_bytes!("fonts/RobotoCondensedBoldFont0.ttf");
pub static FONT_DEJAVU_SANS_MONO: &[u8] = include_bytes!("fonts/DejaVuSansMono.ttf");
pub static FONT_DEJAVU_SANS_MONO_BOLD: &[u8] = include_bytes!("fonts/DejaVuSansMonoBold.ttf");
pub static FONT_ZPL_GS: &[u8] = include_bytes!("fonts/ZplGSCustom.ttf");
/// Printer-resident TrueType substitute: Arimo (metric-compatible with
/// Arial/Helvetica) for Zebra's Swiss 721 `TT0003M_`. OFL-1.1; see
/// licenses/Arimo-OFL.txt.
pub static FONT_RESIDENT_SANS: &[u8] = include_bytes!("fonts/ArimoRegular.ttf");
