//! Imposition for folded work, derived by simulating the folds.
//!
//! The page order on a folded sheet is not a table to be looked up. It is a
//! consequence of how the paper is folded, so this module folds the sheet in
//! software and reads the answer off the result. That keeps the numbering
//! correct for any grid the paper sizes produce — 2 pages per side on A4
//! landscape, 4 on A4 portrait, 8 on A3 — instead of only the one case
//! somebody thought to write down.
//!
//! Because the layout is produced by simulating a specific fold sequence,
//! the instructions given to the user are the same sequence. They cannot
//! drift apart.

use serde::{Deserialize, Serialize};

/// The axis a fold turns about, described by the crease it leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoldAxis {
    /// A crease running top to bottom; the sheet is folded left-to-right.
    Vertical,
    /// A crease running left to right; the sheet is folded bottom-to-top.
    Horizontal,
}

/// Where one page of the signature is printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// Row on the sheet, 0 at the top.
    pub row: u32,
    /// Column on the sheet, 0 at the left.
    pub col: u32,
    /// True when the page prints on the reverse of the sheet.
    pub back: bool,
    /// 0 or 180 — right-angle folds land alternate rows upside down.
    pub rotation: i64,
}

/// One leaf of paper as it sits in the folded stack.
#[derive(Debug, Clone, Copy)]
struct Leaf {
    row: u32,
    col: u32,
    /// True when the sheet's reverse currently faces up.
    flipped: bool,
    /// True when the leaf has been turned end for end by a horizontal fold.
    inverted: bool,
}

/// The order the sheet is folded in.
///
/// The last fold makes the spine, so for a booklet bound on a side edge it
/// has to be the vertical one — fold A4 the other way round and the staples
/// end up along the top. The rest alternate, which is how paper is actually
/// folded: two creases in the same direction running one after another are
/// awkward to make square.
///
/// Built back to front for that reason: the final fold is fixed first.
///
/// `spine` is the crease the staples go through. It is vertical for pages
/// sitting upright on the sheet and horizontal when they are turned 90° to
/// fit — the crease has to run along the page's binding edge either way.
pub fn fold_sequence(rows: u32, cols: u32, spine: FoldAxis) -> Result<Vec<FoldAxis>, String> {
    if rows == 0 || cols == 0 {
        return Err("a sheet grid needs at least one row and one column".into());
    }
    if !rows.is_power_of_two() || !cols.is_power_of_two() {
        return Err(format!(
            "a folded sheet halves each time, so the grid must be powers of two — {rows} × {cols} cannot be folded"
        ));
    }
    let mut vertical = cols.trailing_zeros();
    let mut horizontal = rows.trailing_zeros();
    if (spine == FoldAxis::Vertical && vertical == 0) || (spine == FoldAxis::Horizontal && horizontal == 0) {
        if vertical + horizontal > 0 {
            return Err("the spine crease must be one of the folds the grid needs".into());
        }
        return Ok(Vec::new());
    }
    let mut reversed = Vec::new();
    // Working from the spine outwards, take the other axis whenever it still
    // has a fold owing, so the creases alternate.
    let mut want_vertical = spine == FoldAxis::Vertical;
    while vertical > 0 || horizontal > 0 {
        let take_vertical = if want_vertical { vertical > 0 } else { horizontal == 0 };
        if take_vertical {
            reversed.push(FoldAxis::Vertical);
            vertical -= 1;
        } else {
            reversed.push(FoldAxis::Horizontal);
            horizontal -= 1;
        }
        want_vertical = !take_vertical;
    }
    reversed.reverse();
    Ok(reversed)
}

