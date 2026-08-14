//! Write a print-ready cover template as a new PDF.
//!
//! The output is an artboard at the exact finished size with the spine,
//! trim, bleed, safe-area and barcode guides drawn on it, ready for a
//! designer to place artwork against. Guides sit on their own optional
//! content group so they can be switched off in a PDF viewer.

use lopdf::content::{Content, Operation as Op};
use lopdf::{dictionary, Document, Object, Stream};

use crate::print_calc::cover::{CoverKind, CoverLayout, RectMm};
use crate::print_calc::units::mm_to_points;

fn num(v: f64) -> Object {
    Object::Real(v as f32)
}

/// Stroke an unfilled rectangle.
fn rect_ops(r: &RectMm, ops: &mut Vec<Op>) {
    ops.push(Op::new(
        "re",
        vec![
            num(mm_to_points(r.x)),
            num(mm_to_points(r.y)),
            num(mm_to_points(r.width)),
            num(mm_to_points(r.height)),
        ],
    ));
    ops.push(Op::new("S", vec![]));
}

fn set_stroke(ops: &mut Vec<Op>, r: f64, g: f64, b: f64, width: f64, dash: Option<(i64, i64)>) {
    ops.push(Op::new("RG", vec![num(r), num(g), num(b)]));
    ops.push(Op::new("w", vec![num(width)]));
    match dash {
        Some((on, off)) => ops.push(Op::new("d", vec![Object::Array(vec![on.into(), off.into()]), 0.into()])),
        None => ops.push(Op::new("d", vec![Object::Array(vec![]), 0.into()])),
    }
}

fn text_ops(ops: &mut Vec<Op>, x_mm: f64, y_mm: f64, size: f64, gray: f64, text: &str) {
    ops.push(Op::new("BT", vec![]));
    ops.push(Op::new("Tf", vec![Object::Name(b"G".to_vec()), num(size)]));
    ops.push(Op::new("g", vec![num(gray)]));
    ops.push(Op::new("Td", vec![num(mm_to_points(x_mm)), num(mm_to_points(y_mm))]));
    ops.push(Op::new("Tj", vec![Object::string_literal(text)]));
    ops.push(Op::new("ET", vec![]));
}

