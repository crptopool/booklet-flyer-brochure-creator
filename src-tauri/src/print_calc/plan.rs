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
    /// True when the cover is a separate wrap: its own dedicated sheet,
    /// printed on the outside only, folded around the text block.
    pub separate_cover: bool,
    /// Positions the cover wrap has — 4 faces, or 0 when there is no
    /// separate cover. Only the two outside faces are ever printed; the
    /// inside of the wrap is always blank.
    pub cover_pages: u32,
    /// The document's own pages printed on the cover's outside, in order
    /// (front cover, back cover) — empty when the cover is blank and
    /// carries none of the manuscript.
    pub cover_source_pages: Vec<u32>,
    /// Pages the user marked as chapter starts, which must land on a
    /// right-hand (odd) reading position with a blank behind them.
    pub recto_pages: Vec<u32>,
    /// Blanks inserted to put the chapter starts on the right-hand side —
    /// separate from `blanks_needed`, which pads the end to the binding's
    /// page-count rule.
    pub chapter_blanks: u32,
    /// Pages in the text block, which is everything the cover does not carry.
    pub text_pages: u32,
    /// Sheets of text stock.
    pub text_sheet_count: u32,
    /// Sheets of cover stock: one wrap, or none.
    pub cover_sheet_count: u32,
    /// Weight of the cover stock, when it differs from the text.
    pub cover_gsm: Option<f64>,
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
    // Whether the booklet gets a wrap-around cover as its own dedicated
    // sheet. Independent of the paper: a cover on the same stock is still
    // its own sheet, printed on the outside only.
    with_cover: bool,
    // Weight of the cover stock when it differs from the text; `None`
    // means the cover prints on the same paper as everything else.
    cover_gsm: Option<f64>,
    // The document's own pages to print on the cover's outside: front
    // cover, then back cover. `None` or empty means the cover is blank
    // and every source page stays in the text block; when given, must be
    // exactly two distinct page numbers within `1..=source_pages`. The
    // inside of the wrap is always blank either way.
    cover_source_pages: Option<Vec<u32>>,
    // Chapter starts: pages that must open on a right-hand (odd) reading
    // position, each with a blank behind it. Blanks are inserted into the
    // body to make that happen.
    recto_pages: Option<Vec<u32>>,
) -> Result<BookletPlan, String> {
    if source_pages == 0 {
        return Err("page count must be positive".into());
    }
    if pages_per_side == 0 {
        return Err("pages per side must be positive".into());
    }
    let cover_source_pages = cover_source_pages.unwrap_or_default();
    if !cover_source_pages.is_empty() {
        if cover_source_pages.len() != 2 {
            return Err(format!(
                "the cover's outside has two printed faces (front cover, back cover) — \
                 {} page(s) were given",
                cover_source_pages.len()
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for &p in &cover_source_pages {
            if p == 0 || p > source_pages {
                return Err(format!(
                    "cover page {p} is outside the document's {source_pages} pages"
                ));
            }
            if !seen.insert(p) {
                return Err(format!("page {p} cannot be on the cover twice"));
            }
        }
    }

    let profile = binding_profile(binding);
    let duplex = duplex_plan(duplex_mode, sheet_is_landscape);
    let is_duplex = duplex_mode != DuplexMode::Simplex;

    let mut recto_pages = recto_pages.unwrap_or_default();
    recto_pages.sort_unstable();
    recto_pages.dedup();
    for &p in &recto_pages {
        if p == 0 || p > source_pages {
            return Err(format!(
                "chapter-start page {p} is outside the document's {source_pages} pages"
            ));
        }
    }

    // A separate cover is a wrap of its own: one dedicated sheet — the
    // first of the job — printed on its outside only and folded around the
    // text block. The paper is a free choice: same stock as the text or a
    // heavier one, the geometry is identical either way. By default the
    // wrap carries none of the manuscript's pages — a plus cover is
    // normally blank card, or artwork laid out on the Cover Creator screen
    // — but the user may instead designate the front and back cover from
    // the document's own pages, which pulls exactly those out of the text
    // block. Only folded work wraps this way — a glued or punched cover is
    // a different job, and the Cover Creator handles it.
    let separate_cover = with_cover && profile.folded;
    // Only take pages out of the body when there is actually somewhere for
    // them to go; an exclusion list supplied without a separate cover is
    // simply not acted on.
    let cover_source_pages = if separate_cover { cover_source_pages } else { Vec::new() };

    // A chapter start on a cover page is a contradiction — the cover is
    // not part of the body, so there is no reading position to force.
    if let Some(&clash) = recto_pages.iter().find(|p| cover_source_pages.contains(p)) {
        return Err(format!(
            "page {clash} is on the cover, so it cannot also be a chapter start in the body"
        ));
    }
    // Forcing a right-hand start only means something in a bound book of
    // facing pages; punched or single-sided work has no verso to avoid.
    let recto_requested = !recto_pages.is_empty();
    let recto_pages = if profile.folded { recto_pages } else { Vec::new() };

    // Pages the cover has taken out of the manuscript are not part of the
    // body being folded, so the binding's page-count rule (and any padding
    // blanks) apply to what is left, not to the document as a whole. The
    // body is laid out as reading positions first — chapter starts pull in
    // their blanks here — and the rule then pads what that produces.
    let body_pages = source_pages - cover_source_pages.len() as u32;
    let slots = body_slots(source_pages, &cover_source_pages, &recto_pages);
    let chapter_blanks = slots.len() as u32 - body_pages;
    let blanks = blanks_for_binding(binding, slots.len() as u32)?;
    let total_pages = slots.len() as u32 + blanks;

    let pages_per_sheet = pages_per_side * if is_duplex { 2 } else { 1 };

    // Blank (or designated) positions the wrap has, not extra pages of text.
    let cover_pages = if separate_cover { 4 } else { 0 };
    let text_pages = total_pages;
    let text_sheet_count = text_pages.div_ceil(pages_per_sheet);
    let cover_sheet_count = u32::from(separate_cover);
    let sheet_count = text_sheet_count + cover_sheet_count;

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
        // The rule applies to the body being folded, so when the cover has
        // taken pages — or chapter starts have pulled in blanks — the
        // message must count what is actually laid out; telling the user
        // that 36 is not a multiple of 4 would be nonsense.
        let counted = match (cover_source_pages.is_empty(), chapter_blanks == 0) {
            (true, true) => format!("{source_pages} pages"),
            (false, true) => format!(
                "{body_pages} pages (after {} moved to the cover)",
                cover_source_pages.len()
            ),
            (true, false) => format!(
                "a body of {} positions ({body_pages} pages plus {chapter_blanks} chapter blanks)",
                slots.len()
            ),
            (false, false) => format!(
                "a body of {} positions ({body_pages} pages after {} moved to the cover, \
                 plus {chapter_blanks} chapter blanks)",
                slots.len(),
                cover_source_pages.len()
            ),
        };
        notes.push(note(
            "WARNING",
            format!(
                "{counted} is not a multiple of {} for {} — {blanks} blank page(s) must be added. \
                 Choose where they go; pages are never added silently.",
                profile.page_count_rule.multiple(),
                profile.name
            ),
        ));
    } else if text_sheet_count * pages_per_sheet == text_pages {
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
    let sheet_capacity = text_sheet_count * pages_per_sheet;
    if sheet_capacity > text_pages {
        let short = sheet_capacity - text_pages;
        let what = if separate_cover { "text sheets hold" } else { "sheets hold" };
        notes.push(note(
            "WARNING",
            format!(
                "{text_sheet_count} {what} {sheet_capacity} pages, so {short} position(s) are left \
                 empty by {text_pages} pages. Add {short} blank page(s), or change the pages per \
                 side so the sheets come out full."
            ),
        ));
    }

    if !recto_pages.is_empty() {
        notes.push(note(
            "INFO",
            format!(
                "Page{} {} start{} on a right-hand page: {chapter_blanks} blank(s) are inserted \
                 — one before any chapter that would otherwise open on the left, and one behind \
                 each chapter page so its back stays empty.",
                if recto_pages.len() == 1 { "" } else { "s" },
                recto_pages.iter().map(u32::to_string).collect::<Vec<_>>().join(", "),
                if recto_pages.len() == 1 { "s" } else { "" },
            ),
        ));
    } else if recto_requested {
        notes.push(note(
            "INFO",
            format!(
                "Right-hand chapter starts apply to folded booklets; {} keeps pages in their \
                 normal order, so the selection is not acted on.",
                profile.name
            ),
        ));
    }

    if separate_cover {
        if cover_source_pages.is_empty() {
            notes.push(note(
                "INFO",
                "The cover is sheet 1 of the job: printed on its outside only, inside blank. \
                 None of the manuscript's pages go on it — the sheet carries cut and fold \
                 marks so you know where to trim your card or pre-printed cover.",
            ));
        } else {
            notes.push(note(
                "INFO",
                format!(
                    "The cover is sheet 1 of the job: page {} as the front cover and page {} as \
                     the back, side by side on the outside; the inside is blank. Those 2 pages \
                     are removed from the {text_pages}-page text block, not duplicated in it.",
                    cover_source_pages[0],
                    cover_source_pages
                        .get(1)
                        .copied()
                        .unwrap_or(cover_source_pages[0]),
                ),
            ));
        }
        match cover_gsm {
            Some(cover) => {
                notes.push(note(
                    "INFO",
                    format!(
                        "The cover stock differs: feed one sheet of {cover:.0} GSM for sheet 1, \
                         or print sheet 1 as its own run on the cover stock; the text prints on \
                         {gsm:.0} GSM."
                    ),
                ));
                if cover <= gsm {
                    notes.push(note(
                        "INFO",
                        format!(
                            "The cover stock ({cover:.0} GSM) is no heavier than the text \
                             ({gsm:.0} GSM). A cover is usually the heavier of the two."
                        ),
                    ));
                }
            }
            None => notes.push(note(
                "INFO",
                format!(
                    "The cover prints on the same {gsm:.0} GSM paper as the text, so the whole \
                     job runs as one stack — sheet 1 simply comes out with a blank back."
                ),
            )),
        }
    } else if with_cover && !profile.folded {
        notes.push(note(
            "INFO",
            format!(
                "{} does not wrap a folded cover — its cover is a separate component. Lay it out \
                 on the Cover Creator screen.",
                profile.name
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
        separate_cover,
        cover_pages,
        cover_source_pages,
        recto_pages,
        chapter_blanks,
        text_pages,
        text_sheet_count,
        cover_sheet_count,
        cover_gsm: if separate_cover { cover_gsm } else { None },
        spine_width_mm,
        caliper_mm: caliper,
        notes,
    })
}

/// Printer-spread order when the plan uses the classic saddle-stitch fold.
///
/// Empty when the cover has pulled pages out of the manuscript: this table
/// numbers a plain 1..N document, and a document with pages missing from
/// the middle of that range needs the exclusion-aware imposition instead,
/// which is what the "Printed sheets" simulation and the export both use.
pub fn plan_spreads(plan: &BookletPlan) -> Result<Vec<crate::print_calc::booklet::SheetSpread>, String> {
    if !plan.uses_printer_spreads || !plan.cover_source_pages.is_empty() || plan.chapter_blanks > 0 {
        return Ok(vec![]);
    }
    saddle_stitch_order(plan.source_pages)
}

/// The body of the book as ordered reading positions.
///
/// Position 1 is the body's first right-hand page; odd positions are
/// rectos, even ones versos, and each even position is the back of the
/// leaf before it. Cover pages are simply absent — removing one leaves no
/// hole and shifts nothing. A chapter-start page gets a blank pushed in
/// front of it when it would otherwise open on the left, and always gets a
/// blank behind it so the back of the chapter page stays empty.
///
/// This is the one place that order is decided: the plan's counts, the
/// imposition and the bound-book preview all read it from here, so they
/// cannot disagree about where a blank sits.
pub fn body_slots(
    source_pages: u32,
    cover_source_pages: &[u32],
    recto_pages: &[u32],
) -> Vec<Option<u32>> {
    let mut slots: Vec<Option<u32>> = Vec::with_capacity(source_pages as usize);
    for p in 1..=source_pages {
        if cover_source_pages.contains(&p) {
            continue;
        }
        if recto_pages.contains(&p) {
            if slots.len() % 2 == 1 {
                slots.push(None);
            }
            slots.push(Some(p));
            slots.push(None);
        } else {
            slots.push(Some(p));
        }
    }
    slots
}

impl BookletPlan {
    /// The text block as reading positions — see [`body_slots`]. Trailing
    /// rule-padding blanks are not included; positions past the end of the
    /// list are blank by construction wherever the list is consumed.
    pub fn body_slots(&self) -> Vec<Option<u32>> {
        body_slots(self.source_pages, &self.cover_source_pages, &self.recto_pages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(binding: BindingType, pages: u32, per_side: u32, mode: DuplexMode, landscape: bool) -> BookletPlan {
        booklet_plan(binding, pages, per_side, mode, landscape, 80.0, false, None, None, None).unwrap()
    }

    /// Designating cover pages pulls exactly those out of the body, and the
    /// binding's page-count rule then applies to what's left, not the raw
    /// document length.
    #[test]
    fn designated_cover_pages_are_removed_from_the_body() {
        let p = booklet_plan(
            BindingType::SaddleStitch,
            26,
            2,
            DuplexMode::ShortEdge,
            true,
            80.0,
            true,
            Some(200.0),
            Some(vec![1, 26]), None,
        )
        .unwrap();
        assert_eq!(p.cover_source_pages, vec![1, 26]);
        assert_eq!(p.text_pages, 24, "26 supplied minus 2 on the cover");
        assert_eq!(p.blanks_needed, 0);
        assert!(p.notes.iter().any(|n| {
            n.message.contains("page 1 as the front cover") && n.message.contains("page 26 as the back")
        }));
    }

    /// The blank rule applies to the body, not to the document as a whole:
    /// a multiple-of-four document stops being one when the cover takes
    /// two of its pages.
    #[test]
    fn the_blank_rule_is_checked_against_the_body_not_the_document() {
        // 24 pages total; removing 2 for the cover leaves 22, which is not
        // a multiple of 4 — so 2 blanks are expected, not a blanks count
        // computed against 24.
        let p = booklet_plan(
            BindingType::SaddleStitch,
            24,
            2,
            DuplexMode::ShortEdge,
            true,
            80.0,
            true,
            Some(200.0),
            Some(vec![1, 24]), None,
        )
        .unwrap();
        assert_eq!(p.text_pages, 24, "22 body pages padded to the next multiple of 4");
        assert_eq!(p.blanks_needed, 2);
    }

    /// Two page numbers are required — front cover and back cover — because
    /// only the outside of the wrap is printed; its inside is always blank.
    #[test]
    fn cover_pages_must_be_exactly_two() {
        let err = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, true, Some(200.0),
            Some(vec![1, 2, 19, 20]), None,
        )
        .unwrap_err();
        assert!(err.contains("two printed faces"), "{err}");
    }

    /// A page outside the document, or repeated, is refused before any
    /// sheet is built — not discovered later as a missing or doubled page.
    #[test]
    fn cover_pages_must_be_real_and_distinct() {
        let out_of_range = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, true, Some(200.0),
            Some(vec![1, 21]), None,
        )
        .unwrap_err();
        assert!(out_of_range.contains("outside the document"), "{out_of_range}");

        let repeated = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, true, Some(200.0),
            Some(vec![5, 5]), None,
        )
        .unwrap_err();
        assert!(repeated.contains("cannot be on the cover twice"), "{repeated}");
    }

    /// Designating cover pages without asking for a cover at all is quietly
    /// ignored rather than mysteriously shrinking the body.
    #[test]
    fn cover_pages_are_ignored_without_a_separate_cover() {
        let p = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, false, None,
            Some(vec![1, 20]), None,
        )
        .unwrap();
        assert!(!p.separate_cover);
        assert!(p.cover_source_pages.is_empty());
        assert_eq!(p.text_pages, 20);
    }

    /// The cover no longer depends on a different paper: the same stock
    /// still gets a dedicated cover sheet, and the plan says the job can
    /// run as one stack.
    #[test]
    fn a_cover_on_the_same_stock_is_still_its_own_sheet() {
        let p = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, true, None, None, None,
        )
        .unwrap();
        assert!(p.separate_cover);
        assert_eq!(p.cover_gsm, None, "same stock — no separate weight");
        assert_eq!(p.cover_sheet_count, 1);
        assert_eq!(p.sheet_count, 6, "five text sheets plus the cover");
        assert!(p.notes.iter().any(|n| n.message.contains("same 80 GSM paper")));
    }

    /// A different weight is a stock note, not a geometry change: the sheet
    /// counts match the same-stock case exactly.
    #[test]
    fn a_different_cover_weight_only_changes_the_stock_note() {
        let same = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, true, None, None, None,
        )
        .unwrap();
        let heavier = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, true, Some(200.0), None, None,
        )
        .unwrap();
        assert_eq!(same.sheet_count, heavier.sheet_count);
        assert_eq!(same.text_sheet_count, heavier.text_sheet_count);
        assert_eq!(heavier.cover_gsm, Some(200.0));
        assert!(heavier.notes.iter().any(|n| n.message.contains("feed one sheet of 200 GSM")));
    }

    /// A chapter start that would fall on a left-hand page gets a blank
    /// pushed in front of it, and every chapter page gets a blank behind
    /// it so its back stays empty.
    #[test]
    fn chapter_starts_land_on_odd_positions_with_blank_backs() {
        // Page 4 would naturally read at position 4 (a left-hand page).
        let slots = body_slots(8, &[], &[4]);
        // 1,2,3 | blank to skip position 4 | 4 on position 5 | blank back | 5..8
        assert_eq!(
            slots,
            vec![
                Some(1), Some(2), Some(3), None, Some(4), None,
                Some(5), Some(6), Some(7), Some(8),
            ]
        );
        assert_eq!(slots.iter().position(|s| *s == Some(4)).unwrap() % 2, 0, "odd 1-based position");
    }

    /// A chapter already destined for a right-hand page needs no blank in
    /// front — only the blank behind it.
    #[test]
    fn a_chapter_already_on_the_right_only_gets_the_blank_back() {
        let slots = body_slots(6, &[], &[3]);
        assert_eq!(slots, vec![Some(1), Some(2), Some(3), None, Some(4), Some(5), Some(6)]);
    }

    /// Cover pages leave the body before positions are counted, so a
    /// chapter's position is measured in the book the reader holds.
    #[test]
    fn chapter_positions_are_counted_after_the_cover_leaves() {
        // Cover takes 1 and 8; page 4 then reads at body position 3 — a
        // right-hand page already, so only its back-blank is added.
        let slots = body_slots(8, &[1, 8], &[4]);
        assert_eq!(slots, vec![Some(2), Some(3), Some(4), None, Some(5), Some(6), Some(7)]);
    }

    /// The plan counts chapter blanks apart from rule padding, and the
    /// page-count rule applies to the laid-out body.
    #[test]
    fn chapter_blanks_are_counted_and_then_padded() {
        let p = booklet_plan(
            BindingType::SaddleStitch, 8, 2, DuplexMode::ShortEdge, true, 80.0, false, None, None,
            Some(vec![4]),
        )
        .unwrap();
        assert_eq!(p.recto_pages, vec![4]);
        assert_eq!(p.chapter_blanks, 2);
        // 8 pages + 2 chapter blanks = 10 positions, padded to 12.
        assert_eq!(p.blanks_needed, 2);
        assert_eq!(p.total_pages, 12);
        assert_eq!(p.sheet_count, 3);
        assert!(p.notes.iter().any(|n| n.message.contains("right-hand page")));
    }

    /// A chapter page cannot also be a cover page — there is no body
    /// position to force for a page that is not in the body.
    #[test]
    fn a_chapter_start_on_the_cover_is_refused() {
        let err = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, true, None,
            Some(vec![1, 20]),
            Some(vec![1]),
        )
        .unwrap_err();
        assert!(err.contains("cannot also be a chapter start"), "{err}");
    }

    /// Punched and single-page work has no facing pages, so the selection
    /// is dropped with a note rather than silently changing the layout.
    #[test]
    fn recto_starts_are_ignored_for_unfolded_bindings() {
        let p = booklet_plan(
            BindingType::Spiral, 10, 1, DuplexMode::Simplex, false, 80.0, false, None, None,
            Some(vec![3]),
        )
        .unwrap();
        assert!(p.recto_pages.is_empty());
        assert_eq!(p.chapter_blanks, 0);
        assert!(p.notes.iter().any(|n| n.message.contains("not acted on")));
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
        assert!(booklet_plan(BindingType::Perfect, 0, 2, DuplexMode::LongEdge, false, 80.0, false, None, None, None).is_err());
        assert!(booklet_plan(BindingType::Perfect, 10, 0, DuplexMode::LongEdge, false, 80.0, false, None, None, None).is_err());
    }
}
