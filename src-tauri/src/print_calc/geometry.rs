//! Printable region, bleed/trim boxes and binding-margin geometry.
//!
//! All values are in points unless a name says otherwise.

use serde::{Deserialize, Serialize};

use crate::print_calc::presets::{BindingSide, BindingType};
use crate::print_calc::units::mm_to_points;

/// Axis-aligned rectangle, PDF convention (origin bottom-left).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn top(&self) -> f64 {
        self.y + self.height
    }

    pub fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x && other.y >= self.y && other.right() <= self.right() && other.top() <= self.top()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Margins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

/// Region of the sheet the printer can actually mark.
pub fn printable_region(sheet_w: f64, sheet_h: f64, m: Margins) -> Result<Rect, String> {
    let w = sheet_w - m.left - m.right;
    let h = sheet_h - m.top - m.bottom;
    if w <= 0.0 || h <= 0.0 {
        return Err("printer margins exceed sheet size".into());
    }
    Ok(Rect {
        x: m.left,
        y: m.bottom,
        width: w,
        height: h,
    })
}

/// Bleed box around a trim box placed at the origin.
pub fn bleed_box(trim_w: f64, trim_h: f64, bleed: f64) -> Result<Rect, String> {
    if bleed < 0.0 {
        return Err("bleed cannot be negative".into());
    }
    Ok(Rect {
        x: -bleed,
        y: -bleed,
        width: trim_w + 2.0 * bleed,
        height: trim_h + 2.0 * bleed,
    })
}

/// Content-safe area inside the trim box, honouring the binding edge.
pub fn safe_area(
    trim_w: f64,
    trim_h: f64,
    safe_margin: f64,
    binding_margin: f64,
    binding_side: BindingSide,
) -> Result<Rect, String> {
    let left = safe_margin + if binding_side == BindingSide::Left { binding_margin } else { 0.0 };
    let right = safe_margin + if binding_side == BindingSide::Right { binding_margin } else { 0.0 };
    let top = safe_margin + if binding_side == BindingSide::Top { binding_margin } else { 0.0 };
    let bottom = safe_margin;
    let w = trim_w - left - right;
    let h = trim_h - top - bottom;
    if w <= 0.0 || h <= 0.0 {
        return Err("margins leave no safe area".into());
    }
    Ok(Rect {
        x: left,
        y: bottom,
        width: w,
        height: h,
    })
}

/// Recommended extra inside/binding margin (mm) by binding method.
/// Punched bindings need punch-safe clearance; glued/sewn bindings need
/// gutter allowance so content does not disappear into the spine.
pub fn binding_margin_mm(binding: BindingType) -> f64 {
    match binding {
        BindingType::None | BindingType::Custom => 0.0,
        BindingType::SaddleStitch | BindingType::Staple => 5.0,
        BindingType::Perfect => 10.0,
        BindingType::Spiral | BindingType::WireO => 12.0,
        BindingType::Comb | BindingType::Ring => 13.0,
        BindingType::Hardcover => 12.0,
    }
}

/// Recommended binding margin in points (user may override).
pub fn recommended_binding_margin(binding: BindingType) -> f64 {
    mm_to_points(binding_margin_mm(binding))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_region_subtracts_margins() {
        let r = printable_region(
            595.0,
            842.0,
            Margins {
                top: 10.0,
                bottom: 10.0,
                left: 15.0,
                right: 15.0,
            },
        )
        .unwrap();
        assert_eq!(r.x, 15.0);
        assert_eq!(r.y, 10.0);
        assert_eq!(r.width, 565.0);
        assert_eq!(r.height, 822.0);
    }

    #[test]
    fn excessive_margins_error() {
        assert!(printable_region(
            100.0,
            100.0,
            Margins {
                left: 60.0,
                right: 60.0,
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn bleed_box_extends_all_edges() {
        let b = bleed_box(100.0, 200.0, 8.5).unwrap();
        assert_eq!(b.x, -8.5);
        assert_eq!(b.width, 117.0);
        assert_eq!(b.height, 217.0);
    }

    #[test]
    fn safe_area_honours_left_binding() {
        let s = safe_area(200.0, 300.0, 10.0, 30.0, BindingSide::Left).unwrap();
        assert_eq!(s.x, 40.0);
        assert_eq!(s.width, 150.0);
        assert_eq!(s.height, 280.0);
    }

    #[test]
    fn safe_area_honours_top_binding() {
        let s = safe_area(200.0, 300.0, 10.0, 30.0, BindingSide::Top).unwrap();
        assert_eq!(s.height, 250.0);
        assert_eq!(s.y, 10.0);
    }

    #[test]
    fn rect_containment() {
        let outer = Rect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 };
        let inner = Rect { x: 10.0, y: 10.0, width: 50.0, height: 50.0 };
        assert!(outer.contains(&inner));
        assert!(!inner.contains(&outer));
    }

    #[test]
    fn punched_bindings_have_larger_margins() {
        assert!(binding_margin_mm(BindingType::Spiral) > binding_margin_mm(BindingType::SaddleStitch));
        assert_eq!(binding_margin_mm(BindingType::None), 0.0);
    }
}
