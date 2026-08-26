//! The rank operator `⍤` (mirrors `Bif_OPER2_RANK` in the C++).
//!
//! `(f⍤k)B` applies `f` to each rank-`k` CELL of B and reassembles the
//! results. The leading axes of B that are not part of a cell are the FRAME;
//! the result's shape is the frame followed by the shape common to every
//! result.
//!
//! Examples verified against the reference GNU APL binary:
//!
//! ```text
//! (⌽⍤1)2 3⍴⍳6      → 3 2 1 / 6 5 4     each ROW reversed
//! (≢⍤1)2 3⍴⍳6      → 3 3               one scalar per row
//! (≢⍤2)2 3⍴⍳6      → 2                 whole matrix is one cell
//! (≢⍤1)2 2 2⍴⍳8    → 2 2⍴2 2 2 2       frame 2 2, scalar per row
//! ```
//!
//! A rank at or above `≢⍴B` means the whole argument is a single cell; the
//! frame is then empty and the result is just `f B`.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// Apply `f` to each rank-`k` cell of `b` and reassemble.
///
/// `f` receives one cell at a time as a fully-formed value and may return any
/// shape, as long as every call agrees (LENGTH ERROR otherwise — matching
/// GNU APL, which cannot build a ragged result).
pub fn rank_monadic(
    b: &ValueP,
    k: i64,
    f: impl Fn(&ValueP) -> AplResult<ValueP>,
) -> AplResult<ValueP> {
    if k < 0 {
        return Err(ErrorCode::DomainError);
    }
    let rank = b.rank() as usize;
    let k = k as usize;

    // rank at or above the argument's rank → the whole array is one cell
    if k >= rank {
        return f(b);
    }

    let dims: Vec<i64> = (0..rank).map(|i| b.get_shape_item(i as i16)).collect();
    let frame_len = rank - k;
    let frame: Vec<i64> = dims[..frame_len].to_vec();
    let cell_dims: Vec<i64> = dims[frame_len..].to_vec();
    let cell_count: i64 = cell_dims.iter().product();
    let frame_count: i64 = frame.iter().product();

    let src = b.cells();
    let mut results: Vec<ValueP> = Vec::with_capacity(frame_count.max(0) as usize);
    for i in 0..frame_count {
        let from = (i * cell_count) as usize;
        let slice = src[from..from + cell_count as usize].to_vec();
        let cell_shape = if cell_dims.is_empty() {
            Shape::scalar()
        } else {
            Shape::from_dims(&cell_dims)?
        };
        let cell = ValueP::from_parts(cell_shape, slice)?;
        results.push(f(&cell)?);
    }

    assemble(&frame, results)
}

/// Dyadic rank: `A(f⍤kl kr)B` pairs the cells of A and B.
///
/// `kl` is the cell rank for the LEFT argument and `kr` for the right; the
/// single-rank spelling `f⍤k` passes the same value for both. Each argument is
/// split independently, then the frames must either match or one side must be
/// a single cell that is reused for every pairing (RANK ERROR otherwise, which
/// is what the reference gives for mismatched multi-cell frames).
///
/// Reference-verified for vectors A←1 2 3, B←4 5 6:
///
/// ```text
/// A(,⍤0 0)B → 1 4 2 5 3 6        shape 3 2   scalar cells pair up
/// A(,⍤0 1)B → 1 4 5 6 2 4 5 6 …  shape 3 4   each scalar with the whole B
/// A(,⍤1 0)B → 1 2 3 4 1 2 3 5 …  shape 3 4   whole A with each scalar
/// ```
pub fn rank_dyadic(
    a: &ValueP,
    b: &ValueP,
    kl: i64,
    kr: i64,
    f: impl Fn(&ValueP, &ValueP) -> AplResult<ValueP>,
) -> AplResult<ValueP> {
    if kl < 0 || kr < 0 {
        return Err(ErrorCode::DomainError);
    }
    let (a_frame, a_cells) = split_cells(a, kl)?;
    let (b_frame, b_cells) = split_cells(b, kr)?;

    // choose the frame: equal frames pair up; a single cell broadcasts
    let (frame, n) = if a_frame == b_frame {
        (a_frame.clone(), a_cells.len())
    } else if a_cells.len() == 1 {
        (b_frame.clone(), b_cells.len())
    } else if b_cells.len() == 1 {
        (a_frame.clone(), a_cells.len())
    } else {
        return Err(ErrorCode::RankError);
    };

    let mut results = Vec::with_capacity(n);
    for i in 0..n {
        let av = if a_cells.len() == 1 {
            &a_cells[0]
        } else {
            &a_cells[i]
        };
        let bv = if b_cells.len() == 1 {
            &b_cells[0]
        } else {
            &b_cells[i]
        };
        results.push(f(av, bv)?);
    }
    assemble(&frame, results)
}

