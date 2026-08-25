//! Not-match ≢ operation.
//!
//! Mirrors `Bif_F12_NEQUIV` in C++ (simplified).

use crate::cell::Cell;
use crate::types::AplResult;
use crate::value::ValueP;

/// A≢B — not match: 0 if A and B match (same shape, tolerant elementwise equal), 1 otherwise.
pub fn not_match(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    if a.shape() != b.shape() {
        return Ok(ValueP::scalar_from(Cell::Int(1)));
    }
    let ac = a.cells();
    let bc = b.cells();
    if ac.len() != bc.len() {
        return Ok(ValueP::scalar_from(Cell::Int(1)));
    }
    let qct = Cell::DEFAULT_CT;
    for (ca, cb) in ac.iter().zip(bc.iter()) {
        if !ca.equal(cb, qct) {
            return Ok(ValueP::scalar_from(Cell::Int(1)));
        }
    }
    Ok(ValueP::scalar_from(Cell::Int(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_match_same() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        let r = not_match(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 0);
    }

    #[test]
    fn test_not_match_diff() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[1, 2, 4]);
        let r = not_match(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 1);
    }

    #[test]
    fn test_not_match_diff_shape() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[1, 2]);
        let r = not_match(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 1);
    }
}
