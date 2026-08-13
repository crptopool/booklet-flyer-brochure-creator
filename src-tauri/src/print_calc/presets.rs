//! Document, sheet, bleed, DPI, GSM and binding presets.
//!
//! Dimensions are millimetres unless stated otherwise.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperSize {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl PaperSize {
    pub fn landscape(&self) -> PaperSize {
        PaperSize {
            name: self.name.clone(),
            width_mm: self.height_mm,
            height_mm: self.width_mm,
        }
    }

    pub fn is_landscape(&self) -> bool {
        self.width_mm > self.height_mm
    }
}

/// Standard trim / sheet sizes (portrait, mm).
pub fn paper_sizes() -> Vec<PaperSize> {
    let sizes: &[(&str, f64, f64)] = &[
        ("A3", 297.0, 420.0),
        ("A4", 210.0, 297.0),
        ("A5", 148.0, 210.0),
        ("A6", 105.0, 148.0),
        ("B5", 176.0, 250.0),
        ("Letter", 215.9, 279.4),
        ("Legal", 215.9, 355.6),
        ("Tabloid", 279.4, 431.8),
        ("SRA3", 320.0, 450.0),
        ("12x18in", 304.8, 457.2),
        ("13x19in", 330.2, 482.6),
    ];
    sizes
        .iter()
        .map(|(n, w, h)| PaperSize {
            name: n.to_string(),
            width_mm: *w,
            height_mm: *h,
        })
        .collect()
}

pub fn get_paper_size(name: &str) -> Result<PaperSize, String> {
    paper_sizes()
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Unknown paper size: {name}"))
}

/// Bleed presets in mm (0.125 in = 3.175 mm).
pub const BLEED_PRESETS_MM: [f64; 5] = [0.0, 2.0, 3.0, 5.0, 3.175];
pub const DEFAULT_BLEED_MM: f64 = 3.0;

/// Safe-margin presets in mm.
pub const SAFE_MARGIN_PRESETS_MM: [f64; 4] = [3.0, 5.0, 8.0, 10.0];

pub const RECOMMENDED_PRINT_DPI: f64 = 300.0;
pub const MINIMUM_PRINT_DPI: f64 = 200.0;

/// DPI presets with guidance text.
pub fn dpi_presets() -> Vec<(u32, &'static str)> {
    vec![
        (72, "Screen/web only"),
        (96, "Screen/web only"),
        (150, "Draft / large-format viewing"),
        (200, "Acceptable general print"),
        (300, "Recommended standard print"),
        (600, "High-detail line art / premium print"),
        (1200, "Specialized high-resolution workflows"),
    ]
}

/// Guidance text for a paper grammage.
pub fn describe_gsm(gsm: f64) -> &'static str {
    match gsm {
        g if (70.0..=90.0).contains(&g) => "Standard text/office pages",
        g if (90.0..=120.0).contains(&g) => "Premium text / brochures",
        g if (130.0..=170.0).contains(&g) => "Flyers / light covers",
        g if (200.0..=250.0).contains(&g) => "Cards / booklet covers",
        g if (300.0..=350.0).contains(&g) => "Heavy covers/cards",
        _ => "Custom GSM (actual caliper varies by manufacturer and finish)",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingType {
    None,
    SaddleStitch,
    Staple,
    Perfect,
    Spiral,
    WireO,
    Comb,
    Ring,
    Hardcover,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingSide {
    Left,
    Right,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplexMode {
    Simplex,
    LongEdge,
    ShortEdge,
    Manual,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_dimensions() {
        let a4 = get_paper_size("A4").unwrap();
        assert_eq!(a4.width_mm, 210.0);
        assert_eq!(a4.height_mm, 297.0);
        assert!(!a4.is_landscape());
    }

    #[test]
    fn landscape_swaps_dimensions() {
        let a4 = get_paper_size("A4").unwrap().landscape();
        assert_eq!(a4.width_mm, 297.0);
        assert!(a4.is_landscape());
    }

    #[test]
    fn unknown_size_errors() {
        assert!(get_paper_size("A9").is_err());
    }

    #[test]
    fn gsm_guidance() {
        assert_eq!(describe_gsm(80.0), "Standard text/office pages");
        assert_eq!(describe_gsm(150.0), "Flyers / light covers");
        assert_eq!(describe_gsm(320.0), "Heavy covers/cards");
        assert!(describe_gsm(500.0).starts_with("Custom GSM"));
    }
}
