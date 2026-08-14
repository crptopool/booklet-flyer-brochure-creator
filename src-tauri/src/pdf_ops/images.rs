//! Finding the images inside a PDF and checking their effective DPI.
//!
//! Walks each page's resources for image XObjects, works out how large
//! each one is drawn from the content stream's transformation matrices,
//! and compares its pixel count against that printed size.

use std::collections::BTreeMap;

use lopdf::content::Content;
use lopdf::{Document, Object, ObjectId};

use crate::print_calc::dpi::effective_dpi;
use crate::print_calc::units::points_to_inches;

/// One image as it is actually placed on a page.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlacedImage {
    /// 1-based page number.
    pub page: u32,
    /// Resource name, e.g. "Im0".
    pub name: String,
    pub pixel_width: u32,
    pub pixel_height: u32,
    /// Printed size in points, from the placement matrix.
    pub printed_width_pt: f64,
    pub printed_height_pt: f64,
    pub effective_dpi: f64,
}

impl PlacedImage {
    pub fn printed_width_mm(&self) -> f64 {
        points_to_inches(self.printed_width_pt) * 25.4
    }
}

fn as_u32(obj: Option<&Object>, doc: &Document) -> Option<u32> {
    let obj = obj?;
    let resolved = match obj {
        Object::Reference(r) => doc.get_object(*r).ok()?,
        other => other,
    };
    resolved.as_i64().ok().and_then(|v| u32::try_from(v).ok())
}

/// Image XObjects declared in a page's resources, by resource name.
fn page_images(doc: &Document, page_id: ObjectId) -> BTreeMap<String, (u32, u32)> {
    let mut out = BTreeMap::new();
    let (inline, referenced) = match doc.get_page_resources(page_id) {
        Ok(r) => r,
        Err(_) => return out,
    };
    let mut dicts = Vec::new();
    for id in referenced {
        if let Ok(d) = doc.get_object(id).and_then(|o| o.as_dict()) {
            dicts.push(d.clone());
        }
    }
    if let Some(d) = inline {
        dicts.push(d.clone());
    }

    for res in dicts {
        let Ok(xobjects) = res.get(b"XObject") else { continue };
        let xdict = match xobjects {
            Object::Reference(r) => doc.get_object(*r).ok().and_then(|o| o.as_dict().ok()).cloned(),
            Object::Dictionary(d) => Some(d.clone()),
            _ => None,
        };
        let Some(xdict) = xdict else { continue };
        for (name, value) in xdict.iter() {
            let Ok(id) = value.as_reference() else { continue };
            let Ok(stream) = doc.get_object(id).and_then(|o| o.as_stream()) else { continue };
            let is_image = stream
                .dict
                .get(b"Subtype")
                .and_then(|v| v.as_name())
                .map(|n| n == b"Image")
                .unwrap_or(false);
            if !is_image {
                continue;
            }
            let w = as_u32(stream.dict.get(b"Width").ok(), doc);
            let h = as_u32(stream.dict.get(b"Height").ok(), doc);
            if let (Some(w), Some(h)) = (w, h) {
                out.insert(String::from_utf8_lossy(name).to_string(), (w, h));
            }
        }
    }
    out
}

