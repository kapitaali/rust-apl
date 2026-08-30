use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PLOT B — disabled (returns DOMAIN ERROR when plugin not compiled in).
pub fn quad_plot(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
