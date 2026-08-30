use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕SQL B — SQL database query (stub).
pub fn quad_sql(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