/// Split `v` into its frame dimensions and the list of rank-`k` cells.
fn split_cells(v: &ValueP, k: i64) -> AplResult<(Vec<i64>, Vec<ValueP>)> {
    let rank = v.rank() as usize;
    let k = k as usize;
    if k >= rank {
        return Ok((Vec::new(), vec![v.clone()]));
    }
    let dims: Vec<i64> = (0..rank).map(|i| v.get_shape_item(i as i16)).collect();
    let frame_len = rank - k;
    let frame = dims[..frame_len].to_vec();
    let cell_dims = &dims[frame_len..];
    let cell_count: i64 = cell_dims.iter().product();
    let frame_count: i64 = frame.iter().product();

    let src = v.cells();
    let mut cells = Vec::with_capacity(frame_count.max(0) as usize);
    for i in 0..frame_count {
        let from = (i * cell_count) as usize;
        let slice = src[from..from + cell_count as usize].to_vec();
        let shape = if cell_dims.is_empty() {
            Shape::scalar()
        } else {
            Shape::from_dims(cell_dims)?
        };
        cells.push(ValueP::from_parts(shape, slice)?);
    }
    Ok((frame, cells))
}

/// Reassemble per-cell results under a common frame.
///
/// Every result must share one shape; the output is `frame , result_shape`.
/// An empty frame means there was a single cell, so its result passes through
/// unchanged (this is what makes `(≢⍤2)2 3⍴⍳6` a plain scalar).
fn assemble(frame: &[i64], results: Vec<ValueP>) -> AplResult<ValueP> {
    if frame.is_empty() {
        return results.into_iter().next().ok_or(ErrorCode::DomainError);
    }
    let first = results.first().ok_or(ErrorCode::DomainError)?;
    let item_dims: Vec<i64> = (0..first.rank())
        .map(|i| first.get_shape_item(i as i16))
        .collect();
    let item_len = first.element_count();

    let mut ravel: Vec<Cell> =
        Vec::with_capacity((results.len() as i64 * item_len).max(0) as usize);
    for r in &results {
        // all results must agree in shape — GNU APL cannot build a ragged array
        if r.element_count() != item_len {
            return Err(ErrorCode::LengthError);
        }
        let r_dims: Vec<i64> = (0..r.rank()).map(|i| r.get_shape_item(i as i16)).collect();
        if r_dims != item_dims {
            return Err(ErrorCode::LengthError);
        }
        ravel.extend(r.cells().iter().cloned());
    }

    let mut out_dims = frame.to_vec();
    out_dims.extend(item_dims);
    ValueP::from_parts(Shape::from_dims(&out_dims)?, ravel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ints(v: &ValueP) -> Vec<i64> {
        v.cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect()
    }

    fn mat(rows: i64, cols: i64, vals: &[i64]) -> ValueP {
        ValueP::from_parts(
            Shape::matrix(rows, cols),
            vals.iter().map(|&i| Cell::Int(i)).collect(),
        )
        .unwrap()
    }

    // All expectations verified against the reference C++ GNU APL binary.

    #[test]
    fn test_rank1_reverses_each_row() {
        // (⌽⍤1)2 3⍴⍳6 → 3 2 1 / 6 5 4
        let m = mat(2, 3, &[1, 2, 3, 4, 5, 6]);
        let r = rank_monadic(&m, 1, crate::rotate::reverse).unwrap();
        assert_eq!(ints(&r), vec![3, 2, 1, 6, 5, 4]);
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 2);
        assert_eq!(r.get_shape_item(1), 3);
    }

    #[test]
    fn test_rank_at_or_above_argument_rank_is_one_cell() {
        // (⌽⍤3) on a matrix treats the whole thing as one cell
        let m = mat(2, 3, &[1, 2, 3, 4, 5, 6]);
        let r = rank_monadic(&m, 3, crate::rotate::reverse).unwrap();
        let direct = crate::rotate::reverse(&m).unwrap();
        assert_eq!(ints(&r), ints(&direct));
    }

    #[test]
    fn test_rank1_tally_gives_one_scalar_per_row() {
        // (≢⍤1)2 3⍴⍳6 → 3 3
        let m = mat(2, 3, &[1, 2, 3, 4, 5, 6]);
        let r = rank_monadic(&m, 1, crate::comma1::tally).unwrap();
        assert_eq!(ints(&r), vec![3, 3]);
        assert_eq!(r.rank(), 1);
    }

    #[test]
    fn test_rank2_tally_of_matrix_is_a_scalar() {
        // (≢⍤2)2 3⍴⍳6 → 2 (whole matrix is one cell, empty frame)
        let m = mat(2, 3, &[1, 2, 3, 4, 5, 6]);
        let r = rank_monadic(&m, 2, crate::comma1::tally).unwrap();
        assert_eq!(ints(&r), vec![2]);
        assert_eq!(r.rank(), 0);
    }

    #[test]
    fn test_rank1_on_cube_frames_correctly() {
        // (≢⍤1)2 2 2⍴⍳8 → 2 2 matrix of 2s
        let c = ValueP::from_parts(
            Shape::cube(2, 2, 2),
            (1..=8).map(Cell::Int).collect::<Vec<_>>(),
        )
        .unwrap();
        let r = rank_monadic(&c, 1, crate::comma1::tally).unwrap();
        assert_eq!(ints(&r), vec![2, 2, 2, 2]);
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 2);
        assert_eq!(r.get_shape_item(1), 2);
    }

    #[test]
    fn test_rank1_reverse_on_cube() {
        // (⌽⍤1)2 2 2⍴⍳8 → 2 1 4 3 6 5 8 7
        let c = ValueP::from_parts(
            Shape::cube(2, 2, 2),
            (1..=8).map(Cell::Int).collect::<Vec<_>>(),
        )
        .unwrap();
        let r = rank_monadic(&c, 1, crate::rotate::reverse).unwrap();
        assert_eq!(ints(&r), vec![2, 1, 4, 3, 6, 5, 8, 7]);
        assert_eq!(r.rank(), 3);
    }

    #[test]
    fn test_rank1_on_vector_is_the_whole_vector() {
        // (⌽⍤1)1 2 3 → 3 2 1
        let v = ValueP::int_vector(&[1, 2, 3]);
        let r = rank_monadic(&v, 1, crate::rotate::reverse).unwrap();
        assert_eq!(ints(&r), vec![3, 2, 1]);
    }

    #[test]
    fn test_negative_rank_is_domain_error() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        assert!(rank_monadic(&v, -1, crate::rotate::reverse).is_err());
    }

    #[test]
    fn test_dyadic_rank_pairs_matching_frames() {
        // rows of A catenated with rows of B, per row
        let a = mat(2, 2, &[1, 2, 3, 4]);
        let b = mat(2, 2, &[5, 6, 7, 8]);
        let r = rank_dyadic(&a, &b, 1, 1, crate::comma::catenate).unwrap();
        // row 0: 1 2 , 5 6 ; row 1: 3 4 , 7 8
        assert_eq!(ints(&r), vec![1, 2, 5, 6, 3, 4, 7, 8]);
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(1), 4);
    }

    #[test]
    fn test_dyadic_rank_with_separate_left_right_ranks() {
        // A←1 2 3, B←4 5 6
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[4, 5, 6]);

        // f⍤0 0: scalar cells pair up
        let r = rank_dyadic(&a, &b, 0, 0, crate::comma::catenate).unwrap();
        assert_eq!(ints(&r), vec![1, 4, 2, 5, 3, 6]);
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 3);
        assert_eq!(r.get_shape_item(1), 2);

        // f⍤0 1: each scalar paired with the whole B
        let r = rank_dyadic(&a, &b, 0, 1, crate::comma::catenate).unwrap();
        assert_eq!(ints(&r), vec![1, 4, 5, 6, 2, 4, 5, 6, 3, 4, 5, 6]);
        assert_eq!(r.get_shape_item(0), 3);
        assert_eq!(r.get_shape_item(1), 4);

        // f⍤1 0: whole A paired with each scalar
        let r = rank_dyadic(&a, &b, 1, 0, crate::comma::catenate).unwrap();
        assert_eq!(ints(&r), vec![1, 2, 3, 4, 1, 2, 3, 5, 1, 2, 3, 6]);
        assert_eq!(r.get_shape_item(0), 3);
        assert_eq!(r.get_shape_item(1), 4);
    }

    #[test]
    fn test_dyadic_rank_broadcasts_a_single_cell() {
        let a = ValueP::int_vector(&[9, 9]);
        let b = mat(2, 2, &[1, 2, 3, 4]);
        let r = rank_dyadic(&a, &b, 1, 1, crate::comma::catenate).unwrap();
        assert_eq!(ints(&r), vec![9, 9, 1, 2, 9, 9, 3, 4]);
    }

    #[test]
    fn test_ragged_results_are_length_error() {
        // a function whose result length varies per cell cannot be assembled
        let m = mat(2, 3, &[1, 2, 3, 4, 5, 6]);
        let r = rank_monadic(&m, 1, |c| {
            // return 1 cell for the first row, 2 for the second
            let n = if c.cells()[0].get_int_value()? == 1 {
                1
            } else {
                2
            };
            ValueP::from_parts(Shape::vector(n), vec![Cell::Int(0); n as usize])
        });
        assert!(r.is_err());
    }
}
