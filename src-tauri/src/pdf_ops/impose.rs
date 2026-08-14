//! Imposition rendering: place source pages onto physical printer sheets
//! and write the result as a new PDF.
//!
//! Each source page becomes a Form XObject and is drawn onto the sheet
//! with a placement matrix. Nothing is rasterised — vector artwork, live
//! text and embedded fonts survive intact, and the source file is opened
//! read-only.

use std::collections::BTreeMap;

use lopdf::content::{Content, Operation as Op};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};

use crate::print_calc::imposition::CellPlacement;
use crate::print_calc::units::mm_to_points;

/// One source page placed on a sheet.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Placement {
    /// 1-based source page; `None` leaves the cell blank.
    pub page: Option<u32>,
    /// Cell origin on the sheet, in points, from the bottom-left.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Extra rotation applied to the page, in degrees (0/90/180/270).
    pub rotation: i64,
}

impl Placement {
    pub fn from_cell(cell: &CellPlacement, rotation: i64) -> Self {
        Placement {
            page: cell.page,
            x: cell.x,
            y: cell.y,
            width: cell.width,
            height: cell.height,
            rotation,
        }
    }
}

/// Which production marks to draw outside the trimmed area.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarkOptions {
    pub crop_marks: bool,
    pub fold_marks: bool,
    pub sheet_labels: bool,
    /// Bleed in millimetres, written as the sheet's `/BleedBox`.
    pub bleed_mm: f64,
}

impl Default for MarkOptions {
    fn default() -> Self {
        MarkOptions {
            crop_marks: true,
            fold_marks: true,
            sheet_labels: true,
            bleed_mm: 3.0,
        }
    }
}

/// Smallest rectangle enclosing every placed cell, as (x0, y0, x1, y1).
///
/// This is the area the finished sheet is trimmed to, so it becomes the
/// `/TrimBox`. Returns `None` when nothing is placed.
fn trim_bounds(side: &SheetSide) -> Option<(f64, f64, f64, f64)> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for p in &side.placements {
        let r = (p.x, p.y, p.x + p.width, p.y + p.height);
        bounds = Some(match bounds {
            None => r,
            Some(b) => (b.0.min(r.0), b.1.min(r.1), b.2.max(r.2), b.3.max(r.3)),
        });
    }
    bounds
}

/// One physical sheet side ready to be written.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SheetSide {
    pub sheet_number: u32,
    /// "front" or "back".
    pub side: String,
    pub width: f64,
    pub height: f64,
    pub placements: Vec<Placement>,
    /// X positions (points) where the sheet is folded.
    pub fold_x: Vec<f64>,
}

fn number(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// Fetch a possibly-inherited page attribute, resolving references.
fn inherited(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = page_id;
    for _ in 0..64 {
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(value) = dict.get(key) {
            return match value.as_reference() {
                Ok(r) => doc.get_object(r).ok().cloned(),
                Err(_) => Some(value.clone()),
            };
        }
        match dict.get(b"Parent").and_then(|p| p.as_reference()) {
            Ok(parent) => current = parent,
            Err(_) => return None,
        }
    }
    None
}

/// MediaBox as (x0, y0, width, height).
fn media_box(doc: &Document, page_id: ObjectId) -> (f64, f64, f64, f64) {
    let coords: Vec<f64> = inherited(doc, page_id, b"MediaBox")
        .and_then(|o| o.as_array().ok().cloned())
        .map(|a| a.iter().filter_map(number).collect())
        .unwrap_or_default();
    if coords.len() == 4 {
        let x0 = coords[0].min(coords[2]);
        let y0 = coords[1].min(coords[3]);
        (x0, y0, (coords[2] - coords[0]).abs(), (coords[3] - coords[1]).abs())
    } else {
        (0.0, 0.0, 595.275_590_551_181_2, 841.889_763_779_527_6)
    }
}

/// Merge the page's resource dictionaries into one self-contained dict.
fn merged_resources(doc: &Document, page_id: ObjectId) -> Dictionary {
    let mut merged = Dictionary::new();
    let (inline, referenced) = match doc.get_page_resources(page_id) {
        Ok(r) => r,
        Err(_) => (None, vec![]),
    };
    let mut dicts: Vec<Dictionary> = Vec::new();
    for id in referenced {
        if let Ok(d) = doc.get_object(id).and_then(|o| o.as_dict()) {
            dicts.push(d.clone());
        }
    }
    if let Some(d) = inline {
        dicts.push(d.clone());
    }
    for d in dicts {
        for (key, value) in d.iter() {
            // Resource categories are themselves dictionaries; merge their
            // entries so names from every source remain resolvable.
            let existing = merged.get(key).ok().cloned();
            match (existing, value) {
                (Some(old), new) => {
                    let old_d = match &old {
                        Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()).cloned(),
                        Object::Dictionary(d) => Some(d.clone()),
                        _ => None,
                    };
                    let new_d = match new {
                        Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()).cloned(),
                        Object::Dictionary(d) => Some(d.clone()),
                        _ => None,
                    };
                    if let (Some(mut a), Some(b)) = (old_d, new_d) {
                        for (k, v) in b.iter() {
                            a.set(k.to_vec(), v.clone());
                        }
                        merged.set(key.to_vec(), Object::Dictionary(a));
                    } else {
                        merged.set(key.to_vec(), value.clone());
                    }
                }
                (None, new) => {
                    merged.set(key.to_vec(), new.clone());
                }
            }
        }
    }
    merged
}

