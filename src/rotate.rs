//! Rotate `A⌽B` and Reverse `⌽B` (mirrors `src/PrimitiveFunction.cc`
//! `Bif_ROTATE::rotate()` / `Bif_ROTATE::reverse()`, last-axis only).
//!
//! - `⌽B` reverses B along the last axis (scalars unchanged)
//! - `A⌽B` rotates by A positions: positive = left, negative = right.
//!   Scalar A is a global shift; vector A shifts each row individually.

use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// `⌽B` — reverse along the last axis.
pub fn reverse(b: &ValueP) -> AplResult<ValueP> {
    if b.is_scalar() {
        return Ok(b.clone());
    }
    let n = axis_len(b);
    let outer = b.element_count() / n.max(1);
    let cells = b.cells();

    let mut out = Vec::with_capacity(b.element_count() as usize);
    for row in 0..outer as usize {
        let base = row * n as usize;
        for k in (0..n as usize).rev() {
            out.push(cells[base + k].clone());
        }
    }
    Ok(ValueP::from_ravel_like(b, out))
}

/// `A⌽B` — rotate along the last axis.
///
/// A scalar or single-element A rotates every row by the same amount;
/// a full-shape A (shape of B minus last axis) gives per-row amounts.
pub fn rotate(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let n = axis_len(b);
    let rank = b.rank();
    let outer = b.element_count() / n.max(1);

    // global shift or per-row shifts?
    let global_shift: Option<i64> = if a.element_count() == 1 {
        Some(
            a.first_cell()
                .and_then(|c| c.get_int_value().ok())
                .ok_or(ErrorCode::DomainError)?,
        )
    } else {
        // per-row: shape must match B without the last axis
        let want_rank = rank.saturating_sub(1);
        if a.rank() != want_rank || a.element_count() != outer {
            return Err(ErrorCode::RankError);
        }
        None
    };

    // collect per-row shift values
    let shifts: Vec<i64> = match global_shift {
        Some(g) => vec![g; outer as usize],
        None => a
            .cells()
            .iter()
            .map(|c| c.get_int_value())
            .collect::<Result<Vec<_>, _>>()?,
    };

    let cells = b.cells();
    let mut out = Vec::with_capacity(b.element_count() as usize);
    for (row, shift_row) in shifts.iter().enumerate().take(outer as usize) {
        let base = row * n as usize;
        // C++: src = shift + m + n; normalized into [0, n) → positive
        // shift moves left (element at index m comes from src index m+shift).
        for m in 0..n as usize {
            let mut src = shift_row + m as i64;
            while src < 0 {
                src += n;
            }
            while src >= n {
                src -= n;
            }
            out.push(cells[base + src as usize].clone());
        }
    }

    Ok(ValueP::from_ravel_like(b, out))
}

fn axis_len(b: &ValueP) -> i64 {
    let rank = b.rank();
    if rank == 0 {
        1
    } else {
        b.get_shape_item(rank as i16 - 1)
    }
}

/// `A⌽[a]B` — rotate along axis `a` (0-based). Same shift semantics as the
/// last-axis version, applied to lines along axis a.
pub fn rotate_axis(a: &ValueP, b: &ValueP, axis: i64) -> AplResult<ValueP> {
    let rank = b.rank();
    if axis < 0 || axis >= rank as i64 {
        return Err(ErrorCode::RankError);
    }
    let n = b.get_shape_item(axis as i16);
    let pre: i64 = (0..axis).map(|k| b.get_shape_item(k as i16)).product();
    let post: i64 = ((axis + 1)..rank as i64)
        .map(|k| b.get_shape_item(k as i16))
        .product();

    // global shift or one per line?
    let nlines = pre * post;
    let global: Option<i64> = if a.element_count() == 1 {
        Some(
            a.first_cell()
                .and_then(|c| c.get_int_value().ok())
                .ok_or(ErrorCode::DomainError)?,
        )
    } else {
        None
    };
    let shifts: Vec<i64> = match global {
        Some(g) => vec![g; nlines as usize],
        None => a
            .cells()
            .iter()
            .map(|c| c.get_int_value())
            .collect::<Result<Vec<_>, _>>()?,
    };

    let cells = b.cells();
    let mut out = vec![Cell::Int(0); b.element_count() as usize];
    let mut si = 0usize; // shift index, walks lines in row-major line order
    for p in 0..pre.max(1) as usize {
        for s in 0..post.max(1) as usize {
            let shift = shifts[si % shifts.len()];
            si += 1;
            for k in 0..n as usize {
                let dst = (p * n as usize + k) * post as usize + s;
                let mut src_k = k as i64 - shift;
                while src_k < 0 {
                    src_k += n;
                }
                while src_k >= n {
                    src_k -= n;
                }
                out[dst] = cells[(p * n as usize + src_k as usize) * post as usize + s].clone();
            }
        }
    }

    Ok(ValueP::from_ravel_like(b, out))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;
    use crate::shape::Shape;

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
    fn test_reverse_vector() {
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(ints(&reverse(&b).unwrap()), [3, 2, 1]);
    }

    #[test]
    fn test_reverse_scalar_is_identity() {
        let b = ValueP::scalar_from(Cell::Int(7));
        assert!(reverse(&b).unwrap().is_scalar());
        assert_eq!(ints(&reverse(&b).unwrap()), [7]);
    }

    #[test]
    fn test_reverse_matrix_rows() {
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        assert_eq!(ints(&reverse(&b).unwrap()), [3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn test_rotate_positive_left() {
        // 1⌽1 2 3 = 2 3 1
        let b = ValueP::int_vector(&[1, 2, 3]);
        let a = ValueP::int_vector(&[1]);
        assert_eq!(ints(&rotate(&a, &b).unwrap()), [2, 3, 1]);
    }

    #[test]
    fn test_rotate_negative_right() {
        // ¯1⌽1 2 3 = 3 1 2
        let b = ValueP::int_vector(&[1, 2, 3]);
        let a = ValueP::int_vector(&[-1]);
        assert_eq!(ints(&rotate(&a, &b).unwrap()), [3, 1, 2]);
    }

    #[test]
    fn test_rotate_by_n_is_identity() {
        let b = ValueP::int_vector(&[1, 2, 3]);
        let a = ValueP::int_vector(&[3]);
        assert_eq!(ints(&rotate(&a, &b).unwrap()), [1, 2, 3]);
        let a = ValueP::int_vector(&[-3]);
        assert_eq!(ints(&rotate(&a, &b).unwrap()), [1, 2, 3]);
    }

    #[test]
    fn test_rotate_per_row() {
        // 1 0⌽ matrix rotates row 0 left, row 1 not at all
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let a = ValueP::int_vector(&[1, 0]);
        assert_eq!(ints(&rotate(&a, &b).unwrap()), [2, 3, 1, 4, 5, 6]);
    }

    #[test]
    fn test_rotate_per_row_shape_mismatch() {
        let b = ValueP::int_vector(&[1, 2, 3]);
        let a = ValueP::int_vector(&[1, 0]); // wrong length for a vector B
        assert!(rotate(&a, &b).is_err());
    }
}
