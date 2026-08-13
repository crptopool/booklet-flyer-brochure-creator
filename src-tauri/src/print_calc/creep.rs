//! Creep / shingling compensation for folded signatures.
//!
//! Nested folded sheets push inner sheets outward by the wrapping paper
//! thickness; trimming then moves inner content toward the fore-edge
//! unless compensated.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreepMode {
    None,
    Automatic,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreepResult {
    pub total_creep_mm: f64,
    /// Inward shift (mm) per sheet, index 0 = outermost sheet.
    pub per_sheet_offset_mm: Vec<f64>,
    pub exceeds_limit: bool,
}

/// Maximum creep at the innermost sheet. Each nested sheet displaces
/// inner sheets by two paper thicknesses (both leaves) per fold.
pub fn total_creep_mm(sheet_count: u32, caliper_mm: f64, fold_count: u32) -> Result<f64, String> {
    if sheet_count == 0 || caliper_mm <= 0.0 || fold_count == 0 {
        return Err("sheet_count, caliper_mm and fold_count must be positive".into());
    }
    Ok((sheet_count - 1) as f64 * 2.0 * caliper_mm * fold_count as f64)
}

/// Per-sheet inward compensation offsets.
pub fn creep_compensation(
    sheet_count: u32,
    caliper_mm: f64,
    fold_count: u32,
    max_creep_mm: Option<f64>,
    mode: CreepMode,
    custom_total_mm: Option<f64>,
) -> Result<CreepResult, String> {
    if mode == CreepMode::None {
        return Ok(CreepResult {
            total_creep_mm: 0.0,
            per_sheet_offset_mm: vec![0.0; sheet_count as usize],
            exceeds_limit: false,
        });
    }
    let total = match mode {
        CreepMode::Automatic => total_creep_mm(sheet_count, caliper_mm, fold_count)?,
        CreepMode::Custom => match custom_total_mm {
            Some(v) if v >= 0.0 => v,
            _ => return Err("custom mode requires a non-negative custom_total_mm".into()),
        },
        CreepMode::None => unreachable!(),
    };
    let step = if sheet_count > 1 {
        total / (sheet_count - 1) as f64
    } else {
        0.0
    };
    Ok(CreepResult {
        total_creep_mm: total,
        per_sheet_offset_mm: (0..sheet_count).map(|i| i as f64 * step).collect(),
        exceeds_limit: max_creep_mm.is_some_and(|m| total > m),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_creep_grows_with_sheets() {
        // 9 sheets (36-page A5 booklet), 0.1 mm caliper -> 1.6 mm.
        let t = total_creep_mm(9, 0.1, 1).unwrap();
        assert!((t - 1.6).abs() < 1e-9);
    }

    #[test]
    fn single_sheet_has_no_creep() {
        assert_eq!(total_creep_mm(1, 0.1, 1).unwrap(), 0.0);
    }

    #[test]
    fn automatic_offsets_are_linear() {
        let r = creep_compensation(5, 0.1, 1, None, CreepMode::Automatic, None).unwrap();
        assert_eq!(r.per_sheet_offset_mm.len(), 5);
        assert_eq!(r.per_sheet_offset_mm[0], 0.0);
        assert!((r.per_sheet_offset_mm[4] - r.total_creep_mm).abs() < 1e-9);
    }

    #[test]
    fn none_mode_yields_zero() {
        let r = creep_compensation(5, 0.1, 1, None, CreepMode::None, None).unwrap();
        assert_eq!(r.total_creep_mm, 0.0);
        assert!(r.per_sheet_offset_mm.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn custom_total_distributed() {
        let r = creep_compensation(3, 0.1, 1, None, CreepMode::Custom, Some(1.0)).unwrap();
        assert!((r.per_sheet_offset_mm[1] - 0.5).abs() < 1e-9);
        assert!((r.per_sheet_offset_mm[2] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn limit_flagged() {
        let r = creep_compensation(9, 0.1, 1, Some(1.0), CreepMode::Automatic, None).unwrap();
        assert!(r.exceeds_limit);
    }

    #[test]
    fn custom_requires_value() {
        assert!(creep_compensation(3, 0.1, 1, None, CreepMode::Custom, None).is_err());
    }
}
