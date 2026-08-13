//! Deterministic print-geometry and page-sequencing calculations.
//!
//! Per the technical design principles, all page order and measurement
//! maths is deterministic code — never language-model inference.

pub mod booklet;
pub mod creep;
pub mod dpi;
pub mod geometry;
pub mod imposition;
pub mod presets;
pub mod scaling;
pub mod signatures;
pub mod spine;
pub mod units;
