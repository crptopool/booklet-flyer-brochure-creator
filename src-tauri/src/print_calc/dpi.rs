//! Effective-DPI detection and warnings.
//!
//! Image DPI (pixels per printed inch) is distinct from printer hardware
//! DPI; only the former is validated here. Vector artwork is never
//! flagged or degraded.

use serde::{Deserialize, Serialize};

use crate::print_calc::presets::{MINIMUM_PRINT_DPI, RECOMMENDED_PRINT_DPI};
use crate::print_calc::units::points_to_inches;

/// Effective DPI of an image at its final printed size (points).
/// Returns the lower of the horizontal and vertical resolutions.
pub fn effective_dpi(
    pixel_width: u32,
    pixel_height: u32,
    printed_width_pt: f64,
    printed_height_pt: f64,
) -> Result<f64, String> {
    if pixel_width == 0 || pixel_height == 0 || printed_width_pt <= 0.0 || printed_height_pt <= 0.0 {
        return Err("dimensions must be positive".into());
    }
    let dpi_x = pixel_width as f64 / points_to_inches(printed_width_pt);
    let dpi_y = pixel_height as f64 / points_to_inches(printed_height_pt);
    Ok(dpi_x.min(dpi_y))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DpiWarning {
    pub page: u32,
    pub effective_dpi: f64,
    pub minimum_dpi: f64,
    pub recommended_dpi: f64,
}

impl DpiWarning {
    pub fn message(&self) -> String {
        format!(
            "Image on Page {} will print at approximately {:.0} DPI. \
             Recommended minimum: {:.0} DPI; preferred: {:.0} DPI.",
            self.page, self.effective_dpi, self.minimum_dpi, self.recommended_dpi
        )
    }
}

/// Returns a warning when the image prints below the minimum threshold.
pub fn check_image_dpi(
    page: u32,
    pixel_width: u32,
    pixel_height: u32,
    printed_width_pt: f64,
    printed_height_pt: f64,
    minimum_dpi: Option<f64>,
    recommended_dpi: Option<f64>,
) -> Result<Option<DpiWarning>, String> {
    let minimum = minimum_dpi.unwrap_or(MINIMUM_PRINT_DPI);
    let recommended = recommended_dpi.unwrap_or(RECOMMENDED_PRINT_DPI);
    let dpi = effective_dpi(pixel_width, pixel_height, printed_width_pt, printed_height_pt)?;
    if dpi < minimum {
        Ok(Some(DpiWarning {
            page,
            effective_dpi: dpi,
            minimum_dpi: minimum,
            recommended_dpi: recommended,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print_calc::units::inches_to_points;

    #[test]
    fn dpi_at_print_size() {
        // 600x600 px printed at 2x2 in -> 300 DPI.
        let dpi = effective_dpi(600, 600, inches_to_points(2.0), inches_to_points(2.0)).unwrap();
        assert!((dpi - 300.0).abs() < 1e-9);
    }

    #[test]
    fn lower_axis_wins() {
        let dpi = effective_dpi(600, 300, inches_to_points(2.0), inches_to_points(2.0)).unwrap();
        assert!((dpi - 150.0).abs() < 1e-9);
    }

    #[test]
    fn warning_below_threshold() {
        let w = check_image_dpi(7, 236, 236, inches_to_points(2.0), inches_to_points(2.0), None, None)
            .unwrap()
            .expect("should warn");
        assert!((w.effective_dpi - 118.0).abs() < 0.01);
        assert!(w.message().contains("Page 7"));
        assert!(w.message().contains("118 DPI"));
    }

    #[test]
    fn no_warning_at_300_dpi() {
        let w = check_image_dpi(1, 600, 600, inches_to_points(2.0), inches_to_points(2.0), None, None).unwrap();
        assert!(w.is_none());
    }

    #[test]
    fn configurable_threshold() {
        let w = check_image_dpi(1, 600, 600, inches_to_points(2.0), inches_to_points(2.0), Some(400.0), None).unwrap();
        assert!(w.is_some());
    }
}
