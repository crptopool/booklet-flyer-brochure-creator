//! Turn a booklet plan into the physical sheet sides that will be printed.
//!
//! This is the bridge between the deterministic calculations and the
//! imposition renderer: it decides which source page lands in which cell
//! of which sheet side, and where that cell sits on the paper.

use crate::pdf_ops::impose::{Placement, SheetSide};
use crate::print_calc::booklet::saddle_stitch_order;
use crate::print_calc::fold::{cell_size, folded_grid, nested_signature_pages, signature_slots};
use crate::print_calc::imposition::{grid_placements, sequential_nup};
use crate::print_calc::plan::BookletPlan;
use crate::print_calc::units::mm_to_points;

/// Rows and columns for a given number of pages on one sheet side.
///
/// Used for work that is not folded, where any rectangle will do and the
/// pages simply run in order. Folded work derives its grid from the paper
/// instead — see [`crate::print_calc::fold::folded_grid`].
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

/// Sheet sides for folded work, imposed by simulating the folds.
///
/// Works for any grid the chosen papers produce, so the numbering follows
/// the configuration rather than a table of one remembered case.
fn folded_sheets(
    plan: &BookletPlan,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
) -> Result<Vec<SheetSide>, String> {
    if plan.pages_per_sheet <= plan.pages_per_side {
        return Err(format!(
            "{} folds its sheets, so both sides carry pages — choose double-sided printing.",
            plan.profile.name
        ));
    }
    let grid = folded_grid(plan.pages_per_side, sheet_mm, trim_mm)?;
    let (rows, cols) = (grid.rows, grid.cols);
    let invert_back = plan.duplex.back_side_inverted;
    let slots = signature_slots(rows, cols, invert_back, grid.spine)?;

    // Pages turned on their side take the cell the other way round, and every
    // page carries the same quarter turn on top of the fold's own rotation.
    //
    // The turn goes clockwise, not anticlockwise: it has to put each page's
    // left edge against the spine crease. Turning the other way produces a
    // booklet that reads perfectly well but opens from the right.
    let (cell_mm_w, cell_mm_h) = cell_size(grid, trim_mm);
    let turn = if grid.turned { 270 } else { 0 };
    let cell_w = mm_to_points(cell_mm_w);
    let cell_h = mm_to_points(cell_mm_h);
    let sheet_w = mm_to_points(sheet_mm.0);
    let sheet_h = mm_to_points(sheet_mm.1);
    let per_sheet = plan.pages_per_sheet;
    // Every sheet is filled, so the document is padded to the sheets it needs.
    let capacity = plan.sheet_count * per_sheet;

    let mut result = Vec::with_capacity(plan.sheet_count as usize * 2);
    for sheet_no in 1..=plan.sheet_count {
        let pages = nested_signature_pages(capacity, per_sheet, sheet_no, plan.source_pages)?;

        for back in [false, true] {
            // Lay the signature's pages into this side's grid.
            let mut cells = vec![None; (rows * cols) as usize];
            let mut rotations = vec![0i64; (rows * cols) as usize];
            for (i, slot) in slots.iter().enumerate() {
                if slot.back != back {
                    continue;
                }
                let index = (slot.row * cols + slot.col) as usize;
                cells[index] = pages[i];
                rotations[index] = (slot.rotation + turn) % 360;
            }

            let side_name = if back { "back" } else { "front" };
            let layout = grid_placements(
                &cells, rows, cols, sheet_w, sheet_h, cell_w, cell_h, 0.0, sheet_no, side_name,
            )?;

            result.push(SheetSide {
                sheet_number: sheet_no,
                side: side_name.to_string(),
                width: sheet_w,
                height: sheet_h,
                placements: layout
                    .cells
                    .iter()
                    .zip(rotations.iter())
                    .map(|(cell, rotation)| Placement::from_cell(cell, *rotation))
                    .collect(),
                fold_x: creases(cols, sheet_w, cell_w),
                fold_y: creases(rows, sheet_h, cell_h),
            });
        }
    }
    Ok(result)
}

