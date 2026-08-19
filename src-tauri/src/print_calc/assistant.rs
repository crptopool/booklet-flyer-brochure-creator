//! The print assistant.
//!
//! Reads a plain-language request, works out what the user is asking
//! for, and answers with a full recommendation.
//!
//! Per the specification's technical design principles, the assistant
//! *recommends and explains*, but every number it quotes comes from the
//! deterministic modules in this crate. No page order or measurement is
//! ever inferred. The parser is rule-based and runs entirely offline, so
//! the same question always produces the same answer.

use serde::{Deserialize, Serialize};

use crate::print_calc::binding::binding_profile;
use crate::print_calc::creep::{creep_compensation, CreepMode};
use crate::print_calc::duplex::recommended_mode;
use crate::print_calc::plan::{booklet_plan, BookletPlan};
use crate::print_calc::presets::{describe_gsm, get_paper_size, BindingType, DuplexMode};
use crate::print_calc::spine::{approximate_caliper_mm, DEFAULT_BULK_FACTOR};

/// What the assistant understood from the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Understanding {
    pub page_count: Option<u32>,
    pub trim_size: Option<String>,
    pub sheet_size: Option<String>,
    pub binding: Option<BindingType>,
    pub duplex: Option<bool>,
    pub gsm: Option<f64>,
    /// Assumptions the assistant had to make, stated before it answers.
    pub assumptions: Vec<String>,
    /// Anything it could not work out and needs the user to confirm.
    pub unresolved: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Advice {
    pub understanding: Understanding,
    /// The deterministic plan, when enough was understood to build one.
    pub plan: Option<BookletPlan>,
    /// Plain-language findings, in the order they should be read.
    pub explanation: Vec<String>,
    pub warnings: Vec<String>,
    /// Settings the UI can apply directly.
    pub suggested_trim: Option<String>,
    pub suggested_sheet: Option<String>,
    pub suggested_pages_per_side: u32,
    pub suggested_duplex: DuplexMode,
    pub sheet_is_landscape: bool,
}

const KNOWN_SIZES: [&str; 11] = [
    "SRA3", "A3", "A4", "A5", "A6", "B5", "Letter", "Legal", "Tabloid", "12x18in", "13x19in",
];

/// Longest first, so "SRA3" is not swallowed by "A3".
fn find_sizes(lower: &str) -> Vec<String> {
    let mut hits: Vec<(usize, String)> = Vec::new();
    for name in KNOWN_SIZES {
        let needle = name.to_lowercase();
        let mut from = 0;
        while let Some(rel) = lower[from..].find(&needle) {
            let at = from + rel;
            // Reject a match sitting inside a longer word, so "a4paper"
            // matches but "sra3" does not also register as "a3".
            let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
            if before_ok && !hits.iter().any(|(p, _)| *p <= at && at < *p + 6 && lower[*p..].starts_with("sra3")) {
                hits.push((at, name.to_string()));
            }
            from = at + needle.len();
        }
    }
    // Drop any A3 hit that is really the tail of an SRA3 mention.
    let sra: Vec<usize> = hits.iter().filter(|(_, n)| n == "SRA3").map(|(p, _)| *p).collect();
    hits.retain(|(p, n)| !(n == "A3" && sra.iter().any(|s| *p == s + 2)));
    hits.sort_by_key(|(p, _)| *p);
    hits.into_iter().map(|(_, n)| n).collect()
}

