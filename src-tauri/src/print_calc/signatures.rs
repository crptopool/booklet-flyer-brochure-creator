//! Signature division for large bound books.

use serde::{Deserialize, Serialize};

use crate::print_calc::booklet::blanks_needed;

pub const STANDARD_SIGNATURE_SIZES: [u32; 7] = [4, 8, 12, 16, 20, 24, 32];

/// A signature: a consecutive run of reading-order pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub number: u32,
    pub first_page: u32,
    /// Inclusive; may exceed the source page count when padded.
    pub last_page: u32,
    /// Padding blanks inside this signature.
    pub blank_pages: u32,
}

impl Signature {
    pub fn page_count(&self) -> u32 {
        self.last_page - self.first_page + 1
    }
}

/// Split pages into signatures of `signature_size` pages (multiple of 4).
/// The final signature is padded with blanks so every signature folds.
pub fn divide_into_signatures(page_count: u32, signature_size: u32) -> Result<Vec<Signature>, String> {
    if page_count == 0 {
        return Err("page_count must be positive".into());
    }
    if signature_size == 0 || !signature_size.is_multiple_of(4) {
        return Err("signature_size must be a positive multiple of 4".into());
    }
    let padded = page_count + blanks_needed(page_count, signature_size)?;
    Ok((0..padded / signature_size)
        .map(|i| {
            let first = i * signature_size + 1;
            let last = first + signature_size - 1;
            Signature {
                number: i + 1,
                first_page: first,
                last_page: last,
                blank_pages: last.saturating_sub(page_count),
            }
        })
        .collect())
}

/// Signature split minimising blanks: the final signature shrinks to the
/// smallest multiple of 4 that still holds the remaining pages.
pub fn balanced_signatures(page_count: u32, signature_size: u32) -> Result<Vec<Signature>, String> {
    let mut signatures = divide_into_signatures(page_count, signature_size)?;
    if let Some(last) = signatures.last().cloned() {
        if last.blank_pages > 0 {
            let real_pages = last.page_count() - last.blank_pages;
            let reduced = real_pages + blanks_needed(real_pages, 4)?;
            let new_last_page = last.first_page + reduced - 1;
            let idx = signatures.len() - 1;
            signatures[idx] = Signature {
                number: last.number,
                first_page: last.first_page,
                last_page: new_last_page,
                blank_pages: new_last_page.saturating_sub(page_count),
            };
        }
    }
    Ok(signatures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_division() {
        let sigs = divide_into_signatures(64, 16).unwrap();
        assert_eq!(sigs.len(), 4);
        assert_eq!(sigs[0].first_page, 1);
        assert_eq!(sigs[0].last_page, 16);
        assert_eq!(sigs[3].first_page, 49);
        assert_eq!(sigs[3].last_page, 64);
        assert!(sigs.iter().all(|s| s.blank_pages == 0));
    }

    #[test]
    fn padded_final_signature() {
        let sigs = divide_into_signatures(70, 16).unwrap();
        assert_eq!(sigs.len(), 5);
        assert_eq!(sigs[4].blank_pages, 10);
        assert_eq!(sigs[4].last_page, 80);
    }

    #[test]
    fn balanced_reduces_final_signature() {
        let sigs = balanced_signatures(70, 16).unwrap();
        assert_eq!(sigs.len(), 5);
        // Remaining 6 pages need only an 8-page signature -> 2 blanks.
        assert_eq!(sigs[4].page_count(), 8);
        assert_eq!(sigs[4].blank_pages, 2);
    }

    #[test]
    fn signature_size_must_be_multiple_of_four() {
        assert!(divide_into_signatures(64, 10).is_err());
    }

    #[test]
    fn signature_numbers_are_sequential() {
        let sigs = divide_into_signatures(100, 8).unwrap();
        let numbers: Vec<u32> = sigs.iter().map(|s| s.number).collect();
        assert_eq!(numbers, (1..=13).collect::<Vec<_>>());
    }
}
