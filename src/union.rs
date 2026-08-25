//! Set operations: union ∪ and intersection ∩.
//!
//! Mirrors `Bif_F12_UNION` and `Bif_F2_INTER` in C++ (simplified: rank ≤ 1).

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ∪B — unique: elements of B without duplicates (preserving first occurrence order).
pub fn unique(b: &ValueP) -> AplResult<ValueP> {
    if b.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    let cells = b.cells();
    if cells.len() <= 1 {
        return Ok(b.clone());
    }
    let qct = Cell::DEFAULT_CT;
    let mut out = Vec::new();
    for c in cells {
        let is_dup = out.iter().any(|o: &Cell| o.equal(c, qct));
        if !is_dup {
            out.push(c.clone());
        }
    }
    let shape = if out.is_empty() {
        Shape::vector(0)
    } else {
        Shape::vector(out.len() as i64)
    };
    ValueP::from_parts(shape, out)
}

/// A∪B — union: unique of A,B concatenated (symmetric, order-preserving).
pub fn union(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    if a.rank() > 1 || b.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    let mut combined = Vec::new();
    combined.extend_from_slice(a.cells());
    combined.extend_from_slice(b.cells());
    let shape = if combined.is_empty() {
        Shape::vector(0)
    } else {
        Shape::vector(combined.len() as i64)
    };
    let temp = ValueP::from_parts(shape, combined)?;
    unique(&temp)
}

/// A∩B — intersection: elements of A that are also in B (preserving A's order).
pub fn intersection(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    if a.rank() > 1 || b.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    let acells = a.cells();
    let bcells = b.cells();
    let qct = Cell::DEFAULT_CT;
    let mut out = Vec::new();
    for ca in acells {
        let found = bcells.iter().any(|cb| ca.equal(cb, qct));
        if found {
            out.push(ca.clone());
        }
    }
    let shape = if out.is_empty() {
        Shape::vector(0)
    } else {
        Shape::vector(out.len() as i64)
    };
    ValueP::from_parts(shape, out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_empty() {
        let v = ValueP::int_vector(&[]);
        let r = unique(&v).unwrap();
        assert_eq!(r.element_count(), 0);
    }

    #[test]
    fn test_unique_single() {
        let v = ValueP::int_vector(&[42]);
        let r = unique(&v).unwrap();
        assert_eq!(r.element_count(), 1);
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 42);
    }

    #[test]
    fn test_unique_no_dups() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let r = unique(&v).unwrap();
        assert_eq!(r.element_count(), 3);
    }

    #[test]
    fn test_unique_with_dups() {
        let v = ValueP::int_vector(&[1, 2, 1, 3, 2]);
        let r = unique(&v).unwrap();
        assert_eq!(r.element_count(), 3);
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3]);
    }

    #[test]
    fn test_union_basic() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[3, 4, 5]);
        let r = union(&a, &b).unwrap();
        assert_eq!(r.element_count(), 5);
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_union_with_dups() {
        let a = ValueP::int_vector(&[1, 1, 2]);
        let b = ValueP::int_vector(&[2, 2, 3]);
        let r = union(&a, &b).unwrap();
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3]);
    }

    #[test]
    fn test_intersection_basic() {
        let a = ValueP::int_vector(&[1, 2, 3, 4]);
        let b = ValueP::int_vector(&[2, 4, 6]);
        let r = intersection(&a, &b).unwrap();
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![2, 4]);
    }

    #[test]
    fn test_intersection_no_overlap() {
        let a = ValueP::int_vector(&[1, 2]);
        let b = ValueP::int_vector(&[3, 4]);
        let r = intersection(&a, &b).unwrap();
        assert_eq!(r.element_count(), 0);
    }
}
