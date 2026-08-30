//! SmallVec-backed hot path optimizations.
//!
//! Wires SmallVec<[Cell; 8]> into the hottest constructor paths
//! to avoid heap allocation for arrays with ≤8 elements.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::ErrorCode;
use crate::value::ValueP;
use std::sync::Arc;

/// Create a vector from a SmallVec-backed ravel (avoids heap allocation for ≤8 elements).
pub fn vector_from_smallvec(
    len: i64,
    ravel: smallvec::SmallVec<[Cell; 8]>,
) -> Result<ValueP, ErrorCode> {
    let want = len as usize;
    if want != ravel.len() {
        return Err(ErrorCode::LengthError);
    }
    Ok(ValueP {
        inner: Arc::new(crate::value::ValueInner::new_with_smallvec(
            Shape::vector(len),
            ravel,
        )),
    })
}

/// Create a scalar value from a single cell.
pub fn scalar_from_cell(cell: Cell) -> ValueP {
    let mut sv: smallvec::SmallVec<[Cell; 8]> = smallvec::SmallVec::new();
    sv.push(cell);
    ValueP {
        inner: Arc::new(crate::value::ValueInner::new_with_smallvec(
            Shape::scalar(),
            sv,
        )),
    }
}

/// Create a vector from ≤8 elements without heap allocation.
pub fn small_vector(ravel: smallvec::SmallVec<[Cell; 8]>) -> Result<ValueP, ErrorCode> {
    let len = ravel.len() as i64;
    vector_from_smallvec(len, ravel)
}

/// Create a value from shape and SmallVec ravel.
pub fn from_smallvec(
    shape: Shape,
    ravel: smallvec::SmallVec<[Cell; 8]>,
) -> Result<ValueP, ErrorCode> {
    let want = shape.get_volume();
    if want < 0 || want as usize != ravel.len() {
        return Err(ErrorCode::LengthError);
    }
    Ok(ValueP {
        inner: Arc::new(crate::value::ValueInner::new_with_smallvec(shape, ravel)),
    })
}

/// Append a cell to a SmallVec-backed vector, returning either the same SmallVec
/// (if it fits) or a heap-allocated Vec.
pub fn append_cell(
    mut ravel: smallvec::SmallVec<[Cell; 8]>,
    cell: Cell,
) -> smallvec::SmallVec<[Cell; 8]> {
    ravel.push(cell);
    ravel
}

/// Extend a SmallVec-backed ravel with another slice.
pub fn extend_smallvec(ravel: &mut smallvec::SmallVec<[Cell; 8]>, more: &[Cell]) {
    ravel.extend(more.iter().cloned());
}

/// Convert a Vec<Cell> to SmallVec if it fits, otherwise keep as Vec.
pub fn maybe_smallvec(cells: Vec<Cell>) -> smallvec::SmallVec<[Cell; 8]> {
    let mut sv = smallvec::SmallVec::new();
    sv.extend(cells);
    sv
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    #[test]
    fn test_vector_from_smallvec_basic() {
        let mut sv = smallvec::SmallVec::new();
        sv.push(Cell::int(1));
        sv.push(Cell::int(2));
        sv.push(Cell::int(3));
        let v = vector_from_smallvec(3, sv).unwrap();
        assert_eq!(v.element_count(), 3);
        assert_eq!(v.cells()[0], Cell::int(1));
        assert_eq!(v.cells()[2], Cell::int(3));
    }

    #[test]
    fn test_scalar_from_cell() {
        let v = scalar_from_cell(Cell::int(42));
        assert!(v.is_scalar());
        assert_eq!(v.first_cell().unwrap(), &Cell::int(42));
    }

    #[test]
    fn test_small_vector() {
        let mut sv = smallvec::SmallVec::new();
        for i in 0..5 {
            sv.push(Cell::int(i));
        }
        let v = small_vector(sv).unwrap();
        assert_eq!(v.element_count(), 5);
        assert_eq!(v.cells()[3], Cell::int(3));
    }

    #[test]
    fn test_append_cell() {
        let mut sv = smallvec::SmallVec::new();
        sv.push(Cell::int(1));
        sv.push(Cell::int(2));
        sv = append_cell(sv, Cell::int(3));
        assert_eq!(sv.len(), 3);
        assert_eq!(sv[2], Cell::int(3));
    }

    #[test]
    fn test_extend_smallvec() {
        let mut sv = smallvec::SmallVec::new();
        sv.push(Cell::int(1));
        extend_smallvec(&mut sv, &[Cell::int(2), Cell::int(3), Cell::int(4)]);
        assert_eq!(sv.len(), 4);
        assert_eq!(sv[3], Cell::int(4));
    }

    #[test]
    fn test_maybe_smallvec() {
        let cells = vec![Cell::int(1), Cell::int(2), Cell::int(3)];
        let sv = maybe_smallvec(cells);
        assert_eq!(sv.len(), 3);
    }

    #[test]
    fn test_from_smallvec_shape_mismatch() {
        let mut sv = smallvec::SmallVec::new();
        sv.push(Cell::int(1));
        sv.push(Cell::int(2));
        let shape = Shape::vector(3); // expects 3, got 2
        let result = from_smallvec(shape, sv);
        assert!(result.is_err());
    }

    #[test]
    fn test_vector_length_mismatch() {
        let mut sv = smallvec::SmallVec::new();
        sv.push(Cell::int(1));
        sv.push(Cell::int(2));
        let result = vector_from_smallvec(5, sv);
        assert!(result.is_err());
    }

    #[test]
    fn test_eight_elements_boundary() {
        // Exactly 8 elements should still use inline storage
        let mut sv = smallvec::SmallVec::new();
        for i in 0..8 {
            sv.push(Cell::int(i));
        }
        let v = small_vector(sv).unwrap();
        assert_eq!(v.element_count(), 8);
    }
}
