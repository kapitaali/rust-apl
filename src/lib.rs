//! GNU APL 2.0 — Rust implementation
//!
//! A rewrite of the GNU APL interpreter (ISO/IEC 13751) in Rust.
//! See REWRITE_STRATEGY.md in the repository root for design rationale.

pub mod boxdisplay;
pub mod cell;
pub mod depth;
pub mod domino;
pub mod enclose;
pub mod enlist;
pub mod epsilon;
pub mod functions;
pub mod functions_def;
pub mod index_of;
pub mod inner;
pub mod operators;
pub mod outer;
#[cfg(test)]
pub mod parallel_tests;
pub mod parser;
pub mod pick;
pub mod replicate;
pub mod rotate;
pub mod shape;
pub mod sort;
pub mod sysvars;
pub mod take_drop;
pub mod tokenizer;
pub mod transpose;
pub mod types;
pub mod value;
pub mod workspace;
