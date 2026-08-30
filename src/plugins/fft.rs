use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕FFT B — FFT (stub).
pub fn quad_fft(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}
