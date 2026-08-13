//! N-up imposition: sequential, step-and-repeat and cut-and-stack.
//!
//! Page numbers are 1-based; `None` denotes a blank sheet position.
//! Geometry is in points, PDF convention (origin bottom-left).

use serde::{Deserialize, Serialize};

/// Placement of one source page on a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellPlacement {
    pub page: Option<u32>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetLayout {
    pub sheet_number: u32,
    pub side: String,
    pub cells: Vec<CellPlacement>,
}

/// Number of physical sheets for an N-up layout.
///
/// 100 pages 4-up simplex -> ceil(100/4) = 25; 4-up duplex -> ceil(100/8) = 13.
pub fn sheet_count(page_count: u32, pages_per_sheet: u32, duplex: bool) -> Result<u32, String> {
    if page_count == 0 || pages_per_sheet == 0 {
        return Err("page_count and pages_per_sheet must be positive".into());
    }
    let per_sheet = pages_per_sheet * if duplex { 2 } else { 1 };
    Ok(page_count.div_ceil(per_sheet))
}

/// Sequential N-up: pages row-major across each sheet side.
pub fn sequential_nup(
    page_count: u32,
    rows: u32,
    cols: u32,
    duplex: bool,
) -> Result<Vec<Vec<Option<u32>>>, String> {
    if page_count == 0 || rows == 0 || cols == 0 {
        return Err("page_count, rows and cols must be positive".into());
    }
    let per_side = rows * cols;
    let mut sides_total = page_count.div_ceil(per_side);
    if duplex && sides_total % 2 == 1 {
        sides_total += 1;
    }
    Ok((0..sides_total)
        .map(|s| {
            (0..per_side)
                .map(|i| {
                    let n = s * per_side + i + 1;
                    if n <= page_count { Some(n) } else { None }
                })
                .collect()
        })
        .collect())
}

/// Repeat the same page across every position of one sheet side.
pub fn step_and_repeat(page: u32, rows: u32, cols: u32) -> Vec<u32> {
    vec![page; (rows * cols) as usize]
}

/// Total sheets to produce `copies_needed` copies at rows x cols per sheet.
pub fn step_and_repeat_sheets(copies_needed: u32, rows: u32, cols: u32) -> Result<u32, String> {
    let per_sheet = rows * cols;
    if copies_needed == 0 || per_sheet == 0 {
        return Err("copies_needed and grid must be positive".into());
    }
    Ok(copies_needed.div_ceil(per_sheet))
}

/// Best rows x columns fitting pages at 100% scale on the sheet.
///
/// Considers both page orientations; returns the grid with the most
/// copies per sheet (unrotated preferred on ties). Values in points.
pub fn optimum_grid(
    sheet_width: f64,
    sheet_height: f64,
    page_width: f64,
    page_height: f64,
    spacing: f64,
    margin: f64,
) -> (u32, u32) {
    let fit = |pw: f64, ph: f64| -> (u32, u32) {
        let avail_w = sheet_width - 2.0 * margin;
        let avail_h = sheet_height - 2.0 * margin;
        let cols = if pw > 0.0 {
            (((avail_w + spacing) / (pw + spacing)).floor()).max(0.0) as u32
        } else {
            0
        };
        let rows = if ph > 0.0 {
            (((avail_h + spacing) / (ph + spacing)).floor()).max(0.0) as u32
        } else {
            0
        };
        (rows, cols)
    };
    let upright = fit(page_width, page_height);
    let rotated = fit(page_height, page_width);
    if rotated.0 * rotated.1 > upright.0 * upright.1 {
        rotated
    } else {
        upright
    }
}

/// Cut-and-stack imposition: after cutting each sheet into rows x cols
/// piles and stacking them, reading order becomes sequential. Position
/// `p` of sheet `s` holds page `p * sheets + s + 1`.
pub fn cut_and_stack(page_count: u32, rows: u32, cols: u32) -> Result<Vec<Vec<Option<u32>>>, String> {
    if page_count == 0 || rows == 0 || cols == 0 {
        return Err("page_count, rows and cols must be positive".into());
    }
    let per_sheet = rows * cols;
    let sheets = page_count.div_ceil(per_sheet);
    Ok((0..sheets)
        .map(|s| {
            (0..per_sheet)
                .map(|p| {
                    let n = p * sheets + s + 1;
                    if n <= page_count { Some(n) } else { None }
                })
                .collect()
        })
        .collect())
}

