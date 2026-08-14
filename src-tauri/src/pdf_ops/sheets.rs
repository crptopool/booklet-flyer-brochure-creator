//! Turn a booklet plan into the physical sheet sides that will be printed.
//!
//! This is the bridge between the deterministic calculations and the
//! imposition renderer: it decides which source page lands in which cell
//! of which sheet side, and where that cell sits on the paper.

use crate::pdf_ops::impose::{Placement, SheetSide};
use crate::print_calc::booklet::saddle_stitch_order;
use crate::print_calc::imposition::{grid_placements, sequential_nup};
use crate::print_calc::plan::BookletPlan;
use crate::print_calc::units::mm_to_points;

/// Rows and columns for a given number of pages on one sheet side.
fn grid_for(pages_per_side: u32) -> Result<(u32, u32), String> {
    match pages_per_side {
        1 => Ok((1, 1)),
        2 => Ok((1, 2)),
        4 => Ok((2, 2)),
        6 => Ok((2, 3)),
        8 => Ok((2, 4)),
        9 => Ok((3, 3)),
        16 => Ok((4, 4)),
        n => Err(format!("{n} pages per side is not a supported grid")),
    }
}

/// Build every sheet side for a plan.
///
/// `trim` and `sheet` are (width, height) in millimetres. The sheet is
/// used exactly as given — call sites decide orientation.
pub fn sheets_for_plan(
    plan: &BookletPlan,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
) -> Result<Vec<SheetSide>, String> {
    let (rows, cols) = grid_for(plan.pages_per_side)?;
    let cell_w = mm_to_points(trim_mm.0);
    let cell_h = mm_to_points(trim_mm.1);
    let sheet_w = mm_to_points(sheet_mm.0);
    let sheet_h = mm_to_points(sheet_mm.1);

    if cell_w <= 0.0 || cell_h <= 0.0 || sheet_w <= 0.0 || sheet_h <= 0.0 {
        return Err("page and sheet sizes must be positive".into());
    }

    // Folded bindings only have a deterministic printer-spread order for
    // the classic single fold. Anything else would need a signature
    // imposition we do not compute, and guessing would silently reorder
    // the user's pages.
    if plan.profile.folded && !plan.uses_printer_spreads {
        return Err(format!(
            "{} folds its sheets, so imposition needs 2 pages per side printed double-sided. \
             Change the printing configuration, or choose a binding that does not fold.",
            plan.profile.name
        ));
    }

    let duplex = plan.pages_per_sheet > plan.pages_per_side;
    let back_rotation = plan.duplex.back_side_rotation;

    // Sequence: which source page goes in which cell of which side.
    // `None` is a blank position.
    let sides: Vec<(u32, String, Vec<Option<u32>>)> = if plan.uses_printer_spreads {
        let spreads = saddle_stitch_order(plan.source_pages)?;
        let mut out = Vec::with_capacity(spreads.len() * 2);
        for s in spreads {
            out.push((s.sheet_number, "front".to_string(), vec![s.front.0, s.front.1]));
            out.push((s.sheet_number, "back".to_string(), vec![s.back.0, s.back.1]));
        }
        out
    } else {
        let seq = sequential_nup(plan.source_pages, rows, cols, duplex)?;
        seq.into_iter()
            .enumerate()
            .map(|(i, cells)| {
                let sheet_no = if duplex { (i as u32) / 2 + 1 } else { i as u32 + 1 };
                let side = if duplex && i % 2 == 1 { "back" } else { "front" };
                (sheet_no, side.to_string(), cells)
            })
            .collect()
    };

    let mut result = Vec::with_capacity(sides.len());
    for (sheet_no, side_name, cells) in sides {
        let layout = grid_placements(
            &cells, rows, cols, sheet_w, sheet_h, cell_w, cell_h, 0.0, sheet_no, &side_name,
        )?;

        // Back sides carry the rotation needed to survive the flip.
        let rotation = if side_name == "back" { back_rotation } else { 0 };

        // Fold lines run down the internal column divisions.
        let block_w = cols as f64 * cell_w;
        let offset_x = (sheet_w - block_w) / 2.0;
        let fold_x: Vec<f64> = if plan.folds_per_sheet > 0 {
            (1..cols).map(|c| offset_x + c as f64 * cell_w).collect()
        } else {
            vec![]
        };

        result.push(SheetSide {
            sheet_number: sheet_no,
            side: side_name,
            width: sheet_w,
            height: sheet_h,
            placements: layout
                .cells
                .iter()
                .map(|c| Placement::from_cell(c, rotation))
                .collect(),
            fold_x,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print_calc::plan::booklet_plan;
    use crate::print_calc::presets::{BindingType, DuplexMode};

    const A5: (f64, f64) = (148.0, 210.0);
    const A4_LANDSCAPE: (f64, f64) = (297.0, 210.0);
    const A4: (f64, f64) = (210.0, 297.0);

    #[test]
    fn saddle_stitch_produces_printer_spreads() {
        let plan = booklet_plan(BindingType::SaddleStitch, 8, 2, DuplexMode::ShortEdge, true, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A5, A4_LANDSCAPE).unwrap();
        assert_eq!(sides.len(), 4);
        // Spec example: sheet 1 front is 8 | 1, back is 2 | 7.
        let pages = |s: &SheetSide| s.placements.iter().map(|p| p.page).collect::<Vec<_>>();
        assert_eq!(pages(&sides[0]), vec![Some(8), Some(1)]);
        assert_eq!(pages(&sides[1]), vec![Some(2), Some(7)]);
        assert_eq!(pages(&sides[2]), vec![Some(6), Some(3)]);
        assert_eq!(pages(&sides[3]), vec![Some(4), Some(5)]);
    }

    #[test]
    fn every_page_is_placed_exactly_once() {
        let plan = booklet_plan(BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A5, A4_LANDSCAPE).unwrap();
        let mut seen: Vec<u32> = sides
            .iter()
            .flat_map(|s| s.placements.iter().filter_map(|p| p.page))
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (1..=20).collect::<Vec<_>>());
    }

    #[test]
    fn cells_are_centred_and_side_by_side() {
        let plan = booklet_plan(BindingType::SaddleStitch, 4, 2, DuplexMode::ShortEdge, true, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A5, A4_LANDSCAPE).unwrap();
        let cells = &sides[0].placements;
        assert_eq!(cells.len(), 2);
        // Two A5 cells on an A4 landscape sheet leave a 1 mm total margin.
        assert!((cells[1].x - cells[0].x - mm_to_points(148.0)).abs() < 1e-6);
        assert!((cells[0].x - mm_to_points(0.5)).abs() < 1e-6);
    }

    #[test]
    fn fold_line_sits_on_the_centre_of_the_sheet() {
        let plan = booklet_plan(BindingType::SaddleStitch, 4, 2, DuplexMode::ShortEdge, true, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A5, A4_LANDSCAPE).unwrap();
        assert_eq!(sides[0].fold_x.len(), 1);
        assert!((sides[0].fold_x[0] - mm_to_points(148.5)).abs() < 1e-6);
    }

    #[test]
    fn back_sides_carry_the_duplex_rotation() {
        // Long-edge flip on a landscape sheet inverts the back.
        let plan = booklet_plan(BindingType::SaddleStitch, 8, 2, DuplexMode::LongEdge, true, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A5, A4_LANDSCAPE).unwrap();
        assert_eq!(sides[0].placements[0].rotation, 0);
        assert_eq!(sides[1].placements[0].rotation, 180);
    }

    #[test]
    fn correct_flip_needs_no_rotation() {
        let plan = booklet_plan(BindingType::SaddleStitch, 8, 2, DuplexMode::ShortEdge, true, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A5, A4_LANDSCAPE).unwrap();
        assert!(sides.iter().all(|s| s.placements.iter().all(|p| p.rotation == 0)));
    }

    #[test]
    fn perfect_binding_keeps_reading_order_one_per_side() {
        let plan = booklet_plan(BindingType::Perfect, 4, 1, DuplexMode::LongEdge, false, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A4, A4).unwrap();
        assert_eq!(sides.len(), 4);
        let pages: Vec<_> = sides.iter().map(|s| s.placements[0].page).collect();
        assert_eq!(pages, vec![Some(1), Some(2), Some(3), Some(4)]);
        // Two leaves, front and back each.
        assert_eq!(sides[0].sheet_number, 1);
        assert_eq!(sides[1].side, "back");
        assert_eq!(sides[2].sheet_number, 2);
    }

    #[test]
    fn spiral_simplex_puts_one_page_on_each_sheet_front() {
        let plan = booklet_plan(BindingType::Spiral, 3, 1, DuplexMode::Simplex, false, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A4, A4).unwrap();
        assert_eq!(sides.len(), 3);
        assert!(sides.iter().all(|s| s.side == "front"));
        assert!(sides.iter().all(|s| s.fold_x.is_empty()));
    }

    #[test]
    fn folded_binding_rejects_non_standard_grids() {
        // 4-up folded would need a signature imposition we do not compute.
        let plan = booklet_plan(BindingType::SaddleStitch, 32, 4, DuplexMode::ShortEdge, true, 80.0).unwrap();
        let err = sheets_for_plan(&plan, A5, A4_LANDSCAPE).unwrap_err();
        assert!(err.contains("2 pages per side"));
    }

    #[test]
    fn pages_that_do_not_fit_the_sheet_are_rejected() {
        let plan = booklet_plan(BindingType::Perfect, 4, 1, DuplexMode::LongEdge, false, 80.0).unwrap();
        // A4 pages cannot fit on an A5 sheet at 100%.
        assert!(sheets_for_plan(&plan, A4, (148.0, 210.0)).is_err());
    }

    #[test]
    fn unsupported_grid_is_rejected() {
        let mut plan = booklet_plan(BindingType::Spiral, 8, 1, DuplexMode::Simplex, false, 80.0).unwrap();
        plan.pages_per_side = 5;
        assert!(sheets_for_plan(&plan, A5, A4).is_err());
    }
}
