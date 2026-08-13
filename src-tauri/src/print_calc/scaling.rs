//! Scaling and fit-mode calculations (all values in points).

fn validate(values: &[f64]) -> Result<(), String> {
    if values.iter().any(|&v| v <= 0.0) {
        Err("dimensions must be positive".into())
    } else {
        Ok(())
    }
}

/// Uniform scale so the page fits entirely inside the target.
pub fn fit_scale(page_w: f64, page_h: f64, target_w: f64, target_h: f64) -> Result<f64, String> {
    validate(&[page_w, page_h, target_w, target_h])?;
    Ok((target_w / page_w).min(target_h / page_h))
}

/// Uniform scale so the page fully covers the target (may crop).
pub fn fill_scale(page_w: f64, page_h: f64, target_w: f64, target_h: f64) -> Result<f64, String> {
    validate(&[page_w, page_h, target_w, target_h])?;
    Ok((target_w / page_w).max(target_h / page_h))
}

/// Shrink pages larger than the target; never enlarge (max 1.0).
pub fn shrink_oversized_scale(page_w: f64, page_h: f64, target_w: f64, target_h: f64) -> Result<f64, String> {
    Ok(fit_scale(page_w, page_h, target_w, target_h)?.min(1.0))
}

/// Scale factor as a percentage.
pub fn scale_percentage(scale: f64) -> f64 {
    scale * 100.0
}

/// True when rotating the page 90 degrees yields a larger fit scale.
pub fn auto_rotate_fits_better(page_w: f64, page_h: f64, target_w: f64, target_h: f64) -> Result<bool, String> {
    Ok(fit_scale(page_h, page_w, target_w, target_h)? > fit_scale(page_w, page_h, target_w, target_h)?)
}

/// Bottom-left offset that centers the scaled page in the target.
pub fn centered_offset(page_w: f64, page_h: f64, target_w: f64, target_h: f64, scale: f64) -> (f64, f64) {
    (
        (target_w - page_w * scale) / 2.0,
        (target_h - page_h * scale) / 2.0,
    )
}

/// True when scaling would alter the intended final trim size — the
/// application must warn the user in this case.
pub fn changes_trim_size(scale: f64) -> bool {
    (scale - 1.0).abs() > 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print_calc::units::mm_to_points;

    #[test]
    fn a4_fits_a3_at_100_percent() {
        // Scenario E: A4 on half of A3 -> scale 1.0.
        let s = fit_scale(
            mm_to_points(210.0),
            mm_to_points(297.0),
            mm_to_points(210.0),
            mm_to_points(297.0),
        )
        .unwrap();
        assert!((s - 1.0).abs() < 1e-9);
        assert!(!changes_trim_size(s));
    }

    #[test]
    fn a3_shrinks_to_a4() {
        let s = fit_scale(
            mm_to_points(297.0),
            mm_to_points(420.0),
            mm_to_points(210.0),
            mm_to_points(297.0),
        )
        .unwrap();
        assert!((s - 210.0 / 297.0).abs() < 1e-9);
        assert!(changes_trim_size(s));
    }

    #[test]
    fn fill_covers_target() {
        let s = fill_scale(100.0, 200.0, 100.0, 100.0).unwrap();
        assert!((s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn shrink_never_enlarges() {
        let s = shrink_oversized_scale(50.0, 50.0, 100.0, 100.0).unwrap();
        assert_eq!(s, 1.0);
    }

    #[test]
    fn rotation_detection() {
        // Landscape page into portrait target: rotating helps.
        assert!(auto_rotate_fits_better(200.0, 100.0, 100.0, 200.0).unwrap());
        assert!(!auto_rotate_fits_better(100.0, 200.0, 100.0, 200.0).unwrap());
    }

    #[test]
    fn centered_offsets() {
        let (x, y) = centered_offset(100.0, 100.0, 200.0, 300.0, 1.0);
        assert_eq!((x, y), (50.0, 100.0));
    }

    #[test]
    fn percentage() {
        assert_eq!(scale_percentage(0.5), 50.0);
    }
}