/// Fold the sheet and return its leaves, outermost first.
fn fold_stack(rows: u32, cols: u32, folds: &[FoldAxis]) -> Vec<Leaf> {
    let mut piles: Vec<Vec<Vec<Leaf>>> = (0..rows)
        .map(|row| {
            (0..cols)
                .map(|col| {
                    vec![Leaf { row, col, flipped: false, inverted: false }]
                })
                .collect()
        })
        .collect();

    // The part of the grid still carrying paper. Folding right over left and
    // top over bottom leaves the packet in the bottom-left cell, which puts
    // page 1 on the outside at the bottom right — the classic layout.
    let (mut row_lo, mut row_hi) = (0usize, rows as usize - 1);
    let (mut col_lo, mut col_hi) = (0usize, cols as usize - 1);

    for &axis in folds {
        match axis {
            FoldAxis::Vertical => {
                let width = col_hi - col_lo + 1;
                let half = width / 2;
                for r in row_lo..=row_hi {
                    for c in (col_lo + half)..=col_hi {
                        let dest = col_lo + (col_hi - c);
                        let mut moved = std::mem::take(&mut piles[r][c]);
                        // Turning the paper over reverses the order of what
                        // it carries and shows the other side of each leaf.
                        moved.reverse();
                        for leaf in moved.iter_mut() {
                            leaf.flipped = !leaf.flipped;
                        }
                        moved.extend(std::mem::take(&mut piles[r][dest]));
                        piles[r][dest] = moved;
                    }
                }
                col_hi = col_lo + half - 1;
            }
            FoldAxis::Horizontal => {
                let height = row_hi - row_lo + 1;
                let half = height / 2;
                for c in col_lo..=col_hi {
                    for r in row_lo..(row_lo + half) {
                        let dest = row_hi - (r - row_lo);
                        let mut moved = std::mem::take(&mut piles[r][c]);
                        moved.reverse();
                        for leaf in moved.iter_mut() {
                            leaf.flipped = !leaf.flipped;
                            // A fold about a horizontal axis also turns the
                            // paper end for end, so its content is upside
                            // down relative to the rest of the sheet.
                            leaf.inverted = !leaf.inverted;
                        }
                        moved.extend(std::mem::take(&mut piles[dest][c]));
                        piles[dest][c] = moved;
                    }
                }
                row_lo += half;
            }
        }
    }

    std::mem::take(&mut piles[row_hi][col_lo])
}

/// Where every page of one folded signature is printed.
///
/// Returns `pages_per_sheet` slots: index `i` is the position of the
/// signature's page `i + 1`, counting from its own first page.
///
/// The sheet side carrying page 1 is called the front, which is what the
/// word means to whoever is loading the printer. Back-side positions are
/// given as the printer lays them down after turning the sheet about its
/// vertical axis; a press that turns it the other way is corrected by
/// `invert_back`.
pub fn signature_slots(
    rows: u32,
    cols: u32,
    invert_back: bool,
    spine: FoldAxis,
) -> Result<Vec<Slot>, String> {
    let folds = fold_sequence(rows, cols, spine)?;
    let stack = fold_stack(rows, cols, &folds);
    debug_assert_eq!(stack.len(), (rows * cols) as usize);

    // Whichever side of the paper faces up on the outermost leaf is the one
    // page 1 prints on, and that is the side called "front".
    let front_face = stack[0].flipped;

    let mut slots = Vec::with_capacity(stack.len() * 2);
    for leaf in &stack {
        let rotation = if leaf.inverted { 180 } else { 0 };
        // The leaf's two faces, in reading order: the one facing up in the
        // folded stack, then its reverse.
        for back in [leaf.flipped != front_face, leaf.flipped == front_face] {
            let (mut row, mut col) = (leaf.row, leaf.col);
            let mut rotation = rotation;
            if back {
                // Turning the sheet about its vertical axis mirrors the
                // columns; the leaf keeps its own orientation.
                col = cols - 1 - col;
                if invert_back {
                    // The press turns it about the horizontal axis instead,
                    // which is the same thing rotated by half a turn.
                    row = rows - 1 - row;
                    col = cols - 1 - col;
                    rotation = (rotation + 180) % 360;
                }
            }
            slots.push(Slot { row, col, back, rotation });
        }
    }
    Ok(slots)
}

/// Reading pages carried by one sheet of a nested (saddle-stitched) set.
///
/// Nested sheets wrap one another, so a sheet does not carry a run of
/// consecutive pages: it carries a block from the front of the document and
/// the matching block from the back. Sheet 1 is the outermost.
///
/// The result is indexed by position within the signature, so it lines up
/// with [`signature_slots`]. `None` is a blank position.
pub fn nested_signature_pages(
    total_slots: u32,
    pages_per_sheet: u32,
    sheet_index: u32,
    source_pages: u32,
) -> Result<Vec<Option<u32>>, String> {
    let content: Vec<u32> = (1..=source_pages).collect();
    nested_signature_pages_over(total_slots, pages_per_sheet, sheet_index, &content)
}