/// Centered cell geometry for one sheet side (points). Pages are laid
/// out row-major from the top-left; PDF origin is bottom-left.
#[allow(clippy::too_many_arguments)]
pub fn grid_placements(
    pages: &[Option<u32>],
    rows: u32,
    cols: u32,
    sheet_width: f64,
    sheet_height: f64,
    cell_width: f64,
    cell_height: f64,
    spacing: f64,
    sheet_number: u32,
    side: &str,
) -> Result<SheetLayout, String> {
    if pages.len() != (rows * cols) as usize {
        return Err("pages list must contain rows*cols entries".into());
    }
    let block_w = cols as f64 * cell_width + (cols as f64 - 1.0) * spacing;
    let block_h = rows as f64 * cell_height + (rows as f64 - 1.0) * spacing;
    if block_w > sheet_width || block_h > sheet_height {
        return Err("Grid does not fit on the sheet at 100% scale".into());
    }
    let offset_x = (sheet_width - block_w) / 2.0;
    let offset_y = (sheet_height - block_h) / 2.0;
    let mut cells = Vec::with_capacity(pages.len());
    for r in 0..rows {
        for c in 0..cols {
            cells.push(CellPlacement {
                page: pages[(r * cols + c) as usize],
                x: offset_x + c as f64 * (cell_width + spacing),
                y: offset_y + (rows - 1 - r) as f64 * (cell_height + spacing),
                width: cell_width,
                height: cell_height,
            });
        }
    }
    Ok(SheetLayout {
        sheet_number,
        side: side.to_string(),
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print_calc::units::mm_to_points;

    #[test]
    fn spec_example_4up_simplex() {
        assert_eq!(sheet_count(100, 4, false).unwrap(), 25);
    }

    #[test]
    fn spec_example_4up_duplex() {
        assert_eq!(sheet_count(100, 4, true).unwrap(), 13);
    }

    #[test]
    fn sequential_4up_first_side() {
        let sides = sequential_nup(10, 2, 2, false).unwrap();
        assert_eq!(sides.len(), 3);
        assert_eq!(sides[0], vec![Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(sides[2], vec![Some(9), Some(10), None, None]);
    }

    #[test]
    fn sequential_duplex_pads_to_even_sides() {
        let sides = sequential_nup(4, 2, 2, true).unwrap();
        assert_eq!(sides.len(), 2);
        assert_eq!(sides[1], vec![None, None, None, None]);
    }

    #[test]
    fn step_and_repeat_fills_grid() {
        assert_eq!(step_and_repeat(1, 2, 2), vec![1, 1, 1, 1]);
    }

    #[test]
    fn scenario_c_a6_flyers_on_a3() {
        // A6 on A3: A3 = 297x420, A6 = 105x148 -> 2 cols x 2 rows upright,
        // but rotated (148x105) fits 2 cols x 4 rows = 8 per sheet.
        let (rows, cols) = optimum_grid(
            mm_to_points(297.0),
            mm_to_points(420.0),
            mm_to_points(105.0),
            mm_to_points(148.0),
            0.0,
            0.0,
        );
        assert_eq!(rows * cols, 8);
        // 100 flyers at 8 per sheet -> 13 sheets.
        assert_eq!(step_and_repeat_sheets(100, rows, cols).unwrap(), 13);
    }

    #[test]
    fn cut_and_stack_sequential_after_stacking() {
        // 8 pages, 2x2 -> 2 sheets. Pile p of sheet s = p*2 + s + 1.
        let sheets = cut_and_stack(8, 2, 2).unwrap();
        assert_eq!(sheets.len(), 2);
        assert_eq!(sheets[0], vec![Some(1), Some(3), Some(5), Some(7)]);
        assert_eq!(sheets[1], vec![Some(2), Some(4), Some(6), Some(8)]);
    }

    #[test]
    fn cut_and_stack_partial_final_positions() {
        let sheets = cut_and_stack(7, 2, 2).unwrap();
        assert_eq!(sheets[1], vec![Some(2), Some(4), Some(6), None]);
    }

    #[test]
    fn grid_placements_centered_2up_a4_on_a3() {
        // Scenario E: two A4 pages per A3 landscape side at 100%.
        let sheet_w = mm_to_points(420.0);
        let sheet_h = mm_to_points(297.0);
        let cell_w = mm_to_points(210.0);
        let cell_h = mm_to_points(297.0);
        let layout = grid_placements(
            &[Some(1), Some(2)],
            1,
            2,
            sheet_w,
            sheet_h,
            cell_w,
            cell_h,
            0.0,
            1,
            "front",
        )
        .unwrap();
        assert_eq!(layout.cells.len(), 2);
        assert!((layout.cells[0].x - 0.0).abs() < 1e-9);
        assert!((layout.cells[1].x - cell_w).abs() < 1e-9);
        assert!((layout.cells[0].y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn grid_rejects_oversized_pages() {
        assert!(grid_placements(&[Some(1)], 1, 1, 100.0, 100.0, 200.0, 100.0, 0.0, 1, "front").is_err());
    }
}
