//! Placing artwork into a PDF.
//!
//! Raster artwork (PNG/JPEG) becomes an Image XObject; a PDF page
//! becomes a Form XObject so its vectors and text survive. Either can
//! then be drawn into a rectangle with a placement matrix.

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};

/// How artwork is fitted into its target rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    /// Scale until the rectangle is covered; overflow is clipped.
    Fill,
    /// Scale until the artwork fits entirely; may leave margins.
    Fit,
    /// Ignore the aspect ratio and stretch to the rectangle.
    Stretch,
}

/// An embedded artwork object ready to be drawn.
pub struct Artwork {
    pub id: ObjectId,
    /// Intrinsic size in the artwork's own units.
    pub width: f64,
    pub height: f64,
    /// True for a Form XObject (a placed PDF page).
    pub is_form: bool,
}

fn num(v: f64) -> Object {
    Object::Real(v as f32)
}

/// Embed a PNG or JPEG file as an Image XObject.
///
/// JPEG data is carried through untouched with `DCTDecode`, so no
/// recompression happens. PNG is decoded and re-compressed losslessly
/// with Flate, which is what PDF supports natively.
pub fn embed_image(doc: &mut Document, path: &str) -> Result<Artwork, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read {path}: {e}"))?;
    let lower = path.to_lowercase();
    let is_jpeg = lower.ends_with(".jpg") || lower.ends_with(".jpeg");

    if is_jpeg {
        // Read the dimensions without re-encoding the pixel data.
        let img = image::load_from_memory(&bytes).map_err(|e| format!("Not a readable JPEG: {e}"))?;
        let (w, h) = (img.width(), img.height());
        let stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => Object::Integer(w as i64),
                "Height" => Object::Integer(h as i64),
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            bytes,
        );
        // The stream is already compressed; keep lopdf from touching it.
        let id = doc.add_object(stream);
        return Ok(Artwork { id, width: w as f64, height: h as f64, is_form: false });
    }

    let img = image::load_from_memory(&bytes).map_err(|e| format!("Not a readable image: {e}"))?;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(rgb.as_raw())
        .map_err(|e| format!("Failed to compress the image: {e}"))?;
    let data = encoder.finish().map_err(|e| format!("Failed to compress the image: {e}"))?;

    let stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => Object::Integer(w as i64),
            "Height" => Object::Integer(h as i64),
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "Filter" => "FlateDecode",
        },
        data,
    );
    let id = doc.add_object(stream);
    Ok(Artwork { id, width: w as f64, height: h as f64, is_form: false })
}

/// Placement matrix mapping a `w x h` artwork into a rectangle.
///
/// Images are drawn in a unit square, so their matrix carries the full
/// target size; forms keep their own units and are scaled instead.
pub fn fit_matrix(
    art_w: f64,
    art_h: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    mode: FitMode,
    unit_square: bool,
) -> [f64; 6] {
    if art_w <= 0.0 || art_h <= 0.0 {
        return [w, 0.0, 0.0, h, x, y];
    }
    let (sx, sy) = match mode {
        FitMode::Stretch => (w / art_w, h / art_h),
        FitMode::Fill => {
            let s = (w / art_w).max(h / art_h);
            (s, s)
        }
        FitMode::Fit => {
            let s = (w / art_w).min(h / art_h);
            (s, s)
        }
    };
    let dw = art_w * sx;
    let dh = art_h * sy;
    let ox = x + (w - dw) / 2.0;
    let oy = y + (h - dh) / 2.0;
    if unit_square {
        // An image XObject occupies [0,1]x[0,1]; the matrix sizes it.
        [dw, 0.0, 0.0, dh, ox, oy]
    } else {
        [sx, 0.0, 0.0, sy, ox, oy]
    }
}

/// Content operators drawing `art` into a rectangle, clipped to it.
pub fn draw_ops(
    art: &Artwork,
    name: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    mode: FitMode,
) -> Vec<lopdf::content::Operation> {
    use lopdf::content::Operation as Op;
    let m = fit_matrix(art.width, art.height, x, y, w, h, mode, !art.is_form);
    vec![
        Op::new("q", vec![]),
        // Clip so Fill mode cannot spill outside its panel.
        Op::new("re", vec![num(x), num(y), num(w), num(h)]),
        Op::new("W", vec![]),
        Op::new("n", vec![]),
        Op::new("cm", vec![num(m[0]), num(m[1]), num(m[2]), num(m[3]), num(m[4]), num(m[5])]),
        Op::new("Do", vec![Object::Name(name.as_bytes().to_vec())]),
        Op::new("Q", vec![]),
    ]
}