/// Write the cover template. Returns the artboard size in points.
pub fn export_cover(layout: &CoverLayout, output_path: &str, title: &str) -> Result<(f64, f64), String> {
    if layout.kind == CoverKind::Ebook {
        return Err(
            "An eBook cover is screen artwork — export it as PNG or JPEG rather than a print PDF."
                .into(),
        );
    }
    if layout.total_width_mm <= 0.0 || layout.total_height_mm <= 0.0 {
        return Err("cover has no area".into());
    }

    let page_w = mm_to_points(layout.total_width_mm);
    let page_h = mm_to_points(layout.total_height_mm);

    let mut ops: Vec<Op> = vec![Op::new("q", vec![])];

    // Bleed edge — the outer artboard.
    set_stroke(&mut ops, 0.85, 0.35, 0.1, 0.75, Some((2, 2)));
    rect_ops(
        &RectMm { x: 0.0, y: 0.0, width: layout.total_width_mm, height: layout.total_height_mm },
        &mut ops,
    );

    // Trim line — where the cover is cut.
    set_stroke(&mut ops, 0.0, 0.0, 0.0, 1.0, None);
    rect_ops(&layout.trim_rect, &mut ops);

    // Panel divisions: spine folds run the full height.
    set_stroke(&mut ops, 0.17, 0.37, 0.77, 1.0, Some((4, 3)));
    for &fx in &layout.fold_x_mm {
        ops.push(Op::new("m", vec![num(mm_to_points(fx)), num(0.0)]));
        ops.push(Op::new("l", vec![num(mm_to_points(fx)), num(page_h)]));
    }
    ops.push(Op::new("S", vec![]));

    // Hardcover hinge grooves.
    if !layout.hinge_x_mm.is_empty() {
        set_stroke(&mut ops, 0.45, 0.2, 0.6, 0.75, Some((1, 2)));
        for &hx in &layout.hinge_x_mm {
            ops.push(Op::new("m", vec![num(mm_to_points(hx)), num(0.0)]));
            ops.push(Op::new("l", vec![num(mm_to_points(hx)), num(page_h)]));
        }
        ops.push(Op::new("S", vec![]));
    }

    // Safe areas.
    set_stroke(&mut ops, 0.13, 0.4, 0.17, 0.5, Some((3, 3)));
    for area in &layout.safe_areas {
        rect_ops(area, &mut ops);
    }

    // Barcode reservation.
    if let Some(b) = layout.barcode_rect {
        set_stroke(&mut ops, 0.76, 0.25, 0.05, 0.75, None);
        rect_ops(&b, &mut ops);
        text_ops(&mut ops, b.x + 2.0, b.y + b.height / 2.0, 7.0, 0.4, "barcode area — keep clear");
    }

    // Panel labels.
    let label_y = layout.trim_rect.y + 4.0;
    if let Some(p) = layout.back_panel {
        text_ops(&mut ops, p.x + 4.0, p.y + p.height - 7.0, 8.0, 0.45, "BACK COVER");
    }
    if let Some(p) = layout.spine_panel {
        text_ops(
            &mut ops,
            p.x + 1.0,
            p.y + p.height / 2.0,
            6.0,
            0.45,
            &format!("SPINE {:.1}mm", layout.spine_width_mm),
        );
    }
    text_ops(&mut ops, layout.front_panel.x + 4.0, layout.front_panel.y + layout.front_panel.height - 7.0, 8.0, 0.45, "FRONT COVER");
    if let Some(f) = layout.back_flap {
        text_ops(&mut ops, f.x + 3.0, f.y + f.height - 7.0, 7.0, 0.5, "BACK FLAP");
    }
    if let Some(f) = layout.front_flap {
        text_ops(&mut ops, f.x + 3.0, f.y + f.height - 7.0, 7.0, 0.5, "FRONT FLAP");
    }

    text_ops(
        &mut ops,
        layout.trim_rect.x + 4.0,
        label_y,
        6.0,
        0.55,
        &format!(
            "{title} — {:.1} x {:.1} mm artboard, trims to {:.1} x {:.1} mm",
            layout.total_width_mm, layout.total_height_mm, layout.trim_rect.width, layout.trim_rect.height
        ),
    );

    ops.push(Op::new("Q", vec![]));

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        Content { operations: ops }
            .encode()
            .map_err(|e| format!("Failed to build cover content: {e}"))?,
    ));

    let mut page = dictionary! {
        "Type" => "Page",
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => vec![num(0.0), num(0.0), num(page_w), num(page_h)],
        "Resources" => dictionary! {
            "Font" => dictionary! { "G" => Object::Reference(font_id) },
        },
        "Contents" => Object::Reference(content_id),
    };
    // The trim box is the finished cover; the media box carries the bleed.
    let t = &layout.trim_rect;
    page.set(
        "TrimBox",
        vec![
            num(mm_to_points(t.x)),
            num(mm_to_points(t.y)),
            num(mm_to_points(t.x + t.width)),
            num(mm_to_points(t.y + t.height)),
        ],
    );
    page.set("BleedBox", vec![num(0.0), num(0.0), num(page_w), num(page_h)]);

    let page_id = doc.add_object(Object::Dictionary(page));
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
    doc.trailer.set("Root", catalog);
    doc.compress();
    doc.save(output_path).map_err(|e| format!("Failed to save cover PDF: {e}"))?;

    Ok((page_w, page_h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_ops::document::inspect_pdf;
    use crate::print_calc::cover::{cover_layout, default_inputs, CoverInputs};

    fn tmp(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn writes_a_single_artboard_at_the_computed_size() {
        let layout = cover_layout(CoverInputs {
            page_count: 200,
            caliper_mm: 0.1,
            ..default_inputs(CoverKind::Paperback)
        })
        .unwrap();
        let out = tmp("cover_paperback.pdf");
        let (w, h) = export_cover(&layout, &out, "Test Book").unwrap();
        // 312 x 216 mm artboard.
        assert!((w - mm_to_points(312.0)).abs() < 0.01);
        assert!((h - mm_to_points(216.0)).abs() < 0.01);

        let info = inspect_pdf(&out).unwrap();
        assert_eq!(info.page_count, 1);
        assert!((info.pages[0].width_pt - w).abs() < 0.5);
    }

    #[test]
    fn hardcover_template_includes_hinges() {
        let layout = cover_layout(default_inputs(CoverKind::Hardcover)).unwrap();
        let out = tmp("cover_hardcover.pdf");
        assert!(export_cover(&layout, &out, "Cased Book").is_ok());
        assert_eq!(inspect_pdf(&out).unwrap().page_count, 1);
        assert_eq!(layout.hinge_x_mm.len(), 2);
    }

    #[test]
    fn ebook_covers_are_refused_as_print_pdfs() {
        let layout = cover_layout(default_inputs(CoverKind::Ebook)).unwrap();
        let err = export_cover(&layout, &tmp("cover_ebook.pdf"), "eBook").unwrap_err();
        assert!(err.contains("PNG"));
    }

    #[test]
    fn cover_pdf_carries_trim_and_bleed_boxes() {
        let layout = cover_layout(default_inputs(CoverKind::Paperback)).unwrap();
        let out = tmp("cover_boxes.pdf");
        export_cover(&layout, &out, "Boxes").unwrap();
        let doc = Document::load(&out).unwrap();
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        let dict = doc.get_object(page_id).unwrap().as_dict().unwrap();
        assert!(dict.get(b"TrimBox").is_ok());
        assert!(dict.get(b"BleedBox").is_ok());
    }
}