/// As [`nested_signature_pages`], but nesting an explicit, ordered list of
/// real page numbers rather than the whole document.
///
/// This is how a document whose cover pages have been pulled out to print
/// separately still nests correctly: `content` is the document's pages with
/// the cover's pages already removed, in their original order, so the body
/// folds exactly as it would if those pages had never existed.
pub fn nested_signature_pages_over(
    total_slots: u32,
    pages_per_sheet: u32,
    sheet_index: u32,
    content: &[u32],
) -> Result<Vec<Option<u32>>, String> {
    if pages_per_sheet == 0 || pages_per_sheet % 2 != 0 {
        return Err("a folded sheet carries an even number of pages".into());
    }
    if sheet_index == 0 {
        return Err("sheets are numbered from 1".into());
    }
    let nth = |position_1based: u32| -> Option<u32> {
        if position_1based >= 1 && (position_1based as usize) <= content.len() {
            Some(content[position_1based as usize - 1])
        } else {
            None
        }
    };
    let half = pages_per_sheet / 2;
    let first = (sheet_index - 1) * half;
    let mut pages = Vec::with_capacity(pages_per_sheet as usize);
    // Front block: the k-th run of pages from the start of the content.
    for i in 0..half {
        pages.push(nth(first + i + 1));
    }
    // Back block: the matching run counted from the end.
    for i in 0..half {
        pages.push(nth(total_slots - first - half + i + 1));
    }
    Ok(pages)
}

/// Pages past the end of the supplied document are blank positions.
pub fn page_or_blank(page: u32, source_pages: u32) -> Option<u32> {
    if page >= 1 && page <= source_pages {
        Some(page)
    } else {
        None
    }
}

/// How many trim pages fit on one side of the sheet, upright, at 100%.
///
/// Returned as `(rows, cols)`. This is what makes the imposition follow the
/// paper the user actually chose rather than a fixed table: A4 landscape
/// with A5 pages gives 1 × 2, A4 portrait with A6 gives 2 × 2, A3 portrait
/// with A5 gives 2 × 2.
pub fn fit_grid(sheet_mm: (f64, f64), trim_mm: (f64, f64)) -> Result<(u32, u32), String> {
    if sheet_mm.0 <= 0.0 || sheet_mm.1 <= 0.0 || trim_mm.0 <= 0.0 || trim_mm.1 <= 0.0 {
        return Err("page and sheet sizes must be positive".into());
    }
    // A hair of tolerance: A-series halving is exact on paper but the stored
    // sizes are rounded to the millimetre, so 2 × 148 must still fit 297.
    const TOL: f64 = 0.6;
    let cols = ((sheet_mm.0 + TOL) / trim_mm.0).floor() as u32;
    let rows = ((sheet_mm.1 + TOL) / trim_mm.1).floor() as u32;
    if rows == 0 || cols == 0 {
        return Err(format!(
            "a {:.0} × {:.0} mm page does not fit on a {:.0} × {:.0} mm sheet at 100% — \
             turn the sheet, or choose a larger one",
            trim_mm.0, trim_mm.1, sheet_mm.0, sheet_mm.1
        ));
    }
    Ok((rows, cols))
}

/// A grid that the chosen papers can actually fold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldedGrid {
    pub rows: u32,
    pub cols: u32,
    /// True when the pages are turned 90° to fit — the A-series alternates
    /// orientation at every halving, so eight A6 pages only fit on A3 on
    /// their side.
    pub turned: bool,
    /// The crease the staples go through, which is the last fold made.
    pub spine: FoldAxis,
}

/// Cell size on the sheet for this grid, in the same units as the trim.
pub fn cell_size(grid: FoldedGrid, trim_mm: (f64, f64)) -> (f64, f64) {
    if grid.turned {
        (trim_mm.1, trim_mm.0)
    } else {
        trim_mm
    }
}

