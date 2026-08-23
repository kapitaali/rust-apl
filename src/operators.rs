//! Primitive operators: reduce `/` and scan `\`.
//!
//! Mirrors `src/Bif_OPER1_REDUCE.cc` and `src/Bif_OPER1_SCAN.cc`
//! (simplified: last-axis only, no n-wise, no axis specification yet).
//!
//! APL semantics for `LO/B` with B of shape `(m, n)`:
//!   result shape is `(m)`; each element folds LO over the corresponding
//!   row of B walking the axis **backward** (`B[k] LO B[k-1] ...`),
//!   which matters for non-commutative functions like `-`.

use crate::cell::Cell;
use crate::functions::Prim;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// `LO/B` — reduce along the last axis.
///
/// For a vector of length n → scalar. For a matrix (m, n) → vector (m).
/// Empty axis → identity element if the primitive has one, else error
/// (mirrors `NonscalarFunction_default_identity` handling in C++).
pub fn reduce(lo: Prim, b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    let n = if rank == 0 {
        1
    } else {
        b.get_shape_item(rank as i16 - 1) // last axis length
    };

    // result shape: all axes except the last
    let out_len = {
        let mut vol: i64 = 1;
        for a in 0..rank as usize - 1 {
            vol *= b.get_shape_item(a as i16);
        }
        vol.max(1)
    };

    if n == 0 {
        // empty reduction axis → identity value per primitive
        return match identity(lo) {
            Some(cell) => {
                let mut shape = *b.shape();
                if rank >= 1 {
                    // drop last axis; rank-0 result for vectors
                    let dims: Vec<i64> = (0..rank as usize - 1)
                        .map(|a| b.get_shape_item(a as i16))
                        .collect();
                    shape = Shape::from_dims(&dims)?;
                }
                ValueP::from_parts(shape, vec![cell])
            }
            None => Err(ErrorCode::DomainError),
        };
    }

    let cells = b.cells();
    let mut out = Vec::with_capacity(out_len as usize);

    for row in 0..out_len as usize {
        // right-to-left fold: Z = B[0] f (B[1] f (... f B[n-1]))
        let base = row * n as usize;
        let mut acc = cells[base + n as usize - 1].clone();
        for k in (0..n as usize - 1).rev() {
            acc = apply_prim(lo, &cells[base + k], &acc)?;
        }
        out.push(acc);
    }

    // build result shape: drop the last axis
    let shape = if rank <= 1 {
        Shape::scalar()
    } else {
        let dims: Vec<i64> = (0..rank as usize - 1)
            .map(|a| b.get_shape_item(a as i16))
            .collect();
        Shape::from_dims(&dims)?
    };
    ValueP::from_parts(shape, out)
}

/// `LO\B` — scan (prefix reduce) along the last axis.
///
/// For a vector of length n → vector of length n where
/// `Z[k] = B[k] LO B[k-1] LO ... LO B[0]` (backward fold prefixes,
/// matching APL's right-to-left scan direction). Result has same
/// shape as B.
pub fn scan(lo: Prim, b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    let n = if rank == 0 {
        1
    } else {
        b.get_shape_item(rank as i16 - 1)
    };
    let outer = b.element_count() / n.max(1);

    let cells = b.cells();
    let mut out = Vec::with_capacity(b.element_count() as usize);

    for row in 0..outer as usize {
        let base = row * n as usize;
        // APL scan = reduce over each prefix, with reduce folding
        // RIGHT-to-left: Z[k] = B[0] f (B[1] f (... f B[k])).
        // O(n²), matching semantics-first correctness.
        for k in 0..n as usize {
            let mut acc = cells[base + k].clone();
            for j in (0..k).rev() {
                acc = apply_prim(lo, &cells[base + j], &acc)?;
            }
            out.push(acc);
        }
    }

    Ok(ValueP::from_ravel_like(b, out))
}

/// Apply a dyadic primitive to two cells (shared by reduce/scan/each).
pub fn apply_prim_pub(lo: Prim, a: &Cell, b: &Cell) -> AplResult<Cell> {
    apply_prim(lo, a, b)
}

/// Apply a dyadic primitive to two cells (shared by reduce/scan).
fn apply_prim(lo: Prim, a: &Cell, b: &Cell) -> AplResult<Cell> {
    match lo {
        Prim::Add => crate::cell::bif_add(a, b),
        Prim::Subtract => crate::cell::bif_subtract(a, b),
        Prim::Multiply => crate::cell::bif_multiply(a, b),
        Prim::Divide => crate::cell::bif_divide(a, b),
        Prim::Ceiling => crate::cell::bif_maximum(a, b),
        Prim::Floor => crate::cell::bif_minimum(a, b),
        Prim::Magnitude => crate::cell::bif_residue(a, b),
        Prim::Power => crate::cell::bif_power(a, b),
        _ => Err(ErrorCode::SyntaxError),
    }
}