/// Multiply two PDF matrices, `a` then `b`.
fn mul(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn operand_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// Every image drawn on a page, with the size it is drawn at.
///
/// An image XObject is painted into the unit square, so the current
/// transformation matrix at the `Do` operator gives its printed size
/// directly: the column lengths are its width and height in points.
pub fn images_on_page(doc: &Document, page_id: ObjectId, page: u32) -> Vec<PlacedImage> {
    let declared = page_images(doc, page_id);
    if declared.is_empty() {
        return vec![];
    }
    let data = doc.get_page_content(page_id);
    let Ok(content) = Content::decode(&data) else { return vec![] };

    let mut out = Vec::new();
    let mut ctm: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut stack: Vec<[f64; 6]> = Vec::new();

    for op in content.operations {
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            "cm" => {
                let v: Vec<f64> = op.operands.iter().filter_map(operand_f64).collect();
                if v.len() == 6 {
                    ctm = mul([v[0], v[1], v[2], v[3], v[4], v[5]], ctm);
                }
            }
            "Do" => {
                let Some(Object::Name(name)) = op.operands.first() else { continue };
                let name = String::from_utf8_lossy(name).to_string();
                let Some(&(pw, ph)) = declared.get(&name) else { continue };
                // Column lengths of the CTM give the drawn width and height,
                // which stays correct under rotation as well as scaling.
                let w = (ctm[0] * ctm[0] + ctm[1] * ctm[1]).sqrt();
                let h = (ctm[2] * ctm[2] + ctm[3] * ctm[3]).sqrt();
                if w <= 0.0 || h <= 0.0 {
                    continue;
                }
                if let Ok(dpi) = effective_dpi(pw, ph, w, h) {
                    out.push(PlacedImage {
                        page,
                        name,
                        pixel_width: pw,
                        pixel_height: ph,
                        printed_width_pt: w,
                        printed_height_pt: h,
                        effective_dpi: dpi,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// Every image in the document, with its effective DPI.
pub fn scan_images(path: &str) -> Result<Vec<PlacedImage>, String> {
    let doc = Document::load(path).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let mut out = Vec::new();
    for (number, page_id) in doc.get_pages() {
        out.extend(images_on_page(&doc, page_id, number));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::Operation as Op;
    use lopdf::{dictionary, Stream};

    fn tmp(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    /// A one-page PDF with an image drawn at a chosen size in points.
    fn make_pdf_with_image(path: &str, px: u32, py: u32, draw_w: f64, draw_h: f64) {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let img = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => Object::Integer(px as i64),
                "Height" => Object::Integer(py as i64),
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            vec![0u8; (px * py * 3) as usize],
        ));
        let content = Content {
            operations: vec![
                Op::new("q", vec![]),
                Op::new("cm", vec![draw_w.into(), 0.into(), 0.into(), draw_h.into(), 40.into(), 40.into()]),
                Op::new("Do", vec![Object::Name(b"Im0".to_vec())]),
                Op::new("Q", vec![]),
            ],
        };
        let cid = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => dictionary! {
                "XObject" => dictionary! { "Im0" => Object::Reference(img) },
            },
            "Contents" => Object::Reference(cid),
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    #[test]
    fn finds_an_image_and_computes_its_effective_dpi() {
        // 300 px drawn across 72 pt (one inch) is exactly 300 DPI.
        let p = tmp("dpi_300.pdf");
        make_pdf_with_image(&p, 300, 300, 72.0, 72.0);
        let found = scan_images(&p).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].page, 1);
        assert_eq!(found[0].pixel_width, 300);
        assert!((found[0].effective_dpi - 300.0).abs() < 0.01);
    }

    #[test]
    fn enlarging_an_image_lowers_its_effective_dpi() {
        // The same 300 px across two inches halves the resolution.
        let p = tmp("dpi_150.pdf");
        make_pdf_with_image(&p, 300, 300, 144.0, 144.0);
        let found = scan_images(&p).unwrap();
        assert!((found[0].effective_dpi - 150.0).abs() < 0.01);
    }

    #[test]
    fn the_lower_axis_decides_the_reported_dpi() {
        // Stretched wide: 300 px over 2 inches horizontally, 1 vertically.
        let p = tmp("dpi_axis.pdf");
        make_pdf_with_image(&p, 300, 300, 144.0, 72.0);
        let found = scan_images(&p).unwrap();
        assert!((found[0].effective_dpi - 150.0).abs() < 0.01);
    }

    #[test]
    fn printed_size_is_reported_in_millimetres() {
        let p = tmp("dpi_mm.pdf");
        make_pdf_with_image(&p, 300, 300, 72.0, 72.0);
        let found = scan_images(&p).unwrap();
        assert!((found[0].printed_width_mm() - 25.4).abs() < 0.01);
    }

    #[test]
    fn a_document_with_no_images_returns_nothing() {
        let p = tmp("dpi_none.pdf");
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
        }));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
        doc.trailer.set("Root", catalog);
        doc.save(&p).unwrap();
        assert!(scan_images(&p).unwrap().is_empty());
    }

    #[test]
    fn matrix_multiplication_composes_nested_transforms() {
        // Scale by 2, then by 3, gives 6.
        let m = mul([2.0, 0.0, 0.0, 2.0, 0.0, 0.0], [3.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
        assert_eq!(m[0], 6.0);
        assert_eq!(m[3], 6.0);
    }

    #[test]
    fn a_rotated_placement_still_measures_its_drawn_size() {
        // A 90-degree rotation puts the scale in the off-diagonal terms.
        let ctm: [f64; 6] = [0.0, 100.0, -50.0, 0.0, 0.0, 0.0];
        let w = (ctm[0] * ctm[0] + ctm[1] * ctm[1]).sqrt();
        let h = (ctm[2] * ctm[2] + ctm[3] * ctm[3]).sqrt();
        assert!((w - 100.0).abs() < 1e-9);
        assert!((h - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_missing_file_is_an_error() {
        assert!(scan_images("/nonexistent/none.pdf").is_err());
    }
}

/// What colour spaces a document actually uses.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ColorUsage {
    pub device_rgb: bool,
    pub device_cmyk: bool,
    pub device_gray: bool,
    pub icc_based: bool,
    pub separation: bool,
    /// Names of any separation (spot) colourants found.
    pub spot_names: Vec<String>,
}

impl ColorUsage {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.device_cmyk {
            parts.push("CMYK");
        }
        if self.device_rgb {
            parts.push("RGB");
        }
        if self.device_gray {
            parts.push("Grayscale");
        }
        if self.icc_based {
            parts.push("ICC-based");
        }
        if self.separation {
            parts.push("Spot");
        }
        if parts.is_empty() {
            "No colour spaces declared".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Scan every object for the colour spaces the document declares.
///
/// This reports what is *there*; it never converts anything. The
/// specification is explicit that colour space is preserved unless the
/// user asks otherwise.
pub fn scan_colors(path: &str) -> Result<ColorUsage, String> {
    let doc = Document::load(path).map_err(|e| format!("Failed to load PDF: {e}"))?;
    let mut usage = ColorUsage::default();

    fn note(usage: &mut ColorUsage, name: &[u8]) {
        match name {
            b"DeviceRGB" | b"CalRGB" => usage.device_rgb = true,
            b"DeviceCMYK" => usage.device_cmyk = true,
            b"DeviceGray" | b"CalGray" => usage.device_gray = true,
            b"ICCBased" => usage.icc_based = true,
            b"Separation" | b"DeviceN" => usage.separation = true,
            _ => {}
        }
    }

    fn walk(usage: &mut ColorUsage, obj: &Object, depth: u32) {
        if depth > 12 {
            return;
        }
        match obj {
            Object::Name(n) => note(usage, n),
            Object::Array(items) => {
                // A Separation array names its colourant second.
                if let Some(Object::Name(head)) = items.first() {
                    note(usage, head);
                    if head == b"Separation" {
                        if let Some(Object::Name(spot)) = items.get(1) {
                            let name = String::from_utf8_lossy(spot).to_string();
                            if !usage.spot_names.contains(&name) {
                                usage.spot_names.push(name);
                            }
                        }
                    }
                }
                for item in items {
                    walk(usage, item, depth + 1);
                }
            }
            Object::Dictionary(d) => {
                for (key, value) in d.iter() {
                    if key == b"ColorSpace" {
                        walk(usage, value, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }

    for obj in doc.objects.values() {
        match obj {
            Object::Stream(s) => {
                if let Ok(cs) = s.dict.get(b"ColorSpace") {
                    walk(&mut usage, cs, 0);
                }
                walk(&mut usage, &Object::Dictionary(s.dict.clone()), 0);
            }
            other => walk(&mut usage, other, 0),
        }
    }
    Ok(usage)
}

#[cfg(test)]
mod color_tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    fn tmp(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    fn doc_with_colorspace(path: &str, cs: Object) {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let img = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image",
                "Width" => 10, "Height" => 10, "BitsPerComponent" => 8,
                "ColorSpace" => cs,
            },
            vec![0u8; 300],
        ));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => Object::Reference(img) } },
        });
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
        }));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }

    #[test]
    fn detects_rgb_artwork() {
        let p = tmp("color_rgb.pdf");
        doc_with_colorspace(&p, Object::Name(b"DeviceRGB".to_vec()));
        let u = scan_colors(&p).unwrap();
        assert!(u.device_rgb);
        assert!(!u.device_cmyk);
        assert!(u.summary().contains("RGB"));
    }

    #[test]
    fn detects_cmyk_artwork() {
        let p = tmp("color_cmyk.pdf");
        doc_with_colorspace(&p, Object::Name(b"DeviceCMYK".to_vec()));
        let u = scan_colors(&p).unwrap();
        assert!(u.device_cmyk);
        assert!(u.summary().contains("CMYK"));
    }

    #[test]
    fn detects_a_named_spot_colour() {
        let p = tmp("color_spot.pdf");
        doc_with_colorspace(&p, Object::Array(vec![
            Object::Name(b"Separation".to_vec()),
            Object::Name(b"PANTONE 185 C".to_vec()),
            Object::Name(b"DeviceCMYK".to_vec()),
        ]));
        let u = scan_colors(&p).unwrap();
        assert!(u.separation);
        assert!(u.spot_names.iter().any(|n| n.contains("PANTONE")));
    }

    #[test]
    fn a_document_with_no_colour_declarations_says_so() {
        let p = tmp("color_none.pdf");
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => Object::Reference(pages_id),
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1,
        }));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) });
        doc.trailer.set("Root", catalog);
        doc.save(&p).unwrap();
        assert_eq!(scan_colors(&p).unwrap().summary(), "No colour spaces declared");
    }

    #[test]
    fn a_missing_file_is_an_error() {
        assert!(scan_colors("/nonexistent/none.pdf").is_err());
    }
}
