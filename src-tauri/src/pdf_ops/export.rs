//! Export: materialise a virtual page list into a new PDF.
//!
//! The source file is opened read-only; output is always written to a
//! new file. Vector content, text and fonts are preserved because pages
//! are copied by object reference, never rasterised.

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId};

use crate::pdf_ops::operations::VirtualPage;

const DEFAULT_PAGE_W: f64 = 595.2755905511812; // A4 width in points
const DEFAULT_PAGE_H: f64 = 841.8897637795276; // A4 height in points

/// Fetch a page attribute without resolving references, walking up the
/// page-tree `Parent` chain for inheritable attributes.
fn inherited_raw(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = page_id;
    for _ in 0..64 {
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(value) = dict.get(key) {
            return Some(value.clone());
        }
        match dict.get(b"Parent").and_then(|p| p.as_reference()) {
            Ok(parent) => current = parent,
            Err(_) => return None,
        }
    }
    None
}

fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> Option<Object> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok().cloned(),
        other => Some(other.clone()),
    }
}

fn media_box_size(doc: &Document, page_id: ObjectId) -> Option<(f64, f64)> {
    let raw = inherited_raw(doc, page_id, b"MediaBox")?;
    let arr = resolve(doc, &raw)?.as_array().ok()?.clone();
    let nums: Vec<f64> = arr
        .iter()
        .filter_map(|o| match o {
            Object::Integer(i) => Some(*i as f64),
            Object::Real(r) => Some(*r as f64),
            _ => None,
        })
        .collect();
    if nums.len() == 4 {
        Some(((nums[2] - nums[0]).abs(), (nums[3] - nums[1]).abs()))
    } else {
        None
    }
}

