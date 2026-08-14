//! Cover geometry for eBook, paperback, hardcover and dust-jacket covers.
//!
//! Every dimension is derived deterministically from the trim size, page
//! count and paper caliper. Case-binding adds the board overhang, hinge
//! groove and turn-in allowance a hard case needs on top of the spine.
//! All allowances are inputs, because they vary by printer.

use serde::{Deserialize, Serialize};

use crate::print_calc::spine::{perfect_bound_sheet_count, spine_width_from_pages};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverKind {
    /// Front panel only, sized in pixels for a store listing.
    Ebook,
    /// Back + spine + front, wrapped around a glued text block.
    Paperback,
    /// Case-bound: boards, hinge grooves and turn-in around the edges.
    Hardcover,
    /// A printed jacket wrapped around a hardcover, with flaps.
    DustJacket,
}

/// Rectangle in millimetres, origin at the artboard's bottom-left.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectMm {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RectMm {
    fn inset(&self, by: f64) -> RectMm {
        RectMm {
            x: self.x + by,
            y: self.y + by,
            width: (self.width - 2.0 * by).max(0.0),
            height: (self.height - 2.0 * by).max(0.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverNote {
    pub severity: String,
    pub message: String,
}

fn note(severity: &str, message: impl Into<String>) -> CoverNote {
    CoverNote {
        severity: severity.into(),
        message: message.into(),
    }
}

/// Everything the user can control about a cover.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoverInputs {
    pub kind: CoverKind,
    pub trim_width_mm: f64,
    pub trim_height_mm: f64,
    pub page_count: u32,
    /// Single-sheet caliper in mm; drives the spine width.
    pub caliper_mm: f64,
    pub bleed_mm: f64,
    /// Distance content must stay inside the trim.
    pub safe_margin_mm: f64,
    /// Board overhang beyond the text block, per edge (hardcover).
    pub board_overhang_mm: f64,
    /// Hinge groove between spine and board (hardcover).
    pub hinge_mm: f64,
    /// Material folded around the boards (hardcover turn-in / jacket wrap).
    pub turn_in_mm: f64,
    /// Jacket flap width (dust jacket only).
    pub flap_mm: f64,
    /// Reserve the standard barcode area on the back panel.
    pub barcode: bool,
    /// eBook pixel dimensions.
    pub pixel_width: u32,
    pub pixel_height: u32,
}

/// Standard retail barcode block: 2 x 1.2 inches.
pub const BARCODE_W_MM: f64 = 50.8;
pub const BARCODE_H_MM: f64 = 30.5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverLayout {
    pub kind: CoverKind,
    /// Whole artboard including bleed, wrap and flaps.
    pub total_width_mm: f64,
    pub total_height_mm: f64,
    pub spine_width_mm: f64,
    /// The area trimmed to, inside the bleed.
    pub trim_rect: RectMm,
    pub back_panel: Option<RectMm>,
    pub spine_panel: Option<RectMm>,
    pub front_panel: RectMm,
    pub back_flap: Option<RectMm>,
    pub front_flap: Option<RectMm>,
    /// Content-safe areas inside each printed panel.
    pub safe_areas: Vec<RectMm>,
    pub barcode_rect: Option<RectMm>,
    /// X positions of spine folds, in mm from the artboard's left edge.
    pub fold_x_mm: Vec<f64>,
    /// X positions of hardcover hinge grooves.
    pub hinge_x_mm: Vec<f64>,
    /// eBook only: effective DPI at the stated trim size.
    pub effective_dpi: Option<f64>,
    pub notes: Vec<CoverNote>,
}

/// Build the complete cover layout.
pub fn cover_layout(input: CoverInputs) -> Result<CoverLayout, String> {
    if input.kind == CoverKind::Ebook {
        return ebook_layout(input);
    }
    if input.trim_width_mm <= 0.0 || input.trim_height_mm <= 0.0 {
        return Err("trim size must be positive".into());
    }
    if input.page_count == 0 {
        return Err("page count must be positive".into());
    }
    if input.caliper_mm <= 0.0 {
        return Err("paper caliper must be positive".into());
    }
    if input.bleed_mm < 0.0 || input.safe_margin_mm < 0.0 {
        return Err("bleed and safe margin cannot be negative".into());
    }

    let spine = spine_width_from_pages(input.page_count, input.caliper_mm)?;
    let mut notes = Vec::new();

    let bleed = input.bleed_mm;
    // Hardcover panels are larger than the text block by the board overhang.
    let hard = input.kind == CoverKind::Hardcover;
    let panel_w = input.trim_width_mm + if hard { input.board_overhang_mm } else { 0.0 };
    let panel_h = input.trim_height_mm + if hard { 2.0 * input.board_overhang_mm } else { 0.0 };
    // A hard case's spine must clear the boards themselves.
    let spine_panel_w = if hard { spine + 2.0 * input.board_overhang_mm } else { spine };
    let hinge = if hard { input.hinge_mm.max(0.0) } else { 0.0 };
    let flap = if input.kind == CoverKind::DustJacket { input.flap_mm.max(0.0) } else { 0.0 };
    // Turn-in wraps a hardcover case or a jacket; a paperback has none.
    let wrap = if hard { input.turn_in_mm.max(0.0) } else { 0.0 };

    // Left to right: [wrap][flap][back][hinge][spine][hinge][front][flap][wrap]
    let trim_w = 2.0 * flap + 2.0 * panel_w + spine_panel_w + 2.0 * hinge;
    let trim_h = panel_h;
    let total_w = trim_w + 2.0 * bleed + 2.0 * wrap;
    let total_h = trim_h + 2.0 * bleed + 2.0 * wrap;

    let origin = bleed + wrap;
    let trim_rect = RectMm {
        x: origin,
        y: origin,
        width: trim_w,
        height: trim_h,
    };

    let mut x = origin;
    let back_flap = if flap > 0.0 {
        let r = RectMm { x, y: origin, width: flap, height: trim_h };
        x += flap;
        Some(r)
    } else {
        None
    };
    let back_panel = RectMm { x, y: origin, width: panel_w, height: trim_h };
    x += panel_w;
    let hinge_left = x;
    x += hinge;
    let spine_panel = RectMm { x, y: origin, width: spine_panel_w, height: trim_h };
    let spine_left = x;
    x += spine_panel_w;
    let spine_right = x;
    let hinge_right = x;
    x += hinge;
    let front_panel = RectMm { x, y: origin, width: panel_w, height: trim_h };
    x += panel_w;
    let front_flap = if flap > 0.0 {
        Some(RectMm { x, y: origin, width: flap, height: trim_h })
    } else {
        None
    };

    let safe = input.safe_margin_mm;
    let mut safe_areas = vec![back_panel.inset(safe), front_panel.inset(safe)];
    if let Some(f) = back_flap {
        safe_areas.push(f.inset(safe));
    }
    if let Some(f) = front_flap {
        safe_areas.push(f.inset(safe));
    }

    // Barcode sits bottom-right of the back panel, clear of the safe margin.
    let barcode_rect = if input.barcode {
        let bx = back_panel.x + back_panel.width - BARCODE_W_MM - safe.max(6.0);
        let by = back_panel.y + safe.max(6.0);
        if bx < back_panel.x || BARCODE_H_MM + 2.0 * safe > back_panel.height {
            notes.push(note(
                "WARNING",
                "The back panel is too small to hold the standard 50.8 x 30.5 mm barcode block clear of the safe margin.",
            ));
            None
        } else {
            Some(RectMm { x: bx, y: by, width: BARCODE_W_MM, height: BARCODE_H_MM })
        }
    } else {
        None
    };

    notes.push(note(
        "INFO",
        format!(
            "Spine width {spine:.2} mm from {} leaves at {:.3} mm caliper. Confirm the caliper with your printer — it varies by paper make and finish.",
            perfect_bound_sheet_count(input.page_count)?,
            input.caliper_mm
        ),
    ));

    if spine < 3.0 && input.kind != CoverKind::Ebook {
        notes.push(note(
            "WARNING",
            format!(
                "A {spine:.2} mm spine is too narrow to carry legible type. Most printers want at least 3 mm before placing text on the spine."
            ),
        ));
    }
    if bleed <= 0.0 {
        notes.push(note(
            "WARNING",
            "No bleed set. A full-wrap cover is trimmed on all four edges, so background artwork needs 3 mm of bleed or white slivers will show.",
        ));
    }
    if hard && wrap <= 0.0 {
        notes.push(note(
            "ERROR",
            "A case-bound cover must have a turn-in allowance — the printed sheet folds around the boards and is glued to the inside.",
        ));
    }
    if hard && hinge <= 0.0 {
        notes.push(note(
            "WARNING",
            "No hinge groove set. Without it the case cannot flex where the boards meet the spine.",
        ));
    }
    notes.push(note(
        "INFO",
        format!("Finished artboard {total_w:.1} x {total_h:.1} mm, trimming to {trim_w:.1} x {trim_h:.1} mm."),
    ));

    Ok(CoverLayout {
        kind: input.kind,
        total_width_mm: total_w,
        total_height_mm: total_h,
        spine_width_mm: spine,
        trim_rect,
        back_panel: Some(back_panel),
        spine_panel: Some(spine_panel),
        front_panel,
        back_flap,
        front_flap,
        safe_areas,
        barcode_rect,
        fold_x_mm: vec![spine_left, spine_right],
        hinge_x_mm: if hard { vec![hinge_left, hinge_right] } else { vec![] },
        effective_dpi: None,
        notes,
    })
}

/// eBook covers are a single front panel measured in pixels.
fn ebook_layout(input: CoverInputs) -> Result<CoverLayout, String> {
    if input.pixel_width == 0 || input.pixel_height == 0 {
        return Err("pixel dimensions must be positive".into());
    }
    let mut notes = Vec::new();
    let ratio = input.pixel_height as f64 / input.pixel_width as f64;

    // Effective DPI only means something once a print size is stated.
    let effective_dpi = if input.trim_width_mm > 0.0 {
        Some(input.pixel_width as f64 / (input.trim_width_mm / 25.4))
    } else {
        None
    };

    if input.pixel_width < 1400 {
        notes.push(note(
            "WARNING",
            format!(
                "{} px wide is below the 1400 px minimum most stores accept; 1600–2560 px is the usual range.",
                input.pixel_width
            ),
        ));
    }
    if (ratio - 1.5).abs() > 0.25 {
        notes.push(note(
            "INFO",
            format!(
                "Aspect ratio is 1:{ratio:.2}. Most eBook stores expect about 1:1.5 (for example 1600 x 2400 px), and will letterbox anything far from it."
            ),
        ));
    } else {
        notes.push(note("INFO", format!("Aspect ratio 1:{ratio:.2} suits the common eBook store requirement.")));
    }
    if let Some(dpi) = effective_dpi {
        notes.push(note(
            if dpi < 200.0 { "WARNING" } else { "INFO" },
            format!("At {:.0} mm wide this artwork prints at approximately {dpi:.0} DPI.", input.trim_width_mm),
        ));
    }
    notes.push(note(
        "INFO",
        "An eBook cover is screen artwork: it needs no bleed, no spine and no trim allowance.",
    ));

    let front = RectMm {
        x: 0.0,
        y: 0.0,
        width: input.pixel_width as f64,
        height: input.pixel_height as f64,
    };
    Ok(CoverLayout {
        kind: CoverKind::Ebook,
        total_width_mm: front.width,
        total_height_mm: front.height,
        spine_width_mm: 0.0,
        trim_rect: front,
        back_panel: None,
        spine_panel: None,
        front_panel: front,
        back_flap: None,
        front_flap: None,
        safe_areas: vec![],
        barcode_rect: None,
        fold_x_mm: vec![],
        hinge_x_mm: vec![],
        effective_dpi,
        notes,
    })
}

/// Sensible starting values for a kind of cover.
pub fn default_inputs(kind: CoverKind) -> CoverInputs {
    CoverInputs {
        kind,
        trim_width_mm: 148.0,
        trim_height_mm: 210.0,
        page_count: 200,
        caliper_mm: 0.092,
        bleed_mm: 3.0,
        safe_margin_mm: 6.0,
        board_overhang_mm: 3.0,
        hinge_mm: 8.0,
        turn_in_mm: 16.0,
        flap_mm: 80.0,
        barcode: kind != CoverKind::Ebook,
        pixel_width: 1600,
        pixel_height: 2400,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paperback() -> CoverInputs {
        CoverInputs {
            page_count: 200,
            caliper_mm: 0.1,
            ..default_inputs(CoverKind::Paperback)
        }
    }

    #[test]
    fn paperback_width_is_two_panels_plus_spine_plus_bleed() {
        let l = cover_layout(paperback()).unwrap();
        // 200 pages -> 100 leaves -> 10 mm spine.
        assert!((l.spine_width_mm - 10.0).abs() < 1e-9);
        // 148 + 10 + 148 = 306 trim, + 3 mm bleed each side.
        assert!((l.trim_rect.width - 306.0).abs() < 1e-9);
        assert!((l.total_width_mm - 312.0).abs() < 1e-9);
        assert!((l.total_height_mm - 216.0).abs() < 1e-9);
    }

    #[test]
    fn panels_tile_the_trim_without_gaps() {
        let l = cover_layout(paperback()).unwrap();
        let back = l.back_panel.unwrap();
        let spine = l.spine_panel.unwrap();
        assert!((back.x - l.trim_rect.x).abs() < 1e-9);
        assert!((back.x + back.width - spine.x).abs() < 1e-9);
        assert!((spine.x + spine.width - l.front_panel.x).abs() < 1e-9);
        assert!((l.front_panel.x + l.front_panel.width - (l.trim_rect.x + l.trim_rect.width)).abs() < 1e-9);
    }

    #[test]
    fn spine_folds_sit_on_the_spine_edges() {
        let l = cover_layout(paperback()).unwrap();
        let spine = l.spine_panel.unwrap();
        assert_eq!(l.fold_x_mm.len(), 2);
        assert!((l.fold_x_mm[0] - spine.x).abs() < 1e-9);
        assert!((l.fold_x_mm[1] - (spine.x + spine.width)).abs() < 1e-9);
    }

    #[test]
    fn barcode_sits_inside_the_back_panel() {
        let l = cover_layout(paperback()).unwrap();
        let b = l.barcode_rect.unwrap();
        let back = l.back_panel.unwrap();
        assert!(b.x >= back.x && b.x + b.width <= back.x + back.width);
        assert!(b.y >= back.y && b.y + b.height <= back.y + back.height);
        assert!((b.width - BARCODE_W_MM).abs() < 1e-9);
    }

    #[test]
    fn tiny_back_panel_cannot_hold_a_barcode() {
        let l = cover_layout(CoverInputs {
            trim_width_mm: 40.0,
            trim_height_mm: 60.0,
            ..paperback()
        })
        .unwrap();
        assert!(l.barcode_rect.is_none());
        assert!(l.notes.iter().any(|n| n.message.contains("barcode")));
    }

    #[test]
    fn hardcover_adds_boards_hinge_and_turn_in() {
        let hard = CoverInputs { page_count: 200, caliper_mm: 0.1, ..default_inputs(CoverKind::Hardcover) };
        let l = cover_layout(hard).unwrap();
        let soft = cover_layout(paperback()).unwrap();
        // A case is larger than a paperback of the same book on every axis.
        assert!(l.total_width_mm > soft.total_width_mm);
        assert!(l.total_height_mm > soft.total_height_mm);
        assert_eq!(l.hinge_x_mm.len(), 2);
        // Board overhang: 3 mm top and bottom on a 210 mm page.
        assert!((l.trim_rect.height - 216.0).abs() < 1e-9);
    }

    #[test]
    fn hardcover_without_turn_in_is_an_error_note() {
        let l = cover_layout(CoverInputs { turn_in_mm: 0.0, ..default_inputs(CoverKind::Hardcover) }).unwrap();
        assert!(l.notes.iter().any(|n| n.severity == "ERROR"));
    }

    #[test]
    fn dust_jacket_adds_two_flaps() {
        let l = cover_layout(CoverInputs {
            page_count: 200,
            caliper_mm: 0.1,
            flap_mm: 80.0,
            ..default_inputs(CoverKind::DustJacket)
        })
        .unwrap();
        assert!(l.back_flap.is_some() && l.front_flap.is_some());
        // 306 mm cover + two 80 mm flaps.
        assert!((l.trim_rect.width - 466.0).abs() < 1e-9);
    }

    #[test]
    fn narrow_spine_is_flagged() {
        let l = cover_layout(CoverInputs { page_count: 20, ..paperback() }).unwrap();
        assert!(l.notes.iter().any(|n| n.message.contains("too narrow")));
    }

    #[test]
    fn missing_bleed_is_flagged() {
        let l = cover_layout(CoverInputs { bleed_mm: 0.0, ..paperback() }).unwrap();
        assert!(l.notes.iter().any(|n| n.message.contains("bleed")));
    }

    #[test]
    fn safe_areas_are_inset_from_the_panels() {
        let l = cover_layout(paperback()).unwrap();
        let back = l.back_panel.unwrap();
        let safe = l.safe_areas[0];
        assert!((safe.x - back.x - 6.0).abs() < 1e-9);
        assert!((safe.width - back.width + 12.0).abs() < 1e-9);
    }

    #[test]
    fn ebook_reports_ratio_and_dpi() {
        let l = cover_layout(default_inputs(CoverKind::Ebook)).unwrap();
        assert_eq!(l.spine_width_mm, 0.0);
        assert!(l.back_panel.is_none());
        assert_eq!(l.total_width_mm, 1600.0);
        // 1600 px across 148 mm is about 274 DPI.
        let dpi = l.effective_dpi.unwrap();
        assert!((dpi - 274.6).abs() < 1.0);
    }

    #[test]
    fn small_ebook_artwork_is_flagged() {
        let l = cover_layout(CoverInputs { pixel_width: 600, pixel_height: 900, ..default_inputs(CoverKind::Ebook) }).unwrap();
        assert!(l.notes.iter().any(|n| n.severity == "WARNING" && n.message.contains("1400")));
    }

    #[test]
    fn unusual_ebook_ratio_is_flagged() {
        let l = cover_layout(CoverInputs { pixel_width: 2000, pixel_height: 2000, ..default_inputs(CoverKind::Ebook) }).unwrap();
        assert!(l.notes.iter().any(|n| n.message.contains("Aspect ratio is")));
    }

    #[test]
    fn invalid_inputs_error() {
        assert!(cover_layout(CoverInputs { page_count: 0, ..paperback() }).is_err());
        assert!(cover_layout(CoverInputs { caliper_mm: 0.0, ..paperback() }).is_err());
        assert!(cover_layout(CoverInputs { trim_width_mm: 0.0, ..paperback() }).is_err());
        assert!(cover_layout(CoverInputs { pixel_width: 0, ..default_inputs(CoverKind::Ebook) }).is_err());
    }
}
