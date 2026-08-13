//! Saddle-stitch booklet sequencing and page-count handling.
//!
//! Page numbers are 1-based reading-order numbers. `None` denotes a
//! blank position introduced by padding to a multiple of 4.

use serde::{Deserialize, Serialize};

/// One physical sheet of a booklet. `front` and `back` are
/// `(left, right)` page pairs as seen on each side before folding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetSpread {
    pub sheet_number: u32,
    pub front: (Option<u32>, Option<u32>),
    pub back: (Option<u32>, Option<u32>),
}

/// Blank pages required to reach the next multiple (4 for saddle stitch).
pub fn blanks_needed(page_count: u32, multiple: u32) -> Result<u32, String> {
    if page_count == 0 || multiple == 0 {
        return Err("page_count and multiple must be positive".into());
    }
    let rem = page_count % multiple;
    Ok(if rem == 0 { 0 } else { multiple - rem })
}

/// Physical sheets = total pages / 4 (after blank padding).
pub fn saddle_stitch_sheet_count(page_count: u32) -> Result<u32, String> {
    let padded = page_count + blanks_needed(page_count, 4)?;
    Ok(padded / 4)
}

/// Printer-spread order for a saddle-stitched booklet.
///
/// For an 8-page booklet:
/// ```text
/// Sheet 1 front: 8 | 1   back: 2 | 7
/// Sheet 2 front: 6 | 3   back: 4 | 5
/// ```
pub fn saddle_stitch_order(page_count: u32) -> Result<Vec<SheetSpread>, String> {
    let total = page_count + blanks_needed(page_count, 4)?;
    let page = |n: u32| if n <= page_count { Some(n) } else { None };

    Ok((0..total / 4)
        .map(|i| SheetSpread {
            sheet_number: i + 1,
            front: (page(total - 2 * i), page(2 * i + 1)),
            back: (page(2 * i + 2), page(total - 2 * i - 1)),
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlankStrategy {
    /// Append blanks after the last page.
    End,
    /// Insert blanks just before the final page so the back cover stays last.
    BeforeBackCover,
}

/// 1-based insertion indices for the blanks required for booklet folding.
///
/// The application must surface these to the user — pages are never
/// silently added.
pub fn blank_insertion_positions(
    page_count: u32,
    strategy: BlankStrategy,
) -> Result<Vec<u32>, String> {
    let count = blanks_needed(page_count, 4)?;
    Ok(match strategy {
        BlankStrategy::End => (0..count).map(|i| page_count + 1 + i).collect(),
        BlankStrategy::BeforeBackCover => vec![page_count; count as usize],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_page_booklet_matches_spec_example() {
        let spreads = saddle_stitch_order(8).unwrap();
        assert_eq!(spreads.len(), 2);
        assert_eq!(spreads[0].front, (Some(8), Some(1)));
        assert_eq!(spreads[0].back, (Some(2), Some(7)));
        assert_eq!(spreads[1].front, (Some(6), Some(3)));
        assert_eq!(spreads[1].back, (Some(4), Some(5)));
    }

    #[test]
    fn twenty_page_a5_booklet_scenario_a() {
        // Scenario A: 20-page booklet -> 5 physical sheets.
        assert_eq!(saddle_stitch_sheet_count(20).unwrap(), 5);
        let spreads = saddle_stitch_order(20).unwrap();
        assert_eq!(spreads.len(), 5);
        assert_eq!(spreads[0].front, (Some(20), Some(1)));
        assert_eq!(spreads[4].back, (Some(10), Some(11)));
    }

    #[test]
    fn thirty_two_pages_is_eight_sheets() {
        assert_eq!(saddle_stitch_sheet_count(32).unwrap(), 8);
    }

    #[test]
    fn twenty_two_pages_needs_two_blanks_scenario_b() {
        // Scenario B: 22 pages -> add 2 blanks -> 24 pages / 6 sheets.
        assert_eq!(blanks_needed(22, 4).unwrap(), 2);
        assert_eq!(saddle_stitch_sheet_count(22).unwrap(), 6);
    }

    #[test]
    fn ten_pages_needs_two_blanks() {
        assert_eq!(blanks_needed(10, 4).unwrap(), 2);
    }

    #[test]
    fn padded_positions_are_none() {
        let spreads = saddle_stitch_order(10).unwrap();
        assert_eq!(spreads.len(), 3);
        // Pages 11 and 12 don't exist -> blanks.
        assert_eq!(spreads[0].front, (None, Some(1)));
        assert_eq!(spreads[0].back, (Some(2), None));
    }

    #[test]
    fn every_page_appears_exactly_once() {
        let spreads = saddle_stitch_order(36).unwrap();
        let mut seen: Vec<u32> = spreads
            .iter()
            .flat_map(|s| [s.front.0, s.front.1, s.back.0, s.back.1])
            .flatten()
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (1..=36).collect::<Vec<_>>());
    }

    #[test]
    fn blank_positions_end() {
        assert_eq!(
            blank_insertion_positions(22, BlankStrategy::End).unwrap(),
            vec![23, 24]
        );
    }

    #[test]
    fn blank_positions_before_back_cover() {
        assert_eq!(
            blank_insertion_positions(22, BlankStrategy::BeforeBackCover).unwrap(),
            vec![22, 22]
        );
    }

    #[test]
    fn zero_pages_is_error() {
        assert!(saddle_stitch_order(0).is_err());
    }
}
