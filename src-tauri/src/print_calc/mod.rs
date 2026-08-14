//! Deterministic print-geometry and page-sequencing calculations.
//!
//! Per the technical design principles, all page order and measurement
//! maths is deterministic code — never language-model inference.

pub mod assistant;
pub mod binding;
pub mod booklet;
pub mod cover;
pub mod creep;
pub mod dpi;
pub mod duplex;
pub mod geometry;
pub mod imposition;
pub mod plan;
pub mod presets;
pub mod printer;
pub mod scaling;
pub mod signatures;
pub mod spine;
pub mod units;
