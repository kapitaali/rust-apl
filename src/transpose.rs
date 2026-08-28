//! Transpose `⍉` (mirrors `Bif_F12_TRANSPOSE` in `src/PrimitiveFunction.cc`).
//!
//! - `⍉B` REVERSES ALL AXES (so `⍴⍉2 3 4⍴⍳24` is `4 3 2`). For rank ≤ 1 it
//!   is the identity, and for rank 2 reversing all axes IS the familiar
//!   matrix transpose.
//! - `A⍉B` permutes axes: A[k] names the axis of B that supplies axis k of
//!   the result. A is in ⎕IO origin, so callers pass `io` from the
//!   environment. REPEATED axes are legal and select a DIAGONAL:
//!   `1 1⍉3 3⍴⍳9` is `1 5 9`.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// `⍉B` — monadic transpose: reverse the order of all axes.
pub fn transpose(b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank() as usize;
    if rank < 2 {
        return Ok(b.clone());
    }
    let dims: Vec<i64> = (0..rank).map(|a| b.get_shape_item(a as i16)).collect();
    // reversing all axes is the permutation [rank-1, rank-2, ..., 0]
    let perm: Vec<usize> = (0..rank).rev().collect();
    permute(b, &dims, &perm)
}

/// Shared output-cell walker for transpose (monadic and dyadic).
/// Each result cell is independent — embarrassingly parallel.
fn transpose_cells(
    cells: &[Cell],
    strides: &[i64],
    target: &[usize],
    out_dims: &[i64],
    total: usize,
) -> Vec<Cell> {
    if total >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        (0..total)
            .into_par_iter()
            .map(|idx| {
                // decode idx → out_rank coordinates
                let mut coord = vec![0i64; out_dims.len()];
                let mut rem = idx as i64;
                for ax in (0..out_dims.len()).rev() {
                    coord[ax] = rem % out_dims[ax];
                    rem /= out_dims[ax];
                }
                let mut src = 0i64;
                for (b_ax, &t) in target.iter().enumerate() {
                    src += coord[t] * strides[b_ax];
                }
                cells[src as usize].clone()
            })
            .collect()
    } else {
        let mut out = Vec::with_capacity(total);
        let mut coord = vec![0i64; out_dims.len()];
        for _ in 0..total {
            let mut src = 0i64;
            for (b_ax, &t) in target.iter().enumerate() {
                src += coord[t] * strides[b_ax];
            }
            out.push(cells[src as usize].clone());
            bump(&mut coord, out_dims);
        }
        out
    }
}

/// `A⍉B` — dyadic transpose (axis permutation, ⎕IO-relative).
///
/// `A[k]` says which axis of the RESULT the k-th axis of B maps onto. When a
/// value repeats, those axes of B are traversed together, which extracts a
/// diagonal and lowers the rank.
pub fn transpose_dyadic_io(a: &ValueP, b: &ValueP, io: i64) -> AplResult<ValueP> {
    let rank_b = b.rank() as usize;
    if a.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    let raw: Vec<i64> = a
        .cells()
        .iter()
        .map(|c| c.get_int_value())
        .collect::<Result<_, _>>()?;
    if raw.len() != rank_b {
        return Err(ErrorCode::LengthError);
    }
    // shift out of ⎕IO origin into 0-based target axis numbers
    let target: Vec<usize> = raw
        .iter()
        .map(|&v| {
            let z = v - io;
            if z < 0 || z as usize >= rank_b {
                Err(ErrorCode::DomainError)
            } else {
                Ok(z as usize)
            }
        })
        .collect::<AplResult<Vec<_>>>()?;

    let dims: Vec<i64> = (0..rank_b).map(|a| b.get_shape_item(a as i16)).collect();

    // The result rank is the number of DISTINCT target axes, and they must be
    // exactly 0..out_rank with no gaps (GNU APL: DOMAIN ERROR otherwise).
    let out_rank = target.iter().copied().max().map(|m| m + 1).unwrap_or(0);
    let mut used = vec![false; out_rank];
    for &t in &target {
        used[t] = true;
    }
    if used.iter().any(|&u| !u) {
        return Err(ErrorCode::DomainError);
    }

    // each result axis is as long as the SHORTEST B axis mapped onto it
    // (matters only for the diagonal case, where several axes share a target)
    let mut out_dims = vec![i64::MAX; out_rank];
    for (b_ax, &t) in target.iter().enumerate() {
        out_dims[t] = out_dims[t].min(dims[b_ax]);
    }

    let strides = row_major_strides(&dims);
    let cells = b.cells();
    let total: i64 = out_dims.iter().product();
    let out = transpose_cells(&cells, &strides, &target, &out_dims, total.max(0) as usize);
    ValueP::from_parts(Shape::from_dims(&out_dims)?, out)
}

