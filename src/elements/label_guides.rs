/// Physical label margins, in millimetres from each label edge.
///
/// Printer languages describe what to print, not the stock it lands on, so
/// these come from the caller (or a label-design file) rather than from the
/// parsed data.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LabelMargins {
    pub left_mm: f64,
    pub top_mm: f64,
    pub right_mm: f64,
    pub bottom_mm: f64,
}

impl LabelMargins {
    /// The same margin on all four sides.
    pub fn uniform(mm: f64) -> Self {
        LabelMargins {
            left_mm: mm,
            top_mm: mm,
            right_mm: mm,
            bottom_mm: mm,
        }
    }

    /// Reject margins that are negative, non-finite, or leave no printable
    /// area on a `width_mm` × `height_mm` label.
    pub fn validate(&self, width_mm: f64, height_mm: f64) -> Result<(), String> {
        let sides = [self.left_mm, self.top_mm, self.right_mm, self.bottom_mm];
        if sides.iter().any(|m| !m.is_finite() || *m < 0.0) {
            return Err(format!("margins must be finite and >= 0 mm, got {self:?}"));
        }
        if self.left_mm + self.right_mm >= width_mm || self.top_mm + self.bottom_mm >= height_mm {
            return Err(format!(
                "margins {self:?} leave no printable area on a {width_mm}x{height_mm} mm label"
            ));
        }
        Ok(())
    }
}

/// Parses `"2"` (all sides) or `"top,right,bottom,left"` (CSS order), in mm.
impl std::str::FromStr for LabelMargins {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let values = s
            .split(',')
            .map(|v| {
                v.trim()
                    .parse::<f64>()
                    .map_err(|_| format!("invalid margin '{}' in '{s}'", v.trim()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        match values[..] {
            [all] => Ok(LabelMargins::uniform(all)),
            [top, right, bottom, left] => Ok(LabelMargins {
                left_mm: left,
                top_mm: top,
                right_mm: right,
                bottom_mm: bottom,
            }),
            _ => Err(format!(
                "margins take 1 value or 4 (top,right,bottom,left), got '{s}'"
            )),
        }
    }
}

/// Preview overlay drawn on top of the rendered label: the label outline,
/// the printable area inside `margins`, and a tint over the margin bands so
/// ink that strays into them stands out.
///
/// Guides are not ink. When they are enabled the PNG is encoded in colour so
/// the overlay stays distinguishable from the label content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LabelGuides {
    pub margins: LabelMargins,
}