/// Resources entry mapping `name` to the embedded artwork.
pub fn xobject_dict(name: &str, art: &Artwork) -> Dictionary {
    let mut d = Dictionary::new();
    d.set(name.to_string(), Object::Reference(art.id));
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(path: &str, w: u32, h: u32) {
        let mut img = image::RgbImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        img.save(path).unwrap();
    }

    fn tmp(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn embeds_a_png_with_its_real_dimensions() {
        let p = tmp("art_test.png");
        png(&p, 120, 80);
        let mut doc = Document::with_version("1.7");
        let art = embed_image(&mut doc, &p).unwrap();
        assert_eq!((art.width, art.height), (120.0, 80.0));
        assert!(!art.is_form);
        let stream = doc.get_object(art.id).unwrap().as_stream().unwrap();
        assert_eq!(stream.dict.get(b"Filter").unwrap().as_name().unwrap(), b"FlateDecode");
        assert_eq!(stream.dict.get(b"Width").unwrap().as_i64().unwrap(), 120);
    }

    #[test]
    fn embeds_a_jpeg_without_recompressing_it() {
        let p = tmp("art_test.jpg");
        png(&tmp("art_src.png"), 60, 40);
        image::open(tmp("art_src.png")).unwrap().to_rgb8().save(&p).unwrap();
        let mut doc = Document::with_version("1.7");
        let art = embed_image(&mut doc, &p).unwrap();
        let stream = doc.get_object(art.id).unwrap().as_stream().unwrap();
        assert_eq!(stream.dict.get(b"Filter").unwrap().as_name().unwrap(), b"DCTDecode");
        // The original JPEG bytes are carried through untouched.
        assert_eq!(stream.content, std::fs::read(&p).unwrap());
    }

    #[test]
    fn missing_or_invalid_files_error() {
        let mut doc = Document::with_version("1.7");
        assert!(embed_image(&mut doc, "/nonexistent/nope.png").is_err());
        let bad = tmp("art_bad.png");
        std::fs::write(&bad, b"not an image").unwrap();
        assert!(embed_image(&mut doc, &bad).is_err());
    }

    #[test]
    fn fill_covers_the_rectangle_and_centres_the_overflow() {
        // A square image into a wide box: scaled on width, cropped top/bottom.
        let m = fit_matrix(100.0, 100.0, 0.0, 0.0, 200.0, 100.0, FitMode::Fill, true);
        assert!((m[0] - 200.0).abs() < 1e-9);
        assert!((m[3] - 200.0).abs() < 1e-9);
        // Centred vertically means it starts below the box.
        assert!((m[5] + 50.0).abs() < 1e-9);
    }

    #[test]
    fn fit_keeps_the_whole_image_inside() {
        let m = fit_matrix(100.0, 100.0, 0.0, 0.0, 200.0, 100.0, FitMode::Fit, true);
        assert!((m[0] - 100.0).abs() < 1e-9);
        assert!((m[3] - 100.0).abs() < 1e-9);
        // Centred horizontally in the wider box.
        assert!((m[4] - 50.0).abs() < 1e-9);
        assert!(m[5].abs() < 1e-9);
    }

    #[test]
    fn stretch_ignores_the_aspect_ratio() {
        let m = fit_matrix(100.0, 50.0, 10.0, 20.0, 200.0, 100.0, FitMode::Stretch, true);
        assert!((m[0] - 200.0).abs() < 1e-9);
        assert!((m[3] - 100.0).abs() < 1e-9);
        assert!((m[4] - 10.0).abs() < 1e-9);
        assert!((m[5] - 20.0).abs() < 1e-9);
    }

    #[test]
    fn forms_are_scaled_rather_than_sized() {
        // A form keeps its own coordinate system, so the matrix is a scale.
        let m = fit_matrix(200.0, 100.0, 0.0, 0.0, 400.0, 200.0, FitMode::Fit, false);
        assert!((m[0] - 2.0).abs() < 1e-9);
        assert!((m[3] - 2.0).abs() < 1e-9);
    }

    #[test]
    fn zero_sized_artwork_falls_back_to_the_target_box() {
        let m = fit_matrix(0.0, 0.0, 5.0, 6.0, 100.0, 50.0, FitMode::Fill, true);
        assert_eq!(m, [100.0, 0.0, 0.0, 50.0, 5.0, 6.0]);
    }

    #[test]
    fn draw_operations_clip_to_the_target_rectangle() {
        let art = Artwork { id: (1, 0), width: 10.0, height: 10.0, is_form: false };
        let ops = draw_ops(&art, "Ax", 0.0, 0.0, 100.0, 100.0, FitMode::Fill);
        let names: Vec<&str> = ops.iter().map(|o| o.operator.as_str()).collect();
        assert_eq!(names, vec!["q", "re", "W", "n", "cm", "Do", "Q"]);
    }
}