/// Where the creases fall along one axis, for the fold marks on the sheet.
///
/// A crease sits on every internal division of the grid, so a sheet folded
/// twice is marked on both axes rather than only across.
fn creases(divisions: u32, sheet: f64, cell: f64) -> Vec<f64> {
    let block = divisions as f64 * cell;
    let offset = (sheet - block) / 2.0;
    (1..divisions).map(|i| offset + i as f64 * cell).collect()
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
    if trim_mm.0 <= 0.0 || trim_mm.1 <= 0.0 || sheet_mm.0 <= 0.0 || sheet_mm.1 <= 0.0 {
        return Err("page and sheet sizes must be positive".into());
    }

    // Folded work is imposed by folding the sheet in software, which covers
    // every grid the chosen papers allow rather than one remembered case.
    if plan.profile.folded {
        return folded_sheets(plan, trim_mm, sheet_mm);
    }

    let (rows, cols) = grid_for(plan.pages_per_side)?;
    let cell_w = mm_to_points(trim_mm.0);
    let cell_h = mm_to_points(trim_mm.1);
    let sheet_w = mm_to_points(sheet_mm.0);
    let sheet_h = mm_to_points(sheet_mm.1);
    let duplex = plan.pages_per_sheet > plan.pages_per_side;
    let back_rotation = plan.duplex.back_side_rotation;

    // Unfolded work reads in order down the stack; `None` is a blank.
    let seq = sequential_nup(plan.source_pages, rows, cols, duplex)?;
    let sides: Vec<(u32, String, Vec<Option<u32>>)> = seq
        .into_iter()
        .enumerate()
        .map(|(i, cells)| {
            let sheet_no = if duplex { (i as u32) / 2 + 1 } else { i as u32 + 1 };
            let side = if duplex && i % 2 == 1 { "back" } else { "front" };
            (sheet_no, side.to_string(), cells)
        })
        .collect();

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
            fold_y: vec![],
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
    const A6: (f64, f64) = (105.0, 148.0);
    const A3: (f64, f64) = (297.0, 420.0);

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

    /// The grid comes from the paper, so the same request imposes correctly
    /// on whatever sheet the user picked.
    #[test]
    fn four_up_folded_work_is_imposed_from_the_fold() {
        // 16 A6 pages, 4 a side on portrait A4: two sheets, folded twice.
        let plan = booklet_plan(BindingType::SaddleStitch, 16, 4, DuplexMode::LongEdge, false, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A6, A4).unwrap();
        assert_eq!(sides.len(), 4, "two sheets, both sides");

        let pages = |s: &SheetSide| s.placements.iter().map(|p| p.page).collect::<Vec<_>>();
        // Outer sheet carries 1-4 and 13-16, laid out as the classic quarto:
        // top row inverted, page 1 at the bottom right.
        assert_eq!(pages(&sides[0]), vec![Some(13), Some(4), Some(16), Some(1)]);
        assert_eq!(pages(&sides[1]), vec![Some(3), Some(14), Some(2), Some(15)]);
        // Inner sheet carries 5-12.
        assert_eq!(pages(&sides[2]), vec![Some(9), Some(8), Some(12), Some(5)]);
        assert_eq!(pages(&sides[3]), vec![Some(7), Some(10), Some(6), Some(11)]);
    }

    /// The top row of a twice-folded sheet lands upside down, and the
    /// imposition has to say so or the printed pages come out inverted.
    #[test]
    fn the_inverted_row_of_a_quarto_is_marked() {
        let plan = booklet_plan(BindingType::SaddleStitch, 16, 4, DuplexMode::LongEdge, false, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A6, A4).unwrap();
        for side in &sides {
            let rotations: Vec<i64> = side.placements.iter().map(|p| p.rotation).collect();
            assert_eq!(rotations, vec![180, 180, 0, 0], "{} {}", side.sheet_number, side.side);
        }
    }

    /// The same document on bigger paper: A5 pages, 4 a side on A3.
    #[test]
    fn the_algorithm_follows_the_chosen_paper_sizes() {
        let plan = booklet_plan(BindingType::SaddleStitch, 16, 4, DuplexMode::LongEdge, false, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A5, A3).unwrap();
        assert_eq!(sides.len(), 4);
        let pages = |s: &SheetSide| s.placements.iter().map(|p| p.page).collect::<Vec<_>>();
        // Same numbering as on A4 with A6 pages — only the paper changed.
        assert_eq!(pages(&sides[0]), vec![Some(13), Some(4), Some(16), Some(1)]);
        // ...and the cells are A5-sized on an A3 sheet.
        let cell = &sides[0].placements[0];
        assert!((cell.width - mm_to_points(148.0)).abs() < 0.5);
        assert!((cell.height - mm_to_points(210.0)).abs() < 0.5);
    }

    /// Pages past the end of the document become blanks, rather than the
    /// imposition silently shortening the booklet.
    #[test]
    fn a_part_filled_signature_leaves_blanks_at_the_back() {
        // 12 pages at 8 a sheet needs 2 sheets = 16 slots, so 4 blanks.
        let plan = booklet_plan(BindingType::SaddleStitch, 12, 4, DuplexMode::LongEdge, false, 80.0).unwrap();
        let sides = sheets_for_plan(&plan, A6, A4).unwrap();
        let placed: Vec<u32> = sides
            .iter()
            .flat_map(|s| s.placements.iter().filter_map(|p| p.page))
            .collect();
        assert_eq!(placed.len(), 12, "every supplied page is placed once");
        let mut sorted = placed.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=12).collect::<Vec<_>>());
        let blanks = sides.iter().flat_map(|s| &s.placements).filter(|p| p.page.is_none()).count();
        assert_eq!(blanks, 4);
    }

    /// A sheet that cannot hold the requested pages says what it can hold
    /// instead of imposing something that will not print.
    #[test]
    fn a_grid_that_does_not_fit_the_sheet_is_refused_with_the_numbers() {
        let plan = booklet_plan(BindingType::SaddleStitch, 32, 8, DuplexMode::LongEdge, false, 80.0).unwrap();
        let err = sheets_for_plan(&plan, A6, A4).unwrap_err();
        assert!(err.contains("does not fit"), "{err}");
        assert!(err.contains("4 pages"), "{err}");
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

/// Sheet sides for one signature of a large book.
///
/// Each signature is imposed as its own saddle-stitched booklet, using
/// the signature's own page range. Sheet numbers restart at 1 within the
/// signature so the labels match the physical stack.
pub fn sheets_for_signature(
    signature: &crate::print_calc::signatures::Signature,
    source_pages: u32,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
    back_rotation: i64,
) -> Result<Vec<SheetSide>, String> {
    let count = signature.page_count();
    let spreads = saddle_stitch_order(count)?;
    let cell_w = mm_to_points(trim_mm.0);
    let cell_h = mm_to_points(trim_mm.1);
    let sheet_w = mm_to_points(sheet_mm.0);
    let sheet_h = mm_to_points(sheet_mm.1);

    // Map a position inside the signature to a real source page.
    let absolute = |local: Option<u32>| -> Option<u32> {
        let n = local? + signature.first_page - 1;
        if n <= source_pages {
            Some(n)
        } else {
            None
        }
    };

    let mut out = Vec::with_capacity(spreads.len() * 2);
    for s in spreads {
        for (side_name, pair) in [("front", s.front), ("back", s.back)] {
            let cells = vec![absolute(pair.0), absolute(pair.1)];
            let layout = grid_placements(
                &cells, 1, 2, sheet_w, sheet_h, cell_w, cell_h, 0.0, s.sheet_number, side_name,
            )?;
            let rotation = if side_name == "back" { back_rotation } else { 0 };
            let offset_x = (sheet_w - 2.0 * cell_w) / 2.0;
            out.push(SheetSide {
                sheet_number: s.sheet_number,
                side: side_name.to_string(),
                width: sheet_w,
                height: sheet_h,
                placements: layout.cells.iter().map(|c| Placement::from_cell(c, rotation)).collect(),
                fold_x: vec![offset_x + cell_w],
                fold_y: vec![],
            });
        }
    }
    Ok(out)
}

/// Sheet sides that repeat one page to fill each sheet.
///
/// Used for business cards, labels and flyers: the same artwork is
/// stepped across the sheet as many times as fits.
pub fn sheets_for_step_and_repeat(
    page: u32,
    copies: u32,
    rows: u32,
    cols: u32,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
    spacing_mm: f64,
) -> Result<Vec<SheetSide>, String> {
    if page == 0 || copies == 0 || rows == 0 || cols == 0 {
        return Err("page, copies, rows and columns must all be positive".into());
    }
    let per_sheet = rows * cols;
    let sheets = copies.div_ceil(per_sheet);
    let cell_w = mm_to_points(trim_mm.0);
    let cell_h = mm_to_points(trim_mm.1);
    let sheet_w = mm_to_points(sheet_mm.0);
    let sheet_h = mm_to_points(sheet_mm.1);
    let spacing = mm_to_points(spacing_mm.max(0.0));

    let mut out = Vec::with_capacity(sheets as usize);
    let mut remaining = copies;
    for n in 1..=sheets {
        // The last sheet carries only the copies still owed.
        let on_this_sheet = remaining.min(per_sheet);
        remaining -= on_this_sheet;
        let cells: Vec<Option<u32>> = (0..per_sheet)
            .map(|i| if i < on_this_sheet { Some(page) } else { None })
            .collect();
        let layout = grid_placements(
            &cells, rows, cols, sheet_w, sheet_h, cell_w, cell_h, spacing, n, "front",
        )?;
        out.push(SheetSide {
            sheet_number: n,
            side: "front".to_string(),
            width: sheet_w,
            height: sheet_h,
            placements: layout.cells.iter().map(|c| Placement::from_cell(c, 0)).collect(),
            fold_x: vec![],
            fold_y: vec![],
        });
    }
    Ok(out)
}

#[cfg(test)]
mod extra_tests {
    use super::*;
    use crate::print_calc::signatures::divide_into_signatures;

    const A5: (f64, f64) = (148.0, 210.0);
    const A4_LANDSCAPE: (f64, f64) = (297.0, 210.0);
    /// A6 fed rotated, which is how 8 fit on an A3 sheet: 2 across
    /// (2 x 148 = 296 mm) by 4 down (4 x 105 = 420 mm).
    const A6_ROTATED: (f64, f64) = (148.0, 105.0);
    const A6: (f64, f64) = (105.0, 148.0);
    const A3: (f64, f64) = (297.0, 420.0);

    #[test]
    fn each_signature_is_imposed_as_its_own_booklet() {
        // 40 pages in 16-page signatures: 16, 16, 8 (padded to 16).
        let sigs = divide_into_signatures(40, 16).unwrap();
        assert_eq!(sigs.len(), 3);
        let sides = sheets_for_signature(&sigs[0], 40, A5, A4_LANDSCAPE, 0).unwrap();
        // 16 pages -> 4 sheets -> 8 sides.
        assert_eq!(sides.len(), 8);
        let first: Vec<_> = sides[0].placements.iter().map(|p| p.page).collect();
        assert_eq!(first, vec![Some(16), Some(1)]);
    }

    #[test]
    fn later_signatures_carry_their_own_page_range() {
        let sigs = divide_into_signatures(40, 16).unwrap();
        let sides = sheets_for_signature(&sigs[1], 40, A5, A4_LANDSCAPE, 0).unwrap();
        // Signature 2 starts at page 17 and ends at 32.
        let first: Vec<_> = sides[0].placements.iter().map(|p| p.page).collect();
        assert_eq!(first, vec![Some(32), Some(17)]);
    }

    #[test]
    fn padding_in_the_final_signature_becomes_blanks() {
        let sigs = divide_into_signatures(40, 16).unwrap();
        let sides = sheets_for_signature(&sigs[2], 40, A5, A4_LANDSCAPE, 0).unwrap();
        let pages: Vec<u32> = sides
            .iter()
            .flat_map(|s| s.placements.iter().filter_map(|p| p.page))
            .collect();
        // Only pages 33..=40 exist; the rest of the signature is blank.
        assert_eq!(pages.len(), 8);
        assert!(pages.iter().all(|p| (33..=40).contains(p)));
    }

    #[test]
    fn signature_backs_carry_the_duplex_rotation() {
        let sigs = divide_into_signatures(16, 16).unwrap();
        let sides = sheets_for_signature(&sigs[0], 16, A5, A4_LANDSCAPE, 180).unwrap();
        assert_eq!(sides[0].placements[0].rotation, 0);
        assert_eq!(sides[1].placements[0].rotation, 180);
    }

    #[test]
    fn step_and_repeat_fills_whole_sheets() {
        // Scenario C: A6 flyers on A3, rotated, 4 rows x 2 cols = 8 per sheet.
        let sides = sheets_for_step_and_repeat(1, 100, 4, 2, A6_ROTATED, A3, 0.0).unwrap();
        assert_eq!(sides.len(), 13);
        assert_eq!(sides[0].placements.len(), 8);
        assert!(sides[0].placements.iter().all(|p| p.page == Some(1)));
    }

    #[test]
    fn the_last_sheet_only_carries_the_copies_still_owed() {
        // 10 copies at 8 per sheet leaves 2 on the second sheet.
        let sides = sheets_for_step_and_repeat(1, 10, 4, 2, A6_ROTATED, A3, 0.0).unwrap();
        assert_eq!(sides.len(), 2);
        let filled = sides[1].placements.iter().filter(|p| p.page.is_some()).count();
        assert_eq!(filled, 2);
    }

    #[test]
    fn step_and_repeat_rejects_a_grid_that_does_not_fit() {
        // Upright A6 stacks four deep to 592 mm, past an A3 sheet.
        assert!(sheets_for_step_and_repeat(1, 10, 4, 2, A6, A3, 0.0).is_err());
        assert!(sheets_for_step_and_repeat(1, 10, 9, 9, A6, A3, 0.0).is_err());
    }

    #[test]
    fn upright_a6_still_fits_four_to_an_a3_sheet() {
        let sides = sheets_for_step_and_repeat(1, 4, 2, 2, A6, A3, 0.0).unwrap();
        assert_eq!(sides.len(), 1);
        assert_eq!(sides[0].placements.len(), 4);
    }

    #[test]
    fn step_and_repeat_rejects_zero_inputs() {
        assert!(sheets_for_step_and_repeat(0, 10, 2, 2, A6, A3, 0.0).is_err());
        assert!(sheets_for_step_and_repeat(1, 0, 2, 2, A6, A3, 0.0).is_err());
    }
}