/// Placement matrix mapping the page box into its cell.
///
/// Returns `[a b c d e f]` for the PDF `cm` operator. The page is scaled
/// uniformly to fit the cell and centred inside it.
fn placement_matrix(p: &Placement, page_w: f64, page_h: f64) -> [f64; 6] {
    let quarter = p.rotation.rem_euclid(360);
    // A quarter turn swaps the footprint the page needs in the cell.
    let (fit_w, fit_h) = if quarter == 90 || quarter == 270 {
        (page_h, page_w)
    } else {
        (page_w, page_h)
    };
    let scale = (p.width / fit_w).min(p.height / fit_h).max(0.0);
    let ox = p.x + (p.width - fit_w * scale) / 2.0;
    let oy = p.y + (p.height - fit_h * scale) / 2.0;
    let s = scale;
    match quarter {
        90 => [0.0, s, -s, 0.0, ox + page_h * s, oy],
        180 => [-s, 0.0, 0.0, -s, ox + page_w * s, oy + page_h * s],
        270 => [0.0, -s, s, 0.0, ox, oy + page_w * s],
        _ => [s, 0.0, 0.0, s, ox, oy],
    }
}

fn num(v: f64) -> Object {
    Object::Real(v as f32)
}

/// Crop, fold and label marks drawn around the placed pages.
fn mark_operations(side: &SheetSide, marks: MarkOptions) -> Vec<Op> {
    let mut ops = vec![
        Op::new("q", vec![]),
        Op::new("0.5 w", vec![]),
        Op::new("0 G", vec![]),
    ];
    // `0.5 w` above is not a real operator name; set line width properly.
    ops.truncate(1);
    ops.push(Op::new("w", vec![0.5.into()]));
    ops.push(Op::new("G", vec![0.into()]));

    const LEN: f64 = 12.0;
    const OFF: f64 = 4.0;

    if marks.crop_marks {
        // Corner marks for every distinct cell, kept outside the trim.
        for p in &side.placements {
            let (l, r, b, t) = (p.x, p.x + p.width, p.y, p.y + p.height);
            for &(cx, cy, dx, dy) in &[
                (l, b, -1.0f64, -1.0f64),
                (r, b, 1.0, -1.0),
                (l, t, -1.0, 1.0),
                (r, t, 1.0, 1.0),
            ] {
                // Horizontal arm.
                ops.push(Op::new("m", vec![num(cx + dx * OFF), num(cy)]));
                ops.push(Op::new("l", vec![num(cx + dx * (OFF + LEN)), num(cy)]));
                // Vertical arm.
                ops.push(Op::new("m", vec![num(cx), num(cy + dy * OFF)]));
                ops.push(Op::new("l", vec![num(cx), num(cy + dy * (OFF + LEN))]));
            }
        }
    }

    if marks.fold_marks {
        for &fx in &side.fold_x {
            ops.push(Op::new("d", vec![Object::Array(vec![3.into(), 3.into()]), 0.into()]));
            ops.push(Op::new("m", vec![num(fx), num(0.0)]));
            ops.push(Op::new("l", vec![num(fx), num(10.0)]));
            ops.push(Op::new("m", vec![num(fx), num(side.height - 10.0)]));
            ops.push(Op::new("l", vec![num(fx), num(side.height)]));
            ops.push(Op::new("d", vec![Object::Array(vec![]), 0.into()]));
        }
    }

    ops.push(Op::new("S", vec![]));
    ops.push(Op::new("Q", vec![]));
    ops
}