/// First integer immediately preceding a "page" word.
fn find_page_count(lower: &str) -> Option<u32> {
    let bytes = lower.as_bytes();
    let mut at = 0;
    while let Some(rel) = lower[at..].find("page") {
        let idx = at + rel;
        // Walk back over separators, then over the digits.
        let mut end = idx;
        while end > 0 && (bytes[end - 1] == b' ' || bytes[end - 1] == b'-' || bytes[end - 1] == b'_') {
            end -= 1;
        }
        let mut start = end;
        while start > 0 && bytes[start - 1].is_ascii_digit() {
            start -= 1;
        }
        if start < end {
            if let Ok(n) = lower[start..end].parse::<u32>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
        at = idx + 4;
    }
    None
}

/// Integer preceding "gsm".
fn find_gsm(lower: &str) -> Option<f64> {
    let bytes = lower.as_bytes();
    let idx = lower.find("gsm")?;
    let mut end = idx;
    while end > 0 && bytes[end - 1] == b' ' {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && bytes[start - 1].is_ascii_digit() {
        start -= 1;
    }
    lower[start..end].parse::<f64>().ok().filter(|g| *g > 0.0)
}

fn find_binding(lower: &str) -> Option<BindingType> {
    // Most specific phrases first.
    for (needle, binding) in [
        ("saddle", BindingType::SaddleStitch),
        ("stitch", BindingType::SaddleStitch),
        ("staple", BindingType::SaddleStitch),
        ("perfect", BindingType::Perfect),
        ("paperback", BindingType::Perfect),
        ("glued", BindingType::Perfect),
        ("spiral", BindingType::Spiral),
        ("coil", BindingType::Spiral),
        ("wire-o", BindingType::WireO),
        ("wire o", BindingType::WireO),
        ("wiro", BindingType::WireO),
        ("comb", BindingType::Comb),
        ("hardcover", BindingType::Hardcover),
        ("hard cover", BindingType::Hardcover),
        ("case bound", BindingType::Hardcover),
        ("case-bound", BindingType::Hardcover),
        ("casebound", BindingType::Hardcover),
    ] {
        if lower.contains(needle) {
            return Some(binding);
        }
    }
    None
}

/// Work out what the request is asking for.
pub fn understand(request: &str) -> Understanding {
    let lower = request.to_lowercase();
    let sizes = find_sizes(&lower);
    let page_count = find_page_count(&lower);
    let binding = find_binding(&lower);
    let gsm = find_gsm(&lower);

    let duplex = if lower.contains("single-sided")
        || lower.contains("single sided")
        || lower.contains("one side")
        || lower.contains("simplex")
    {
        Some(false)
    } else if lower.contains("double-sided") || lower.contains("double sided") || lower.contains("duplex") {
        Some(true)
    } else {
        None
    };

    // With two sizes named, the document size is stated first and the
    // paper it prints onto second — "an A5 PDF ... onto A4 sheets".
    let (trim_size, sheet_size) = match sizes.len() {
        0 => (None, None),
        1 => (Some(sizes[0].clone()), None),
        _ => (Some(sizes[0].clone()), Some(sizes[sizes.len() - 1].clone())),
    };

    let mut assumptions = Vec::new();
    let mut unresolved = Vec::new();

    if page_count.is_none() {
        unresolved.push("How many pages does the document have?".into());
    }
    if trim_size.is_none() {
        unresolved.push("What size should the finished pages be?".into());
    }
    if binding.is_none() {
        assumptions.push("No binding was named, so this answer assumes saddle stitch — the usual choice for a short booklet.".into());
    }
    if gsm.is_none() {
        assumptions.push("Paper weight was not given, so 80 GSM office stock is assumed.".into());
    }

    Understanding {
        page_count,
        trim_size,
        sheet_size,
        binding,
        duplex,
        gsm,
        assumptions,
        unresolved,
    }
}

/// Answer the request with a deterministic plan and an explanation.
pub fn advise(request: &str) -> Result<Advice, String> {
    let u = understand(request);

    let binding = u.binding.unwrap_or(BindingType::SaddleStitch);
    let profile = binding_profile(binding);
    let gsm = u.gsm.unwrap_or(80.0);

    let mut explanation = Vec::new();
    let mut warnings = Vec::new();

    // Without a page count there is nothing deterministic to compute.
    let Some(pages) = u.page_count else {
        return Ok(Advice {
            understanding: u,
            plan: None,
            explanation: vec![
                "Tell me the page count and the finished page size and I can work out the sheets, the page order and the settings.".into(),
            ],
            warnings,
            suggested_trim: None,
            suggested_sheet: None,
            suggested_pages_per_side: 2,
            suggested_duplex: DuplexMode::ShortEdge,
            sheet_is_landscape: true,
        });
    };

    // Folded bindings put two pages on each side of a landscape sheet;
    // flat bindings print one page per side.
    let pages_per_side = if profile.folded { 2 } else { 1 };
    let landscape = profile.folded;
    let duplex_mode = if u.duplex == Some(false) && !profile.requires_duplex {
        DuplexMode::Simplex
    } else {
        recommended_mode(landscape)
    };

    let plan = booklet_plan(binding, pages, pages_per_side, duplex_mode, landscape, gsm, false, None, None, None)?;

    // If the sheet was not named, a folded booklet needs paper one size
    // up from the finished page, fed landscape.
    let trim = u.trim_size.clone();
    let sheet = u.sheet_size.clone().or_else(|| {
        trim.as_deref().and_then(|t| {
            Some(
                match t {
                    "A6" => "A5",
                    "A5" => "A4",
                    "A4" => "A3",
                    "A3" => "SRA3",
                    _ => return None,
                }
                .to_string(),
            )
        })
    });

    explanation.push(format!(
        "{} pages, finished at {}, bound by {}.",
        pages,
        trim.clone().unwrap_or_else(|| "the size you choose".into()),
        profile.name.to_lowercase()
    ));

    if let (Some(t), Some(s)) = (trim.as_deref(), sheet.as_deref()) {
        if profile.folded {
            explanation.push(format!(
                "Two {t} pages sit side by side on each side of a {s} sheet fed landscape, so the sheet is folded down the middle."
            ));
            // Sanity-check that the pages actually fit the named sheet.
            if let (Ok(tp), Ok(sp)) = (get_paper_size(t), get_paper_size(s)) {
                let need_w = tp.width_mm * 2.0;
                let have_w = sp.height_mm.max(sp.width_mm);
                let have_h = sp.width_mm.min(sp.height_mm);
                if need_w > have_w + 0.5 || tp.height_mm > have_h + 0.5 {
                    warnings.push(format!(
                        "Two {t} pages ({:.0} x {:.0} mm together) will not fit a {s} sheet at full size — they would have to be scaled down.",
                        need_w, tp.height_mm
                    ));
                }
            }
        } else {
            explanation.push(format!("Each {t} page prints on its own {s} sheet."));
        }
    }

    explanation.push(format!(
        "That works out at {} sheet{} of paper, {} pages per sheet.",
        plan.sheet_count,
        if plan.sheet_count == 1 { "" } else { "s" },
        plan.pages_per_sheet
    ));

    if plan.blanks_needed > 0 {
        warnings.push(format!(
            "{pages} is not a multiple of {} for {}, so {} blank page{} must be added. You choose where they go — I will not add them silently.",
            profile.page_count_rule.multiple(),
            profile.name.to_lowercase(),
            plan.blanks_needed,
            if plan.blanks_needed == 1 { "" } else { "s" }
        ));
    } else {
        explanation.push(format!(
            "{pages} divides evenly for {}, so no blank pages are needed.",
            profile.name.to_lowercase()
        ));
    }

    if duplex_mode == DuplexMode::Simplex {
        explanation.push("Printing single-sided, one page per sheet.".into());
    } else {
        explanation.push(format!(
            "Print double-sided with the flip set to {}. {}",
            if duplex_mode == DuplexMode::ShortEdge { "short edge" } else { "long edge" },
            plan.duplex.explanation
        ));
    }

    if plan.uses_printer_spreads {
        let first = plan.sheet_count;
        explanation.push(format!(
            "The pages are re-ordered into printer spreads: the first sheet carries page {} beside page 1 on the front, and page 2 beside page {} on the back.",
            plan.total_pages,
            plan.total_pages - 1
        ));
        let _ = first;
    } else if !profile.folded {
        explanation.push("Pages stay in normal reading order — no imposition is needed for the text block.".into());
    }

    explanation.push(format!(
        "{gsm:.0} GSM paper — {}. That is about {:.3} mm per sheet.",
        describe_gsm(gsm).to_lowercase(),
        plan.caliper_mm
    ));

    if let Some(spine) = plan.spine_width_mm {
        explanation.push(format!(
            "The spine works out at {spine:.2} mm. Build the cover around that, and confirm the caliper with your printer first."
        ));
    }

    if profile.creep_applies && plan.sheet_count > 1 {
        if let Ok(creep) = creep_compensation(plan.sheet_count, plan.caliper_mm, 1, None, CreepMode::Automatic, None) {
            let msg = format!(
                "With {} nested sheets the innermost pages creep about {:.2} mm towards the fore-edge.",
                plan.sheet_count, creep.total_creep_mm
            );
            if creep.total_creep_mm >= 1.0 {
                warnings.push(format!("{msg} Turn on creep compensation, or trim the fore-edge after folding."));
            } else {
                explanation.push(format!("{msg} That is small enough to ignore."));
            }
        }
    }

    explanation.push(format!(
        "Leave at least {:.0} mm clear of the binding edge{}.",
        profile.recommended_binding_margin_mm,
        if profile.punched { " — the punch destroys that strip" } else { "" }
    ));

    if profile.folded {
        explanation.push("Fold each sheet down the centre, nest them inside one another, then staple through the fold and trim the fore-edge.".into());
    }

    explanation.push("Add 3 mm bleed if any artwork runs to the edge of the page.".into());

    // Carry over anything the plan flagged that the prose above has not
    // already covered. Matching on the message text would miss re-worded
    // duplicates, so compare the subject instead.
    let already_said = |note: &str| {
        let covered = [
            ("blank", plan.blanks_needed > 0),
            ("180 degrees", true),
            ("creep", profile.creep_applies),
            ("Spine width", plan.spine_width_mm.is_some()),
        ];
        covered.iter().any(|(topic, mentioned)| *mentioned && note.contains(topic))
    };
    for note in &plan.notes {
        if note.severity != "INFO" && !already_said(&note.message) {
            warnings.push(note.message.clone());
        }
    }

    Ok(Advice {
        understanding: u,
        suggested_trim: trim,
        suggested_sheet: sheet,
        suggested_pages_per_side: pages_per_side,
        suggested_duplex: duplex_mode,
        sheet_is_landscape: landscape,
        plan: Some(plan),
        explanation,
        warnings,
    })
}

/// One glossary entry, in the shape the guidance panel requires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub term: String,
    pub short: String,
    pub recommended: String,
    pub why: String,
    pub example: String,
    pub consequence: String,
}

fn entry(term: &str, short: &str, recommended: &str, why: &str, example: &str, consequence: &str) -> GlossaryEntry {
    GlossaryEntry {
        term: term.into(),
        short: short.into(),
        recommended: recommended.into(),
        why: why.into(),
        example: example.into(),
        consequence: consequence.into(),
    }
}

/// Plain-language explanations of the print terms the app uses.
pub fn glossary() -> Vec<GlossaryEntry> {
    vec![
        entry(
            "Bleed",
            "Artwork extended past the trim line so cutting variation cannot leave white edges.",
            "3 mm on commercial work; 0 mm if nothing touches the edge.",
            "Guillotines drift by a millimetre or two. Bleed gives the blade somewhere to land that is still inside your artwork.",
            "A photo that fills the page is drawn 3 mm larger on every side, then cut back.",
            "Without it, a thin white sliver appears along one or more edges.",
        ),
        entry(
            "Trim size",
            "The size of the finished page after cutting.",
            "Whatever the reader should hold — A5 for a booklet, A4 for a report.",
            "It is the only size the reader ever sees, and it drives the sheet size, the imposition and the spine.",
            "An A5 booklet has a 148 x 210 mm trim size, printed two-up on A4.",
            "Confusing it with the sheet size produces pages at half or double the intended size.",
        ),
        entry(
            "Gutter / binding margin",
            "Extra inside margin that disappears into the binding.",
            "5 mm saddle stitch, 10 mm perfect bound, 12–13 mm punched.",
            "A bound book does not open flat, and a punched one loses a strip entirely.",
            "A perfect-bound novel keeps body text 10 mm from the spine edge.",
            "Text runs into the spine and becomes unreadable, or is cut away by the punch.",
        ),
        entry(
            "Creep / shingling",
            "Inner sheets of a folded booklet push outwards, so their pages sit closer to the fore-edge.",
            "Compensate above about 16 pages, or trim the fore-edge after folding.",
            "Each nested sheet wraps the ones inside it, and paper has thickness.",
            "A 40-page booklet on 80 GSM creeps roughly 1.8 mm at the centre.",
            "Page margins visibly narrow towards the middle of the booklet.",
        ),
        entry(
            "Duplex flip edge",
            "Which edge the printer turns the sheet about when printing the reverse.",
            "Long edge for portrait sheets, short edge for landscape booklet sheets.",
            "Turning about a horizontal axis inverts the back; turning about a vertical one does not.",
            "A5 booklet pages on A4 landscape sheets need a short-edge flip.",
            "Every second side prints upside down, ruining the whole run.",
        ),
        entry(
            "Spine width",
            "The thickness of the bound block, which the cover must wrap.",
            "Pages ÷ 2 × paper caliper, confirmed with your printer.",
            "The cover is a single sheet: get the spine wrong and the fold lines miss the block.",
            "200 pages of 0.1 mm stock gives a 10 mm spine.",
            "Spine text sits on the front cover instead of the spine.",
        ),
        entry(
            "Effective DPI",
            "How much image detail lands in each printed inch, at the final size.",
            "300 DPI for photographs; 200 DPI is the practical minimum.",
            "Enlarging an image spreads its pixels over more paper, so detail falls away.",
            "A 1000 px wide photo printed 100 mm wide runs at 254 DPI.",
            "Printed images look soft or visibly blocky.",
        ),
        entry(
            "GSM and caliper",
            "GSM is weight per square metre; caliper is the actual thickness.",
            "80–90 GSM text, 130–170 GSM flyers, 200–250 GSM covers.",
            "Spine and creep both depend on thickness, and two papers of equal GSM can differ in bulk.",
            "80 GSM uncoated is roughly 0.092 mm per sheet.",
            "Spine width and creep come out wrong, so the cover does not fit.",
        ),
        entry(
            "Imposition",
            "Arranging document pages onto printer sheets so they read correctly after folding.",
            "Let the application calculate it — never re-order pages by hand.",
            "The order on the sheet bears no relation to reading order once the sheet is folded.",
            "An 8-page booklet prints 8|1, 2|7, 6|3, 4|5.",
            "The finished booklet reads in the wrong order and has to be reprinted.",
        ),
    ]
}

/// Words too common to identify an entry on their own.
const STOP_WORDS: [&str; 14] = [
    "what", "is", "the", "a", "an", "how", "do", "does", "my", "for", "and", "with", "why", "of",
];

/// Look up glossary entries matching a search term.
///
/// The query is matched word by word so a typed question — "What is
/// creep?" — finds the same entry as the bare term.
pub fn explain(term: &str) -> Vec<GlossaryEntry> {
    let query = term.trim().to_lowercase();
    if query.is_empty() {
        return glossary();
    }
    let words: Vec<String> = query
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 3 && !STOP_WORDS.contains(w))
        .map(|w| w.to_string())
        .collect();

    glossary()
        .into_iter()
        .filter(|e| {
            let haystack = format!("{} {}", e.term, e.short).to_lowercase();
            // Whole-phrase match, or any meaningful word from the query.
            haystack.contains(&query) || words.iter().any(|w| haystack.contains(w.as_str()))
        })
        .collect()
}

