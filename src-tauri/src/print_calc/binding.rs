//! Booklet binding methods and their per-method settings.
//!
//! Each binding method imposes different constraints on page count, sheet
//! handling, gutter allowance and spine geometry. This module turns those
//! constraints into a single deterministic profile the UI can render and
//! validate against — no recommendation here comes from inference.

use serde::{Deserialize, Serialize};

use crate::print_calc::geometry::binding_margin_mm;
use crate::print_calc::presets::{BindingSide, BindingType};

/// How a binding method consumes pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageCountRule {
    /// Pages must be a multiple of 4 (folded, nested sheets).
    MultipleOfFour,
    /// Pages must be a multiple of 2 (each leaf prints two sides).
    MultipleOfTwo,
    /// Any page count is acceptable (single leaves).
    Any,
}

impl PageCountRule {
    pub fn multiple(&self) -> u32 {
        match self {
            PageCountRule::MultipleOfFour => 4,
            PageCountRule::MultipleOfTwo => 2,
            PageCountRule::Any => 1,
        }
    }
}

/// Complete settings profile for one binding method.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingProfile {
    pub binding: BindingType,
    /// Machine-readable key matching the serde representation.
    pub key: String,
    pub name: String,
    /// One-line description of the physical binding.
    pub description: String,
    /// How the pages are physically held together.
    pub mechanism: String,
    pub page_count_rule: PageCountRule,
    /// Sheets are folded and nested inside one another.
    pub folded: bool,
    /// Number of folds per sheet (0 for flat, glued or punched bindings).
    pub folds_per_sheet: u32,
    /// Creep compensation is meaningful for this binding.
    pub creep_applies: bool,
    /// The binding has a measurable spine that needs a width calculation.
    pub has_spine: bool,
    /// Content is punched, so a punch-safe clearance zone is required.
    pub punched: bool,
    /// Recommended gutter / inside margin in mm.
    pub recommended_binding_margin_mm: f64,
    /// Binding edges this method supports.
    pub allowed_sides: Vec<BindingSide>,
    /// Practical page range for the method.
    pub min_pages: u32,
    pub max_pages: u32,
    /// Whether the method normally requires duplex printing.
    pub requires_duplex: bool,
    /// Whether a separate cover is produced (printed on heavier stock).
    pub separate_cover: bool,
    /// Why the recommended values matter, per requirement 28.
    pub guidance: String,
    /// Typical use cases.
    pub typical_use: String,
}

/// Every booklet binding method the application supports.
pub const BOOKLET_BINDINGS: [BindingType; 5] = [
    BindingType::SaddleStitch,
    BindingType::Perfect,
    BindingType::Spiral,
    BindingType::WireO,
    BindingType::Hardcover,
];

