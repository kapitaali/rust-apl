use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PNG B — PNG image I/O (stub).
pub fn quad_png(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
