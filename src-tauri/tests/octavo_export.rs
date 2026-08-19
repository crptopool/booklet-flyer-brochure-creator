//! Eight A6 pages on one side of A3, which only fit turned on their side.

use printprep_lib::pdf_ops::impose::{export_imposed, MarkOptions};
use printprep_lib::pdf_ops::sheets::sheets_for_plan;
use printprep_lib::print_calc::plan::booklet_plan;
use printprep_lib::print_calc::presets::{BindingType, DuplexMode};

const A6: (f64, f64) = (105.0, 148.0);
const A3: (f64, f64) = (297.0, 420.0);

#[test]
fn thirty_two_a6_pages_impose_eight_up_on_a3() {
    let source = std::env::var("PRINTPREP_TEST_PDF").unwrap_or_else(|_| "/tmp/demo36.pdf".into());
    if !std::path::Path::new(&source).exists() {
        eprintln!("skipping: no test PDF at {source}");
        return;
    }
    let out = std::env::temp_dir().join("printprep-octavo.pdf");

    let plan =
        booklet_plan(BindingType::SaddleStitch, 32, 8, DuplexMode::LongEdge, false, 80.0, false, None, None, None).unwrap();
    let sides = sheets_for_plan(&plan, A6, A3).unwrap();
    assert_eq!(sides.len(), 4, "two sheets of sixteen pages, both sides");

    // Every page placed once, and every one turned a quarter turn to fit.
    let mut placed: Vec<u32> = sides
        .iter()
        .flat_map(|s| s.placements.iter().filter_map(|p| p.page))
        .collect();
    placed.sort_unstable();
    assert_eq!(placed, (1..=32).collect::<Vec<_>>());
    for side in &sides {
        for p in &side.placements {
            assert!(p.rotation == 90 || p.rotation == 270, "turned pages: {}", p.rotation);
        }
    }

    // Page 1 must end up bound on its left edge. The last fold creases
    // between the third and fourth rows, so page 1 — which sits in the third
    // row — has to be turned until its left edge faces down, towards that
    // crease. Turning it the other way gives a booklet that opens from the
    // right, which is the mistake this pins down.
    let front = sides.iter().find(|s| s.sheet_number == 1 && s.side == "front").unwrap();
    let page_one = front
        .placements
        .iter()
        .find(|p| p.page == Some(1))
        .expect("page 1 is on the first sheet");
    assert_eq!(page_one.rotation, 90, "page 1 must be bound on its left edge");

    let written = export_imposed(
        &source,
        &sides,
        out.to_str().unwrap(),
        MarkOptions { crop_marks: true, fold_marks: true, sheet_labels: true, bleed_mm: 3.0 },
    )
    .unwrap();
    assert_eq!(written, 4);
    println!("wrote {}", out.display());
}
