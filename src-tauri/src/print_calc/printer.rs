//! Printer capability profiles.
//!
//! A profile records what a physical printer can actually do, so the
//! application can check a job against it and warn before paper is
//! wasted. Every check here is a deterministic comparison — the profile
//! narrows recommendations, it never changes page geometry.

use serde::{Deserialize, Serialize};

use crate::print_calc::presets::DuplexMode;

/// How a printer behaves when it turns a sheet over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplexBehaviour {
    /// Turns about the long edge — the usual book-style duplex.
    LongEdge,
    /// Turns about the short edge.
    ShortEdge,
    /// Not established yet; run the test sheet.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterProfile {
    pub name: String,
    /// Largest sheet the printer accepts, in mm.
    pub max_width_mm: f64,
    pub max_height_mm: f64,
    pub duplex_supported: bool,
    pub borderless_supported: bool,
    /// Smallest unprintable margin the hardware leaves, in mm.
    pub min_margin_mm: f64,
    /// Named sheet sizes the printer is set up for.
    pub supported_sizes: Vec<String>,
    /// "portrait", "landscape" or "either".
    pub preferred_orientation: String,
    pub duplex_behaviour: DuplexBehaviour,
    pub notes: String,
}

impl Default for PrinterProfile {
    fn default() -> Self {
        PrinterProfile {
            name: "Office printer".into(),
            max_width_mm: 297.0,
            max_height_mm: 420.0,
            duplex_supported: true,
            borderless_supported: false,
            min_margin_mm: 5.0,
            supported_sizes: vec!["A4".into(), "A5".into(), "Letter".into()],
            preferred_orientation: "either".into(),
            duplex_behaviour: DuplexBehaviour::Unknown,
            notes: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileFinding {
    pub severity: String,
    pub message: String,
}

fn finding(severity: &str, message: impl Into<String>) -> ProfileFinding {
    ProfileFinding {
        severity: severity.into(),
        message: message.into(),
    }
}

impl PrinterProfile {
    /// True when a sheet fits, in either orientation.
    pub fn fits(&self, width_mm: f64, height_mm: f64) -> bool {
        let (w, h) = (width_mm, height_mm);
        (w <= self.max_width_mm + 1e-6 && h <= self.max_height_mm + 1e-6)
            || (h <= self.max_width_mm + 1e-6 && w <= self.max_height_mm + 1e-6)
    }
}

/// Check a job against a printer profile.
///
/// `sheet_mm` is the sheet the job wants to print on, `bleed_mm` the
/// bleed it carries, and `duplex` the flip the job expects.
pub fn check_job(
    profile: &PrinterProfile,
    sheet_mm: (f64, f64),
    sheet_name: Option<&str>,
    bleed_mm: f64,
    duplex: DuplexMode,
) -> Vec<ProfileFinding> {
    let mut out = Vec::new();

    if !profile.fits(sheet_mm.0, sheet_mm.1) {
        out.push(finding(
            "ERROR",
            format!(
                "{:.0} x {:.0} mm is larger than {} can take ({:.0} x {:.0} mm). Choose a smaller sheet or a different printer.",
                sheet_mm.0, sheet_mm.1, profile.name, profile.max_width_mm, profile.max_height_mm
            ),
        ));
    }

    if let Some(name) = sheet_name {
        if !profile.supported_sizes.is_empty() && !profile.supported_sizes.iter().any(|s| s == name) {
            out.push(finding(
                "WARNING",
                format!(
                    "{name} is not in this printer's configured sizes ({}). It may still work, but the tray may need setting up.",
                    profile.supported_sizes.join(", ")
                ),
            ));
        }
    }

    let wants_duplex = duplex != DuplexMode::Simplex;
    if wants_duplex && !profile.duplex_supported {
        out.push(finding(
            "ERROR",
            format!(
                "{} cannot print double-sided by itself. Switch the job to manual duplex and reinsert the stack by hand.",
                profile.name
            ),
        ));
    }

    // A known flip behaviour lets us name the right setting outright.
    match (wants_duplex, profile.duplex_behaviour) {
        (true, DuplexBehaviour::Unknown) => out.push(finding(
            "INFO",
            "This printer's duplex flip direction has not been recorded. Print the two-sheet test before a long run, then save the result to the profile.".to_string(),
        )),
        (true, DuplexBehaviour::LongEdge) if duplex == DuplexMode::ShortEdge => out.push(finding(
            "WARNING",
            format!("{} flips on the long edge, but this job is set to short edge. The reverse sides will come out inverted unless you change one of them.", profile.name),
        )),
        (true, DuplexBehaviour::ShortEdge) if duplex == DuplexMode::LongEdge => out.push(finding(
            "WARNING",
            format!("{} flips on the short edge, but this job is set to long edge. The reverse sides will come out inverted unless you change one of them.", profile.name),
        )),
        _ => {}
    }

    if bleed_mm > 0.0 && !profile.borderless_supported {
        out.push(finding(
            "WARNING",
            format!(
                "This job carries {bleed_mm:.0} mm bleed but {} cannot print borderless. Print onto a larger sheet and trim down, or the bleed will be lost.",
                profile.name
            ),
        ));
    }

    if profile.min_margin_mm > 0.0 {
        out.push(finding(
            "INFO",
            format!(
                "{} leaves {:.1} mm unprintable at the sheet edge. Keep crop marks and content inside that.",
                profile.name, profile.min_margin_mm
            ),
        ));
    }

    let landscape = sheet_mm.0 > sheet_mm.1;
    match profile.preferred_orientation.as_str() {
        "portrait" if landscape => out.push(finding(
            "INFO",
            format!("{} feeds portrait by preference; a landscape sheet may need the tray guides moved.", profile.name),
        )),
        "landscape" if !landscape => out.push(finding(
            "INFO",
            format!("{} feeds landscape by preference; a portrait sheet may need the tray guides moved.", profile.name),
        )),
        _ => {}
    }

    if out.iter().all(|f| f.severity == "INFO") {
        out.insert(0, finding("INFO", format!("The job suits {}.", profile.name)));
    }
    out
}

/// The largest usable print area on this printer, in mm.
pub fn printable_area_mm(profile: &PrinterProfile) -> (f64, f64) {
    let m = if profile.borderless_supported { 0.0 } else { profile.min_margin_mm };
    (
        (profile.max_width_mm - 2.0 * m).max(0.0),
        (profile.max_height_mm - 2.0 * m).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a3_duplex() -> PrinterProfile {
        PrinterProfile {
            name: "Studio A3".into(),
            max_width_mm: 297.0,
            max_height_mm: 420.0,
            duplex_supported: true,
            borderless_supported: false,
            min_margin_mm: 4.0,
            supported_sizes: vec!["A3".into(), "A4".into()],
            preferred_orientation: "either".into(),
            duplex_behaviour: DuplexBehaviour::LongEdge,
            notes: String::new(),
        }
    }

    #[test]
    fn a4_fits_an_a3_printer_either_way_round() {
        let p = a3_duplex();
        assert!(p.fits(210.0, 297.0));
        assert!(p.fits(297.0, 210.0));
    }

    #[test]
    fn oversized_sheets_are_an_error() {
        let f = check_job(&a3_duplex(), (330.0, 483.0), Some("13x19in"), 0.0, DuplexMode::Simplex);
        assert!(f.iter().any(|x| x.severity == "ERROR" && x.message.contains("larger than")));
    }

    #[test]
    fn duplex_job_on_a_simplex_printer_is_an_error() {
        let p = PrinterProfile { duplex_supported: false, ..a3_duplex() };
        let f = check_job(&p, (297.0, 210.0), Some("A4"), 0.0, DuplexMode::ShortEdge);
        assert!(f.iter().any(|x| x.severity == "ERROR" && x.message.contains("double-sided")));
    }

    #[test]
    fn mismatched_flip_direction_is_flagged() {
        let f = check_job(&a3_duplex(), (297.0, 210.0), Some("A4"), 0.0, DuplexMode::ShortEdge);
        assert!(f.iter().any(|x| x.severity == "WARNING" && x.message.contains("long edge")));
    }

    #[test]
    fn matching_flip_direction_passes_quietly() {
        let f = check_job(&a3_duplex(), (297.0, 210.0), Some("A4"), 0.0, DuplexMode::LongEdge);
        assert!(!f.iter().any(|x| x.severity == "WARNING"));
    }

    #[test]
    fn unknown_flip_behaviour_suggests_the_test_sheet() {
        let p = PrinterProfile { duplex_behaviour: DuplexBehaviour::Unknown, ..a3_duplex() };
        let f = check_job(&p, (297.0, 210.0), Some("A4"), 0.0, DuplexMode::LongEdge);
        assert!(f.iter().any(|x| x.message.contains("test")));
    }

    #[test]
    fn bleed_without_borderless_is_flagged() {
        let f = check_job(&a3_duplex(), (297.0, 210.0), Some("A4"), 3.0, DuplexMode::Simplex);
        assert!(f.iter().any(|x| x.severity == "WARNING" && x.message.contains("borderless")));
    }

    #[test]
    fn borderless_printer_accepts_bleed() {
        let p = PrinterProfile { borderless_supported: true, ..a3_duplex() };
        let f = check_job(&p, (297.0, 210.0), Some("A4"), 3.0, DuplexMode::Simplex);
        assert!(!f.iter().any(|x| x.message.contains("borderless")));
    }

    #[test]
    fn unconfigured_size_warns_without_blocking() {
        let f = check_job(&a3_duplex(), (215.9, 279.4), Some("Letter"), 0.0, DuplexMode::Simplex);
        assert!(f.iter().any(|x| x.severity == "WARNING" && x.message.contains("configured sizes")));
        assert!(!f.iter().any(|x| x.severity == "ERROR"));
    }

    #[test]
    fn a_clean_job_reports_that_it_suits_the_printer() {
        let f = check_job(&a3_duplex(), (297.0, 210.0), Some("A4"), 0.0, DuplexMode::LongEdge);
        assert!(f[0].message.contains("suits"));
    }

    #[test]
    fn printable_area_subtracts_the_hardware_margin() {
        let (w, h) = printable_area_mm(&a3_duplex());
        assert!((w - 289.0).abs() < 1e-9);
        assert!((h - 412.0).abs() < 1e-9);
        // Borderless printers lose nothing.
        let (bw, _) = printable_area_mm(&PrinterProfile { borderless_supported: true, ..a3_duplex() });
        assert!((bw - 297.0).abs() < 1e-9);
    }

    #[test]
    fn orientation_preference_is_advisory_only() {
        let p = PrinterProfile { preferred_orientation: "portrait".into(), ..a3_duplex() };
        let f = check_job(&p, (297.0, 210.0), Some("A4"), 0.0, DuplexMode::LongEdge);
        assert!(f.iter().any(|x| x.severity == "INFO" && x.message.contains("portrait")));
        assert!(!f.iter().any(|x| x.severity == "ERROR"));
    }
}
