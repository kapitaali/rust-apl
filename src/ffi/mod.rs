//! Foreign-function interface: CAbi (⎕NA), Rust plugins, Java bridge.
//!
//! See META-INF/EXTENSIONS.md for the architecture. Phase F1 = exchange.rs.

pub mod exchange;
#[cfg(test)]
mod exchange_tests;
mod nadecl;
#[cfg(test)]
mod nadecl_tests;

pub use nadecl::{parse_na_decl, CAbiSpec, Direction, LeafType, Special, TypeSpec, Width};

pub use exchange::{
    value_to_xarray, xarray_to_value, CellTag, XArray, XCell, XTaggedCell, EXCHANGE_ABI, MAX_RANK,
};
