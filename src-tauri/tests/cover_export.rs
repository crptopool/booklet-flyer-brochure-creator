//! End-to-end check of the wrap-around cover as the user finishes it.
//!
//! The whole job is one file: sheet 1 is the cover — the trim pair in the
//! bottom-left corner of the same sheet size as the text, printed on the
//! front only, its back a blank page — and the text block follows with the
//! cover's pages excluded. The unit tests cover the numbering; this covers
//! the file that is actually written.

use printprep_lib::pdf_ops::impose::{export_imposed, MarkOptions};
use printprep_lib::pdf_ops::sheets::sheets_for_plan;
use printprep_lib::print_calc::plan::booklet_plan;
use printprep_lib::print_calc::presets::{BindingType, DuplexMode};

const A6: (f64, f64) = (105.0, 148.0);
const A4: (f64, f64) = (210.0, 297.0);

/// The user's job: 36 A6 pages four-up on portrait A4, pages 1 and 36
/// designated as the cover art on a heavier stock.
#[test]
fn a_covered_booklet_exports_as_one_file_with_the_cover_first() {
    let plan = booklet_plan(
        BindingType::SaddleStitch,
        36,
        4,
        DuplexMode::LongEdge,
        false,
        80.0,
        true,
        Some(200.0),
        Some(vec![1, 36]),
    )
    .unwrap();
    assert_eq!(plan.text_pages, 36, "34 body pages padded to a multiple of 4");
    assert_eq!(plan.text_sheet_count, 5);
    assert_eq!(plan.sheet_count, 6, "five text sheets plus the cover");

    let sides = sheets_for_plan(&plan, A6, A4).unwrap();
    assert_eq!(sides.len(), 12, "1 cover sheet + 5 text sheets, two sides each");

    // Sheet 1 of the job is the cover: outside pair printed, back blank.
    assert_eq!(sides[0].stock, "cover");
    let outside: Vec<_> = sides[0].placements.iter().map(|p| p.page).collect();
    assert_eq!(outside, vec![Some(36), Some(1)], "back cover left, front cover right");
    assert!(sides[1].placements.is_empty(), "the inside of the wrap is blank");

    // The pair spans the sheet's width, so the only cut is the horizontal
    // one — fold vertically at the spine, cut once across.
    assert!(sides[0].cut_x.is_empty());
    assert_eq!(sides[0].cut_y.len(), 1);
    assert_eq!(sides[0].fold_x.len(), 1);

    // The text block is pages 2..=35 exactly once each — the cover's pages
    // are out, nothing is duplicated, nothing is missing.
    let mut text_pages: Vec<u32> = sides
        .iter()
        .filter(|s| s.stock == "text")
        .flat_map(|s| s.placements.iter().filter_map(|p| p.page))
        .collect();
    text_pages.sort_unstable();
    assert_eq!(text_pages, (2..=35).collect::<Vec<_>>());

    // And the exporter writes all of it as one PDF, cover first.
    let source = std::env::var("PRINTPREP_TEST_PDF").unwrap_or_else(|_| "/tmp/demo36.pdf".into());
    if !std::path::Path::new(&source).exists() {
        eprintln!("skipping export: no test PDF at {source}");
        return;
    }
    let out = std::env::temp_dir().join("printprep-covered.pdf");
    let written = export_imposed(&source, &sides, out.to_str().unwrap(), MarkOptions::default()).unwrap();
    assert_eq!(written, 12, "one PDF page per sheet side, cover included");
    assert!(out.metadata().unwrap().len() > 1000);
    println!("wrote {}", out.display());
}

/// Same job on the same paper throughout: the cover is still its own
/// dedicated first sheet — the stock choice changes advice, not geometry.
#[test]
fn the_same_stock_cover_has_identical_geometry() {
    let with_note = booklet_plan(
        BindingType::SaddleStitch, 36, 4, DuplexMode::LongEdge, false, 80.0, true, Some(200.0),
        Some(vec![1, 36]),
    )
    .unwrap();
    let same_stock = booklet_plan(
        BindingType::SaddleStitch, 36, 4, DuplexMode::LongEdge, false, 80.0, true, None,
        Some(vec![1, 36]),
    )
    .unwrap();

    let a = sheets_for_plan(&with_note, A6, A4).unwrap();
    let b = sheets_for_plan(&same_stock, A6, A4).unwrap();
    assert_eq!(a, b, "paper choice must not move a single page");
}
