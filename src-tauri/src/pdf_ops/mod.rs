//! Non-destructive PDF handling: inspection, page operations and export.
//!
//! The original source file is never modified. Operations are project
//! instructions applied to a virtual page list and only materialised
//! into a new PDF at export time.

pub mod document;
pub mod export;
pub mod impose;
pub mod operations;
pub mod sheets;
