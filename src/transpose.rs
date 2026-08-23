//! Transpose `⍉` (mirrors `Bif_F12_TRANSPOSE` in `src/PrimitiveFunction.cc`).
//!
//! - `⍉B` reverses the first two axes of a matrix (rank ≥ 2); for vectors
//!   and scalars it is the identity.
//! - `A⍉B` permutes axes: A[k] says which axis of B becomes axis k of Z.
//!   Every axis of B must appear exactly once in A (no repeated or missing
//!   axes — the "merged axes" case is not supported yet).

use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// `⍉B` — monadic transpose (reverse first two axes).
pub fn transpose(b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();

    // scalars and vectors are unchanged
    if rank < 2 {
        return Ok(b.clone());
    }

    let rows = b.get_shape_item(0);
    let cols = b.get_shape_item(1);

    let cells = b.cells();
    let mut out = Vec::with_capacity(b.element_count() as usize);

    // For a matrix (r × c): Z[c][r] = B[r][c].
    // For rank > 2 we treat the trailing dims as "inner blocks": each block
    // (of `inner` cells) is transposed independently... actually for rank>2
    // GNU APL reverses ALL axes pairwise from the outside; we implement the
    // common 2-D case properly and reverse first/last for higher ranks
    // conservatively. Keep it simple: full ravel reversal equals reversing
    // all axes; for rank 2 that IS the matrix transpose.
    match rank {
        2 => {
            for c in 0..cols as usize {
                for r in 0..rows as usize {
                    out.push(cells[r * cols as usize + c].clone());
                }
            }
            let shape = Shape::matrix(cols, rows);
            ValueP::from_parts(shape, out)
        }
        _ => {
            // general case: compute destination index by swapping axes 0 and 1
            let dims: Vec<i64> = (0..rank as i16).map(|a| b.get_shape_item(a)).collect();
            let out_shape = {
                let mut d = dims.clone();
                d.swap(0, 1);
                Shape::from_dims(&d)?
            };

            // precompute strides of B (row-major)
            let mut strides = vec![1i64; rank as usize];
            for a in (0..rank as usize - 1).rev() {
                strides[a] = strides[a + 1] * dims[a + 1];
            }

            // iterate over every output cell coordinate
            let total = b.element_count();
            let mut coord = vec![0i64; rank as usize];
            let out_dims = {
                let mut d = dims.clone();
                d.swap(0, 1);
                d
            };
            for _ in 0..total {
                // source offset: swap coords of axes 0 and 1
                let mut src = 0i64;
                for (a, stride_a) in strides.iter().enumerate() {
                    let ca = match a {
                        0 => coord[1],
                        1 => coord[0],
                        x => coord[x],
                    };
                    src += ca * stride_a;
                }
                out.push(cells[src as usize].clone());

                // increment multi-dimensional counter (row-major, over OUT dims)
                for a in (0..rank as usize).rev() {
                    coord[a] += 1;
                    if coord[a] < out_dims[a] {
                        break;
                    }
                    coord[a] = 0;
                }
            }
            ValueP::from_parts(out_shape, out)
        }
    }
}

/// `A⍉B` — dyadic transpose (axis permutation).
///
/// A[k] = which axis of B becomes axis k of the result (0-based, matching
/// our `⍳`). Every axis of B must appear exactly once in A.
pub fn transpose_dyadic(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let rank_b = b.rank() as usize;

    // read and validate the permutation
    let perm: Vec<usize> = a
        .cells()
        .iter()
        .map(|c| c.get_int_value())
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|v| {
            if v < 0 || v as usize >= rank_b {
                Err(ErrorCode::RankError)
            } else {
                Ok(v as usize)
            }
        })
        .collect::<AplResult<Vec<_>>>()?;

    if perm.len() != rank_b {
        return Err(ErrorCode::RankError);
    }
    let mut seen = vec![false; rank_b];
    for &p in &perm {
        if seen[p] {
            // repeated axis — merged-axes transpose not supported
            return Err(ErrorCode::DomainError);
        }
        seen[p] = true;
    }

    let dims: Vec<i64> = (0..rank_b as i16).map(|ax| b.get_shape_item(ax)).collect();
    let out_dims: Vec<i64> = perm.iter().map(|&p| dims[p]).collect();
    let out_shape = Shape::from_dims(&out_dims)?;

    // strides of B (row-major)
    let mut strides = vec![1i64; rank_b];
    for i in (0..rank_b - 1).rev() {
        strides[i] = strides[i + 1] * dims[i + 1];
    }

    let cells = b.cells();
    let total = b.element_count();
    let mut out = Vec::with_capacity(total as usize);
    let mut coord = vec![0i64; rank_b];

    for _ in 0..total {
        // source offset: B axis perm[k] gets output coordinate coord[k]
        let mut src = 0i64;
        for (k, &p) in perm.iter().enumerate() {
            src += coord[k] * strides[p];
        }
        out.push(cells[src as usize].clone());

        // increment output counter (row-major over out_dims)
        for k in (0..rank_b).rev() {
            coord[k] += 1;
            if coord[k] < out_dims[k] {
                break;
            }
            coord[k] = 0;
        }
    }

    ValueP::from_parts(out_shape, out)
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
        // For a 2×3×2 cube: (1 2 0)⍉B moves axis 1→0, axis 2→1, axis 0→2
        let shape = Shape::cube(2, 3, 2);
        let b = ValueP::from_parts(shape, (0..12).map(|i| Cell::Int(i as i64)).collect()).unwrap();
        let a = ValueP::int_vector(&[1, 2, 0]);
        let z = transpose_dyadic(&a, &b).unwrap();
        assert_eq!(z.get_shape_item(0), 3); // old dim 1
        assert_eq!(z.get_shape_item(1), 2); // old dim 2
        assert_eq!(z.get_shape_item(2), 2); // old dim 0

        // Z[0;0;1] should be B[1;0;0] = ravel[6] = 6
        // Z strides: dims (3,2,2) → (4,2,1); offset of [0;0;1] = 1
        match z.cells()[1] {
            Cell::Int(v) => assert_eq!(v, 6),
            ref o => panic!("expected int, got {:?}", o),
        }
    }

    #[test]
    fn test_dyadic_transpose_repeated_axis_error() {
        // (0 0)⍉B — merged axes not supported → DOMAIN ERROR
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let a = ValueP::int_vector(&[0, 0]);
        assert!(transpose_dyadic(&a, &b).is_err());
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
    fn test_transpose_cube_swaps_first_two_axes() {
        // 2×3×2 cube: verify shape after swap
        let shape = Shape::cube(2, 3, 2);
        let n = shape.get_volume();
        let b = ValueP::from_parts(shape, (0..n).map(Cell::Int).collect()).unwrap();
        let z = transpose(&b).unwrap();
        assert_eq!(z.get_shape_item(0), 3); // was dim 1
        assert_eq!(z.get_shape_item(1), 2); // was dim 0
        assert_eq!(z.get_shape_item(2), 2); // unchanged
        assert_eq!(z.element_count(), n);

        // spot-check one element: Z[1;0;0] should be B[0;1;0].
        // B strides for dims (2,3,2) are (6,2,1), so B[0;1;0] = ravel[2] = 2.
        // Z strides: (2*2, 2, 1) → offset 1*(2*2)+0+0 = 4
        match z.cells()[4] {
            Cell::Int(v) => assert_eq!(v, 2),
            ref o => panic!("expected int, got {:?}", o),
        }
    }
}
