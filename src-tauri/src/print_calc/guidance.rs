//! What to choose in the printer dialog for an exported, imposed file.
//!
//! The exported PDF is already arranged — every correction (nesting, back
//! side rotation, blanks) is baked into its pages. All that can still go
//! wrong happens in the print dialog: scaling, the wrong flip edge, the
//! cover stock in the wrong tray. This module turns the plan into the
//! exact choices to make there, so the guidance always matches the file
//! rather than being a generic leaflet.

use serde::Serialize;

use crate::print_calc::plan::BookletPlan;
use crate::print_calc::presets::DuplexMode;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrintStep {
    /// The dialog control or action this step is about.
    pub title: String,
    /// What to choose, and why it matters for this particular file.
    pub detail: String,
}

fn step(title: &str, detail: impl Into<String>) -> PrintStep {
    PrintStep {
        title: title.to_string(),
        detail: detail.into(),
    }
}

/// The checklist for printing a plan's exported file.
///
/// `sheet_name` is the paper the user chose (for example "A4"), named so
/// the steps can say "select A4" rather than "select your paper".
pub fn printing_guidance(
    plan: &BookletPlan,
    sheet_name: &str,
    sheet_is_landscape: bool,
) -> Vec<PrintStep> {
    let mut steps = Vec::new();
    let duplex = plan.pages_per_sheet > plan.pages_per_side;
    let orientation = if sheet_is_landscape { "landscape" } else { "portrait" };

    // Sides of the file: the cover is pages 1-2, the text block follows.
    let text_sides = plan.text_sheet_count * if duplex { 2 } else { 1 };
    let (text_from, text_to) = if plan.separate_cover {
        (3, 2 + text_sides)
    } else {
        (1, text_sides)
    };

    steps.push(step(
        "Paper size",
        format!(
            "Select {sheet_name} — the file's pages are exact {sheet_name} {orientation} sheets. \
             If the dialog offers a paper *source*, pick the tray that actually holds it."
        ),
    ));

    steps.push(step(
        "Scale",
        "Choose 100% / Actual size. Turn OFF \"fit to page\", \"shrink to printable area\" and \
         \"auto-rotate and centre\". A scaled sheet folds and cuts in the wrong places — this is \
         the single most common cause of a misaligned booklet.",
    ));

    match (duplex, plan.duplex.mode) {
        (false, _) | (_, DuplexMode::Simplex) => {
            steps.push(step(
                "Two-sided",
                "Turn two-sided printing OFF — this layout puts everything on the front of each sheet.",
            ));
        }
        (true, _) => {
            let mut detail = format!(
                "Turn two-sided printing ON and choose \"{}\" — the same flip you configured here. \
                 This exact choice matters: ",
                plan.duplex.flip_axis
            );
            if plan.duplex.back_side_inverted {
                detail.push_str(
                    "the file's back sides are already rotated 180° to survive that flip, so \
                     choosing the other edge prints every back side upside down.",
                );
            } else {
                detail.push_str(
                    "the file's back sides are built for that flip, so choosing the other edge \
                     prints every back side upside down.",
                );
            }
            steps.push(step("Two-sided", detail));
            steps.push(step(
                "No duplex unit?",
                format!(
                    "Print only the odd pages, put the printed stack back in, then print only the \
                     even pages. Test with one sheet first: the way you reinsert the paper must \
                     reproduce \"{}\" — same edge, same face.",
                    plan.duplex.flip_axis
                ),
            ));
        }
    }

    if plan.separate_cover {
        match plan.cover_gsm {
            Some(gsm) => steps.push(step(
                "Cover stock",
                format!(
                    "Two jobs from this one file. First print pages {text_from}–{text_to} (the text) \
                     on the ordinary paper. Then load one sheet of {gsm:.0} GSM — the bypass or \
                     manual-feed tray is ideal for card — and print pages 1–2 (the cover; page 2 is \
                     deliberately blank)."
                ),
            )),
            None => steps.push(step(
                "Cover sheet",
                "Print the whole file as one job — the cover is sheet 1 and its back is \
                 deliberately blank. It comes out ready to cut and fold.",
            )),
        }
    }

    steps.push(step(
        "Test first",
        "Print one sheet before the whole stack and check the marks: the dashed line is a fold, \
         the solid line is a cut, and both should land where the preview showed them.",
    ));

    if plan.profile.folded {
        let staple = plan.profile.name.contains("addle");
        steps.push(step(
            "Finish",
            format!(
                "Cut along the solid marks, fold on the dashed marks (the last fold is the spine), \
                 nest the text sheets inside one another{}{}",
                if plan.separate_cover { ", wrap the cover around the outside" } else { "" },
                if staple { ", and staple through the spine crease." } else { ", and bind." },
            ),
        ));
    }

    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print_calc::plan::booklet_plan;
    use crate::print_calc::presets::{BindingType, DuplexMode};

    fn covered_plan() -> BookletPlan {
        booklet_plan(
            BindingType::SaddleStitch, 36, 4, DuplexMode::LongEdge, false, 80.0, true, Some(200.0),
            Some(vec![1, 36]),
        )
        .unwrap()
    }

    #[test]
    fn the_flip_choice_names_the_configured_edge() {
        let steps = printing_guidance(&covered_plan(), "A4", false);
        let duplex = steps.iter().find(|s| s.title == "Two-sided").unwrap();
        assert!(duplex.detail.contains("ON"));
        assert!(
            duplex.detail.contains(&covered_plan().duplex.flip_axis),
            "the step must name the exact flip the file was built for"
        );
    }

    #[test]
    fn a_pre_rotated_back_side_is_explained() {
        // A short-edge flip on a portrait sheet turns about a horizontal
        // axis and inverts the backs; the file compensates — the guidance
        // must say so, or the user will think something is wrong when they
        // preview the file and see upside-down back sides.
        let plan = booklet_plan(
            BindingType::SaddleStitch, 36, 4, DuplexMode::ShortEdge, false, 80.0, true, Some(200.0),
            Some(vec![1, 36]),
        )
        .unwrap();
        assert!(plan.duplex.back_side_inverted);
        let steps = printing_guidance(&plan, "A4", false);
        let duplex = steps.iter().find(|s| s.title == "Two-sided").unwrap();
        assert!(duplex.detail.contains("already rotated 180°"), "{}", duplex.detail);
    }

    #[test]
    fn different_cover_stock_gets_the_two_job_page_ranges() {
        // 5 text sheets duplex = 10 sides, after the cover's 2: pages 3-12.
        let steps = printing_guidance(&covered_plan(), "A4", false);
        let cover = steps.iter().find(|s| s.title == "Cover stock").unwrap();
        assert!(cover.detail.contains("pages 3–12"), "{}", cover.detail);
        assert!(cover.detail.contains("pages 1–2"), "{}", cover.detail);
        assert!(cover.detail.contains("200 GSM"), "{}", cover.detail);
    }

    #[test]
    fn same_stock_cover_is_one_job() {
        let plan = booklet_plan(
            BindingType::SaddleStitch, 36, 4, DuplexMode::LongEdge, false, 80.0, true, None,
            Some(vec![1, 36]),
        )
        .unwrap();
        let steps = printing_guidance(&plan, "A4", false);
        let cover = steps.iter().find(|s| s.title == "Cover sheet").unwrap();
        assert!(cover.detail.contains("one job"), "{}", cover.detail);
    }

    #[test]
    fn simplex_work_says_to_turn_duplex_off() {
        let plan = booklet_plan(
            BindingType::Spiral, 10, 1, DuplexMode::Simplex, false, 80.0, false, None, None,
        )
        .unwrap();
        let steps = printing_guidance(&plan, "A4", false);
        let duplex = steps.iter().find(|s| s.title == "Two-sided").unwrap();
        assert!(duplex.detail.contains("OFF"));
        assert!(!steps.iter().any(|s| s.title == "No duplex unit?"));
    }

    #[test]
    fn scaling_and_paper_are_always_present() {
        let steps = printing_guidance(&covered_plan(), "A4", false);
        assert!(steps.iter().any(|s| s.title == "Paper size" && s.detail.contains("A4")));
        assert!(steps.iter().any(|s| s.title == "Scale" && s.detail.contains("100%")));
    }

    #[test]
    fn a_self_cover_booklet_has_no_cover_step() {
        let plan = booklet_plan(
            BindingType::SaddleStitch, 20, 2, DuplexMode::ShortEdge, true, 80.0, false, None, None,
        )
        .unwrap();
        let steps = printing_guidance(&plan, "A4", true);
        assert!(!steps.iter().any(|s| s.title.starts_with("Cover")));
        // But the finishing step still describes the saddle.
        assert!(steps.iter().any(|s| s.title == "Finish" && s.detail.contains("staple")));
    }
}