/// Deterministic settings profile for a binding method.
pub fn binding_profile(binding: BindingType) -> BindingProfile {
    let margin = binding_margin_mm(binding);
    match binding {
        BindingType::SaddleStitch | BindingType::Staple => BindingProfile {
            binding,
            key: "saddle_stitch".into(),
            name: "Saddle stitch".into(),
            description: "Sheets are folded in half, nested inside one another and stapled through the centre fold.".into(),
            mechanism: "Wire staples through the spine fold".into(),
            page_count_rule: PageCountRule::MultipleOfFour,
            folded: true,
            folds_per_sheet: 1,
            creep_applies: true,
            has_spine: false,
            punched: false,
            recommended_binding_margin_mm: margin,
            allowed_sides: vec![BindingSide::Left, BindingSide::Right],
            min_pages: 4,
            max_pages: 64,
            requires_duplex: true,
            separate_cover: false,
            guidance: "Every sheet carries four pages, so the total must be a multiple of 4. \
                       Because inner sheets are wrapped by outer ones, they creep towards the \
                       fore-edge and need creep compensation above roughly 16 pages. Beyond \
                       about 64 pages the booklet will not lie flat and the fore-edge step \
                       becomes visible after trimming."
                .into(),
            typical_use: "Programmes, newsletters, thin catalogues, event booklets".into(),
        },
        BindingType::Perfect => BindingProfile {
            binding,
            key: "perfect".into(),
            name: "Perfect binding".into(),
            description: "Pages are stacked flat, the spine edge is ground and glued into a wrapped cover, producing a square spine.".into(),
            mechanism: "Hot-melt or PUR adhesive on a square spine".into(),
            page_count_rule: PageCountRule::MultipleOfTwo,
            folded: false,
            folds_per_sheet: 0,
            creep_applies: false,
            has_spine: true,
            punched: false,
            recommended_binding_margin_mm: margin,
            allowed_sides: vec![BindingSide::Left, BindingSide::Right],
            min_pages: 32,
            max_pages: 800,
            requires_duplex: true,
            separate_cover: true,
            guidance: "Content stays in normal reading order — no imposition is needed for the \
                       text block. The spine width must be calculated from the real page count \
                       and paper caliper before the cover can be laid out. A perfect-bound book \
                       does not open flat, so allow a generous gutter or inner content will \
                       disappear into the glue."
                .into(),
            typical_use: "Paperbacks, manuals, thick catalogues, annual reports".into(),
        },
        BindingType::Spiral => BindingProfile {
            binding,
            key: "spiral".into(),
            name: "Spiral / coil binding".into(),
            description: "A continuous plastic coil is threaded through a line of round punched holes along the binding edge.".into(),
            mechanism: "Threaded plastic coil through round punched holes".into(),
            page_count_rule: PageCountRule::Any,
            folded: false,
            folds_per_sheet: 0,
            creep_applies: false,
            has_spine: false,
            punched: true,
            recommended_binding_margin_mm: margin,
            allowed_sides: vec![BindingSide::Left, BindingSide::Right, BindingSide::Top],
            min_pages: 8,
            max_pages: 470,
            requires_duplex: false,
            separate_cover: false,
            guidance: "Any page count works because pages are punched as single leaves. The \
                       punch line destroys roughly 6 mm of the binding edge, so keep all content \
                       clear of the punch zone. The document opens completely flat and folds \
                       back on itself, which makes it the usual choice for anything used \
                       hands-free."
                .into(),
            typical_use: "Cookbooks, workbooks, manuals, presentations, notebooks".into(),
        },
        BindingType::WireO | BindingType::Comb | BindingType::Ring => BindingProfile {
            binding,
            key: "wire_o".into(),
            name: "Wire-O binding".into(),
            description: "Twin loops of metal wire are closed through rectangular punched holes, forming a neat double-loop spine.".into(),
            mechanism: "Twin-loop metal wire through rectangular punched holes".into(),
            page_count_rule: PageCountRule::Any,
            folded: false,
            folds_per_sheet: 0,
            creep_applies: false,
            has_spine: false,
            punched: true,
            recommended_binding_margin_mm: margin,
            allowed_sides: vec![BindingSide::Left, BindingSide::Right, BindingSide::Top],
            min_pages: 8,
            max_pages: 250,
            requires_duplex: false,
            separate_cover: false,
            guidance: "Like coil binding, any page count works and the document opens flat. \
                       Wire-O gives a more formal finish than plastic coil but has a lower page \
                       ceiling, and the wire diameter must be chosen for the finished thickness. \
                       Keep content outside the rectangular punch zone."
                .into(),
            typical_use: "Reports, proposals, calendars, premium presentations".into(),
        },
        BindingType::Hardcover => BindingProfile {
            binding,
            key: "hardcover".into(),
            name: "Case binding (hardcover)".into(),
            description: "Folded signatures are sewn or glued into a text block, then cased into rigid boards covered with printed material.".into(),
            mechanism: "Sewn or glued text block cased into rigid boards".into(),
            page_count_rule: PageCountRule::MultipleOfFour,
            folded: true,
            folds_per_sheet: 1,
            creep_applies: true,
            has_spine: true,
            punched: false,
            recommended_binding_margin_mm: margin,
            allowed_sides: vec![BindingSide::Left, BindingSide::Right],
            min_pages: 32,
            max_pages: 1000,
            requires_duplex: true,
            separate_cover: true,
            guidance: "The most durable and most expensive option. Pages are gathered into \
                       folded signatures, so the count should be a multiple of the signature \
                       size. The case is larger than the text block — allow for board overhang, \
                       the hinge groove and turn-in around the boards, all on top of the spine \
                       width itself."
                .into(),
            typical_use: "Trade books, photo books, yearbooks, archival editions".into(),
        },
        BindingType::None | BindingType::Custom => BindingProfile {
            binding,
            key: "none".into(),
            name: "No binding".into(),
            description: "Loose sheets with no binding applied.".into(),
            mechanism: "None — loose leaves".into(),
            page_count_rule: PageCountRule::Any,
            folded: false,
            folds_per_sheet: 0,
            creep_applies: false,
            has_spine: false,
            punched: false,
            recommended_binding_margin_mm: margin,
            allowed_sides: vec![BindingSide::Left, BindingSide::Right, BindingSide::Top],
            min_pages: 1,
            max_pages: u32::MAX,
            requires_duplex: false,
            separate_cover: false,
            guidance: "No gutter allowance is added. Use this for loose-leaf output or when the \
                       binding will be decided later."
                .into(),
            typical_use: "Loose-leaf handouts, proofs".into(),
        },
    }
}

