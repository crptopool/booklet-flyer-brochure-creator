//! Duplex printing logic: flip edge, back-side orientation and manual
//! duplex reinsertion instructions.
//!
//! When a sheet is turned over, the axis it is turned about decides
//! whether the reverse side comes out upright or upside down. Getting
//! this wrong is the single most common cause of ruined duplex jobs, so
//! the required back-side rotation is computed here deterministically
//! rather than left to the user to guess.

use serde::{Deserialize, Serialize};

use crate::print_calc::presets::DuplexMode;

/// Complete duplex plan for a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplexPlan {
    pub mode: DuplexMode,
    /// Human-readable name of the flip axis.
    pub flip_axis: String,
    /// Degrees the back side must be rotated so it prints upright.
    pub back_side_rotation: i64,
    /// True when the reverse side needs a 180 degree rotation.
    pub back_side_inverted: bool,
    /// True when this is the orientation-correct choice for the sheet.
    pub is_recommended: bool,
    pub explanation: String,
    /// Step-by-step instructions; only populated for manual duplex.
    pub manual_steps: Vec<String>,
}

/// Rotation (degrees) that must be applied to the back side so its
/// content prints upright after the sheet is flipped.
///
/// A sheet is turned about one of its edges. Turning about a *vertical*
/// axis preserves the up direction; turning about a *horizontal* axis
/// inverts it. Which physical edge is "long" depends on the sheet
/// orientation, so the same flip setting gives opposite results on
/// portrait and landscape sheets.
pub fn back_side_rotation(mode: DuplexMode, sheet_is_landscape: bool) -> i64 {
    match mode {
        // Nothing is flipped.
        DuplexMode::Simplex => 0,
        // Portrait: long edges are the left/right sides -> vertical axis
        // -> upright. Landscape: long edges are top/bottom -> inverted.
        DuplexMode::LongEdge => {
            if sheet_is_landscape {
                180
            } else {
                0
            }
        }
        // Exactly the inverse of long-edge flipping.
        DuplexMode::ShortEdge => {
            if sheet_is_landscape {
                0
            } else {
                180
            }
        }
        // The user reinserts the stack; instructions tell them which way.
        DuplexMode::Manual => 0,
    }
}

/// The flip setting that keeps both sides upright for a given sheet.
///
/// Portrait sheets want a long-edge flip (the familiar book-style
/// duplex); landscape sheets — which is what booklet imposition
/// produces — want a short-edge flip.
pub fn recommended_mode(sheet_is_landscape: bool) -> DuplexMode {
    if sheet_is_landscape {
        DuplexMode::ShortEdge
    } else {
        DuplexMode::LongEdge
    }
}

fn flip_axis(mode: DuplexMode, sheet_is_landscape: bool) -> &'static str {
    match mode {
        DuplexMode::Simplex => "None — single-sided",
        DuplexMode::Manual => "Chosen by the operator when reinserting",
        DuplexMode::LongEdge => {
            if sheet_is_landscape {
                "Horizontal axis (the long top/bottom edges)"
            } else {
                "Vertical axis (the long left/right edges)"
            }
        }
        DuplexMode::ShortEdge => {
            if sheet_is_landscape {
                "Vertical axis (the short left/right edges)"
            } else {
                "Horizontal axis (the short top/bottom edges)"
            }
        }
    }
}

/// Instructions for printing a manual-duplex job.
pub fn manual_duplex_steps(sheet_is_landscape: bool) -> Vec<String> {
    let reinsert = if sheet_is_landscape {
        "Rotate the stack 180 degrees in the plane of the paper (spin it, do not turn it over end-for-end), then place it back in the input tray."
    } else {
        "Flip the stack over about its long edge, like turning the page of a book, keeping the top edge at the top."
    };
    vec![
        "Print the front sides first — these are the odd-numbered sheet sides.".into(),
        "Take the printed stack out without shuffling it and note which edge fed first.".into(),
        reinsert.into(),
        "Print the reverse sides.".into(),
        "Before committing a long run, print the two-sheet test described below and confirm the \
         back side comes out upright and in the right position."
            .into(),
    ]
}

