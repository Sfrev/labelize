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

/// Corner radius of a die-cut label, in millimetres. `x_mm` runs along the
/// label width and `y_mm` along its height; unequal values give elliptical
/// corners. Zero is a square corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadius {
    pub x_mm: f64,
    pub y_mm: f64,
}

impl CornerRadius {
    /// The same radius in both directions.
    pub fn uniform(mm: f64) -> Self {
        CornerRadius { x_mm: mm, y_mm: mm }
    }

    /// True when the corners are square.
    pub fn is_zero(&self) -> bool {
        self.x_mm == 0.0 && self.y_mm == 0.0
    }

    /// Reject radii that are negative or non-finite. Radii larger than half
    /// the label are clamped when drawn, as the designer does.
    pub fn validate(&self) -> Result<(), String> {
        if [self.x_mm, self.y_mm]
            .iter()
            .any(|r| !r.is_finite() || *r < 0.0)
        {
            return Err(format!(
                "corner radius must be finite and >= 0 mm, got {self:?}"
            ));
        }
        Ok(())
    }
}

/// Parses `"1"` (both directions) or `"x,y"` (horizontal, vertical), in mm.
impl std::str::FromStr for CornerRadius {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let values = s
            .split(',')
            .map(|v| {
                v.trim()
                    .parse::<f64>()
                    .map_err(|_| format!("invalid radius '{}' in '{s}'", v.trim()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        match values[..] {
            [both] => Ok(CornerRadius::uniform(both)),
            [x, y] => Ok(CornerRadius { x_mm: x, y_mm: y }),
            _ => Err(format!(
                "radius takes 1 value or 2 (horizontal,vertical), got '{s}'"
            )),
        }
    }
}

/// Preview overlay drawn on top of the rendered label: the label outline,
/// the printable area inside `margins` (with rounded corners when `radius`
/// is set), and a tint over the margin bands and cut-off corners so ink that
/// strays into them stands out.
///
/// Guides are not ink. When they are enabled the PNG is encoded in colour so
/// the overlay stays distinguishable from the label content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LabelGuides {
    pub margins: LabelMargins,
    /// Corner radius of the area inside the margins (the die-cut label).
    pub radius: CornerRadius,
}

impl LabelGuides {
    /// Validate margins and radius for a `width_mm` × `height_mm` canvas.
    pub fn validate(&self, width_mm: f64, height_mm: f64) -> Result<(), String> {
        self.margins.validate(width_mm, height_mm)?;
        self.radius.validate()
    }
}
