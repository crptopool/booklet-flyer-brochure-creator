//! Spine-width and paper-caliper calculations.

/// Typical bulk factor: caliper_mm ~= gsm * bulk / 1000 for uncoated stock.
pub const DEFAULT_BULK_FACTOR: f64 = 1.15;

/// Approximate single-sheet thickness from grammage. Actual caliper
/// varies by manufacturer and finish; callers must allow manual override.
pub fn approximate_caliper_mm(gsm: f64, bulk_factor: f64) -> Result<f64, String> {
    if gsm <= 0.0 || bulk_factor <= 0.0 {
        return Err("gsm and bulk_factor must be positive".into());
    }
    Ok(gsm * bulk_factor / 1000.0)
}

/// Sheets = pages / 2 (each leaf prints two pages).
pub fn perfect_bound_sheet_count(page_count: u32) -> Result<u32, String> {
    if page_count == 0 {
        return Err("page_count must be positive".into());
    }
    Ok(page_count.div_ceil(2))
}

/// Spine width = number of sheets x paper caliper (mm).
pub fn spine_width_from_sheets(sheet_count: u32, caliper_mm: f64) -> Result<f64, String> {
    if sheet_count == 0 || caliper_mm <= 0.0 {
        return Err("sheet_count and caliper_mm must be positive".into());
    }
    Ok(sheet_count as f64 * caliper_mm)
}

/// Spine width = pages / 2 x paper thickness (mm).
pub fn spine_width_from_pages(page_count: u32, caliper_mm: f64) -> Result<f64, String> {
    spine_width_from_sheets(perfect_bound_sheet_count(page_count)?, caliper_mm)
}

/// Printer/manufacturer formula: spine = pages / pages-per-mm.
pub fn spine_width_custom(page_count: u32, pages_per_mm: f64) -> Result<f64, String> {
    if page_count == 0 || pages_per_mm <= 0.0 {
        return Err("page_count and pages_per_mm must be positive".into());
    }
    Ok(page_count as f64 / pages_per_mm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example_200_pages() {
        // 200 pages -> 100 sheets; spine = 100 x caliper.
        assert_eq!(perfect_bound_sheet_count(200).unwrap(), 100);
        let spine = spine_width_from_pages(200, 0.1).unwrap();
        assert!((spine - 10.0).abs() < 1e-9);
    }

    #[test]
    fn odd_page_count_rounds_up() {
        assert_eq!(perfect_bound_sheet_count(201).unwrap(), 101);
    }

    #[test]
    fn caliper_from_gsm() {
        let c = approximate_caliper_mm(80.0, DEFAULT_BULK_FACTOR).unwrap();
        assert!((c - 0.092).abs() < 1e-9);
    }

    #[test]
    fn custom_formula() {
        // e.g. printer says 500 pages per inch ~ 19.685 pages/mm.
        let spine = spine_width_custom(200, 19.685).unwrap();
        assert!((spine - 10.16).abs() < 0.01);
    }

    #[test]
    fn invalid_inputs_error() {
        assert!(spine_width_from_pages(0, 0.1).is_err());
        assert!(spine_width_from_pages(100, 0.0).is_err());
        assert!(approximate_caliper_mm(-1.0, 1.0).is_err());
    }
}
