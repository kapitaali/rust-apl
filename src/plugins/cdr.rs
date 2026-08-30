use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕CDR B — CDR binary interchange (stub).
pub fn quad_cdr(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
