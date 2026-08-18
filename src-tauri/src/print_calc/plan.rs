//! Complete booklet production plan.
//!
//! Combines the binding method, sheet layout, duplex choice and page
//! count into one deterministic answer: how many sheets, how they fold,
//! what must be corrected and what the user should be warned about.

use serde::{Deserialize, Serialize};

use crate::print_calc::binding::{binding_profile, blanks_for_binding, BindingProfile};
use crate::print_calc::booklet::saddle_stitch_order;
use crate::print_calc::duplex::{duplex_plan, DuplexPlan};
use crate::print_calc::presets::{BindingType, DuplexMode};
use crate::print_calc::spine::{approximate_caliper_mm, spine_width_from_pages, DEFAULT_BULK_FACTOR};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanNote {
    pub severity: String,
    pub message: String,
}

fn note(severity: &str, message: impl Into<String>) -> PlanNote {
    PlanNote {
        severity: severity.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookletPlan {
    pub profile: BindingProfile,
    pub duplex: DuplexPlan,
    /// Pages placed on one side of a sheet.
    pub pages_per_side: u32,
    /// Pages carried by one whole sheet (both sides if duplex).
    pub pages_per_sheet: u32,
    /// Reading pages supplied by the user.
    pub source_pages: u32,
    /// Blank pages needed to satisfy the binding's page-count rule.
    pub blanks_needed: u32,
    /// Source pages plus blanks.
    pub total_pages: u32,
    /// Physical sheets of paper required.
    pub sheet_count: u32,
    /// Folds applied to each sheet.
    pub folds_per_sheet: u32,
    /// True when the classic saddle-stitch printer-spread order applies.
    pub uses_printer_spreads: bool,
    /// Spine width in mm, when the binding has a spine.
    pub spine_width_mm: Option<f64>,
    /// Approximate caliper used for the spine calculation.
    pub caliper_mm: f64,
    pub notes: Vec<PlanNote>,
}

/// Build a full production plan.
///
/// `pages_per_side` is how many document pages sit on one side of a
/// sheet (1, 2, 4 …). `sheet_is_landscape` drives the duplex flip
/// recommendation.
#[allow(clippy::too_many_arguments)]
pub fn booklet_plan(
    binding: BindingType,
    source_pages: u32,
    pages_per_side: u32,
    duplex_mode: DuplexMode,
    sheet_is_landscape: bool,
    gsm: f64,
) -> Result<BookletPlan, String> {
    if source_pages == 0 {
        return Err("page count must be positive".into());
    }
    if pages_per_side == 0 {
        return Err("pages per side must be positive".into());
    }

    let profile = binding_profile(binding);
    let duplex = duplex_plan(duplex_mode, sheet_is_landscape);
    let is_duplex = duplex_mode != DuplexMode::Simplex;

    let blanks = blanks_for_binding(binding, source_pages)?;
    let total_pages = source_pages + blanks;

    let pages_per_sheet = pages_per_side * if is_duplex { 2 } else { 1 };
    let sheet_count = total_pages.div_ceil(pages_per_sheet);

    // A folded binding gains one fold each time the pages-per-side
    // doubles: 2-up duplex is a single fold, 4-up duplex folds twice.
    let folds_per_sheet = if profile.folded {
        match (pages_per_side, is_duplex) {
            (2, true) => 1,
            (4, true) => 2,
            (8, true) => 3,
            _ => profile.folds_per_sheet,
        }
    } else {
        0
    };

    // The classic printer-spread sequence only describes the standard
    // single-fold, two-pages-per-side, duplex saddle-stitch case.
    let uses_printer_spreads = profile.folded && pages_per_side == 2 && is_duplex;

    let caliper = approximate_caliper_mm(gsm, DEFAULT_BULK_FACTOR)?;
    let spine_width_mm = if profile.has_spine {
        Some(spine_width_from_pages(total_pages, caliper)?)
    } else {
        None
    };

    let mut notes = Vec::new();

    if blanks > 0 {
        notes.push(note(
            "WARNING",
            format!(
                "{source_pages} pages is not a multiple of {} for {} — {blanks} blank page(s) must be added. \
                 Choose where they go; pages are never added silently.",
                profile.page_count_rule.multiple(),
                profile.name
            ),
        ));
    } else if sheet_count * pages_per_sheet == total_pages {
        // Only claim a clean fit when the sheets also come out full — the
        // binding rule alone does not guarantee that.
        notes.push(note(
            "INFO",
            format!("Page count suits {} with no blanks needed.", profile.name),
        ));
    }

    if !is_duplex && profile.requires_duplex {
        notes.push(note(
            "ERROR",
            format!(
                "{} is printed on both sides of every sheet. Single-sided output cannot be bound this way.",
                profile.name
            ),
        ));
    }

    if duplex.back_side_inverted {
        notes.push(note(
            "WARNING",
            format!(
                "This flip setting turns the sheet about a horizontal axis, so every back side is \
                 rotated 180 degrees to compensate. On {} sheets the other flip setting avoids the \
                 correction entirely.",
                if sheet_is_landscape { "landscape" } else { "portrait" }
            ),
        ));
    }

    if profile.folded && !is_duplex {
        notes.push(note(
            "WARNING",
            "Folded sheets must be printed on both sides — the back of each fold carries pages too.".to_string(),
        ));
    }

    if total_pages > profile.max_pages {
        notes.push(note(
            "WARNING",
            format!(
                "{total_pages} pages exceeds the practical maximum of {} for {}. Consider a different binding.",
                profile.max_pages, profile.name
            ),
        ));
    }
    if total_pages < profile.min_pages {
        notes.push(note(
            "INFO",
            format!(
                "{} normally starts at about {} pages.",
                profile.name, profile.min_pages
            ),
        ));
    }

    if profile.creep_applies && sheet_count > 4 {
        notes.push(note(
            "INFO",
            format!(
                "{sheet_count} nested sheets will creep towards the fore-edge — enable creep \
                 compensation so the inner pages are not trimmed unevenly."
            ),
        ));
    }

    if profile.punched {
        notes.push(note(
            "INFO",
            format!(
                "Keep all content at least {:.0} mm from the binding edge — the punch line removes \
                 material there.",
                profile.recommended_binding_margin_mm
            ),
        ));
    }

    if let Some(spine) = spine_width_mm {
        notes.push(note(
            "INFO",
            format!(
                "Spine width {spine:.2} mm at {gsm:.0} GSM. Confirm the caliper with your printer \
                 before laying out the cover — it varies by paper make and finish."
            ),
        ));
    }

    if profile.folded && pages_per_side > 2 {
        notes.push(note(
            "INFO",
            format!(
                "{pages_per_side} pages per side means each sheet is folded {folds_per_sheet} times \
                 into a signature rather than a single fold."
            ),
        ));
    }

    // The binding's own rule (a multiple of 4 for saddle stitch) is not the
    // whole story: the sheets must also come out full. 36 pages satisfies
    // "multiple of 4" yet still leaves four empty positions at 8 pages per
    // sheet, and saying "no blanks needed" there would be wrong.
    let sheet_capacity = sheet_count * pages_per_sheet;
    if sheet_capacity > total_pages {
        let short = sheet_capacity - total_pages;
        notes.push(note(
            "WARNING",
            format!(
                "{sheet_count} sheets hold {sheet_capacity} pages, so {short} position(s) are left \
                 empty by {total_pages} pages. Add {short} blank page(s), or change the pages per \
                 side so the sheets come out full."
            ),
        ));
    }

    // A folded sheet has to carry pages on both sides; say so here rather
    // than letting the user find out at the save step.
    if profile.folded && !is_duplex {
        notes.push(note(
            "ERROR",
            format!(
                "{} cannot be imposed single-sided: the back of every fold carries pages too. \
                 Choose double-sided printing.",
                profile.name
            ),
        ));
    }

    Ok(BookletPlan {
        profile,
        duplex,
        pages_per_side,
        pages_per_sheet,
        source_pages,
        blanks_needed: blanks,
        total_pages,
        sheet_count,
        folds_per_sheet,
        uses_printer_spreads,
        spine_width_mm,
        caliper_mm: caliper,
        notes,
    })
}

/// Printer-spread order when the plan uses the classic saddle-stitch fold.
pub fn plan_spreads(plan: &BookletPlan) -> Result<Vec<crate::print_calc::booklet::SheetSpread>, String> {
    if !plan.uses_printer_spreads {
        return Ok(vec![]);
    }
    saddle_stitch_order(plan.source_pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(binding: BindingType, pages: u32, per_side: u32, mode: DuplexMode, landscape: bool) -> BookletPlan {
        booklet_plan(binding, pages, per_side, mode, landscape, 80.0).unwrap()
    }

    /// 36 pages at 4 per side satisfies "multiple of 4" but still leaves
    /// four empty positions across five eight-page sheets.
    #[test]
    fn partly_filled_sheets_are_reported_even_when_the_binding_rule_passes() {
        let p = plan(BindingType::SaddleStitch, 36, 4, DuplexMode::LongEdge, false);
        assert_eq!(p.sheet_count, 5);
        assert_eq!(p.pages_per_sheet, 8);
        assert_eq!(p.blanks_needed, 0, "the multiple-of-4 rule is satisfied");
        let empty = p
            .notes
            .iter()
            .find(|n| n.message.contains("positions are left empty") || n.message.contains("position(s) are left"))
            .expect("the four empty positions must be reported");
        assert_eq!(empty.severity, "WARNING");
        assert!(empty.message.contains('4'));
        assert!(
            !p.notes.iter().any(|n| n.message.contains("no blanks needed")),
            "must not claim a clean fit while four positions sit empty"
        );
    }

    #[test]
    fn full_sheets_still_report_a_clean_fit() {
        let p = plan(BindingType::SaddleStitch, 32, 4, DuplexMode::LongEdge, false);
        assert_eq!(p.sheet_count * p.pages_per_sheet, p.total_pages);
        assert!(p.notes.iter().any(|n| n.message.contains("no blanks needed")));
        assert!(!p.notes.iter().any(|n| n.message.contains("left empty")));
    }

    /// Folded work needs both sides of the sheet, and the plan has to say so
    /// before the user commits to a configuration.
    #[test]
    fn folded_work_printed_on_one_side_is_refused_up_front() {
        let p = plan(BindingType::SaddleStitch, 36, 4, DuplexMode::Simplex, false);
        let blocked = p
            .notes
            .iter()
            .find(|n| n.message.contains("cannot be imposed single-sided"))
            .expect("a folded sheet carries pages on its reverse");
        assert_eq!(blocked.severity, "ERROR");

        // Four pages a side, double-sided, is now imposed like any other
        // fold count, so nothing is refused.
        let fine = plan(BindingType::SaddleStitch, 36, 4, DuplexMode::LongEdge, false);
        assert!(!fine.notes.iter().any(|n| n.severity == "ERROR"));
    }

    #[test]
    fn scenario_a_twenty_page_a5_booklet() {
        // 20 A5 pages, 2-up duplex on A4 landscape -> 5 sheets.
        let p = plan(BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true);
        assert_eq!(p.sheet_count, 5);
        assert_eq!(p.pages_per_sheet, 4);
        assert_eq!(p.blanks_needed, 0);
        assert_eq!(p.folds_per_sheet, 1);
        assert!(p.uses_printer_spreads);
        assert!(p.duplex.is_recommended);
        assert_eq!(plan_spreads(&p).unwrap().len(), 5);
    }

    #[test]
    fn scenario_b_twenty_two_pages_warns_and_pads() {
        let p = plan(BindingType::SaddleStitch, 22, 2, DuplexMode::ShortEdge, true);
        assert_eq!(p.blanks_needed, 2);
        assert_eq!(p.total_pages, 24);
        assert_eq!(p.sheet_count, 6);
        assert!(p.notes.iter().any(|n| n.severity == "WARNING" && n.message.contains("blank")));
    }

    #[test]
    fn simplex_saddle_stitch_is_an_error() {
        let p = plan(BindingType::SaddleStitch, 20, 2, DuplexMode::Simplex, true);
        assert!(p.notes.iter().any(|n| n.severity == "ERROR"));
    }

    #[test]
    fn wrong_flip_edge_is_flagged() {
        let p = plan(BindingType::SaddleStitch, 20, 2, DuplexMode::LongEdge, true);
        assert!(p.duplex.back_side_inverted);
        assert!(p.notes.iter().any(|n| n.message.contains("180 degrees")));
    }

    #[test]
    fn perfect_bound_book_gets_a_spine_and_no_folds() {
        // 200 pages -> 100 leaves; spine = 100 x caliper.
        let p = plan(BindingType::Perfect, 200, 1, DuplexMode::LongEdge, false);
        assert_eq!(p.sheet_count, 100);
        assert_eq!(p.folds_per_sheet, 0);
        assert!(!p.uses_printer_spreads);
        let spine = p.spine_width_mm.unwrap();
        assert!((spine - 100.0 * p.caliper_mm).abs() < 1e-9);
    }

    #[test]
    fn perfect_binding_pads_to_whole_leaves_only() {
        let p = plan(BindingType::Perfect, 201, 1, DuplexMode::LongEdge, false);
        assert_eq!(p.blanks_needed, 1);
        assert_eq!(p.total_pages, 202);
    }

    #[test]
    fn punched_bindings_never_pad_and_allow_simplex() {
        let p = plan(BindingType::Spiral, 37, 1, DuplexMode::Simplex, false);
        assert_eq!(p.blanks_needed, 0);
        assert_eq!(p.sheet_count, 37);
        assert!(!p.notes.iter().any(|n| n.severity == "ERROR"));
    }

    #[test]
    fn spiral_duplex_halves_the_sheet_count() {
        let p = plan(BindingType::Spiral, 40, 1, DuplexMode::LongEdge, false);
        assert_eq!(p.sheet_count, 20);
    }

    #[test]
    fn four_up_duplex_folds_twice_and_holds_eight_pages() {
        let p = plan(BindingType::SaddleStitch, 32, 4, DuplexMode::ShortEdge, true);
        assert_eq!(p.pages_per_sheet, 8);
        assert_eq!(p.sheet_count, 4);
        assert_eq!(p.folds_per_sheet, 2);
        // Beyond the single fold the classic spread order no longer applies.
        assert!(!p.uses_printer_spreads);
        assert!(plan_spreads(&p).unwrap().is_empty());
    }

    #[test]
    fn hardcover_has_spine_and_creep() {
        let p = plan(BindingType::Hardcover, 200, 2, DuplexMode::ShortEdge, true);
        assert!(p.spine_width_mm.is_some());
        assert!(p.profile.creep_applies);
        assert!(p.notes.iter().any(|n| n.message.contains("creep")));
    }

    #[test]
    fn excessive_page_count_warns() {
        let p = plan(BindingType::SaddleStitch, 200, 2, DuplexMode::ShortEdge, true);
        assert!(p.notes.iter().any(|n| n.message.contains("practical maximum")));
    }

    #[test]
    fn invalid_inputs_error() {
        assert!(booklet_plan(BindingType::Perfect, 0, 2, DuplexMode::LongEdge, false, 80.0).is_err());
        assert!(booklet_plan(BindingType::Perfect, 10, 0, DuplexMode::LongEdge, false, 80.0).is_err());
    }
}
