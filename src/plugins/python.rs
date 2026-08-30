use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PYTHON B — Python pipe (stub).
pub fn quad_python(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