/// Backwards-compatible entry: assumes ⎕IO=0.
pub fn transpose_dyadic(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    transpose_dyadic_io(a, b, 0)
}

// ---------------------------------------------------------------------------

/// reorder `b`'s axes so result axis k comes from B axis `perm[k]`
fn permute(b: &ValueP, dims: &[i64], perm: &[usize]) -> AplResult<ValueP> {
    let out_dims: Vec<i64> = perm.iter().map(|&p| dims[p]).collect();
    let strides = row_major_strides(dims);
    let cells = b.cells();
    let total: i64 = out_dims.iter().product();
    let out = transpose_cells(&cells, &strides, perm, &out_dims, total.max(0) as usize);
    ValueP::from_parts(Shape::from_dims(&out_dims)?, out)
}

fn row_major_strides(dims: &[i64]) -> Vec<i64> {
    let n = dims.len();
    let mut strides = vec![1i64; n];
    for i in (0..n.saturating_sub(1)).rev() {
        strides[i] = strides[i + 1] * dims[i + 1];
    }
    strides
}

/// increment a row-major coordinate vector in place
fn bump(coord: &mut [i64], dims: &[i64]) {
    for k in (0..coord.len()).rev() {
        coord[k] += 1;
        if coord[k] < dims[k] {
            return;
        }
        coord[k] = 0;
    }
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
                other => panic!("expected ints, got {:?}", other),
            })
            .collect()
    }

    #[test]
    fn test_transpose_matrix() {
        // ⍝ 2×3 matrix → 3×2
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let z = transpose(&b).unwrap();
        assert_eq!(z.get_shape_item(0), 3);
        assert_eq!(z.get_shape_item(1), 2);
        assert_eq!(ints(&z), [1, 4, 2, 5, 3, 6]);
    }

    #[test]
    fn test_transpose_twice_is_identity() {
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let z = transpose(&transpose(&b).unwrap()).unwrap();
        assert_eq!(ints(&z), [1, 2, 3, 4, 5, 6]);
        assert_eq!(z.get_shape_item(0), 2);
        assert_eq!(z.get_shape_item(1), 3);
    }

    #[test]
    fn test_transpose_vector_is_identity() {
        let b = ValueP::int_vector(&[1, 2, 3]);
        let z = transpose(&b).unwrap();
        assert_eq!(ints(&z), [1, 2, 3]);
        assert!(z.is_vector());
    }

    #[test]
    fn test_transpose_scalar_is_identity() {
        let b = ValueP::scalar_from(Cell::Int(7));
        let z = transpose(&b).unwrap();
        assert!(z.is_scalar());
        assert_eq!(ints(&z), [7]);
    }

    #[test]
    fn test_dyadic_transpose_identity_perm() {
        // (0 1)⍉M = M for a matrix
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let a = ValueP::int_vector(&[0, 1]);
        let z = transpose_dyadic(&a, &b).unwrap();
        assert_eq!(z.get_shape_item(0), 2);
        assert_eq!(z.get_shape_item(1), 3);
        assert_eq!(ints(&z), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_dyadic_transpose_swap() {
        // (1 0)⍉M = ⍉M
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let a = ValueP::int_vector(&[1, 0]);
        let z = transpose_dyadic(&a, &b).unwrap();
        assert_eq!(z.get_shape_item(0), 3);
        assert_eq!(z.get_shape_item(1), 2);
        assert_eq!(ints(&z), [1, 4, 2, 5, 3, 6]);
    }

    #[test]
    fn test_dyadic_transpose_cube_rotation() {
        // (1 2 0)⍉B with ⎕IO=0: B axis 0 → result axis 1, B axis 1 → result
        // axis 2, B axis 2 → result axis 0. For dims (2,3,2) the result is
        // (2,2,3). Verified against the reference: ⍴1 2 0⍉2 3 2⍴⍳12 → 2 2 3.
        let shape = Shape::cube(2, 3, 2);
        let b = ValueP::from_parts(shape, (0..12).map(|i| Cell::Int(i as i64)).collect()).unwrap();
        let a = ValueP::int_vector(&[1, 2, 0]);
        let z = transpose_dyadic(&a, &b).unwrap();
        assert_eq!(z.get_shape_item(0), 2);
        assert_eq!(z.get_shape_item(1), 2);
        assert_eq!(z.get_shape_item(2), 3);
        assert_eq!(z.element_count(), 12);
    }

    #[test]
    fn test_dyadic_transpose_repeated_axis_is_a_diagonal() {
        // (0 0)⍉B maps BOTH axes of B onto result axis 0, which walks them in
        // lockstep and extracts the DIAGONAL — it is not an error.
        // Reference: ,0 0⍉2 3⍴1 2 3 4 5 6 → 1 5 (⎕IO=0).
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let a = ValueP::int_vector(&[0, 0]);
        let z = transpose_dyadic(&a, &b).unwrap();
        assert_eq!(z.rank(), 1);
        // the diagonal length is the SHORTER of the two axes (2)
        assert_eq!(ints(&z), [1, 5]);
    }

    #[test]
    fn test_dyadic_transpose_square_diagonal() {
        // 1 1⍉3 3⍴⍳9 → 1 5 9 in ⎕IO=1; here 0 0⍉ with 0-based values
        let shape = Shape::cube(1, 3, 3);
        let _ = shape; // (cube unused; build the 3x3 directly)
        let b = ValueP::from_parts(
            Shape::matrix(3, 3),
            (1..=9).map(Cell::Int).collect::<Vec<_>>(),
        )
        .unwrap();
        let z = transpose_dyadic(&ValueP::int_vector(&[0, 0]), &b).unwrap();
        assert_eq!(ints(&z), [1, 5, 9]);
    }

    #[test]
    fn test_dyadic_transpose_missing_axis_error() {
        // (0)⍉B on a matrix — wrong length → RANK ERROR
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let a = ValueP::int_vector(&[0]);
        assert!(transpose_dyadic(&a, &b).is_err());
    }

    #[test]
    fn test_dyadic_transpose_axis_out_of_range() {
        // (0 5)⍉B — axis 5 doesn't exist → RANK ERROR
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let a = ValueP::int_vector(&[0, 5]);
        assert!(transpose_dyadic(&a, &b).is_err());
    }

    #[test]
    fn test_transpose_cube_reverses_all_axes() {
        // Monadic ⍉ REVERSES ALL AXES, it does not swap the first two.
        // Reference: ⍴⍉2 3 2⍴⍳12 → 2 3 2, and ⍴⍉2 3 4⍴⍳24 → 4 3 2.
        let shape = Shape::cube(2, 3, 2);
        let n = shape.get_volume();
        let b = ValueP::from_parts(shape, (0..n).map(Cell::Int).collect()).unwrap();
        let z = transpose(&b).unwrap();
        assert_eq!(z.get_shape_item(0), 2); // was dim 2
        assert_eq!(z.get_shape_item(1), 3); // was dim 1
        assert_eq!(z.get_shape_item(2), 2); // was dim 0
        assert_eq!(z.element_count(), n);
    }

    #[test]
    fn test_transpose_rank3_shape_matches_reference() {
        // ⍴⍉2 3 4⍴⍳24 → 4 3 2
        let shape = Shape::cube(2, 3, 4);
        let n = shape.get_volume();
        let b = ValueP::from_parts(shape, (0..n).map(Cell::Int).collect()).unwrap();
        let z = transpose(&b).unwrap();
        assert_eq!(z.get_shape_item(0), 4);
        assert_eq!(z.get_shape_item(1), 3);
        assert_eq!(z.get_shape_item(2), 2);
    }

    #[test]
    fn test_transpose_cube_ravel_matches_reference() {
        // ,⍉2 2 2⍴⍳8 → 1 5 3 7 2 6 4 8 (with ⍳ starting at 1)
        let b = ValueP::from_parts(
            Shape::cube(2, 2, 2),
            (1..=8).map(Cell::Int).collect::<Vec<_>>(),
        )
        .unwrap();
        let z = transpose(&b).unwrap();
        assert_eq!(ints(&z), [1, 5, 3, 7, 2, 6, 4, 8]);
    }

    #[test]
    fn test_transpose_matrix_element_mapping() {
        // Z[c;r] = B[r;c] — spot-check the 2×3 case where all-axis reversal
        // and the classic matrix transpose coincide.
        let b = ValueP::from_parts(
            Shape::matrix(2, 3),
            (1..=6).map(Cell::Int).collect::<Vec<_>>(),
        )
        .unwrap();
        let z = transpose(&b).unwrap();
        assert_eq!(z.get_shape_item(0), 3);
        assert_eq!(z.get_shape_item(1), 2);
        // B = 1 2 3 / 4 5 6  →  Z = 1 4 / 2 5 / 3 6
        assert_eq!(ints(&z), [1, 4, 2, 5, 3, 6]);
    }
}
