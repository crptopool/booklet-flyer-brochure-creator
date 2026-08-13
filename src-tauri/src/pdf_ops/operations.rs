//! Non-destructive page operations.
//!
//! Operations are project instructions applied to a *virtual page list*;
//! the original source file is never modified. Each virtual page
//! references a 1-based source page (or none for an inserted blank) plus
//! an extra rotation. Operation indices are 1-based to match the UI.

use serde::{Deserialize, Serialize};

use crate::pdf_ops::document::PdfSource;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VirtualPage {
    /// 1-based source page; `None` = inserted blank.
    pub source_page: Option<u32>,
    /// Extra rotation in degrees (multiple of 90).
    pub rotation: i64,
    /// Blank-page size; falls back to neighbouring pages at export.
    pub width_pt: Option<f64>,
    pub height_pt: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Operation {
    /// Replace page order; `order` lists 1-based current positions.
    ReorderPages { order: Vec<u32> },
    /// Rotate one page by a multiple of 90 degrees.
    RotatePage { position: u32, degrees: i64 },
    DeletePage { position: u32 },
    DuplicatePage { position: u32 },
    /// Insert a blank before `position`; `page_count + 1` appends.
    InsertBlank {
        position: u32,
        width_pt: Option<f64>,
        height_pt: Option<f64>,
    },
}

fn check_position(position: u32, count: usize) -> Result<(), String> {
    if position == 0 || position as usize > count {
        return Err(format!("position {position} out of range 1..{count}"));
    }
    Ok(())
}

impl Operation {
    pub fn apply(&self, pages: &[VirtualPage]) -> Result<Vec<VirtualPage>, String> {
        match self {
            Operation::ReorderPages { order } => {
                let mut sorted: Vec<u32> = order.clone();
                sorted.sort_unstable();
                let expected: Vec<u32> = (1..=pages.len() as u32).collect();
                if sorted != expected {
                    return Err("order must be a permutation of current page positions".into());
                }
                Ok(order.iter().map(|&i| pages[i as usize - 1].clone()).collect())
            }
            Operation::RotatePage { position, degrees } => {
                if degrees % 90 != 0 {
                    return Err("rotation must be a multiple of 90 degrees".into());
                }
                check_position(*position, pages.len())?;
                let mut result = pages.to_vec();
                let vp = &mut result[*position as usize - 1];
                vp.rotation = (vp.rotation + degrees).rem_euclid(360);
                Ok(result)
            }
            Operation::DeletePage { position } => {
                check_position(*position, pages.len())?;
                if pages.len() == 1 {
                    return Err("cannot delete the last remaining page".into());
                }
                let mut result = pages.to_vec();
                result.remove(*position as usize - 1);
                Ok(result)
            }
            Operation::DuplicatePage { position } => {
                check_position(*position, pages.len())?;
                let mut result = pages.to_vec();
                let copy = result[*position as usize - 1].clone();
                result.insert(*position as usize, copy);
                Ok(result)
            }
            Operation::InsertBlank {
                position,
                width_pt,
                height_pt,
            } => {
                if *position == 0 || *position as usize > pages.len() + 1 {
                    return Err(format!(
                        "position {position} out of range 1..{}",
                        pages.len() + 1
                    ));
                }
                let mut result = pages.to_vec();
                result.insert(
                    *position as usize - 1,
                    VirtualPage {
                        source_page: None,
                        rotation: 0,
                        width_pt: *width_pt,
                        height_pt: *height_pt,
                    },
                );
                Ok(result)
            }
        }
    }
}

/// Virtual page list mirroring the source reading order.
pub fn initial_page_list(source: &PdfSource) -> Vec<VirtualPage> {
    source
        .pages
        .iter()
        .map(|p| VirtualPage {
            source_page: Some(p.number),
            rotation: 0,
            width_pt: None,
            height_pt: None,
        })
        .collect()
}

/// Apply instructions in order and return the resulting page list.
pub fn apply_operations(source: &PdfSource, operations: &[Operation]) -> Result<Vec<VirtualPage>, String> {
    let mut pages = initial_page_list(source);
    for op in operations {
        pages = op.apply(&pages)?;
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_ops::document::PageInfo;

    fn source(n: u32) -> PdfSource {
        PdfSource {
            path: "test.pdf".into(),
            page_count: n,
            pages: (1..=n)
                .map(|i| PageInfo {
                    number: i,
                    width_pt: 595.0,
                    height_pt: 842.0,
                    rotation: 0,
                })
                .collect(),
            encrypted: false,
            modification_restricted: false,
            metadata: vec![],
        }
    }

    fn source_pages(pages: &[VirtualPage]) -> Vec<Option<u32>> {
        pages.iter().map(|p| p.source_page).collect()
    }

    #[test]
    fn reorder() {
        let pages = apply_operations(&source(3), &[Operation::ReorderPages { order: vec![3, 1, 2] }]).unwrap();
        assert_eq!(source_pages(&pages), vec![Some(3), Some(1), Some(2)]);
    }

    #[test]
    fn reorder_rejects_non_permutation() {
        assert!(apply_operations(&source(3), &[Operation::ReorderPages { order: vec![1, 1, 2] }]).is_err());
    }

    #[test]
    fn rotate_accumulates() {
        let ops = [
            Operation::RotatePage { position: 2, degrees: 90 },
            Operation::RotatePage { position: 2, degrees: 90 },
        ];
        let pages = apply_operations(&source(3), &ops).unwrap();
        assert_eq!(pages[1].rotation, 180);
    }

    #[test]
    fn rotate_rejects_non_multiple_of_90() {
        assert!(apply_operations(&source(3), &[Operation::RotatePage { position: 1, degrees: 45 }]).is_err());
    }

    #[test]
    fn delete() {
        let pages = apply_operations(&source(3), &[Operation::DeletePage { position: 2 }]).unwrap();
        assert_eq!(source_pages(&pages), vec![Some(1), Some(3)]);
    }

    #[test]
    fn cannot_delete_last_page() {
        assert!(apply_operations(&source(1), &[Operation::DeletePage { position: 1 }]).is_err());
    }

    #[test]
    fn duplicate() {
        let pages = apply_operations(&source(2), &[Operation::DuplicatePage { position: 1 }]).unwrap();
        assert_eq!(source_pages(&pages), vec![Some(1), Some(1), Some(2)]);
    }

    #[test]
    fn insert_blank_at_end() {
        let pages = apply_operations(
            &source(2),
            &[Operation::InsertBlank { position: 3, width_pt: None, height_pt: None }],
        )
        .unwrap();
        assert_eq!(source_pages(&pages), vec![Some(1), Some(2), None]);
    }

    #[test]
    fn insert_blank_before_back_cover() {
        let pages = apply_operations(
            &source(3),
            &[Operation::InsertBlank { position: 3, width_pt: None, height_pt: None }],
        )
        .unwrap();
        assert_eq!(source_pages(&pages), vec![Some(1), Some(2), None, Some(3)]);
    }

    #[test]
    fn chained_operations() {
        let ops = [
            Operation::DeletePage { position: 1 },
            Operation::InsertBlank { position: 1, width_pt: None, height_pt: None },
            Operation::ReorderPages { order: vec![2, 1] },
        ];
        let pages = apply_operations(&source(2), &ops).unwrap();
        assert_eq!(source_pages(&pages), vec![Some(2), None]);
    }

    #[test]
    fn out_of_range_positions_error() {
        assert!(apply_operations(&source(2), &[Operation::DeletePage { position: 5 }]).is_err());
        assert!(apply_operations(
            &source(2),
            &[Operation::InsertBlank { position: 9, width_pt: None, height_pt: None }]
        )
        .is_err());
    }
}