/// Full duplex plan for a sheet orientation and flip choice.
pub fn duplex_plan(mode: DuplexMode, sheet_is_landscape: bool) -> DuplexPlan {
    let rotation = back_side_rotation(mode, sheet_is_landscape);
    let recommended = recommended_mode(sheet_is_landscape);
    let orientation = if sheet_is_landscape { "landscape" } else { "portrait" };

    let explanation = match mode {
        DuplexMode::Simplex => {
            "Only the front of each sheet is printed. Use this when the reverse must stay blank, \
             or when the job will be duplexed by hand later."
                .to_string()
        }
        DuplexMode::Manual => format!(
            "The printer prints one side at a time and you reinsert the stack yourself. On {orientation} \
             sheets the reinsertion direction below is the one that keeps both sides upright."
        ),
        _ => {
            if rotation == 0 {
                format!(
                    "The sheet turns about a vertical axis, so the reverse side comes out upright \
                     with no correction. This is the correct choice for {orientation} sheets."
                )
            } else {
                format!(
                    "The sheet turns about a horizontal axis, so the reverse side would print \
                     upside down. Every back side is therefore rotated 180 degrees before \
                     imposition to compensate. On {orientation} sheets the other flip setting \
                     avoids this."
                )
            }
        }
    };

    DuplexPlan {
        mode,
        flip_axis: flip_axis(mode, sheet_is_landscape).to_string(),
        back_side_rotation: rotation,
        back_side_inverted: rotation != 0,
        is_recommended: mode == recommended,
        explanation,
        manual_steps: if mode == DuplexMode::Manual {
            manual_duplex_steps(sheet_is_landscape)
        } else {
            vec![]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_long_edge_keeps_back_upright() {
        assert_eq!(back_side_rotation(DuplexMode::LongEdge, false), 0);
    }

    #[test]
    fn portrait_short_edge_inverts_the_back() {
        assert_eq!(back_side_rotation(DuplexMode::ShortEdge, false), 180);
    }

    #[test]
    fn landscape_flips_are_the_mirror_of_portrait() {
        // A booklet sheet is landscape: short-edge flip is the upright one.
        assert_eq!(back_side_rotation(DuplexMode::ShortEdge, true), 0);
        assert_eq!(back_side_rotation(DuplexMode::LongEdge, true), 180);
    }

    #[test]
    fn simplex_never_rotates() {
        assert_eq!(back_side_rotation(DuplexMode::Simplex, true), 0);
        assert_eq!(back_side_rotation(DuplexMode::Simplex, false), 0);
    }

    #[test]
    fn recommendation_follows_sheet_orientation() {
        assert_eq!(recommended_mode(false), DuplexMode::LongEdge);
        assert_eq!(recommended_mode(true), DuplexMode::ShortEdge);
    }

    #[test]
    fn recommended_mode_is_always_the_upright_one() {
        for landscape in [true, false] {
            let mode = recommended_mode(landscape);
            assert_eq!(back_side_rotation(mode, landscape), 0);
            assert!(duplex_plan(mode, landscape).is_recommended);
        }
    }

    #[test]
    fn booklet_sheets_recommend_short_edge() {
        // Scenario A: A5 pages on A4 landscape sheets.
        let plan = duplex_plan(DuplexMode::ShortEdge, true);
        assert!(plan.is_recommended);
        assert!(!plan.back_side_inverted);
        assert!(plan.flip_axis.contains("Vertical"));
    }

    #[test]
    fn wrong_flip_is_flagged_and_compensated() {
        let plan = duplex_plan(DuplexMode::LongEdge, true);
        assert!(!plan.is_recommended);
        assert!(plan.back_side_inverted);
        assert_eq!(plan.back_side_rotation, 180);
    }

    #[test]
    fn manual_duplex_has_steps_others_do_not() {
        assert!(!duplex_plan(DuplexMode::Manual, false).manual_steps.is_empty());
        assert!(duplex_plan(DuplexMode::LongEdge, false).manual_steps.is_empty());
    }

    #[test]
    fn manual_instructions_differ_by_orientation() {
        assert_ne!(manual_duplex_steps(true)[2], manual_duplex_steps(false)[2]);
    }
}
