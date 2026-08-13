//! PrintPrep — Tauri 2 backend.
//!
//! Deterministic print calculations live in [`print_calc`], non-destructive
//! PDF handling in [`pdf_ops`], and export validation in [`preflight`].
//! Everything is exposed to the frontend as Tauri commands.

pub mod pdf_ops;
pub mod preflight;
pub mod print_calc;

use pdf_ops::document::PdfSource;
use pdf_ops::operations::{Operation, VirtualPage};
use print_calc::booklet::{BlankStrategy, SheetSpread};
use print_calc::creep::{CreepMode, CreepResult};
use print_calc::presets::{BindingType, PaperSize};

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_paper_sizes() -> Vec<PaperSize> {
    print_calc::presets::paper_sizes()
}

#[tauri::command]
fn describe_gsm(gsm: f64) -> String {
    print_calc::presets::describe_gsm(gsm).to_string()
}

// ---------------------------------------------------------------------------
// Deterministic calculations
// ---------------------------------------------------------------------------

#[tauri::command]
fn booklet_order(page_count: u32) -> Result<Vec<SheetSpread>, String> {
    print_calc::booklet::saddle_stitch_order(page_count)
}

#[tauri::command]
fn booklet_sheet_count(page_count: u32) -> Result<u32, String> {
    print_calc::booklet::saddle_stitch_sheet_count(page_count)
}

#[tauri::command]
fn booklet_blanks_needed(page_count: u32) -> Result<u32, String> {
    print_calc::booklet::blanks_needed(page_count, 4)
}

#[tauri::command]
fn booklet_blank_positions(page_count: u32, strategy: BlankStrategy) -> Result<Vec<u32>, String> {
    print_calc::booklet::blank_insertion_positions(page_count, strategy)
}

#[tauri::command]
fn nup_sheet_count(page_count: u32, pages_per_sheet: u32, duplex: bool) -> Result<u32, String> {
    print_calc::imposition::sheet_count(page_count, pages_per_sheet, duplex)
}

#[tauri::command]
fn nup_sequence(page_count: u32, rows: u32, cols: u32, duplex: bool) -> Result<Vec<Vec<Option<u32>>>, String> {
    print_calc::imposition::sequential_nup(page_count, rows, cols, duplex)
}

#[tauri::command]
fn cut_and_stack_sequence(page_count: u32, rows: u32, cols: u32) -> Result<Vec<Vec<Option<u32>>>, String> {
    print_calc::imposition::cut_and_stack(page_count, rows, cols)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn step_and_repeat_plan(
    copies_needed: u32,
    sheet_width_mm: f64,
    sheet_height_mm: f64,
    page_width_mm: f64,
    page_height_mm: f64,
    spacing_mm: f64,
    margin_mm: f64,
) -> Result<(u32, u32, u32), String> {
    use print_calc::units::mm_to_points;
    let (rows, cols) = print_calc::imposition::optimum_grid(
        mm_to_points(sheet_width_mm),
        mm_to_points(sheet_height_mm),
        mm_to_points(page_width_mm),
        mm_to_points(page_height_mm),
        mm_to_points(spacing_mm),
        mm_to_points(margin_mm),
    );
    let sheets = print_calc::imposition::step_and_repeat_sheets(copies_needed, rows, cols)?;
    Ok((rows, cols, sheets))
}

#[tauri::command]
fn spine_width(page_count: u32, caliper_mm: f64) -> Result<f64, String> {
    print_calc::spine::spine_width_from_pages(page_count, caliper_mm)
}

#[tauri::command]
fn caliper_from_gsm(gsm: f64, bulk_factor: Option<f64>) -> Result<f64, String> {
    print_calc::spine::approximate_caliper_mm(gsm, bulk_factor.unwrap_or(print_calc::spine::DEFAULT_BULK_FACTOR))
}

#[tauri::command]
fn creep(
    sheet_count: u32,
    caliper_mm: f64,
    fold_count: u32,
    max_creep_mm: Option<f64>,
    mode: CreepMode,
    custom_total_mm: Option<f64>,
) -> Result<CreepResult, String> {
    print_calc::creep::creep_compensation(sheet_count, caliper_mm, fold_count, max_creep_mm, mode, custom_total_mm)
}

#[tauri::command]
fn effective_dpi(pixel_width: u32, pixel_height: u32, printed_width_mm: f64, printed_height_mm: f64) -> Result<f64, String> {
    use print_calc::units::mm_to_points;
    print_calc::dpi::effective_dpi(
        pixel_width,
        pixel_height,
        mm_to_points(printed_width_mm),
        mm_to_points(printed_height_mm),
    )
}

#[tauri::command]
fn recommended_binding_margin_mm(binding: BindingType) -> f64 {
    print_calc::geometry::binding_margin_mm(binding)
}

// ---------------------------------------------------------------------------
// PDF foundation (Phase 1)
// ---------------------------------------------------------------------------

#[tauri::command]
fn inspect_pdf(path: String) -> Result<PdfSource, String> {
    pdf_ops::document::inspect_pdf(&path)
}

#[tauri::command]
fn preview_operations(source: PdfSource, operations: Vec<Operation>) -> Result<Vec<VirtualPage>, String> {
    pdf_ops::operations::apply_operations(&source, &operations)
}

#[tauri::command]
fn export_pdf(source_path: String, operations: Vec<Operation>, output_path: String) -> Result<u32, String> {
    let source = pdf_ops::document::inspect_pdf(&source_path)?;
    if source.modification_restricted {
        return Err("The PDF is protected and cannot be modified.".into());
    }
    let pages = pdf_ops::operations::apply_operations(&source, &operations)?;
    pdf_ops::export::export_pdf(&source_path, &pages, &output_path)
}

// ---------------------------------------------------------------------------
// Preflight
// ---------------------------------------------------------------------------

#[tauri::command]
fn run_preflight(
    source: PdfSource,
    binding: BindingType,
    bleed_mm: f64,
    expected_trim_mm: Option<(f64, f64)>,
) -> Vec<preflight::Finding> {
    use print_calc::units::mm_to_points;
    let trim_pt = expected_trim_mm.map(|(w, h)| (mm_to_points(w), mm_to_points(h)));
    preflight::preflight(&source, binding, bleed_mm, trim_pt)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_paper_sizes,
            describe_gsm,
            booklet_order,
            booklet_sheet_count,
            booklet_blanks_needed,
            booklet_blank_positions,
            nup_sheet_count,
            nup_sequence,
            cut_and_stack_sequence,
            step_and_repeat_plan,
            spine_width,
            caliper_from_gsm,
            creep,
            effective_dpi,
            recommended_binding_margin_mm,
            inspect_pdf,
            preview_operations,
            export_pdf,
            run_preflight,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