/// The grid to use for a requested number of pages on one side.
///
/// Tries the pages upright first and turned on their side second, so the
/// same request works out on whichever sheet the user picked: two up on A4
/// landscape, four on A4 portrait, eight on A3.
pub fn folded_grid(
    pages_per_side: u32,
    sheet_mm: (f64, f64),
    trim_mm: (f64, f64),
) -> Result<FoldedGrid, String> {
    if pages_per_side == 0 {
        return Err("pages per side must be positive".into());
    }
    if !pages_per_side.is_power_of_two() {
        return Err(format!(
            "{pages_per_side} pages per side cannot be folded — a folded sheet halves each time, \
             so it carries 1, 2, 4, 8 … pages per side"
        ));
    }

    let upright = fit_grid(sheet_mm, trim_mm);
    let turned = fit_grid(sheet_mm, (trim_mm.1, trim_mm.0));
    for (is_turned, fit, spine) in [
        (false, &upright, FoldAxis::Vertical),
        (true, &turned, FoldAxis::Horizontal),
    ] {
        let Ok(&(fit_rows, fit_cols)) = fit.as_ref() else { continue };
        if let Some((rows, cols)) = best_arrangement(pages_per_side, fit_rows, fit_cols, spine) {
            return Ok(FoldedGrid { rows, cols, turned: is_turned, spine });
        }
    }

    let held = |fit: &Result<(u32, u32), String>| fit.as_ref().map(|(r, c)| r * c).unwrap_or(0);
    Err(format!(
        "{pages_per_side} pages per side does not fit: a {:.0} × {:.0} mm sheet holds \
         {} pages of {:.0} × {:.0} mm upright and {} with the pages turned on their side",
        sheet_mm.0,
        sheet_mm.1,
        held(&upright),
        trim_mm.0,
        trim_mm.1,
        held(&turned),
    ))
}

/// The arrangement of `pages` that fits, with the spine crease available.
fn best_arrangement(pages: u32, fit_rows: u32, fit_cols: u32, spine: FoldAxis) -> Option<(u32, u32)> {
    let mut best: Option<(u32, u32)> = None;
    let mut rows = 1;
    while rows <= pages {
        let cols = pages / rows;
        let fits = rows <= fit_rows && cols <= fit_cols && cols.is_power_of_two();
        // The last fold makes the spine, so that axis must have something to
        // halve — unless the sheet is not folded at all.
        let has_spine = pages == 1
            || match spine {
                FoldAxis::Vertical => cols > 1,
                FoldAxis::Horizontal => rows > 1,
            };
        if fits && has_spine {
            let score = rows.abs_diff(cols);
            if best.is_none_or(|(br, bc)| score < br.abs_diff(bc)) {
                best = Some((rows, cols));
            }
        }
        rows *= 2;
    }
    best
}