/// Write the virtual page list as a new PDF at `output_path`.
///
/// Blank pages default to the size of the nearest preceding real page
/// (falling back to the next real page, then A4).
pub fn export_pdf(source_path: &str, pages: &[VirtualPage], output_path: &str) -> Result<u32, String> {
    if pages.is_empty() {
        return Err("cannot export an empty document".into());
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

    // Sizes of real pages, used for blank-page fallback.
    let mut sizes: Vec<Option<(f64, f64)>> = Vec::with_capacity(pages.len());
    for vp in pages {
        sizes.push(match vp.source_page {
            Some(n) => page_map.get(&n).and_then(|&id| media_box_size(&doc, id)),
            None => vp.width_pt.zip(vp.height_pt),
        });
    }
    let blank_size = |idx: usize| -> (f64, f64) {
        sizes[..idx]
            .iter()
            .rev()
            .chain(sizes[idx + 1..].iter())
            .find_map(|s| *s)
            .unwrap_or((DEFAULT_PAGE_W, DEFAULT_PAGE_H))
    };

    let mut kids: Vec<Object> = Vec::with_capacity(pages.len());
    for (idx, vp) in pages.iter().enumerate() {
        let new_id = match vp.source_page {
            Some(n) => {
                let &page_id = page_map
                    .get(&n)
                    .ok_or_else(|| format!("source page {n} not found"))?;
                let mut dict: Dictionary = doc
                    .get_object(page_id)
                    .and_then(|o| o.as_dict())
                    .map_err(|e| format!("Invalid page object: {e}"))?
                    .clone();
                // Materialise inherited attributes so the copy is
                // self-contained under the new parent.
                for key in [b"MediaBox".as_slice(), b"Resources".as_slice(), b"Rotate".as_slice(), b"CropBox".as_slice()] {
                    if !dict.has(key) {
                        if let Some(value) = inherited_raw(&doc, page_id, key) {
                            dict.set(key, value);
                        }
                    }
                }
                if vp.rotation != 0 {
                    let base = dict
                        .get(b"Rotate")
                        .ok()
                        .and_then(|o| resolve(&doc, o))
                        .and_then(|o| o.as_i64().ok())
                        .unwrap_or(0);
                    dict.set("Rotate", Object::Integer((base + vp.rotation).rem_euclid(360)));
                }
                dict.set("Parent", Object::Reference(pages_root_id));
                doc.add_object(Object::Dictionary(dict))
            }
            None => {
                let (w, h) = vp.width_pt.zip(vp.height_pt).unwrap_or_else(|| blank_size(idx));
                let dict = dictionary! {
                    "Type" => "Page",
                    "Parent" => Object::Reference(pages_root_id),
                    "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
                    "Resources" => dictionary! {},
                };
                doc.add_object(Object::Dictionary(dict))
            }
        };
        kids.push(Object::Reference(new_id));
    }

    let count = kids.len() as i64;
    let pages_root = doc
        .get_object_mut(pages_root_id)
        .and_then(|o| o.as_dict_mut())
        .map_err(|e| format!("Invalid page tree root: {e}"))?;
    pages_root.set("Kids", Object::Array(kids));
    pages_root.set("Count", Object::Integer(count));

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
    use crate::pdf_ops::operations::{apply_operations, Operation};
    use lopdf::content::{Content, Operation as PdfOp};
    use lopdf::Stream;

    /// Build a simple n-page A4 PDF for testing.
    fn make_pdf(path: &str, n: u32) {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        });
        let mut kids = Vec::new();
        for i in 1..=n {
            let content = Content {
                operations: vec![
                    PdfOp::new("BT", vec![]),
                    PdfOp::new("Tf", vec!["F1".into(), 24.into()]),
                    PdfOp::new("Td", vec![100.into(), 700.into()]),
                    PdfOp::new("Tj", vec![Object::string_literal(format!("Page {i}"))]),
                    PdfOp::new("ET", vec![]),
                ],
            };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => Object::Reference(pages_id),
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Contents" => Object::Reference(content_id),
            });
            kids.push(Object::Reference(page_id));
        }
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => Object::Integer(n as i64),
            "Resources" => Object::Reference(resources_id),
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", catalog_id);
        doc.save(path).unwrap();
    }

    fn temp_path(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn inspect_reports_pages_and_sizes() {
        let src = temp_path("printprep_inspect.pdf");
        make_pdf(&src, 3);
        let info = inspect_pdf(&src).unwrap();
        assert_eq!(info.page_count, 3);
        assert!(!info.encrypted);
        assert!(!info.has_mixed_page_sizes());
        assert_eq!(info.pages[0].width_pt, 595.0);
        assert_eq!(info.pages[0].orientation(), "portrait");
    }

    #[test]
    fn export_reordered_and_padded() {
        let src = temp_path("printprep_export_src.pdf");
        let out = temp_path("printprep_export_out.pdf");
        make_pdf(&src, 3);
        let source = inspect_pdf(&src).unwrap();
        let ops = [
            Operation::ReorderPages { order: vec![3, 2, 1] },
            Operation::InsertBlank { position: 4, width_pt: None, height_pt: None },
            Operation::RotatePage { position: 1, degrees: 90 },
        ];
        let pages = apply_operations(&source, &ops).unwrap();
        let count = export_pdf(&src, &pages, &out).unwrap();
        assert_eq!(count, 4);
        let result = inspect_pdf(&out).unwrap();
        assert_eq!(result.page_count, 4);
        assert_eq!(result.pages[0].rotation, 90);
        // Blank inherits neighbour size.
        assert_eq!(result.pages[3].width_pt, 595.0);
        // Source is untouched.
        assert_eq!(inspect_pdf(&src).unwrap().page_count, 3);
    }

    #[test]
    fn export_duplicate_pages() {
        let src = temp_path("printprep_dup_src.pdf");
        let out = temp_path("printprep_dup_out.pdf");
        make_pdf(&src, 2);
        let source = inspect_pdf(&src).unwrap();
        let pages = apply_operations(&source, &[Operation::DuplicatePage { position: 1 }]).unwrap();
        assert_eq!(export_pdf(&src, &pages, &out).unwrap(), 3);
        assert_eq!(inspect_pdf(&out).unwrap().page_count, 3);
    }

    #[test]
    fn export_refuses_overwriting_source() {
        let src = temp_path("printprep_overwrite.pdf");
        make_pdf(&src, 1);
        let source = inspect_pdf(&src).unwrap();
        let pages = apply_operations(&source, &[]).unwrap();
        assert!(export_pdf(&src, &pages, &src).is_err());
    }
}
