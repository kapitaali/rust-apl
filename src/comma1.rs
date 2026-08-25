//! Table ⍪ and catenate-first-axis operations.
//!
//! Mirrors `Bif_F12_COMMA1` in C++ (simplified).

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⍪B — table: turn B into a matrix.
/// For a vector of length n → n×1 matrix.
/// For a scalar → 1×1 matrix.
/// For a matrix → unchanged (already 2D).
pub fn table(b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    if rank <= 1 {
        let n = b.element_count();
        let shape = Shape::matrix(n, 1);
        ValueP::from_parts(shape, b.cells().to_vec())
    } else {
        // already rank >= 2, return as-is
        Ok(b.clone())
    }
}

/// ≢B — tally: number of elements along the first axis.
/// For a vector of length n → n.
/// For a matrix (m, n) → m.
/// For a scalar → 1.
pub fn tally(b: &ValueP) -> AplResult<ValueP> {
    let n = if b.rank() == 0 {
        1
    } else {
        b.get_shape_item(0)
    };
    Ok(ValueP::scalar_from(Cell::Int(n)))
}

/// A⍪B — catenate along first axis.
/// For vectors: simple catenate.
/// For matrices: stack vertically (must have same column count).
pub fn catenate_first(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let ra = a.rank();
    let rb = b.rank();
    // Normalize: treat scalars as 1-element vectors for catenate
    if ra == 0 && rb == 0 {
        // both scalars → 2-element vector
        let mut cells = Vec::new();
        cells.extend_from_slice(a.cells());
        cells.extend_from_slice(b.cells());
        return ValueP::from_parts(Shape::vector(2), cells);
    }
    if ra == 0 {
        // A scalar, B vector → prepend A
        let mut cells = Vec::new();
        cells.extend_from_slice(a.cells());
        cells.extend_from_slice(b.cells());
        return ValueP::from_parts(Shape::vector(cells.len() as i64), cells);
    }
    if rb == 0 {
        // A vector, B scalar → append B
        let mut cells = Vec::new();
        cells.extend_from_slice(a.cells());
        cells.extend_from_slice(b.cells());
        return ValueP::from_parts(Shape::vector(cells.len() as i64), cells);
    }
    // Both rank >= 1: catenate along first axis
    if ra == 1 && rb == 1 {
        let mut cells = Vec::new();
        cells.extend_from_slice(a.cells());
        cells.extend_from_slice(b.cells());
        ValueP::from_parts(Shape::vector(cells.len() as i64), cells)
    } else if ra == 2 && rb == 2 {
        let ca = a.get_shape_item(1);
        let cb = b.get_shape_item(1);
        if ca != cb {
            return Err(ErrorCode::LengthError);
        }
        let rows = a.get_shape_item(0) + b.get_shape_item(0);
        let mut cells = Vec::new();
        cells.extend_from_slice(a.cells());
        cells.extend_from_slice(b.cells());
        ValueP::from_parts(Shape::matrix(rows, ca), cells)
    } else {
        Err(ErrorCode::RankError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_vector() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let r = table(&v).unwrap();
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 3);
        assert_eq!(r.get_shape_item(1), 1);
    }

    #[test]
    fn test_table_scalar() {
        let v = ValueP::scalar_from(Cell::Int(42));
        let r = table(&v).unwrap();
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 1);
        assert_eq!(r.get_shape_item(1), 1);
    }

    #[test]
    fn test_tally_vector() {
        let v = ValueP::int_vector(&[1, 2, 3, 4, 5]);
        let r = tally(&v).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 5);
    }

    #[test]
    fn test_tally_scalar() {
        let v = ValueP::scalar_from(Cell::Int(42));
        let r = tally(&v).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 1);
    }

    #[test]
    fn test_tally_matrix() {
        let shape = Shape::matrix(3, 4);
        let cells = vec![Cell::Int(0); 12];
        let v = ValueP::from_parts(shape, cells).unwrap();
        let r = tally(&v).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 3);
    }

    #[test]
    fn test_catenate_first_vectors() {
        let a = ValueP::int_vector(&[1, 2]);
        let b = ValueP::int_vector(&[3, 4, 5]);
        let r = catenate_first(&a, &b).unwrap();
        assert_eq!(r.element_count(), 5);
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_catenate_first_matrices() {
        let a = ValueP::from_parts(
            Shape::matrix(2, 3),
            vec![
                Cell::Int(1),
                Cell::Int(2),
                Cell::Int(3),
                Cell::Int(4),
                Cell::Int(5),
                Cell::Int(6),
            ],
        )
        .unwrap();
        let b = ValueP::from_parts(
            Shape::matrix(1, 3),
            vec![Cell::Int(7), Cell::Int(8), Cell::Int(9)],
        )
        .unwrap();
        let r = catenate_first(&a, &b).unwrap();
        assert_eq!(r.get_shape_item(0), 3);
        assert_eq!(r.get_shape_item(1), 3);
    }
}