/// `F⌿B` — reduce along the FIRST axis.
///
/// For a matrix (m, n): result shape (n); Z[k] = B[0;k] f B[1;k] f ...
/// (right-to-left fold down each column). Vectors: same as last-axis reduce
/// (a vector's first axis IS its last axis).
pub fn reduce_first(lo: Prim, b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    if rank <= 1 {
        return reduce(lo, b);
    }
    let m = b.get_shape_item(0); // first axis length
    let inner = b.element_count() / m.max(1); // cells per "column slice"

    if m == 0 {
        return match identity(lo) {
            Some(cell) => {
                let dims: Vec<i64> = (1..rank as usize)
                    .map(|a| b.get_shape_item(a as i16))
                    .collect();
                let count: i64 = dims.iter().product::<i64>().max(1);
                ValueP::from_parts(Shape::from_dims(&dims)?, vec![cell; count as usize])
            }
            None => Err(ErrorCode::DomainError),
        };
    }

    let cells = b.cells();
    // result: inner cells, each folded down the column
    let mut out = Vec::with_capacity(inner as usize);
    for k in 0..inner as usize {
        // column elements are at k, k+inner, k+2*inner, ... (row-major)
        let mut acc = cells[(m as usize - 1) * inner as usize + k].clone();
        for r in (0..m as usize - 1).rev() {
            acc = apply_prim(lo, &cells[r * inner as usize + k], &acc)?;
        }
        out.push(acc);
    }

    // drop the first axis
    let dims: Vec<i64> = (1..rank as usize)
        .map(|a| b.get_shape_item(a as i16))
        .collect();
    ValueP::from_parts(Shape::from_dims(&dims)?, out)
}

/// `F⍀B` — scan along the FIRST axis (prefix scans down columns).
pub fn scan_first(lo: Prim, b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    if rank <= 1 {
        return scan(lo, b);
    }
    let m = b.get_shape_item(0);
    let inner = b.element_count() / m.max(1);
    let cells = b.cells();
    let mut out = vec![Cell::Int(0); b.element_count() as usize];

    // row-major output: out[r*inner + k] = cumulative scan down column k
    for k in 0..inner as usize {
        let mut acc = cells[k].clone();
        out[k] = acc.clone();
        for r in 1..m as usize {
            acc = apply_prim(lo, &acc, &cells[r * inner as usize + k])?;
            out[r * inner as usize + k] = acc.clone();
        }
    }
    Ok(ValueP::from_ravel_like(b, out))
}

/// `F¨ B` — each: apply monadic F to every ravel element of B, nesting
/// each result in a scalar pointer cell. The result has B's shape.
pub fn each(lo: Prim, b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    let mut out_ravel = Vec::with_capacity(cells.len());
    for c in cells {
        // wrap the cell as a scalar value and apply F through the
        // standard primitive dispatcher
        let elem = ValueP {
            inner: std::sync::Arc::new(crate::value::ValueInner::new(
                Shape::scalar(),
                vec![c.clone()],
            )),
        };
        let result = lo.eval_monadic(&elem)?;
        // nest the result
        out_ravel.push(Cell::Pointer(crate::cell::PointerCellData {
            value: result.inner,
        }));
    }
    Ok(ValueP::from_ravel_like(b, out_ravel))
}

/// `A F¨ B` — dyadic each: pair corresponding elements of A and B
/// (scalar extension allowed), apply dyadic F, nest each result.
/// Result shape follows A/B whichever is longer.
pub fn each_dyad(lo: Prim, a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let ac = a.element_count();
    let bc = b.element_count();
    let len = ac.max(bc);

    if ac != bc && ac != 1 && bc != 1 {
        return Err(ErrorCode::LengthError);
    }

    let mut out_ravel = Vec::with_capacity(len as usize);
    for i in 0..len as usize {
        let ca = scalar_of(a, i % ac.max(1) as usize);
        let cb = scalar_of(b, i % bc.max(1) as usize);
        // reuse the shared dyadic primitive dispatcher from reduce/scan
        let result = crate::operators::apply_prim_pub(
            lo,
            ca.first_cell().unwrap(),
            cb.first_cell().unwrap(),
        )?;
        out_ravel.push(Cell::Pointer(crate::cell::PointerCellData {
            value: std::sync::Arc::new(crate::value::ValueInner::new(
                Shape::scalar(),
                vec![result],
            )),
        }));
    }
    Ok(ValueP::from_ravel_like(
        if ac > 1 { a } else { b },
        out_ravel,
    ))
}

/// extract cell `i` as a scalar ValueP (wraps simple cells; discloses pointers)
fn scalar_of(v: &ValueP, i: usize) -> ValueP {
    match &v.cells()[i] {
        Cell::Pointer(p) => ValueP {
            inner: p.value.clone(),
        },
        other => ValueP {
            inner: std::sync::Arc::new(crate::value::ValueInner::new(
                Shape::scalar(),
                vec![other.clone()],
            )),
        },
    }
}

