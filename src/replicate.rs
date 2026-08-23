//! Dyadic replicate `A/B` — compress (the dyadic use of `/`).
//!
//! Each B element is repeated A[i] times (scalar A extends to all of B).
//! This powers the classic guarded-branch idiom `→cond/line`, where an
//! empty result means "no branch".

use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

pub fn replicate(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let ac = a.element_count();
    let bc = b.element_count();
    if ac != 1 && ac != bc {
        return Err(ErrorCode::LengthError);
    }

    // counts per B element
    let counts: Vec<i64> = if ac == 1 {
        vec![a.first_cell().unwrap().get_near_int()?; bc as usize]
    } else {
        a.cells()
            .iter()
            .map(|c| c.get_near_int())
            .collect::<Result<_, _>>()?
    };

    let mut out = Vec::new();
    for (i, c) in b.cells().iter().enumerate() {
        for _ in 0..counts[i] {
            out.push(c.clone());
        }
    }

    Ok(ValueP {
        inner: std::sync::Arc::new(crate::value::ValueInner::new(
            Shape::vector(out.len() as i64),
            out,
        )),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    fn ints(v: &ValueP) -> Vec<i64> {
        v.cells()
            .iter()
            .map(|c| match c {
                Cell::Int(i) => *i,
                o => panic!("expected ints, got {:?}", o),
            })
            .collect()
    }

    #[test]
    fn test_replicate_basic() {
        // 1 2 3 / 10 20 30 → 10 20 20 30 30 30
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[10, 20, 30]);
        assert_eq!(ints(&replicate(&a, &b).unwrap()), [10, 20, 20, 30, 30, 30]);
    }

    #[test]
    fn test_replicate_zero_drops() {
        // 1 0 1 / 1 2 3 → 1 3
        let a = ValueP::int_vector(&[1, 0, 1]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(ints(&replicate(&a, &b).unwrap()), [1, 3]);
    }

    #[test]
    fn test_replicate_all_zero_is_empty() {
        let a = ValueP::int_vector(&[0]);
        let b = ValueP::int_vector(&[7, 8]);
        let z = replicate(&a, &b).unwrap();
        assert_eq!(z.element_count(), 0);
    }

    #[test]
    fn test_replicate_scalar_a_extends() {
        let a = ValueP::scalar_from(Cell::Int(2));
        let b = ValueP::int_vector(&[1, 2]);
        assert_eq!(ints(&replicate(&a, &b).unwrap()), [1, 1, 2, 2]);
    }

    #[test]
    fn test_replicate_length_error() {
        let a = ValueP::int_vector(&[1, 2]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert!(matches!(replicate(&a, &b), Err(ErrorCode::LengthError)));
    }
}
