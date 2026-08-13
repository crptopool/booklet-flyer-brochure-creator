//! PDF inspection: page count, dimensions, orientation, metadata,
//! encryption. The source file is opened read-only and never modified.

use std::path::Path;

use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageInfo {
    /// 1-based page number.
    pub number: u32,
    pub width_pt: f64,
    pub height_pt: f64,
    pub rotation: i64,
}

impl PageInfo {
    pub fn orientation(&self) -> &'static str {
        let (mut w, mut h) = (self.width_pt, self.height_pt);
        if self.rotation.rem_euclid(180) == 90 {
            std::mem::swap(&mut w, &mut h);
        }
        if w > h {
            "landscape"
        } else if w < h {
            "portrait"
        } else {
            "square"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfSource {
    pub path: String,
    pub page_count: u32,
    pub pages: Vec<PageInfo>,
    pub encrypted: bool,
    /// True when the file is protected and cannot be modified.
    pub modification_restricted: bool,
    pub metadata: Vec<(String, String)>,
}

impl PdfSource {
    pub fn has_mixed_page_sizes(&self) -> bool {
        let sizes: std::collections::HashSet<(i64, i64)> = self
            .pages
            .iter()
            .map(|p| ((p.width_pt * 100.0).round() as i64, (p.height_pt * 100.0).round() as i64))
            .collect();
        sizes.len() > 1
    }
}

/// Resolve a page attribute, walking up the page-tree `Parent` chain for
/// inheritable attributes such as `MediaBox` and `Rotate`.
fn inherited_attr(doc: &Document, page_id: (u32, u16), key: &[u8]) -> Option<Object> {
    let mut current = page_id;
    for _ in 0..64 {
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(value) = dict.get(key) {
            let value = if let Ok(r) = value.as_reference() {
                doc.get_object(r).ok()?.clone()
            } else {
                value.clone()
            };
            return Some(value);
        }
        match dict.get(b"Parent").and_then(|p| p.as_reference()) {
            Ok(parent) => current = parent,
            Err(_) => return None,
        }
    }
    None
}

fn number(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// Inspect a PDF without altering it. Encrypted files that cannot be
/// opened are flagged `modification_restricted` so the application can
/// inform the user.
pub fn inspect_pdf(path: &str) -> Result<PdfSource, String> {
    if !Path::new(path).is_file() {
        return Err(format!("File not found: {path}"));
    }
    let doc = match Document::load(path) {
        Ok(doc) => doc,
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("encrypt") || msg.contains("password") {
                return Ok(PdfSource {
                    path: path.to_string(),
                    page_count: 0,
                    pages: vec![],
                    encrypted: true,
                    modification_restricted: true,
                    metadata: vec![],
                });
            }
            return Err(format!("Failed to load PDF: {e}"));
        }
    };
    let encrypted = doc.is_encrypted();

    let mut pages = Vec::new();
    for (num, page_id) in doc.get_pages() {
        let media_box = inherited_attr(&doc, page_id, b"MediaBox")
            .and_then(|obj| obj.as_array().ok().cloned())
            .unwrap_or_default();
        let coords: Vec<f64> = media_box.iter().filter_map(number).collect();
        let (width, height) = if coords.len() == 4 {
            ((coords[2] - coords[0]).abs(), (coords[3] - coords[1]).abs())
        } else {
            (0.0, 0.0)
        };
        let rotation = inherited_attr(&doc, page_id, b"Rotate")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0)
            .rem_euclid(360);
        pages.push(PageInfo {
            number: num,
            width_pt: width,
            height_pt: height,
            rotation,
        });
    }

    let mut metadata = Vec::new();
    if let Ok(Object::Reference(info_id)) = doc.trailer.get(b"Info") {
        if let Ok(info) = doc.get_object(*info_id).and_then(|o| o.as_dict()) {
            for (k, v) in info.iter() {
                if let Ok(bytes) = v.as_str() {
                    metadata.push((
                        String::from_utf8_lossy(k).to_string(),
                        String::from_utf8_lossy(bytes).to_string(),
                    ));
                }
            }
        }
    }

    Ok(PdfSource {
        path: path.to_string(),
        page_count: pages.len() as u32,
        pages,
        encrypted,
        modification_restricted: false,
        metadata,
    })
}