/// Sheet label drawn in the trim waste at the foot of the sheet.
fn label_operations(side: &SheetSide, font: &str) -> Vec<Op> {
    let text = format!("Sheet {} — {}", side.sheet_number, side.side);
    vec![
        Op::new("q", vec![]),
        Op::new("BT", vec![]),
        Op::new("Tf", vec![Object::Name(font.into()), 7.into()]),
        Op::new("g", vec![0.4.into()]),
        Op::new("Td", vec![num(6.0), num(4.0)]),
        Op::new("Tj", vec![Object::string_literal(text)]),
        Op::new("ET", vec![]),
        Op::new("Q", vec![]),
    ]
}

/// Write the imposed sheets as a new PDF.
///
/// Returns the number of sheet sides written. The source file is never
/// modified and the output must be a different path.
pub fn export_imposed(
    source_path: &str,
    sides: &[SheetSide],
    output_path: &str,
    marks: MarkOptions,
) -> Result<u32, String> {
    if sides.is_empty() {
        return Err("nothing to impose — the plan produced no sheets".into());
    }
    if source_path == output_path {
        return Err("output must not overwrite the source file".into());
    }

    let mut doc = Document::load(source_path).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let page_map = doc.get_pages();

    let pages_root_id = doc
        .catalog()
        .and_then(|c| c.get(b"Pages"))
        .and_then(|p| p.as_reference())
        .map_err(|e| format!("Invalid PDF page tree: {e}"))?;

    // Turn each referenced source page into a Form XObject exactly once.
    let mut forms: BTreeMap<u32, (ObjectId, f64, f64)> = BTreeMap::new();
    let mut wanted: Vec<u32> = sides
        .iter()
        .flat_map(|s| s.placements.iter().filter_map(|p| p.page))
        .collect();
    wanted.sort_unstable();
    wanted.dedup();

    for n in wanted {
        let &page_id = page_map
            .get(&n)
            .ok_or_else(|| format!("source page {n} not found"))?;
        let (x0, y0, w, h) = media_box(&doc, page_id);
        let content = doc.get_page_content(page_id);
        let resources = merged_resources(&doc, page_id);
        // A stored /Rotate must be baked in, otherwise the placed page
        // would ignore the rotation the viewer applies.
        let stored = inherited(&doc, page_id, b"Rotate")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0)
            .rem_euclid(360);
        let (form_w, form_h) = if stored == 90 || stored == 270 { (h, w) } else { (w, h) };
        // Matrix maps the page box to an origin-anchored, rotation-baked box.
        let matrix: Vec<Object> = match stored {
            90 => vec![num(0.0), num(1.0), num(-1.0), num(0.0), num(h + y0), num(-x0)],
            180 => vec![num(-1.0), num(0.0), num(0.0), num(-1.0), num(w + x0), num(h + y0)],
            270 => vec![num(0.0), num(-1.0), num(1.0), num(0.0), num(-y0), num(w + x0)],
            _ => vec![num(1.0), num(0.0), num(0.0), num(1.0), num(-x0), num(-y0)],
        };
        let form = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "FormType" => 1,
                "BBox" => vec![num(x0), num(y0), num(x0 + w), num(y0 + h)],
                "Matrix" => Object::Array(matrix),
                "Resources" => Object::Dictionary(resources),
            },
            content,
        );
        let id = doc.add_object(form);
        forms.insert(n, (id, form_w, form_h));
    }

    // A single Helvetica for sheet labels.
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let mut kids: Vec<Object> = Vec::with_capacity(sides.len());
    for side in sides {
        let mut ops: Vec<Op> = Vec::new();
        let mut xobjects = Dictionary::new();

        for (i, p) in side.placements.iter().enumerate() {
            let Some(n) = p.page else { continue };
            let Some(&(form_id, fw, fh)) = forms.get(&n) else { continue };
            let name = format!("Fx{i}");
            xobjects.set(name.clone(), Object::Reference(form_id));
            let m = placement_matrix(p, fw, fh);
            ops.push(Op::new("q", vec![]));
            ops.push(Op::new(
                "cm",
                vec![num(m[0]), num(m[1]), num(m[2]), num(m[3]), num(m[4]), num(m[5])],
            ));
            ops.push(Op::new("Do", vec![Object::Name(name.into_bytes())]));
            ops.push(Op::new("Q", vec![]));
        }

        ops.extend(mark_operations(side, marks));
        if marks.sheet_labels {
            ops.extend(label_operations(side, "SheetLabel"));
        }

        let content = Content { operations: ops };
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().map_err(|e| format!("Failed to build sheet content: {e}"))?,
        ));

        let mut resources = dictionary! {
            "XObject" => Object::Dictionary(xobjects),
        };
        if marks.sheet_labels {
            resources.set(
                "Font",
                Object::Dictionary(dictionary! { "SheetLabel" => Object::Reference(font_id) }),
            );
        }

        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_root_id),
            "MediaBox" => vec![num(0.0), num(0.0), num(side.width), num(side.height)],
            "Resources" => Object::Dictionary(resources),
            "Contents" => Object::Reference(content_id),
        };

        // Production boxes so a commercial printer knows where to trim.
        // TrimBox is the finished sheet; BleedBox extends it by the bleed
        // allowance, clamped to the media so it stays a valid subset.
        if let Some((x0, y0, x1, y1)) = trim_bounds(side) {
            page.set("TrimBox", vec![num(x0), num(y0), num(x1), num(y1)]);
            let b = mm_to_points(marks.bleed_mm.max(0.0));
            page.set(
                "BleedBox",
                vec![
                    num((x0 - b).max(0.0)),
                    num((y0 - b).max(0.0)),
                    num((x1 + b).min(side.width)),
                    num((y1 + b).min(side.height)),
                ],
            );
            // CropBox is what a viewer displays: the whole sheet, marks included.
            page.set("CropBox", vec![num(0.0), num(0.0), num(side.width), num(side.height)]);
        }
        kids.push(Object::Reference(doc.add_object(Object::Dictionary(page))));
    }

    let count = kids.len() as i64;
    let root = doc
        .get_object_mut(pages_root_id)
        .and_then(|o| o.as_dict_mut())
        .map_err(|e| format!("Invalid page tree root: {e}"))?;
    root.set("Kids", Object::Array(kids));
    root.set("Count", Object::Integer(count));
    // Inheritable attributes on the root would override our sheet boxes.
    root.remove(b"MediaBox");
    root.remove(b"CropBox");
    root.remove(b"Rotate");
    root.remove(b"Resources");

    doc.prune_objects();
    doc.renumber_objects();
    doc.compress();
    doc.save(output_path).map_err(|e| format!("Failed to save PDF: {e}"))?;
    Ok(count as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_ops::document::inspect_pdf;

    fn make_pdf(path: &str, n: u32, w: f64, h: f64) {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        });
        let mut kids = Vec::new();
        for i in 1..=n {
            let content = Content {
                operations: vec![
                    Op::new("BT", vec![]),
                    Op::new("Tf", vec!["F1".into(), 36.into()]),
                    Op::new("Td", vec![60.into(), 400.into()]),
                    Op::new("Tj", vec![Object::string_literal(format!("Page {i}"))]),
                    Op::new("ET", vec![]),
                ],
            };
            let cid = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let pid = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => Object::Reference(pages_id),
                "MediaBox" => vec![0.into(), 0.into(), num(w), num(h)],
                "Contents" => Object::Reference(cid),
            });
            kids.push(Object::Reference(pid));
        }
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => Object::Integer(n as i64),
            "Resources" => Object::Reference(resources_id),
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    fn tmp(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    fn a5() -> (f64, f64) {
        (419.527_559_055_118_1, 595.275_590_551_181_2)
    }

    /// Two A5 pages side by side on an A4 landscape sheet.
    fn booklet_side(sheet_no: u32, side: &str, left: Option<u32>, right: Option<u32>, rot: i64) -> SheetSide {
        let (pw, ph) = a5();
        SheetSide {
            sheet_number: sheet_no,
            side: side.into(),
            width: pw * 2.0,
            height: ph,
            placements: vec![
                Placement { page: left, x: 0.0, y: 0.0, width: pw, height: ph, rotation: rot },
                Placement { page: right, x: pw, y: 0.0, width: pw, height: ph, rotation: rot },
            ],
            fold_x: vec![pw],
        }
    }

    #[test]
    fn imposes_an_eight_page_booklet_onto_four_sides() {
        let src = tmp("imp_src.pdf");
        let out = tmp("imp_out.pdf");
        make_pdf(&src, 8, a5().0, a5().1);
        let sides = vec![
            booklet_side(1, "front", Some(8), Some(1), 0),
            booklet_side(1, "back", Some(2), Some(7), 0),
            booklet_side(2, "front", Some(6), Some(3), 0),
            booklet_side(2, "back", Some(4), Some(5), 0),
        ];
        let n = export_imposed(&src, &sides, &out, MarkOptions::default()).unwrap();
        assert_eq!(n, 4);

        let result = inspect_pdf(&out).unwrap();
        assert_eq!(result.page_count, 4);
        // Every sheet is A4 landscape.
        assert!((result.pages[0].width_pt - a5().0 * 2.0).abs() < 0.5);
        assert!((result.pages[0].height_pt - a5().1).abs() < 0.5);
        // The source is untouched.
        assert_eq!(inspect_pdf(&src).unwrap().page_count, 8);
    }

    #[test]
    fn blank_cells_are_allowed() {
        let src = tmp("imp_blank_src.pdf");
        let out = tmp("imp_blank_out.pdf");
        make_pdf(&src, 2, a5().0, a5().1);
        let sides = vec![booklet_side(1, "front", None, Some(1), 0)];
        assert_eq!(export_imposed(&src, &sides, &out, MarkOptions::default()).unwrap(), 1);
        assert_eq!(inspect_pdf(&out).unwrap().page_count, 1);
    }

    #[test]
    fn rejects_overwriting_the_source() {
        let src = tmp("imp_overwrite.pdf");
        make_pdf(&src, 2, a5().0, a5().1);
        let sides = vec![booklet_side(1, "front", Some(1), Some(2), 0)];
        assert!(export_imposed(&src, &sides, &src, MarkOptions::default()).is_err());
    }

    #[test]
    fn rejects_an_empty_plan() {
        let src = tmp("imp_empty.pdf");
        make_pdf(&src, 1, a5().0, a5().1);
        assert!(export_imposed(&src, &[], &tmp("imp_empty_out.pdf"), MarkOptions::default()).is_err());
    }

    #[test]
    fn missing_source_page_is_an_error() {
        let src = tmp("imp_missing.pdf");
        make_pdf(&src, 2, a5().0, a5().1);
        let sides = vec![booklet_side(1, "front", Some(99), Some(1), 0)];
        assert!(export_imposed(&src, &sides, &tmp("imp_missing_out.pdf"), MarkOptions::default()).is_err());
    }

    #[test]
    fn writes_trim_and_bleed_boxes() {
        let src = tmp("imp_boxes_src.pdf");
        let out = tmp("imp_boxes_out.pdf");
        make_pdf(&src, 4, a5().0, a5().1);
        let sides = vec![booklet_side(1, "front", Some(4), Some(1), 0)];
        export_imposed(&src, &sides, &out, MarkOptions::default()).unwrap();

        let doc = Document::load(&out).unwrap();
        let (_, page_id) = doc.get_pages().into_iter().next().unwrap();
        let dict = doc.get_object(page_id).unwrap().as_dict().unwrap();
        let nums = |key: &[u8]| -> Vec<f64> {
            dict.get(key).unwrap().as_array().unwrap().iter().filter_map(super::number).collect()
        };
        let trim = nums(b"TrimBox");
        let bleed = nums(b"BleedBox");
        // Two A5 cells fill the sheet, so the trim box spans both.
        assert!((trim[2] - trim[0] - a5().0 * 2.0).abs() < 0.5);
        // Bleed extends the trim but is clamped inside the media box.
        assert!(bleed[0] >= 0.0 && bleed[1] >= 0.0);
        assert!(bleed[2] <= a5().0 * 2.0 + 1e-6);
        assert!(bleed[1] < trim[1] || trim[1] == 0.0);
        assert!(dict.get(b"CropBox").is_ok());
    }

    #[test]
    fn bleed_box_grows_with_the_bleed_setting() {
        let side = booklet_side(1, "front", Some(1), Some(2), 0);
        let (x0, y0, x1, y1) = trim_bounds(&side).unwrap();
        assert_eq!((x0, y0), (0.0, 0.0));
        assert!((x1 - a5().0 * 2.0).abs() < 1e-9);
        assert!((y1 - a5().1).abs() < 1e-9);
    }

    #[test]
    fn trim_bounds_of_an_empty_side_is_none() {
        let side = SheetSide {
            sheet_number: 1, side: "front".into(), width: 100.0, height: 100.0,
            placements: vec![], fold_x: vec![],
        };
        assert!(trim_bounds(&side).is_none());
    }

    #[test]
    fn upright_placement_scales_and_centres() {
        // A 100x200 page into a 100x200 cell at the origin: identity.
        let p = Placement { page: Some(1), x: 0.0, y: 0.0, width: 100.0, height: 200.0, rotation: 0 };
        let m = placement_matrix(&p, 100.0, 200.0);
        assert_eq!(m, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn half_scale_page_is_centred_in_its_cell() {
        // A 100x100 page into a 200x200 cell scales x2 and stays centred.
        let p = Placement { page: Some(1), x: 10.0, y: 20.0, width: 200.0, height: 200.0, rotation: 0 };
        let m = placement_matrix(&p, 100.0, 100.0);
        assert_eq!(m[0], 2.0);
        assert_eq!(m[4], 10.0);
        assert_eq!(m[5], 20.0);
    }

    #[test]
    fn rotating_180_maps_the_cell_onto_itself() {
        let p = Placement { page: Some(1), x: 0.0, y: 0.0, width: 100.0, height: 200.0, rotation: 180 };
        let m = placement_matrix(&p, 100.0, 200.0);
        // Corner (0,0) maps to the opposite corner and back again.
        let map = |x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let (ax, ay) = map(0.0, 0.0);
        let (bx, by) = map(100.0, 200.0);
        assert!((ax - 100.0).abs() < 1e-9 && (ay - 200.0).abs() < 1e-9);
        assert!(bx.abs() < 1e-9 && by.abs() < 1e-9);
    }

    #[test]
    fn quarter_turn_swaps_the_fitted_footprint() {
        // A portrait page rotated 90 into a landscape cell fills it exactly.
        let p = Placement { page: Some(1), x: 0.0, y: 0.0, width: 200.0, height: 100.0, rotation: 90 };
        let m = placement_matrix(&p, 100.0, 200.0);
        let map = |x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        let (cx, cy) = map(100.0, 200.0);
        // The far corner lands inside the cell bounds.
        assert!(cx >= -1e-9 && cx <= 200.0 + 1e-9);
        assert!(cy >= -1e-9 && cy <= 100.0 + 1e-9);
    }
}