/// Common problems and what to do about them.
pub fn troubleshooting() -> Vec<GlossaryEntry> {
    vec![
        entry(
            "Every second page prints upside down",
            "The duplex flip edge does not match the sheet orientation.",
            "Switch the flip between long and short edge.",
            "Landscape sheets need the opposite flip to portrait ones.",
            "A5-on-A4 booklets need short-edge flip.",
            "Half the booklet is unreadable.",
        ),
        entry(
            "The booklet reads in the wrong order",
            "The file was printed in reading order rather than imposed into printer spreads.",
            "Export the imposed PDF and print that file, not the original.",
            "Folding re-orders the pages physically.",
            "Page 2 should sit opposite page 7 on an 8-page booklet.",
            "The whole run is wasted.",
        ),
        entry(
            "White edges after trimming",
            "Artwork stopped at the trim line instead of bleeding past it.",
            "Add 3 mm bleed and re-export.",
            "The cutter cannot hit the line exactly.",
            "A full-page background drawn 3 mm oversize on every side.",
            "A white hairline on one or more edges of every copy.",
        ),
        entry(
            "Pages come out smaller than expected",
            "The printer driver scaled the file to fit.",
            "Set printing to 100% or Actual Size, with fit-to-page turned off.",
            "Drivers shrink to the printable area by default.",
            "An A4 sheet printed at 96% leaves an unintended white border.",
            "The trim size is wrong and crop marks no longer line up.",
        ),
        entry(
            "Inner pages look off-centre",
            "Creep — the nested sheets push outwards as the booklet thickens.",
            "Enable creep compensation, or trim the fore-edge after folding.",
            "Paper thickness accumulates across nested sheets.",
            "A thick saddle-stitched booklet on heavy stock.",
            "Margins narrow noticeably towards the centre pages.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_EXAMPLE: &str =
        "I have a 36-page A5 PDF and want to print it as an A4 saddle-stitched booklet.";

    #[test]
    fn understands_the_specification_example() {
        let u = understand(SPEC_EXAMPLE);
        assert_eq!(u.page_count, Some(36));
        assert_eq!(u.trim_size.as_deref(), Some("A5"));
        assert_eq!(u.sheet_size.as_deref(), Some("A4"));
        assert_eq!(u.binding, Some(BindingType::SaddleStitch));
    }

    #[test]
    fn spec_example_produces_the_expected_advice() {
        let a = advise(SPEC_EXAMPLE).unwrap();
        let plan = a.plan.as_ref().unwrap();
        // 36 pages is divisible by 4 and gives 9 physical sheets.
        assert_eq!(plan.blanks_needed, 0);
        assert_eq!(plan.sheet_count, 9);
        assert_eq!(plan.pages_per_side, 2);
        assert!(a.sheet_is_landscape);
        assert_eq!(a.suggested_duplex, DuplexMode::ShortEdge);
        let text = a.explanation.join(" ");
        assert!(text.contains("9 sheets"));
        assert!(text.contains("short edge"));
        assert!(text.contains("Fold each sheet"));
        assert!(text.contains("bleed"));
        assert!(text.to_lowercase().contains("gsm"));
    }

    #[test]
    fn page_counts_are_read_in_several_forms() {
        assert_eq!(understand("a 20-page booklet").page_count, Some(20));
        assert_eq!(understand("36 pages of A5").page_count, Some(36));
        assert_eq!(understand("I have 200 pages").page_count, Some(200));
        assert_eq!(understand("no number here").page_count, None);
    }

    #[test]
    fn sra3_is_not_mistaken_for_a3() {
        let u = understand("print my A4 pages onto SRA3 sheets");
        assert_eq!(u.trim_size.as_deref(), Some("A4"));
        assert_eq!(u.sheet_size.as_deref(), Some("SRA3"));
    }

    #[test]
    fn binding_words_are_recognised() {
        assert_eq!(understand("spiral bound").binding, Some(BindingType::Spiral));
        assert_eq!(understand("wire-o report").binding, Some(BindingType::WireO));
        assert_eq!(understand("perfect bound novel").binding, Some(BindingType::Perfect));
        assert_eq!(understand("case-bound photo book").binding, Some(BindingType::Hardcover));
        assert_eq!(understand("a paperback").binding, Some(BindingType::Perfect));
    }

    #[test]
    fn assumptions_are_stated_when_details_are_missing() {
        let u = understand("a 16-page A5 booklet");
        assert!(u.assumptions.iter().any(|a| a.contains("saddle stitch")));
        assert!(u.assumptions.iter().any(|a| a.contains("80 GSM")));
    }

    #[test]
    fn a_missing_page_count_is_asked_for_rather_than_guessed() {
        let u = understand("an A5 saddle-stitched booklet");
        assert!(u.unresolved.iter().any(|q| q.contains("How many pages")));
        let a = advise("an A5 saddle-stitched booklet").unwrap();
        assert!(a.plan.is_none());
    }

    #[test]
    fn odd_page_counts_are_warned_about_not_silently_padded() {
        let a = advise("a 22-page A5 saddle-stitched booklet").unwrap();
        assert_eq!(a.plan.as_ref().unwrap().blanks_needed, 2);
        assert!(a.warnings.iter().any(|w| w.contains("blank")));
        assert!(a.warnings.iter().any(|w| w.contains("not add them silently")));
    }

    #[test]
    fn gsm_is_read_and_used() {
        let u = understand("a 40-page A5 booklet on 170 gsm");
        assert_eq!(u.gsm, Some(170.0));
        let a = advise("a 40-page A5 booklet on 170 gsm").unwrap();
        let expected = approximate_caliper_mm(170.0, DEFAULT_BULK_FACTOR).unwrap();
        assert!((a.plan.unwrap().caliper_mm - expected).abs() < 1e-9);
    }

    #[test]
    fn perfect_binding_keeps_reading_order_and_reports_a_spine() {
        let a = advise("a 200-page A5 perfect bound book").unwrap();
        let plan = a.plan.as_ref().unwrap();
        assert!(plan.spine_width_mm.is_some());
        assert_eq!(a.suggested_pages_per_side, 1);
        assert!(!a.sheet_is_landscape);
        assert!(a.explanation.iter().any(|e| e.contains("normal reading order")));
        assert!(a.explanation.iter().any(|e| e.contains("spine")));
    }

    #[test]
    fn spiral_binding_may_be_single_sided() {
        let a = advise("a 37-page A4 spiral bound single-sided manual").unwrap();
        assert_eq!(a.suggested_duplex, DuplexMode::Simplex);
        assert_eq!(a.plan.as_ref().unwrap().blanks_needed, 0);
    }

    #[test]
    fn a_sheet_size_is_suggested_when_none_is_given() {
        let a = advise("a 16-page A5 saddle-stitched booklet").unwrap();
        assert_eq!(a.suggested_sheet.as_deref(), Some("A4"));
    }

    #[test]
    fn pages_that_cannot_fit_the_named_sheet_are_flagged() {
        let a = advise("a 16-page A4 booklet printed on A4 sheets").unwrap();
        assert!(a.warnings.iter().any(|w| w.contains("will not fit")));
    }

    #[test]
    fn thick_booklets_warn_about_creep() {
        let a = advise("a 60-page A5 saddle-stitched booklet on 120 gsm").unwrap();
        assert!(a.warnings.iter().any(|w| w.contains("creep")));
    }

    #[test]
    fn glossary_covers_the_terms_the_app_uses() {
        let g = glossary();
        assert!(g.len() >= 9);
        for e in &g {
            assert!(!e.recommended.is_empty() && !e.why.is_empty() && !e.example.is_empty() && !e.consequence.is_empty());
        }
    }

    #[test]
    fn glossary_lookup_matches_loosely() {
        assert!(!explain("bleed").is_empty());
        assert!(!explain("What is creep?").is_empty());
        assert!(explain("").len() == glossary().len());
        assert!(explain("zzzz").is_empty());
    }

    #[test]
    fn troubleshooting_entries_are_complete() {
        let t = troubleshooting();
        assert!(t.len() >= 5);
        assert!(t.iter().any(|e| e.term.contains("upside down")));
    }
}

#[cfg(test)]
mod dedup_tests {
    use super::*;

    #[test]
    fn each_concern_is_raised_only_once() {
        let a = advise("a 22-page A5 saddle-stitched booklet").unwrap();
        let blank_warnings = a.warnings.iter().filter(|w| w.contains("blank")).count();
        assert_eq!(blank_warnings, 1, "blank-page advice was repeated: {:?}", a.warnings);
    }

    #[test]
    fn creep_is_not_repeated_either() {
        let a = advise("a 60-page A5 saddle-stitched booklet on 120 gsm").unwrap();
        assert_eq!(a.warnings.iter().filter(|w| w.contains("creep")).count(), 1);
    }

    #[test]
    fn genuinely_new_plan_warnings_still_come_through() {
        // A page count far past the practical maximum is not mentioned in
        // the prose, so the plan's warning must survive.
        let a = advise("a 200-page A5 saddle-stitched booklet").unwrap();
        assert!(a.warnings.iter().any(|w| w.contains("practical maximum")));
    }
}