/// The pages-per-side values this paper combination can actually hold.
pub fn foldable_options(sheet_mm: (f64, f64), trim_mm: (f64, f64)) -> Vec<u32> {
    (0..6)
        .map(|shift| 1u32 << shift)
        .filter(|&n| folded_grid(n, sheet_mm, trim_mm).is_ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const A6: (f64, f64) = (105.0, 148.0);
    const A5: (f64, f64) = (148.0, 210.0);
    const A5_LANDSCAPE: (f64, f64) = (210.0, 148.0);
    const A4: (f64, f64) = (210.0, 297.0);
    const A4_LANDSCAPE: (f64, f64) = (297.0, 210.0);
    const A3: (f64, f64) = (297.0, 420.0);

    /// Read the layout back as a grid of page numbers per side, the way it
    /// would be printed, so the assertions read like the sheet looks.
    fn sides(rows: u32, cols: u32, invert_back: bool) -> (Vec<Vec<u32>>, Vec<Vec<u32>>) {
        let slots = signature_slots(rows, cols, invert_back, FoldAxis::Vertical).unwrap();
        let mut front = vec![vec![0; cols as usize]; rows as usize];
        let mut back = vec![vec![0; cols as usize]; rows as usize];
        for (i, s) in slots.iter().enumerate() {
            let target = if s.back { &mut back } else { &mut front };
            target[s.row as usize][s.col as usize] = i as u32 + 1;
        }
        (front, back)
    }

    #[test]
    fn single_fold_reproduces_the_classic_saddle_stitch_spread() {
        // One fold, two pages a side: front is 4 | 1, back is 2 | 3. This is
        // the case the application already produced by hand, so the
        // simulation has to agree with it.
        let (front, back) = sides(1, 2, false);
        assert_eq!(front, vec![vec![4, 1]]);
        assert_eq!(back, vec![vec![2, 3]]);
    }

    #[test]
    fn two_folds_give_the_standard_eight_page_signature() {
        // The classic quarto: 5 | 4 over 8 | 1 on the front, 3 | 6 over
        // 2 | 7 on the back, with the top row upside down.
        let (front, back) = sides(2, 2, false);
        assert_eq!(front, vec![vec![5, 4], vec![8, 1]]);
        assert_eq!(back, vec![vec![3, 6], vec![2, 7]]);
    }

    #[test]
    fn the_right_angle_fold_inverts_the_top_row_only() {
        let slots = signature_slots(2, 2, false, FoldAxis::Vertical).unwrap();
        for s in &slots {
            let expected = if s.row == 0 { 180 } else { 0 };
            assert_eq!(s.rotation, expected, "row {} rotation", s.row);
        }
    }

    #[test]
    fn a_flat_sheet_needs_no_fold_and_no_rotation() {
        let (front, back) = sides(1, 1, false);
        assert_eq!(front, vec![vec![1]]);
        assert_eq!(back, vec![vec![2]]);
        assert!(fold_sequence(1, 1, FoldAxis::Vertical).unwrap().is_empty());
    }

    /// Every page must be placed exactly once, and the two faces of one leaf
    /// must sit back to back — otherwise the sheet cannot be printed.
    #[test]
    fn every_page_is_placed_once_on_a_real_leaf() {
        for (rows, cols) in [(1, 1), (1, 2), (2, 1), (2, 2), (2, 4), (4, 2), (4, 4)] {
            for invert in [false, true] {
                let spine = if cols > 1 { FoldAxis::Vertical } else { FoldAxis::Horizontal };
                let slots = signature_slots(rows, cols, invert, spine).unwrap();
                assert_eq!(slots.len(), (rows * cols * 2) as usize);

                let mut seen = std::collections::HashSet::new();
                for s in &slots {
                    assert!(
                        seen.insert((s.row, s.col, s.back)),
                        "{rows}×{cols}: two pages in one position"
                    );
                    assert!(s.row < rows && s.col < cols);
                }

                // Odd page and the even page after it are one leaf, so they
                // occupy the same cell on opposite sides once the sheet is
                // turned back over.
                for pair in slots.chunks(2) {
                    let (odd, even) = (pair[0], pair[1]);
                    assert_ne!(odd.back, even.back, "{rows}×{cols}: a leaf has two sides");
                    let mut col = even.col;
                    let mut row = even.row;
                    if invert {
                        row = rows - 1 - row;
                        col = cols - 1 - col;
                    }
                    assert_eq!(
                        (row, cols - 1 - col),
                        (odd.row, odd.col),
                        "{rows}×{cols} invert={invert}: the reverse of a page must be the same leaf"
                    );
                }
            }
        }
    }

    #[test]
    fn inverting_the_back_turns_that_side_through_half_a_turn() {
        let plain = signature_slots(2, 2, false, FoldAxis::Vertical).unwrap();
        let turned = signature_slots(2, 2, true, FoldAxis::Vertical).unwrap();
        for (p, t) in plain.iter().zip(turned.iter()) {
            if p.back {
                assert_eq!((t.row, t.col), (1 - p.row, 1 - p.col));
                assert_eq!(t.rotation, (p.rotation + 180) % 360);
            } else {
                assert_eq!((t.row, t.col, t.rotation), (p.row, p.col, p.rotation));
            }
        }
    }

    #[test]
    fn nesting_wraps_the_document_from_both_ends() {
        // 8 pages on 2 sheets of 4: the outer sheet takes 1, 2, 7, 8 and the
        // inner one takes 3, 4, 5, 6.
        let outer = nested_signature_pages(8, 4, 1, 8).unwrap();
        let inner = nested_signature_pages(8, 4, 2, 8).unwrap();
        assert_eq!(outer, vec![Some(1), Some(2), Some(7), Some(8)]);
        assert_eq!(inner, vec![Some(3), Some(4), Some(5), Some(6)]);
    }

    /// Removing the cover pages from the content list must nest the
    /// remaining body pages as if they had never existed — not shift
    /// numbers around, and not leave gaps.
    #[test]
    fn nesting_over_an_explicit_list_skips_excluded_pages_cleanly() {
        // A 10-page document with pages 1, 2, 9, 10 pulled out for a
        // separate cover leaves 6 body pages: 3..=8.
        let content: Vec<u32> = (3..=8).collect();
        let outer = nested_signature_pages_over(8, 4, 1, &content).unwrap();
        let inner = nested_signature_pages_over(8, 4, 2, &content).unwrap();
        // Body-relative positions 1,2,7,8 -> content[0],content[1],blank,blank
        // (the content list only has 6 entries, so positions 7 and 8 are
        // padding, not the excluded pages reappearing).
        assert_eq!(outer, vec![Some(3), Some(4), None, None]);
        assert_eq!(inner, vec![Some(5), Some(6), Some(7), Some(8)]);
    }

    #[test]
    fn nesting_holds_for_eight_page_signatures_too() {
        // 16 pages on 2 sheets of 8.
        let outer = nested_signature_pages(16, 8, 1, 16).unwrap();
        let inner = nested_signature_pages(16, 8, 2, 16).unwrap();
        assert_eq!(
            outer,
            vec![Some(1), Some(2), Some(3), Some(4), Some(13), Some(14), Some(15), Some(16)]
        );
        assert_eq!(
            inner,
            vec![Some(5), Some(6), Some(7), Some(8), Some(9), Some(10), Some(11), Some(12)]
        );
    }

    #[test]
    fn positions_past_the_document_are_blank() {
        // 6 pages padded to 8: the last two positions of the outer sheet.
        let outer = nested_signature_pages(8, 4, 1, 6).unwrap();
        assert_eq!(outer, vec![Some(1), Some(2), None, None]);
        let inner = nested_signature_pages(8, 4, 2, 6).unwrap();
        assert_eq!(inner, vec![Some(3), Some(4), Some(5), Some(6)]);
    }

    #[test]
    fn the_grid_follows_the_paper_rather_than_a_table() {
        assert_eq!(fit_grid(A4_LANDSCAPE, A5).unwrap(), (1, 2));
        assert_eq!(fit_grid(A4, A6).unwrap(), (2, 2));
        assert_eq!(fit_grid(A3, A5).unwrap(), (2, 2));
        assert_eq!(fit_grid(A5_LANDSCAPE, A6).unwrap(), (1, 2));
        assert_eq!(fit_grid(A3, A4).unwrap(), (1, 1));
        // A5 pages upright on a portrait A4 sheet: only one fits.
        assert_eq!(fit_grid(A4, A5).unwrap(), (1, 1));
    }

    #[test]
    fn a_page_larger_than_the_sheet_is_refused_with_the_reason() {
        let err = fit_grid(A5, A4).unwrap_err();
        assert!(err.contains("does not fit"), "{err}");
        assert!(err.contains("turn the sheet"), "{err}");
    }

    #[test]
    fn the_offered_options_are_the_ones_that_fit() {
        assert_eq!(foldable_options(A4_LANDSCAPE, A5), vec![1, 2]);
        assert_eq!(foldable_options(A4, A6), vec![1, 2, 4]);
        assert_eq!(foldable_options(A3, A6), vec![1, 2, 4, 8]);
        // Two A4 pages fit on A3 turned on their side — the usual way of
        // printing an A4 booklet on A3 stock.
        assert_eq!(foldable_options(A3, A4), vec![1, 2]);
    }

    #[test]
    fn the_same_pages_per_side_gives_the_grid_that_fits_the_sheet() {
        // Two up is a row on a landscape sheet and a column on a portrait one.
        let g = |n, sheet, trim| {
            let g = folded_grid(n, sheet, trim).unwrap();
            (g.rows, g.cols, g.turned)
        };
        assert_eq!(g(2, A4_LANDSCAPE, A5), (1, 2, false));
        // Folded work needs the pages side by side, because the last
        // fold is the spine — never stacked, even though they would fit.
        assert_eq!(g(2, A4, A6), (1, 2, false));
        assert_eq!(g(4, A4, A6), (2, 2, false));
        assert_eq!(g(4, A3, A5), (2, 2, false));
        // Eight A6 pages only fit on A3 turned on their side, which is how
        // the A series works: every halving swaps the orientation.
        assert_eq!(g(8, A3, A6), (4, 2, true));
    }

    #[test]
    fn asking_for_more_than_the_sheet_holds_says_what_it_holds() {
        let err = folded_grid(8, A4, A6).unwrap_err();
        assert!(err.contains("does not fit"), "{err}");
        assert!(err.contains("4 pages"), "{err}");
    }

    #[test]
    fn a_count_that_cannot_come_from_folding_is_refused() {
        let err = folded_grid(6, A3, A6).unwrap_err();
        assert!(err.contains("cannot be folded"), "{err}");
    }
}
