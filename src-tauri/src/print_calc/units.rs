//! Measurement units and conversions.
//!
//! All internal geometry uses PDF points (1 pt = 1/72 inch) as the
//! high-precision base unit.

pub const POINTS_PER_INCH: f64 = 72.0;
pub const MM_PER_INCH: f64 = 25.4;
pub const POINTS_PER_MM: f64 = POINTS_PER_INCH / MM_PER_INCH;
pub const POINTS_PER_CM: f64 = POINTS_PER_MM * 10.0;

pub fn mm_to_points(mm: f64) -> f64 {
    mm * POINTS_PER_MM
}

pub fn points_to_mm(pt: f64) -> f64 {
    pt / POINTS_PER_MM
}

pub fn cm_to_points(cm: f64) -> f64 {
    cm * POINTS_PER_CM
}

pub fn points_to_cm(pt: f64) -> f64 {
    pt / POINTS_PER_CM
}

pub fn inches_to_points(inches: f64) -> f64 {
    inches * POINTS_PER_INCH
}

pub fn points_to_inches(pt: f64) -> f64 {
    pt / POINTS_PER_INCH
}

/// Convert between `"mm"`, `"cm"`, `"in"` and `"pt"`.
pub fn convert(value: f64, from_unit: &str, to_unit: &str) -> Result<f64, String> {
    let pt = match from_unit {
        "mm" => mm_to_points(value),
        "cm" => cm_to_points(value),
        "in" => inches_to_points(value),
        "pt" => value,
        other => return Err(format!("Unknown unit: {other}")),
    };
    match to_unit {
        "mm" => Ok(points_to_mm(pt)),
        "cm" => Ok(points_to_cm(pt)),
        "in" => Ok(points_to_inches(pt)),
        "pt" => Ok(pt),
        other => Err(format!("Unknown unit: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn inch_is_72_points() {
        assert!(close(inches_to_points(1.0), 72.0));
    }

    #[test]
    fn mm_round_trip() {
        assert!(close(points_to_mm(mm_to_points(210.0)), 210.0));
    }

    #[test]
    fn a4_width_in_points() {
        assert!((mm_to_points(210.0) - 595.2755905511812).abs() < 1e-9);
    }

    #[test]
    fn convert_mm_to_inches() {
        assert!(close(convert(25.4, "mm", "in").unwrap(), 1.0));
    }

    #[test]
    fn convert_cm_to_mm() {
        assert!(close(convert(2.0, "cm", "mm").unwrap(), 20.0));
    }

    #[test]
    fn convert_rejects_unknown_units() {
        assert!(convert(1.0, "furlong", "mm").is_err());
        assert!(convert(1.0, "mm", "furlong").is_err());
    }
}