/// Every booklet binding profile, in presentation order.
pub fn booklet_binding_profiles() -> Vec<BindingProfile> {
    BOOKLET_BINDINGS.iter().map(|b| binding_profile(*b)).collect()
}

/// Blank pages needed to satisfy this binding's page-count rule.
pub fn blanks_for_binding(binding: BindingType, page_count: u32) -> Result<u32, String> {
    if page_count == 0 {
        return Err("page_count must be positive".into());
    }
    let multiple = binding_profile(binding).page_count_rule.multiple();
    let rem = page_count % multiple;
    Ok(if rem == 0 { 0 } else { multiple - rem })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_booklet_bindings_are_offered() {
        let profiles = booklet_binding_profiles();
        assert_eq!(profiles.len(), 5);
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Saddle stitch",
                "Perfect binding",
                "Spiral / coil binding",
                "Wire-O binding",
                "Case binding (hardcover)"
            ]
        );
    }

    #[test]
    fn saddle_stitch_requires_multiple_of_four() {
        let p = binding_profile(BindingType::SaddleStitch);
        assert_eq!(p.page_count_rule, PageCountRule::MultipleOfFour);
        assert!(p.folded);
        assert!(p.creep_applies);
        assert!(!p.has_spine);
    }

    #[test]
    fn perfect_and_hardcover_have_a_spine() {
        assert!(binding_profile(BindingType::Perfect).has_spine);
        assert!(binding_profile(BindingType::Hardcover).has_spine);
        assert!(binding_profile(BindingType::Perfect).separate_cover);
    }

    #[test]
    fn punched_bindings_accept_any_page_count_and_top_edge() {
        for b in [BindingType::Spiral, BindingType::WireO] {
            let p = binding_profile(b);
            assert!(p.punched);
            assert_eq!(p.page_count_rule, PageCountRule::Any);
            assert!(p.allowed_sides.contains(&BindingSide::Top));
        }
    }

    #[test]
    fn folded_bindings_do_not_allow_top_edge_binding() {
        let p = binding_profile(BindingType::SaddleStitch);
        assert!(!p.allowed_sides.contains(&BindingSide::Top));
    }

    #[test]
    fn blanks_follow_the_page_count_rule() {
        // Saddle stitch pads to a multiple of 4.
        assert_eq!(blanks_for_binding(BindingType::SaddleStitch, 22).unwrap(), 2);
        // Perfect binding only needs whole leaves.
        assert_eq!(blanks_for_binding(BindingType::Perfect, 201).unwrap(), 1);
        // Punched bindings never need padding.
        assert_eq!(blanks_for_binding(BindingType::Spiral, 37).unwrap(), 0);
    }

    #[test]
    fn punched_bindings_have_the_largest_gutters() {
        let coil = binding_profile(BindingType::Spiral).recommended_binding_margin_mm;
        let saddle = binding_profile(BindingType::SaddleStitch).recommended_binding_margin_mm;
        assert!(coil > saddle);
    }

    #[test]
    fn zero_pages_is_an_error() {
        assert!(blanks_for_binding(BindingType::Perfect, 0).is_err());
    }
}
