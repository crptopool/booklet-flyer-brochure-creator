//! PrintPrep — Tauri 2 backend.
//!
//! Deterministic print calculations live in [`print_calc`], non-destructive
//! PDF handling in [`pdf_ops`], and export validation in [`preflight`].
//! Everything is exposed to the frontend as Tauri commands.

pub mod pdf_ops;
pub mod preflight;
pub mod project;
pub mod print_calc;

use pdf_ops::document::PdfSource;
use pdf_ops::operations::{Operation, VirtualPage};
use print_calc::binding::BindingProfile;
use print_calc::booklet::{BlankStrategy, SheetSpread};
use print_calc::creep::{CreepMode, CreepResult};
use print_calc::duplex::DuplexPlan;
use pdf_ops::impose::{MarkOptions, SheetSide};
use pdf_ops::cover_pdf::CoverArtwork;
use print_calc::assistant::{Advice, GlossaryEntry};
use print_calc::cover::{CoverInputs, CoverKind, CoverLayout};
use print_calc::printer::{PrinterProfile, ProfileFinding};
use print_calc::plan::BookletPlan;
use pdf_ops::images::{ColorUsage, PlacedImage};
use print_calc::presets::{BindingType, DuplexMode, PaperSize};
use print_calc::signatures::Signature;
use project::Project;

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
// Binding methods, duplex logic and the combined booklet plan
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_booklet_bindings() -> Vec<BindingProfile> {
    print_calc::binding::booklet_binding_profiles()
}

#[tauri::command]
fn get_binding_profile(binding: BindingType) -> BindingProfile {
    print_calc::binding::binding_profile(binding)
}

#[tauri::command]
fn get_duplex_plan(mode: DuplexMode, sheet_is_landscape: bool) -> DuplexPlan {
    print_calc::duplex::duplex_plan(mode, sheet_is_landscape)
}

/// What the chosen papers can hold, and how the sheet folds to hold it.
///
/// The booklet screen offers only the counts this answers with, so the
/// configuration cannot ask for an imposition the paper will not take.
#[derive(serde::Serialize)]
struct SheetCapacity {
    /// Pages of the chosen trim size that fit upright on one side.
    fit_rows: u32,
    fit_cols: u32,
    /// Pages-per-side values that can be folded and still fit.
    options: Vec<u32>,
    /// Grid used for the requested count, when it is one of the options.
    rows: Option<u32>,
    cols: Option<u32>,
    /// True when the pages have to be turned 90° on the sheet to fit.
    turned: bool,
    /// Creases in the order they are made, last one being the spine.
    folds: Vec<String>,
}

