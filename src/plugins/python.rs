use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PYTHON B — Python pipe.
///
/// B is a character vector (Python code).
/// Requires pyo3 and Python interpreter.
pub fn quad_python(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
