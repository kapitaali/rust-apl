//! Interval index `A⍸B` (mirrors `Bif_F12_INTERVAL_INDEX` in C++).
//!
//! Classifies each element of B into intervals defined by the sorted array A.
//! If A has n elements, there are n+1 possible intervals:
//! - 0: B < A[0]
//! - 1: A[0] ≤ B < A[1]
//! - ...
//! - n: B ≥ A[n-1]
//!
//! Example: `1 2 3⍸1.5 2.5` → `1 2`

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::AplResult;
use crate::value::ValueP;

/// A⍸B — interval index: classify B into intervals of A.
pub fn interval_index(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let ac = a.cells();
    let bc = b.cells();
    let n = ac.len();
    let mut out = Vec::with_capacity(bc.len());

    for cb in bc.iter() {
        let val = cb.get_real_value()?;
        // Binary search for the interval
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let mid_val = ac[mid].get_real_value()?;
            if val < mid_val {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        out.push(Cell::Int(lo as i64));
    }

    let shape = if b.rank() == 0 {
        Shape::vector(1)
    } else {
        let mut dims = Vec::new();
        for i in 0..b.rank() as usize {
            dims.push(b.get_shape_item(i as i16));
        }
        Shape::from_dims(&dims)?
    };
    ValueP::from_parts(shape, out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &ValueP) -> Vec<i64> {
        v.cells()
            .iter()
            .map(|c| match c {
                Cell::Int(i) => *i,
                other => panic!("expected ints, got {:?}", other),
            })
            .collect()
    }

    #[test]
    fn test_interval_index_basic() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::from_parts(
            Shape::vector(2),
            vec![Cell::Float(1.5), Cell::Float(2.5)],
        ).unwrap();
        assert_eq!(ints(&interval_index(&a, &b).unwrap()), [1, 2]);
    }

    #[test]
    fn test_interval_index_boundaries() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::from_parts(
            Shape::vector(4),
            vec![Cell::Float(0.5), Cell::Float(1.5), Cell::Float(2.5), Cell::Float(3.5)],
        ).unwrap();
        assert_eq!(ints(&interval_index(&a, &b).unwrap()), [0, 1, 2, 3]);
    }

    #[test]
    fn test_interval_index_exact() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(ints(&interval_index(&a, &b).unwrap()), [1, 2, 3]);
    }

    #[test]
    fn test_interval_index_all_below() {
        let a = ValueP::int_vector(&[10, 20, 30]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(ints(&interval_index(&a, &b).unwrap()), [0, 0, 0]);
    }

    #[test]
    fn test_interval_index_all_above() {
        let a = ValueP::int_vector(&[10, 20, 30]);
        let b = ValueP::int_vector(&[40, 50, 60]);
        assert_eq!(ints(&interval_index(&a, &b).unwrap()), [3, 3, 3]);
    }
}