#[tauri::command]
fn sheet_capacity(
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
    pages_per_side: u32,
) -> Result<SheetCapacity, String> {
    use print_calc::fold::{fit_grid, foldable_options, fold_sequence, folded_grid, FoldAxis};
    let (fit_rows, fit_cols) = fit_grid(sheet_mm, trim_mm)?;
    let grid = folded_grid(pages_per_side, sheet_mm, trim_mm).ok();
    let folds = grid
        .and_then(|g| fold_sequence(g.rows, g.cols, g.spine).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|axis| match axis {
            FoldAxis::Vertical => "vertical".to_string(),
            FoldAxis::Horizontal => "horizontal".to_string(),
        })
        .collect();
    Ok(SheetCapacity {
        fit_rows,
        fit_cols,
        options: foldable_options(sheet_mm, trim_mm),
        rows: grid.map(|g| g.rows),
        cols: grid.map(|g| g.cols),
        turned: grid.is_some_and(|g| g.turned),
        folds,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn build_booklet_plan(
    binding: BindingType,
    source_pages: u32,
    pages_per_side: u32,
    duplex_mode: DuplexMode,
    sheet_is_landscape: bool,
    gsm: f64,
    // Whether the booklet gets a wrap-around cover as its own dedicated
    // sheet 1 — independent of whether its paper differs.
    with_cover: bool,
    // Weight of the cover stock when it differs from the text.
    cover_gsm: Option<f64>,
    // The document's own pages to print on the cover's outside, when it is
    // not blank: front cover, then back cover.
    cover_source_pages: Option<Vec<u32>>,
    // Chapter starts: pages that must open on a right-hand page, each with
    // a blank behind it.
    recto_pages: Option<Vec<u32>>,
) -> Result<BookletPlan, String> {
    print_calc::plan::booklet_plan(
        binding,
        source_pages,
        pages_per_side,
        duplex_mode,
        sheet_is_landscape,
        gsm,
        with_cover,
        cover_gsm,
        cover_source_pages,
        recto_pages,
    )
}

#[tauri::command]
fn booklet_plan_spreads(plan: BookletPlan) -> Result<Vec<SheetSpread>, String> {
    print_calc::plan::plan_spreads(&plan)
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
// Imposition: arrange the document per the binding plan
// ---------------------------------------------------------------------------

/// Sheet sides for a plan — used by both the preview and the export so
/// the simulation always matches the file that gets written.
#[tauri::command]
fn plan_sheets(
    plan: BookletPlan,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
) -> Result<Vec<SheetSide>, String> {
    pdf_ops::sheets::sheets_for_plan(&plan, trim_mm, sheet_mm)
}

/// Write the imposed, print-ready PDF arranged for the chosen binding.
///
/// One file for the whole job: the cover — when there is one — is sheet 1,
/// a dedicated sheet whose back is blank, so it can be printed with the
/// stack or pulled out and run separately on heavier stock.
#[tauri::command]
fn export_imposed_pdf(
    source_path: String,
    plan: BookletPlan,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
    output_path: String,
    marks: MarkOptions,
) -> Result<u32, String> {
    let source = pdf_ops::document::inspect_pdf(&source_path)?;
    if source.modification_restricted {
        return Err("The PDF is protected and cannot be modified.".into());
    }
    if plan.source_pages > source.page_count {
        return Err(format!(
            "The plan covers {} pages but the document has {}.",
            plan.source_pages, source.page_count
        ));
    }
    let sides = pdf_ops::sheets::sheets_for_plan(&plan, trim_mm, sheet_mm)?;
    pdf_ops::impose::export_imposed(&source_path, &sides, &output_path, marks)
}

/// The printer-dialog checklist for a plan's exported file.
///
/// Built from the same plan as the export, so the flip edge, the page
/// ranges and the stock advice always describe the file that was written
/// rather than a generic leaflet.
#[tauri::command]
fn printing_guidance(
    plan: BookletPlan,
    sheet_name: String,
    sheet_is_landscape: bool,
) -> Vec<print_calc::guidance::PrintStep> {
    print_calc::guidance::printing_guidance(&plan, &sheet_name, sheet_is_landscape)
}

/// Hand an exported file to the operating system's print path.
///
/// There is no portable way to open a native print dialog directly from a
/// webview, so this does the honest next-best thing on each platform and
/// reports exactly what happened, so the frontend can tell the user what
/// to press next.
#[tauri::command]
fn send_to_printer(path: String) -> Result<String, String> {
    if !std::path::Path::new(&path).exists() {
        return Err(format!("{path} does not exist — export the file first."));
    }

    #[cfg(target_os = "windows")]
    {
        // The Print verb hands the file to the default PDF app's print
        // command, which opens its dialog with the file already loaded.
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &format!("Start-Process -FilePath '{}' -Verb Print", path.replace('\'', "''")),
            ])
            .spawn()
            .map_err(|e| format!("Could not start the system print handler: {e}"))?;
        Ok("The file was handed to your PDF app's Print command — its print dialog opens with the file loaded. Check every setting against the checklist.".to_string())
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Could not open the file: {e}"))?;
        Ok("The file is open in your PDF viewer — press ⌘P and set the dialog up from the checklist.".to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Could not open the file: {e}"))?;
        Ok("The file is open in your PDF viewer — press Ctrl+P and set the dialog up from the checklist.".to_string())
    }
}

/// Reading order of the finished, bound document.
///
/// Returns the source page for each position in the bound book, so the
/// preview can simulate turning the pages after binding. `None` is a
/// blank inserted to satisfy the binding's page-count rule.
#[tauri::command]
fn bound_reading_order(plan: BookletPlan) -> Vec<Option<u32>> {
    // The body is the plan's reading positions — cover pages absent,
    // chapter-start blanks in place — exactly as the imposition lays them
    // out, so the bound preview shows the same book that will be printed.
    let body = plan.body_slots();
    let blanks = (plan.total_pages as usize).saturating_sub(body.len());

    let mut order = Vec::new();
    // A cover carrying designated pages is part of what the reader holds:
    // front cover first, blank inside faces, back cover last.
    if !plan.cover_source_pages.is_empty() {
        order.push(plan.cover_source_pages.first().copied());
        order.push(None);
    }
    order.extend(body);
    order.extend(std::iter::repeat_n(None, blanks));
    if !plan.cover_source_pages.is_empty() {
        order.push(None);
        order.push(plan.cover_source_pages.get(1).copied());
    }
    order
}

// ---------------------------------------------------------------------------
// Signature and step-and-repeat export (§22 modes 5 and 7)
// ---------------------------------------------------------------------------

#[tauri::command]
fn divide_signatures(page_count: u32, signature_size: u32, balanced: bool) -> Result<Vec<Signature>, String> {
    if balanced {
        print_calc::signatures::balanced_signatures(page_count, signature_size)
    } else {
        print_calc::signatures::divide_into_signatures(page_count, signature_size)
    }
}

/// Export one PDF per signature, or a single combined file.
///
/// Returns the paths written.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn export_signature_pdfs(
    source_path: String,
    page_count: u32,
    signature_size: u32,
    balanced: bool,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
    back_rotation: i64,
    output_path: String,
    combined: bool,
    marks: MarkOptions,
) -> Result<Vec<String>, String> {
    let source = pdf_ops::document::inspect_pdf(&source_path)?;
    if source.modification_restricted {
        return Err("The PDF is protected and cannot be modified.".into());
    }
    if page_count > source.page_count {
        return Err(format!(
            "The plan covers {page_count} pages but the document has {}.",
            source.page_count
        ));
    }
    let signatures = divide_signatures(page_count, signature_size, balanced)?;

    if combined {
        let mut all = Vec::new();
        for sig in &signatures {
            all.extend(pdf_ops::sheets::sheets_for_signature(
                sig, page_count, trim_mm, sheet_mm, back_rotation,
            )?);
        }
        pdf_ops::impose::export_imposed(&source_path, &all, &output_path, marks)?;
        return Ok(vec![output_path]);
    }

    // One file per signature, numbered alongside the chosen name.
    let stem = output_path.strip_suffix(".pdf").unwrap_or(&output_path).to_string();
    let mut written = Vec::new();
    for sig in &signatures {
        let sides = pdf_ops::sheets::sheets_for_signature(sig, page_count, trim_mm, sheet_mm, back_rotation)?;
        let path = format!("{stem}-signature-{:02}.pdf", sig.number);
        pdf_ops::impose::export_imposed(&source_path, &sides, &path, marks)?;
        written.push(path);
    }
    Ok(written)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn export_step_and_repeat_pdf(
    source_path: String,
    page: u32,
    copies: u32,
    rows: u32,
    cols: u32,
    trim_mm: (f64, f64),
    sheet_mm: (f64, f64),
    spacing_mm: f64,
    output_path: String,
    marks: MarkOptions,
) -> Result<u32, String> {
    let source = pdf_ops::document::inspect_pdf(&source_path)?;
    if page > source.page_count {
        return Err(format!("Page {page} does not exist — the document has {}.", source.page_count));
    }
    let sides = pdf_ops::sheets::sheets_for_step_and_repeat(
        page, copies, rows, cols, trim_mm, sheet_mm, spacing_mm,
    )?;
    pdf_ops::impose::export_imposed(&source_path, &sides, &output_path, marks)
}

// ---------------------------------------------------------------------------
// Image resolution (§7, §27)
// ---------------------------------------------------------------------------

/// Colour spaces the document uses, with guidance. Never converts.
#[tauri::command]
fn scan_color_usage(path: String, commercial_print: bool) -> Result<(ColorUsage, Vec<preflight::Finding>), String> {
    let usage = pdf_ops::images::scan_colors(&path)?;
    let mut findings = Vec::new();
    if commercial_print && usage.device_rgb && !usage.device_cmyk {
        findings.push(preflight::Finding {
            severity: preflight::Severity::Warning,
            code: "rgb_for_commercial_print".into(),
            message: "The artwork is RGB but a commercial press prints CMYK. Your printer will convert it, and saturated RGB colours will shift. Convert it yourself if the exact colour matters — this application will not convert it for you.".into(),
            page: None,
        });
    }
    if usage.separation {
        findings.push(preflight::Finding {
            severity: preflight::Severity::Info,
            code: "spot_colour".into(),
            message: format!(
                "Spot colours found: {}. Confirm with your printer that these plates are actually being run.",
                if usage.spot_names.is_empty() { "unnamed".to_string() } else { usage.spot_names.join(", ") }
            ),
            page: None,
        });
    }
    if findings.is_empty() {
        findings.push(preflight::Finding {
            severity: preflight::Severity::Info,
            code: "colour_ok".into(),
            message: format!("Colour spaces in use: {}. Left exactly as they are.", usage.summary()),
            page: None,
        });
    }
    Ok((usage, findings))
}

#[tauri::command]
fn scan_image_resolution(path: String) -> Result<Vec<PlacedImage>, String> {
    pdf_ops::images::scan_images(&path)
}

/// Preflight findings for every image that prints below the threshold.
#[tauri::command]
fn preflight_images(path: String, minimum_dpi: Option<f64>) -> Result<Vec<preflight::Finding>, String> {
    let minimum = minimum_dpi.unwrap_or(print_calc::presets::MINIMUM_PRINT_DPI);
    let recommended = print_calc::presets::RECOMMENDED_PRINT_DPI;
    let images = pdf_ops::images::scan_images(&path)?;
    let mut findings: Vec<preflight::Finding> = images
        .iter()
        .filter(|i| i.effective_dpi < minimum)
        .map(|i| preflight::Finding {
            severity: preflight::Severity::Warning,
            code: "low_dpi".into(),
            message: format!(
                "Image on Page {} will print at approximately {:.0} DPI. Recommended minimum: {:.0} DPI; preferred: {:.0} DPI.",
                i.page, i.effective_dpi, minimum, recommended
            ),
            page: Some(i.page),
        })
        .collect();
    if findings.is_empty() {
        findings.push(preflight::Finding {
            severity: preflight::Severity::Info,
            code: "dpi_ok".into(),
            message: if images.is_empty() {
                "No raster images found — nothing to check for resolution.".into()
            } else {
                format!("All {} image(s) print at or above {minimum:.0} DPI.", images.len())
            },
            page: None,
        });
    }
    Ok(findings)
}

// ---------------------------------------------------------------------------
// Projects (§29)
// ---------------------------------------------------------------------------

#[tauri::command]
fn new_project() -> Project {
    Project::default()
}

#[tauri::command]
fn save_project(project: Project, path: String) -> Result<(), String> {
    project::save(&project, &path)
}

#[tauri::command]
fn load_project(path: String) -> Result<(Project, bool), String> {
    project::load(&path)
}

// ---------------------------------------------------------------------------
// Cover creator (Phase 7)
// ---------------------------------------------------------------------------

#[tauri::command]
fn cover_defaults(kind: CoverKind) -> CoverInputs {
    print_calc::cover::default_inputs(kind)
}

#[tauri::command]
fn build_cover_layout(input: CoverInputs) -> Result<CoverLayout, String> {
    print_calc::cover::cover_layout(input)
}

#[tauri::command]
fn export_cover_pdf(
    layout: CoverLayout,
    output_path: String,
    title: String,
    artwork: Option<CoverArtwork>,
) -> Result<(f64, f64), String> {
    pdf_ops::cover_pdf::export_cover_with_artwork(&layout, &output_path, &title, artwork.as_ref())
}

// ---------------------------------------------------------------------------
// Printer capability profiles
// ---------------------------------------------------------------------------

/// Where saved profiles live, inside the platform config directory.
fn profiles_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Cannot locate the configuration directory: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    Ok(dir.join("printer-profiles.json"))
}

#[tauri::command]
fn list_printer_profiles(app: tauri::AppHandle) -> Result<Vec<PrinterProfile>, String> {
    let path = profiles_path(&app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read saved profiles: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Saved profiles are unreadable: {e}"))
}

/// Add or replace a profile, matched on name.
#[tauri::command]
fn save_printer_profile(app: tauri::AppHandle, profile: PrinterProfile) -> Result<Vec<PrinterProfile>, String> {
    if profile.name.trim().is_empty() {
        return Err("The profile needs a name.".into());
    }
    let mut all = list_printer_profiles(app.clone())?;
    match all.iter_mut().find(|p| p.name == profile.name) {
        Some(existing) => *existing = profile,
        None => all.push(profile),
    }
    let path = profiles_path(&app)?;
    std::fs::write(&path, serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot save profiles: {e}"))?;
    Ok(all)
}

#[tauri::command]
fn delete_printer_profile(app: tauri::AppHandle, name: String) -> Result<Vec<PrinterProfile>, String> {
    let mut all = list_printer_profiles(app.clone())?;
    all.retain(|p| p.name != name);
    let path = profiles_path(&app)?;
    std::fs::write(&path, serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot save profiles: {e}"))?;
    Ok(all)
}

#[tauri::command]
fn default_printer_profile() -> PrinterProfile {
    PrinterProfile::default()
}

#[tauri::command]
fn check_job_against_printer(
    profile: PrinterProfile,
    sheet_mm: (f64, f64),
    sheet_name: Option<String>,
    bleed_mm: f64,
    duplex: DuplexMode,
) -> Vec<ProfileFinding> {
    print_calc::printer::check_job(&profile, sheet_mm, sheet_name.as_deref(), bleed_mm, duplex)
}

// ---------------------------------------------------------------------------
// Print assistant (Phase 8)
// ---------------------------------------------------------------------------

#[tauri::command]
fn assistant_advise(request: String) -> Result<Advice, String> {
    print_calc::assistant::advise(&request)
}

#[tauri::command]
fn assistant_explain(term: String) -> Vec<GlossaryEntry> {
    print_calc::assistant::explain(&term)
}

#[tauri::command]
fn assistant_glossary() -> Vec<GlossaryEntry> {
    print_calc::assistant::glossary()
}

#[tauri::command]
fn assistant_troubleshooting() -> Vec<GlossaryEntry> {
    print_calc::assistant::troubleshooting()
}

/// Write raw bytes the frontend produced (used for eBook cover PNG/JPEG).
#[tauri::command]
fn write_bytes(path: String, bytes: Vec<u8>) -> Result<usize, String> {
    if bytes.is_empty() {
        return Err("nothing to write".into());
    }
    std::fs::write(&path, &bytes).map_err(|e| format!("Failed to write {path}: {e}"))?;
    Ok(bytes.len())
}

/// Raw PDF bytes, so the webview can render real page content.
#[tauri::command]
fn read_pdf_bytes(path: String) -> Result<Vec<u8>, String> {
    std::fs::read(&path).map_err(|e| format!("Failed to read {path}: {e}"))
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
            list_booklet_bindings,
            get_binding_profile,
            get_duplex_plan,
            build_booklet_plan,
            sheet_capacity,
            booklet_plan_spreads,
            plan_sheets,
            export_imposed_pdf,
            printing_guidance,
            send_to_printer,
            bound_reading_order,
            cover_defaults,
            build_cover_layout,
            export_cover_pdf,
            write_bytes,
            read_pdf_bytes,
            list_printer_profiles,
            save_printer_profile,
            delete_printer_profile,
            default_printer_profile,
            check_job_against_printer,
            assistant_advise,
            assistant_explain,
            assistant_glossary,
            assistant_troubleshooting,
            divide_signatures,
            export_signature_pdfs,
            export_step_and_repeat_pdf,
            scan_image_resolution,
            scan_color_usage,
            preflight_images,
            new_project,
            save_project,
            load_project,
            inspect_pdf,
            preview_operations,
            export_pdf,
            run_preflight,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
