//! Project files.
//!
//! A project records everything needed to reproduce an output exactly:
//! the source reference, the page operations, the binding plan inputs,
//! trim and sheet sizes, margins, marks and the printer profile. It
//! stores *instructions*, never page content, so the source file stays
//! the single copy of the artwork.

use serde::{Deserialize, Serialize};

use crate::pdf_ops::impose::MarkOptions;
use crate::pdf_ops::operations::Operation;
use crate::print_calc::cover::CoverInputs;
use crate::print_calc::presets::{BindingSide, BindingType, DuplexMode};
use crate::print_calc::printer::PrinterProfile;

/// Bumped only when an older file can no longer be read as-is.
pub const PROJECT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookletSettings {
    pub binding: BindingType,
    pub page_count: u32,
    pub pages_per_side: u32,
    pub duplex: DuplexMode,
    pub sheet_is_landscape: bool,
    pub gsm: f64,
    pub trim_size: String,
    pub sheet_size: String,
    pub binding_side: BindingSide,
    pub binding_margin_mm: f64,
}

impl Default for BookletSettings {
    fn default() -> Self {
        BookletSettings {
            binding: BindingType::SaddleStitch,
            page_count: 20,
            pages_per_side: 2,
            duplex: DuplexMode::ShortEdge,
            sheet_is_landscape: true,
            gsm: 80.0,
            trim_size: "A5".into(),
            sheet_size: "A4".into(),
            binding_side: BindingSide::Left,
            binding_margin_mm: 5.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub name: String,
    /// Path to the source PDF. Absent for a settings-only project.
    pub source_path: Option<String>,
    /// Non-destructive page instructions, replayed on load.
    pub operations: Vec<Operation>,
    pub booklet: BookletSettings,
    pub marks: MarkOptions,
    pub cover: Option<CoverInputs>,
    pub printer_profile: Option<PrinterProfile>,
    /// Free-text note the user can keep with the project.
    pub notes: String,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            version: PROJECT_VERSION,
            name: "Untitled project".into(),
            source_path: None,
            operations: vec![],
            booklet: BookletSettings::default(),
            marks: MarkOptions::default(),
            cover: None,
            printer_profile: None,
            notes: String::new(),
        }
    }
}

/// Serialise a project to pretty JSON.
pub fn to_json(project: &Project) -> Result<String, String> {
    serde_json::to_string_pretty(project).map_err(|e| format!("Cannot serialise the project: {e}"))
}

/// Read a project, checking it is a version we understand.
pub fn from_json(text: &str) -> Result<Project, String> {
    let project: Project = serde_json::from_str(text)
        .map_err(|e| format!("This does not look like a PrintPrep project file: {e}"))?;
    if project.version > PROJECT_VERSION {
        return Err(format!(
            "This project was saved by a newer version of PrintPrep (format {} — this build reads up to {}).",
            project.version, PROJECT_VERSION
        ));
    }
    Ok(project)
}

/// Save to disk.
pub fn save(project: &Project, path: &str) -> Result<(), String> {
    std::fs::write(path, to_json(project)?).map_err(|e| format!("Cannot write {path}: {e}"))
}

/// Load from disk, reporting whether the source file is still there.
pub fn load(path: &str) -> Result<(Project, bool), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {path}: {e}"))?;
    let project = from_json(&text)?;
    let source_present = project
        .source_path
        .as_deref()
        .map(|p| std::path::Path::new(p).is_file())
        .unwrap_or(true);
    Ok((project, source_present))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().into_owned()
    }

    fn sample() -> Project {
        Project {
            name: "Spring catalogue".into(),
            source_path: Some("/docs/catalogue.pdf".into()),
            operations: vec![
                Operation::ReorderPages { order: vec![2, 1] },
                Operation::RotatePage { position: 1, degrees: 90 },
                Operation::InsertBlank { position: 3, width_pt: None, height_pt: None },
            ],
            booklet: BookletSettings {
                binding: BindingType::Perfect,
                page_count: 200,
                pages_per_side: 1,
                duplex: DuplexMode::LongEdge,
                sheet_is_landscape: false,
                gsm: 90.0,
                trim_size: "A5".into(),
                sheet_size: "A4".into(),
                binding_side: BindingSide::Left,
                binding_margin_mm: 10.0,
            },
            notes: "Confirm caliper with the printer".into(),
            ..Project::default()
        }
    }

    #[test]
    fn a_project_round_trips_without_loss() {
        let p = sample();
        let restored = from_json(&to_json(&p).unwrap()).unwrap();
        assert_eq!(restored, p);
    }

    #[test]
    fn page_operations_survive_the_round_trip() {
        let restored = from_json(&to_json(&sample()).unwrap()).unwrap();
        assert_eq!(restored.operations.len(), 3);
        assert_eq!(restored.operations[0], Operation::ReorderPages { order: vec![2, 1] });
    }

    #[test]
    fn saving_then_loading_returns_the_same_project() {
        let path = tmp("printprep_project.json");
        let p = sample();
        save(&p, &path).unwrap();
        let (loaded, source_present) = load(&path).unwrap();
        assert_eq!(loaded, p);
        // The referenced source does not exist in the test environment.
        assert!(!source_present);
    }

    #[test]
    fn a_present_source_is_reported_as_present() {
        let src = tmp("printprep_project_src.pdf");
        std::fs::write(&src, b"%PDF-1.7\n").unwrap();
        let path = tmp("printprep_project2.json");
        save(&Project { source_path: Some(src), ..Project::default() }, &path).unwrap();
        assert!(load(&path).unwrap().1);
    }

    #[test]
    fn a_project_with_no_source_is_not_reported_as_missing() {
        let path = tmp("printprep_project3.json");
        save(&Project::default(), &path).unwrap();
        assert!(load(&path).unwrap().1);
    }

    #[test]
    fn a_newer_format_is_refused_with_a_clear_message() {
        let mut text = to_json(&sample()).unwrap();
        text = text.replace("\"version\": 1", "\"version\": 99");
        let err = from_json(&text).unwrap_err();
        assert!(err.contains("newer version"));
    }

    #[test]
    fn unrelated_json_is_rejected() {
        assert!(from_json("{\"hello\":\"world\"}").is_err());
        assert!(from_json("not json at all").is_err());
    }

    #[test]
    fn missing_files_report_the_path() {
        let err = load("/nonexistent/project.json").unwrap_err();
        assert!(err.contains("/nonexistent/project.json"));
    }

    #[test]
    fn defaults_are_a_usable_saddle_stitch_setup() {
        let d = Project::default();
        assert_eq!(d.version, PROJECT_VERSION);
        assert_eq!(d.booklet.binding, BindingType::SaddleStitch);
        assert_eq!(d.booklet.pages_per_side, 2);
        assert!(d.booklet.sheet_is_landscape);
    }
}
