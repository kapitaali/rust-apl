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
    let n_us = n as usize;
    let fold_row = |row: usize| -> Result<Cell, ErrorCode> {
        // right-to-left fold: Z = B[0] f (B[1] f (... f B[n-1]))
        let base = row * n_us;
        let mut acc = cells[base + n_us - 1].clone();
        for k in (0..n_us - 1).rev() {
            acc = apply_prim(lo, &cells[base + k], &acc)?;
        }
        Ok(acc)
    };

    let out_len_us = out_len as usize;
    let out = if out_len_us >= crate::functions::PARALLEL_THRESHOLD {
        // rows are independent folds; fold DIRECTION stays sequential
        // right-to-left within each row
        use rayon::prelude::*;
        (0..out_len_us)
            .into_par_iter()
            .map(fold_row)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        (0..out_len_us)
            .map(fold_row)
            .collect::<Result<Vec<_>, _>>()?
    };

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
    let n_us = n as usize;

    // scan each row independently; parallelize over rows above threshold
    let scan_row = |row: usize| -> Result<Vec<Cell>, ErrorCode> {
        let base = row * n_us;
        let mut out = Vec::with_capacity(n_us);
        for k in 0..n_us {
            let mut acc = cells[base + k].clone();
            for j in (0..k).rev() {
                acc = apply_prim(lo, &cells[base + j], &acc)?;
            }
            out.push(acc);
        }
        Ok(out)
    };

    let row_results: Vec<Vec<Cell>> = if outer as usize >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        (0..outer as usize)
            .into_par_iter()
            .map(scan_row)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        (0..outer as usize)
            .map(scan_row)
            .collect::<Result<Vec<_>, _>>()?
    };

    let mut out = Vec::with_capacity(b.element_count() as usize);
    for row in row_results {
        out.extend(row);
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
        Prim::And => crate::cell::bif_and(a, b),
        Prim::Or => crate::cell::bif_or(a, b),
        // ,/ — reduce-catenate: not a scalar fold; unsupported here (the
        // parser rejects Comma in reduce position before reaching this arm)
        Prim::Comma => Err(ErrorCode::NonceError),
        Prim::Equal => crate::cell::bif_equal(a, b),
        Prim::NotEqual => crate::cell::bif_not_equal(a, b),
        Prim::Less => crate::cell::bif_less(a, b),
        Prim::LessEq => crate::cell::bif_less_eq(a, b),
        Prim::Greater => crate::cell::bif_greater(a, b),
        Prim::GreaterEq => crate::cell::bif_greater_eq(a, b),
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
    let out: Vec<Cell> = if inner as usize >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        (0..inner as usize)
            .into_par_iter()
            .map(|k| {
                let mut acc = cells[(m as usize - 1) * inner as usize + k].clone();
                for r in (0..m as usize - 1).rev() {
                    acc = apply_prim(lo, &cells[r * inner as usize + k], &acc)?;
                }
                Ok(acc)
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut out = Vec::with_capacity(inner as usize);
        for k in 0..inner as usize {
            let mut acc = cells[(m as usize - 1) * inner as usize + k].clone();
            for r in (0..m as usize - 1).rev() {
                acc = apply_prim(lo, &cells[r * inner as usize + k], &acc)?;
            }
            out.push(acc);
        }
        out
    };

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

    let scan_col = |k: usize| -> Result<Vec<(usize, Cell)>, ErrorCode> {
        let mut acc = cells[k].clone();
        let mut results = Vec::with_capacity(m as usize);
        results.push((k, acc.clone()));
        for r in 1..m as usize {
            acc = apply_prim(lo, &acc, &cells[r * inner as usize + k])?;
            results.push((r * inner as usize + k, acc.clone()));
        }
        Ok(results)
    };

    let col_results: Vec<Vec<(usize, Cell)>> =
        if inner as usize >= crate::functions::PARALLEL_THRESHOLD {
            use rayon::prelude::*;
            (0..inner as usize)
                .into_par_iter()
                .map(scan_col)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            (0..inner as usize)
                .map(scan_col)
                .collect::<Result<Vec<_>, _>>()?
        };

    for col in col_results {
        for (idx, cell) in col {
            out[idx] = cell;
        }
    }
    Ok(ValueP::from_ravel_like(b, out))
}

/// `F¨ B` — each: apply monadic F to every ravel element of B, nesting
/// each result in a scalar pointer cell. The result has B's shape.
pub fn each(lo: Prim, b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    let n = cells.len();

    let out_ravel: Vec<Cell> = if n >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        cells
            .par_iter()
            .map(|c| {
                let elem = ValueP {
                    inner: std::sync::Arc::new(crate::value::ValueInner::new(
                        Shape::scalar(),
                        vec![c.clone()],
                    )),
                };
                let result = lo.eval_monadic(&elem)?;
                Ok(Cell::Pointer(crate::cell::PointerCellData {
                    value: result.inner,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut out = Vec::with_capacity(n);
        for c in cells {
            let elem = ValueP {
                inner: std::sync::Arc::new(crate::value::ValueInner::new(
                    Shape::scalar(),
                    vec![c.clone()],
                )),
            };
            let result = lo.eval_monadic(&elem)?;
            out.push(Cell::Pointer(crate::cell::PointerCellData {
                value: result.inner,
            }));
        }
        out
    };

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

    let a_cells = a.cells();
    let b_cells = b.cells();

    let out_ravel: Vec<Cell> = if len as usize >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        (0..len as usize)
            .into_par_iter()
            .map(|i| {
                let ai = i % ac.max(1) as usize;
                let bi = i % bc.max(1) as usize;
                let ca = &a_cells[ai];
                let cb = &b_cells[bi];
                // If both cells are simple scalars, return result directly
                if !ca.is_pointer_cell() && !cb.is_pointer_cell() {
                    let result = crate::operators::apply_prim_pub(lo, ca, cb)?;
                    Ok(result)
                } else {
                    let ca_v = scalar_of_arr(&a_cells, ai);
                    let cb_v = scalar_of_arr(&b_cells, bi);
                    let result = crate::operators::apply_prim_pub(
                        lo,
                        ca_v.first_cell().unwrap(),
                        cb_v.first_cell().unwrap(),
                    )?;
                    Ok(Cell::Pointer(crate::cell::PointerCellData {
                        value: std::sync::Arc::new(crate::value::ValueInner::new(
                            Shape::scalar(),
                            vec![result],
                        )),
                    }))
                }
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut out_ravel = Vec::with_capacity(len as usize);
        for i in 0..len as usize {
            let ai = i % ac.max(1) as usize;
            let bi = i % bc.max(1) as usize;
            let ca = &a_cells[ai];
            let cb = &b_cells[bi];
            // If both cells are simple scalars, return result directly
            if !ca.is_pointer_cell() && !cb.is_pointer_cell() {
                let result = crate::operators::apply_prim_pub(lo, ca, cb)?;
                out_ravel.push(result);
            } else {
                let ca_v = scalar_of(a, ai);
                let cb_v = scalar_of(b, bi);
                let result = crate::operators::apply_prim_pub(
                    lo,
                    ca_v.first_cell().unwrap(),
                    cb_v.first_cell().unwrap(),
                )?;
                out_ravel.push(Cell::Pointer(crate::cell::PointerCellData {
                    value: std::sync::Arc::new(crate::value::ValueInner::new(
                        Shape::scalar(),
                        vec![result],
                    )),
                }));
            }
        }
        out_ravel
    };

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

/// `f¨ B` — each with a named function: apply f to each ravel element of B,
/// nesting each result. Binds the whole expression to its right.
pub fn each_name<F>(name: &str, b: &ValueP, mut f: F) -> AplResult<ValueP>
where
    F: FnMut(ValueP) -> AplResult<ValueP>,
{
    let cells = b.cells();
    let n = cells.len();

    let out_ravel: Vec<Cell> = (0..n)
        .map(|i| {
            let elem = scalar_of(b, i);
            let result = f(elem)?;
            Ok(Cell::Pointer(crate::cell::PointerCellData {
                value: result.inner,
            }))
        })
        .collect::<Result<Vec<_>, ErrorCode>>()?;

    Ok(ValueP::from_ravel_like(b, out_ravel))
}

/// `A f¨ B` — dyadic each with a named function: pair corresponding elements
/// of A and B (scalar extension allowed), apply dyadic f, nest each result.
pub fn each_dyad_name<F>(name: &str, a: &ValueP, b: &ValueP, mut f: F) -> AplResult<ValueP>
where
    F: FnMut(ValueP, ValueP) -> AplResult<ValueP>,
{
    let ac = a.element_count();
    let bc = b.element_count();
    let len = ac.max(bc);

    if ac != bc && ac != 1 && bc != 1 {
        return Err(ErrorCode::LengthError);
    }

    let a_cells = a.cells();
    let b_cells = b.cells();

    let out_ravel: Vec<Cell> = (0..len as usize)
        .map(|i| {
            let ai = i % ac.max(1) as usize;
            let bi = i % bc.max(1) as usize;
            let ca_v = scalar_of(a, ai);
            let cb_v = scalar_of(b, bi);
            let result = f(ca_v, cb_v)?;
            Ok(Cell::Pointer(crate::cell::PointerCellData {
                value: result.inner,
            }))
        })
        .collect::<Result<Vec<_>, ErrorCode>>()?;

    Ok(ValueP::from_ravel_like(
        if ac > 1 { a } else { b },
        out_ravel,
    ))
}

/// Like `scalar_of` but takes a raw cell slice (for parallel paths)
fn scalar_of_arr(cells: &[Cell], i: usize) -> ValueP {
    match &cells[i] {
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

    #[test]
    fn test_parallel_reduce_preserves_fold_direction() {
        // 5000 rows × 3 cols — above the parallel threshold.
        // Non-commutative − pins the fold direction per row:
        // row (a b c) must reduce as a - (b - c) = a-b+c.
        let rows = 5000i64;
        let mut data = Vec::new();
        for r in 0..rows {
            data.extend_from_slice(&[r, r + 1, r + 2]);
        }
        let b = reshape_matrix(data, rows, 3);
        let z = reduce(Prim::Subtract, &b).unwrap();
        assert_eq!(z.element_count(), rows);
        for r in [0i64, 1, 2499, rows - 1] {
            // (r) - ((r+1) - (r+2)) = r - r - 1 + r + 2 = r + 1
            assert_eq!(z.cells()[r as usize], Cell::Int(r + 1));
        }
        // same via + for value sanity
        let b2 = reshape_matrix((0..rows * 3).collect(), rows, 3);
        let z2 = reduce(Prim::Add, &b2).unwrap();
        assert_eq!(z2.cells()[0], Cell::Int(3));
        assert_eq!(
            z2.cells()[rows as usize - 1],
            Cell::Int(3 * (rows - 1) + 3 * (rows - 1) + 1 + 3 * (rows - 1) + 2)
        );
    }

    #[test]
    fn test_parallel_scan_preserves_values() {
        // 5000 rows × 3 cols — above the parallel threshold.
        // Non-commutative − pins scan direction per row.
        let rows = 5000i64;
        let mut data = Vec::new();
        for r in 0..rows {
            data.extend_from_slice(&[r, r + 1, r + 2]);
        }
        let b = reshape_matrix(data, rows, 3);
        let z = scan(Prim::Subtract, &b).unwrap();
        assert_eq!(z.element_count(), rows * 3);
        // scan direction: Z[0]=B[0], Z[1]=B[0]-B[1], Z[2]=B[0]-(B[1]-B[2])
        for r in [0i64, 1, 2499, rows - 1] {
            let base = r as usize * 3;
            let a = r;
            let b_val = r + 1;
            let c = r + 2;
            assert_eq!(z.cells()[base], Cell::Int(a));
            assert_eq!(z.cells()[base + 1], Cell::Int(a - b_val));
            assert_eq!(z.cells()[base + 2], Cell::Int(a - (b_val - c)));
        }
    }
}

// ---------------------------------------------------------------------------
// Tests for reduce_axis / scan_axis
// ---------------------------------------------------------------------------

#[cfg(test)]
mod axis_tests {
    use super::*;
    use crate::cell::Cell;
    use crate::shape::Shape;

    fn ints(v: &ValueP) -> Vec<i64> {
        v.cells()
            .iter()
            .map(|c| match c {
                Cell::Int(i) => *i,
                Cell::Float(f) => *f as i64,
                other => panic!("expected int, got {:?}", other),
            })
            .collect()
    }

    fn int_matrix(data: &[i64], rows: i64, cols: i64) -> ValueP {
        let shape = Shape::from_dims(&[rows, cols]).unwrap();
        let cells = data.iter().map(|&i| Cell::Int(i)).collect();
        ValueP::from_parts(shape, cells).unwrap()
    }

    #[test]
    fn test_reduce_axis_0_matrix() {
        // 2 3⍴⍳6 = [[0 1 2][3 4 5]]
        // +/[0] = reduce down columns → [0+3, 1+4, 2+5] = [3 5 7]
        let b = int_matrix(&[0, 1, 2, 3, 4, 5], 2, 3);
        let z = reduce_axis(Prim::Add, &b, 0).unwrap();
        assert_eq!(z.rank(), 1);
        assert_eq!(ints(&z), vec![3, 5, 7]);
    }

    #[test]
    fn test_reduce_axis_1_matrix() {
        // 2 3⍴⍳6 = [[0 1 2][3 4 5]]
        // +/[1] = reduce along rows → [0+1+2, 3+4+5] = [3 12]
        let b = int_matrix(&[0, 1, 2, 3, 4, 5], 2, 3);
        let z = reduce_axis(Prim::Add, &b, 1).unwrap();
        assert_eq!(z.rank(), 1);
        assert_eq!(ints(&z), vec![3, 12]);
    }

    #[test]
    fn test_reduce_axis_1_equals_last_axis_reduce() {
        // For a matrix, +/[1] should equal +/
        let b = int_matrix(&[0, 1, 2, 3, 4, 5], 2, 3);
        let z_axis = reduce_axis(Prim::Add, &b, 1).unwrap();
        let z_plain = reduce(Prim::Add, &b).unwrap();
        assert_eq!(ints(&z_axis), ints(&z_plain));
    }

    #[test]
    fn test_scan_axis_0_matrix() {
        // 2 3⍴⍳6 = [[0 1 2][3 4 5]]
        // +\[0] = scan down columns → [[0 1 2][3 5 7]]
        let b = int_matrix(&[0, 1, 2, 3, 4, 5], 2, 3);
        let z = scan_axis(Prim::Add, &b, 0).unwrap();
        assert_eq!(z.rank(), 2);
        assert_eq!(ints(&z), vec![0, 1, 2, 3, 5, 7]);
    }

    #[test]
    fn test_scan_axis_1_matrix() {
        // 2 3⍴⍳6 = [[0 1 2][3 4 5]]
        // +\[1] = scan along rows → [[0 1 3][3 7 12]]
        let b = int_matrix(&[0, 1, 2, 3, 4, 5], 2, 3);
        let z = scan_axis(Prim::Add, &b, 1).unwrap();
        assert_eq!(z.rank(), 2);
        assert_eq!(ints(&z), vec![0, 1, 3, 3, 7, 12]);
    }

    #[test]
    fn test_reduce_axis_3d() {
        // 2 2 3⍴⍳12
        // +/[0] reduces along axis 0 → shape (2,3)
        let data: Vec<i64> = (0..12).collect();
        let b = ValueP::from_parts(
            Shape::from_dims(&[2, 2, 3]).unwrap(),
            data.iter().map(|&i| Cell::Int(i)).collect(),
        )
        .unwrap();
        let z = reduce_axis(Prim::Add, &b, 0).unwrap();
        assert_eq!(z.rank(), 2);
        // axis 0: fold pairs (0,6),(1,7),(2,8),(3,9),(4,10),(5,11) → [6,8,10,12,14,16]
        assert_eq!(ints(&z), vec![6, 8, 10, 12, 14, 16]);
    }

    #[test]
    fn test_reduce_axis_scalar() {
        let b = ValueP::int_vector(&[42]);
        let z = reduce_axis(Prim::Add, &b, 0).unwrap();
        assert_eq!(ints(&z), vec![42]);
    }

    #[test]
    fn test_reduce_axis_bad_axis() {
        let b = int_matrix(&[0, 1, 2, 3, 4, 5], 2, 3);
        assert!(reduce_axis(Prim::Add, &b, 2).is_err());
        assert!(reduce_axis(Prim::Add, &b, -1).is_err());
    }
}

// ---------------------------------------------------------------------------
// Reduce / Scan with axis
// ---------------------------------------------------------------------------

/// `LO/[n] B` — reduce along axis n (0-based).
///
/// The axis is specified as a 0-based index. For a matrix (m, n):
/// - axis 0: reduce down columns → result shape (n)
/// - axis 1: reduce along rows → result shape (m)
pub fn reduce_axis(lo: Prim, b: &ValueP, axis: i64) -> AplResult<ValueP> {
    let rank = b.rank();
    if rank == 0 {
        return Ok(b.clone());
    }
    if axis < 0 || axis >= rank as i64 {
        return Err(ErrorCode::RankError);
    }
    let axis = axis as usize;
    let n = b.get_shape_item(axis as i16);
    let n_us = n as usize;

    // Result shape: same as B but without the reduced axis
    let dims: Vec<i64> = (0..rank as usize)
        .filter(|&k| k != axis)
        .map(|k| b.get_shape_item(k as i16))
        .collect();

    if n == 0 {
        return match identity(lo) {
            Some(cell) => {
                let shape = if dims.is_empty() {
                    Shape::scalar()
                } else {
                    Shape::from_dims(&dims)?
                };
                let out_len: i64 = dims.iter().product::<i64>().max(1);
                ValueP::from_parts(shape, vec![cell; out_len as usize])
            }
            None => Err(ErrorCode::DomainError),
        };
    }

    let cells = b.cells();
    let elem_count = b.element_count() as usize;

    // For axis a, the stride is the product of dimensions after it
    let stride: i64 = ((axis + 1)..rank as usize)
        .map(|k| b.get_shape_item(k as i16))
        .product::<i64>()
        .max(1);
    let stride_us = stride as usize;

    // The "pre" is the product of dimensions before axis a
    let pre: i64 = (0..axis)
        .map(|k| b.get_shape_item(k as i16))
        .product::<i64>()
        .max(1);
    let pre_us = pre as usize;

    // Number of independent lines along the axis = pre * stride
    let nlines = pre_us * stride_us;

    let fold_line = |line: usize| -> Result<Cell, ErrorCode> {
        let pre_idx = line / stride_us;
        let s = line % stride_us;
        let base = pre_idx * n_us * stride_us + s;

        // Fold right-to-left along the axis
        let mut acc = cells[base + (n_us - 1) * stride_us].clone();
        for kk in (0..n_us - 1).rev() {
            acc = apply_prim(lo, &cells[base + kk * stride_us], &acc)?;
        }
        Ok(acc)
    };

    let out: Vec<Cell> = if nlines >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        (0..nlines)
            .into_par_iter()
            .map(fold_line)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        (0..nlines).map(fold_line).collect::<Result<Vec<_>, _>>()?
    };

    let shape = if dims.is_empty() {
        Shape::scalar()
    } else {
        Shape::from_dims(&dims)?
    };
    ValueP::from_parts(shape, out)
}

/// `LO\[n] B` — scan along axis n (0-based).
pub fn scan_axis(lo: Prim, b: &ValueP, axis: i64) -> AplResult<ValueP> {
    let rank = b.rank();
    if rank == 0 {
        return Ok(b.clone());
    }
    if axis < 0 || axis >= rank as i64 {
        return Err(ErrorCode::RankError);
    }
    let axis = axis as usize;
    let n = b.get_shape_item(axis as i16);
    let n_us = n as usize;

    if n == 0 {
        return Ok(b.clone());
    }

    let cells = b.cells();
    let elem_count = b.element_count() as usize;
    let stride: i64 = ((axis + 1)..rank as usize)
        .map(|k| b.get_shape_item(k as i16))
        .product::<i64>()
        .max(1);
    let stride_us = stride as usize;
    let pre: i64 = (0..axis)
        .map(|k| b.get_shape_item(k as i16))
        .product::<i64>()
        .max(1);
    let pre_us = pre as usize;
    let nlines = pre_us * stride_us;

    let mut out = vec![Cell::Int(0); elem_count];

    for line in 0..nlines {
        let pre_idx = line / stride_us;
        let s = line % stride_us;
        let base = pre_idx * n_us * stride_us + s;

        for kk in 0..n_us {
            let idx = base + kk * stride_us;
            if kk == 0 {
                out[idx] = cells[idx].clone();
            } else {
                out[idx] = apply_prim(lo, &out[idx - stride_us], &cells[idx])?;
            }
        }
    }

    Ok(ValueP::from_ravel_like(b, out))
}
