use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕GTK B — GTK GUI (stub).
pub fn quad_gtk(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
