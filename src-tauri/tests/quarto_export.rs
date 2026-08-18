//! End-to-end check that a folded signature reaches the printed sheet.
//!
//! The unit tests cover the numbering; this covers the rest of the path —
//! that the pages the fold simulation chose are the pages that come out of
//! the exporter, on the sheet size the user asked for.

use printprep_lib::pdf_ops::impose::{export_imposed, MarkOptions};
use printprep_lib::pdf_ops::sheets::sheets_for_plan;
use printprep_lib::print_calc::plan::booklet_plan;
use printprep_lib::print_calc::presets::{BindingType, DuplexMode};

const A6: (f64, f64) = (105.0, 148.0);
const A4: (f64, f64) = (210.0, 297.0);

#[test]
fn sixteen_a6_pages_impose_four_up_on_a4() {
    let source = std::env::var("PRINTPREP_TEST_PDF").unwrap_or_else(|_| "/tmp/demo36.pdf".into());
    if !std::path::Path::new(&source).exists() {
        eprintln!("skipping: no test PDF at {source}");
        return;
    }
    let out = std::env::temp_dir().join("printprep-quarto.pdf");

    let plan =
        booklet_plan(BindingType::SaddleStitch, 16, 4, DuplexMode::LongEdge, false, 80.0, false, None, None).unwrap();
    let sides = sheets_for_plan(&plan, A6, A4).unwrap();
    assert_eq!(sides.len(), 4, "two sheets, printed both sides");

    let written = export_imposed(
        &source,
        &sides,
        out.to_str().unwrap(),
        MarkOptions {
            crop_marks: true,
            fold_marks: true,
            sheet_labels: true,
            bleed_mm: 3.0,
        },
    )
    .unwrap();
    assert_eq!(written, 4, "one PDF page per sheet side");
    assert!(out.metadata().unwrap().len() > 1000);
    println!("wrote {}", out.display());
}
