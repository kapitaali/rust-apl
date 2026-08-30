//! GNU APL 2.0 — Rust implementation
//!
//! A rewrite of the GNU APL interpreter (ISO/IEC 13751) in Rust.
//! See REWRITE_STRATEGY.md in the repository root for design rationale.

pub mod ap;
pub mod boxdisplay;
pub mod cell;
pub mod comma;
pub mod comma1;
pub mod depth;
pub mod domino;
pub mod enclose;
pub mod encode_decode;
pub mod enlist;
pub mod epsilon;
pub mod ffi;
pub mod find;
pub mod format;
pub mod functions;
pub mod functions_def;
pub mod index_of;
pub mod inner;
pub mod interval_index;
pub mod not_match;
pub mod operators;
pub mod outer;
pub mod packed_bool;
pub mod packed_int;
pub mod parser;
pub mod partition;
pub mod pick;
pub mod quad;
pub mod quad_plot;
pub mod rank;
pub mod replicate;
pub mod rotate;
pub mod shape;
pub mod smallvec_ops;
pub mod sort;
pub mod squad;
pub mod sysvars;
pub mod take_drop;
pub mod tokenizer;
pub mod transpose;
pub mod types;
pub mod union;
pub mod unparse;
pub mod value;
pub mod workspace;
pub mod xml_archive;

// Plugin system (Phase 6)
pub mod plugin_system;
pub mod plugins;

#[cfg(feature = "unofficial-ext")]
pub mod key;
#[cfg(feature = "unofficial-ext")]
pub mod over;

#[cfg(test)]
pub mod parallel_tests;
#[cfg(test)]
pub mod parallel_phase7_tests;

// Re-export AplError for use in main.rs and elsewhere
pub use types::AplError;