/// The identity element of a primitive under reduce over an empty axis
/// (mirrors `NonscalarFunction_default_identity` in C++).
fn identity(lo: Prim) -> Option<Cell> {
    match lo {
        Prim::Add => Some(Cell::Int(0)),
        Prim::Multiply => Some(Cell::Int(1)),
        _ => None,
    }
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
    fn test_reduce_plus_vector() {
        let b = ValueP::int_vector(&[1, 2, 3, 4]);
        let z = reduce(Prim::Add, &b).unwrap();
        assert!(z.is_scalar());
        assert_eq!(ints(&z), vec![10]);
    }

    #[test]
    fn test_reduce_minus_is_right_to_left() {
        // -/1 2 3 = 1-(2-3) = 2  (APL folds backward!)
        let b = ValueP::int_vector(&[1, 2, 3]);
        let z = reduce(Prim::Subtract, &b).unwrap();
        assert_eq!(ints(&z), vec![2]);
    }

    #[test]
    fn test_reduce_divide_right_to_left() {
        // ÷/6 3 2 = 6÷(3÷2) = 4
        let b = ValueP::int_vector(&[6, 3, 2]);
        let z = reduce(Prim::Divide, &b).unwrap();
        match z.first_cell().unwrap() {
            Cell::Float(f) => assert!((f - 4.0).abs() < 1e-13),
            Cell::Int(i) => assert_eq!(*i, 4),
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn test_reduce_matrix_rows() {
        // +/ of a 2×3 matrix sums each row
        let b = reshape_matrix(vec![1, 2, 3, 4, 5, 6], 2, 3);
        let z = reduce(Prim::Add, &b).unwrap();
        assert!(z.is_vector());
        assert_eq!(ints(&z), vec![6, 15]);
    }

    #[test]
    fn test_reduce_empty_axis_with_identity() {
        // +/⍳0 → 0 (identity of +)
        let b = ValueP::int_vector(&[]);
        let z = reduce(Prim::Add, &b).unwrap();
        assert!(z.is_scalar());
        assert_eq!(ints(&z), vec![0]);

        // ×/⍳0 → 1 (identity of ×)
        let z = reduce(Prim::Multiply, &b).unwrap();
        assert_eq!(ints(&z), vec![1]);
    }

    #[test]
    fn test_reduce_empty_axis_no_identity() {
        // -/⍳0 → DOMAIN ERROR (- has no identity)
        let b = ValueP::int_vector(&[]);
        assert!(reduce(Prim::Subtract, &b).is_err());
    }

    #[test]
    fn test_scan_plus() {
        // +\1 2 3 = 1 3 6
        let b = ValueP::int_vector(&[1, 2, 3]);
        let z = scan(Prim::Add, &b).unwrap();
        assert_eq!(ints(&z), vec![1, 3, 6]);
    }

    #[test]
    fn test_scan_minus_direction() {
        // -\1 2 3 : Z[k] = B[0]-(B[1]-(...-B[k]))
        // Z[0]=1, Z[1]=1-2=¯1, Z[2]=1-(2-3)=2
        let b = ValueP::int_vector(&[1, 2, 3]);
        let z = scan(Prim::Subtract, &b).unwrap();
        assert_eq!(ints(&z), vec![1, -1, 2]);
    }

    #[test]
    fn test_reduce_first_matrix_columns() {
        // +⌿ of a 2×3 matrix sums each COLUMN: rows [1 2 3; 4 5 6] → [5 7 9]
        let b = reshape_matrix(vec![1, 2, 3, 4, 5, 6], 2, 3);
        let z = reduce_first(Prim::Add, &b).unwrap();
        assert!(z.is_vector());
        assert_eq!(ints(&z), [5, 7, 9]);
    }

    #[test]
    fn test_reduce_first_vector_same_as_last() {
        let b = ValueP::int_vector(&[1, 2, 3, 4]);
        let z = reduce_first(Prim::Add, &b).unwrap();
        assert_eq!(ints(&z), [10]);
    }

    #[test]
    fn test_reduce_first_empty_with_identity() {
        // +⌿ 0 3⍴⍳6 → shape (3,) of zeros
        let b = reshape_matrix(vec![], 0, 3);
        let z = reduce_first(Prim::Add, &b).unwrap();
        assert_eq!(ints(&z), [0, 0, 0]);
    }

    #[test]
    fn test_reduce_first_empty_no_identity() {
        let b = reshape_matrix(vec![], 0, 3);
        assert!(reduce_first(Prim::Subtract, &b).is_err());
    }

    #[test]
    fn test_scan_first_columns() {
        // +⍀ of [1 2; 3 4] → [1 2; 4 6] (cumulative down columns)
        let b = reshape_matrix(vec![1, 2, 3, 4], 2, 2);
        let z = scan_first(Prim::Add, &b).unwrap();
        assert_eq!(ints(&z), [1, 2, 4, 6]);
    }

    /// build a matrix value directly from a ravel
    fn reshape_matrix(data: Vec<i64>, rows: i64, cols: i64) -> ValueP {
        let shape = Shape::matrix(rows, cols);
        ValueP::from_parts(shape, data.into_iter().map(Cell::Int).collect()).unwrap()
    }
}
