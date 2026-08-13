//! Automated preflight checks run before export.
//!
//! Errors block export only where the output would likely be invalid;
//! warnings and infos require user confirmation but never block.

use serde::{Deserialize, Serialize};

use crate::pdf_ops::document::PdfSource;
use crate::print_calc::booklet::blanks_needed;
use crate::print_calc::presets::BindingType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    /// 1-based page number, when the finding is page-specific.
    pub page: Option<u32>,
}

fn finding(severity: Severity, code: &str, message: String, page: Option<u32>) -> Finding {
    Finding {
        severity,
        code: code.to_string(),
        message,
        page,
    }
}

/// Preflight a source document for a given output intent.
pub fn preflight(
    source: &PdfSource,
    binding: BindingType,
    bleed_mm: f64,
    expected_trim: Option<(f64, f64)>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    if source.modification_restricted {
        findings.push(finding(
            Severity::Error,
            "encrypted",
            "The PDF is protected and cannot be modified.".into(),
            None,
        ));
        return findings;
    }

    if source.page_count == 0 {
        findings.push(finding(Severity::Error, "empty", "The document has no pages.".into(), None));
        return findings;
    }

    // Page count suitability for folded/stitched bindings.
    if matches!(binding, BindingType::SaddleStitch | BindingType::Staple) {
        let blanks = blanks_needed(source.page_count, 4).unwrap_or(0);
        if blanks > 0 {
            findings.push(finding(
                Severity::Warning,
                "page_count_binding",
                format!(
                    "Page count {} is not divisible by 4; add {blanks} blank page(s) for {binding:?} binding.",
                    source.page_count
                ),
                None,
            ));
        }
    }

    // Mixed page sizes.
    if source.has_mixed_page_sizes() {
        findings.push(finding(
            Severity::Warning,
            "mixed_page_sizes",
            "The document contains mixed page sizes.".into(),
            None,
        ));
    }

    // Unexpected rotations.
    for p in &source.pages {
        if p.rotation != 0 {
            findings.push(finding(
                Severity::Info,
                "page_rotation",
                format!("Page {} carries a stored rotation of {} degrees.", p.number, p.rotation),
                Some(p.number),
            ));
        }
    }

    // Missing bleed.
    if bleed_mm <= 0.0 {
        findings.push(finding(
            Severity::Warning,
            "missing_bleed",
            "No bleed configured; edge-to-edge artwork may show white edges after trimming (3 mm recommended).".into(),
            None,
        ));
    }

    // Wrong page size versus intended trim.
    if let Some((tw, th)) = expected_trim {
        for p in &source.pages {
            let matches_size = (p.width_pt - tw).abs() < 1.0 && (p.height_pt - th).abs() < 1.0;
            let matches_rotated = (p.width_pt - th).abs() < 1.0 && (p.height_pt - tw).abs() < 1.0;
            if !matches_size && !matches_rotated {
                findings.push(finding(
                    Severity::Warning,
                    "wrong_page_size",
                    format!(
                        "Page {} is {:.1} x {:.1} pt but the intended trim is {:.1} x {:.1} pt.",
                        p.number, p.width_pt, p.height_pt, tw, th
                    ),
                    Some(p.number),
                ));
            }
        }
    }

    findings
}

/// True when export should be blocked (any ERROR finding remains).
pub fn blocks_export(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_ops::document::PageInfo;

    fn source(n: u32) -> PdfSource {
        PdfSource {
            path: "test.pdf".into(),
            page_count: n,
            pages: (1..=n)
                .map(|i| PageInfo {
                    number: i,
                    width_pt: 595.0,
                    height_pt: 842.0,
                    rotation: 0,
                })
                .collect(),
            encrypted: false,
            modification_restricted: false,
            metadata: vec![],
        }
    }

    #[test]
    fn scenario_b_22_pages_warns() {
        let findings = preflight(&source(22), BindingType::SaddleStitch, 3.0, None);
        let f = findings.iter().find(|f| f.code == "page_count_binding").unwrap();
        assert_eq!(f.severity, Severity::Warning);
        assert!(f.message.contains("2 blank"));
        assert!(!blocks_export(&findings));
    }

    #[test]
    fn divisible_page_count_is_clean() {
        let findings = preflight(&source(20), BindingType::SaddleStitch, 3.0, None);
        assert!(findings.iter().all(|f| f.code != "page_count_binding"));
    }

    #[test]
    fn missing_bleed_warns() {
        let findings = preflight(&source(4), BindingType::None, 0.0, None);
        assert!(findings.iter().any(|f| f.code == "missing_bleed"));
    }

    #[test]
    fn restricted_pdf_blocks_export() {
        let mut s = source(4);
        s.modification_restricted = true;
        let findings = preflight(&s, BindingType::None, 3.0, None);
        assert!(blocks_export(&findings));
    }

    #[test]
    fn mixed_sizes_detected() {
        let mut s = source(2);
        s.pages[1].width_pt = 400.0;
        let findings = preflight(&s, BindingType::None, 3.0, None);
        assert!(findings.iter().any(|f| f.code == "mixed_page_sizes"));
    }

    #[test]
    fn wrong_trim_size_detected() {
        let findings = preflight(&source(1), BindingType::None, 3.0, Some((420.0, 595.0)));
        assert!(findings.iter().any(|f| f.code == "wrong_page_size"));
    }

    #[test]
    fn rotated_trim_size_accepted() {
        let findings = preflight(&source(1), BindingType::None, 3.0, Some((842.0, 595.0)));
        assert!(findings.iter().all(|f| f.code != "wrong_page_size"));
    }

    #[test]
    fn stored_rotation_is_info() {
        let mut s = source(1);
        s.pages[0].rotation = 90;
        let findings = preflight(&s, BindingType::None, 3.0, None);
        let f = findings.iter().find(|f| f.code == "page_rotation").unwrap();
        assert_eq!(f.severity, Severity::Info);
    }
}
