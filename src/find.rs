//! Find ⍷ — locate occurrences of a subarray pattern within an array.
//!
//! Mirrors `Bif_F12_FIND` in C++. `A⍷B` returns a boolean array with the
//! SAME SHAPE as B, with a 1 at each position where an occurrence of A
//! begins. A is conformed to B's rank: a lower-rank A is treated as having
//! leading 1-length axes.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// A⍷B — find: 1 marks each position in B where A begins.
///
/// The result always has B's shape. When A has higher rank than B, or any
/// axis of A is longer than the corresponding axis of B, no match is
/// possible and the result is all zeros.
pub fn find(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let b_rank = b.rank() as usize;
    let b_dims: Vec<i64> = (0..b_rank).map(|i| b.get_shape_item(i as i16)).collect();
    let b_cells = b.cells();

    // result is always shaped like B
    let zeros = || -> AplResult<ValueP> {
        let out = vec![Cell::Int(0); b_cells.len()];
        rebuild(b, out)
    };

    // A scalar pattern matches each equal element of B
    if a.rank() == 0 {
        let pat = a.first_cell().ok_or(ErrorCode::DomainError)?;
        let out: Vec<Cell> = b_cells
            .iter()
            .map(|c| Cell::Int(cells_equal(pat, c) as i64))
            .collect();
        return rebuild(b, out);
    }

    let a_rank = a.rank() as usize;
    if a_rank > b_rank {
        return zeros();
    }

    // conform A to B's rank by prepending 1-length axes
    let mut a_dims = vec![1i64; b_rank - a_rank];
    for i in 0..a_rank {
        a_dims.push(a.get_shape_item(i as i16));
    }
    // an axis of A longer than B's cannot match anywhere
    if a_dims.iter().zip(&b_dims).any(|(&ad, &bd)| ad > bd) {
        return zeros();
    }
    // an empty pattern never matches
    if a_dims.contains(&0) {
        return zeros();
    }

    let a_cells = a.cells();
    let mut out = vec![Cell::Int(0); b_cells.len()];

    // walk every starting position in B
    for (start, slot) in out.iter_mut().enumerate() {
        let origin = decode(start as i64, &b_dims);
        // the pattern must fit within B from this origin
        if origin
            .iter()
            .zip(&a_dims)
            .zip(&b_dims)
            .any(|((&o, &ad), &bd)| o + ad > bd)
        {
            continue;
        }
        // compare every element of A against the corresponding element of B
        let mut matched = true;
        for (ai, ac) in a_cells.iter().enumerate() {
            let a_sub = decode(ai as i64, &a_dims);
            let b_sub: Vec<i64> = origin.iter().zip(&a_sub).map(|(&o, &s)| o + s).collect();
            let bi = encode(&b_sub, &b_dims);
            if !cells_equal(ac, &b_cells[bi as usize]) {
                matched = false;
                break;
            }
        }
        if matched {
            *slot = Cell::Int(1);
        }
    }

    rebuild(b, out)
}

/// build a result with the same shape as `model`
fn rebuild(model: &ValueP, ravel: Vec<Cell>) -> AplResult<ValueP> {
    let rank = model.rank();
    if rank == 0 {
        return Ok(ValueP::scalar_from(
            ravel.into_iter().next().unwrap_or(Cell::Int(0)),
        ));
    }
    let dims: Vec<i64> = (0..rank).map(|i| model.get_shape_item(i as i16)).collect();
    ValueP::from_parts(Shape::from_dims(&dims)?, ravel)
}

/// decode a linear (row-major) position into per-axis subscripts
fn decode(mut lin: i64, dims: &[i64]) -> Vec<i64> {
    let mut subs = vec![0i64; dims.len()];
    for ax in (0..dims.len()).rev() {
        let d = dims[ax].max(1);
        subs[ax] = lin % d;
        lin /= d;
    }
    subs
}

/// encode per-axis subscripts back into a linear (row-major) position
fn encode(subs: &[i64], dims: &[i64]) -> i64 {
    let mut lin = 0i64;
    for (ax, &s) in subs.iter().enumerate() {
        lin = lin * dims[ax].max(1) + s;
    }
    lin
}

/// element comparison for find: numbers compare across Int/Float, characters
/// compare by codepoint, and unlike types never match
fn cells_equal(x: &Cell, y: &Cell) -> bool {
    match (x, y) {
        (Cell::Char(a), Cell::Char(b)) => a == b,
        (Cell::Char(_), _) | (_, Cell::Char(_)) => false,
        _ => match (num_of(x), num_of(y)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        },
    }
}

fn num_of(c: &Cell) -> Option<f64> {
    match c {
        Cell::Int(i) => Some(*i as f64),
        Cell::Float(f) => Some(*f),
        _ => None,
    }
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

    fn char_vec(s: &str) -> ValueP {
        let cps: Vec<u32> = s.chars().map(|c| c as u32).collect();
        ValueP::char_vector(&cps)
    }

    #[test]
    fn test_find_scalar_in_vector() {
        // 2⍷1 2 3 2 → 0 1 0 1
        let a = ValueP::scalar_from(Cell::Int(2));
        let b = ValueP::int_vector(&[1, 2, 3, 2]);
        assert_eq!(ints(&find(&a, &b).unwrap()), vec![0, 1, 0, 1]);
    }

    #[test]
    fn test_find_subvector() {
        // 1 2⍷1 2 3 1 2 → 1 0 0 1 0
        let a = ValueP::int_vector(&[1, 2]);
        let b = ValueP::int_vector(&[1, 2, 3, 1, 2]);
        assert_eq!(ints(&find(&a, &b).unwrap()), vec![1, 0, 0, 1, 0]);
    }

    #[test]
    fn test_find_result_has_shape_of_b() {
        let a = ValueP::int_vector(&[1, 2]);
        let b = ValueP::int_vector(&[1, 2, 3, 1, 2]);
        let r = find(&a, &b).unwrap();
        assert_eq!(r.element_count(), b.element_count());
        assert_eq!(r.rank(), 1);
    }

    #[test]
    fn test_find_overlapping_occurrences() {
        // 1 1⍷1 1 1 → 1 1 0 (both origins that fit count)
        let a = ValueP::int_vector(&[1, 1]);
        let b = ValueP::int_vector(&[1, 1, 1]);
        assert_eq!(ints(&find(&a, &b).unwrap()), vec![1, 1, 0]);
    }

    #[test]
    fn test_find_no_match() {
        let a = ValueP::int_vector(&[9, 9]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(ints(&find(&a, &b).unwrap()), vec![0, 0, 0]);
    }

    #[test]
    fn test_find_pattern_longer_than_b_is_all_zeros() {
        let a = ValueP::int_vector(&[1, 2, 3, 4]);
        let b = ValueP::int_vector(&[1, 2]);
        assert_eq!(ints(&find(&a, &b).unwrap()), vec![0, 0]);
    }

    #[test]
    fn test_find_string_in_string() {
        // 'ab'⍷'xabyab' → 0 1 0 0 1 0
        let r = find(&char_vec("ab"), &char_vec("xabyab")).unwrap();
        assert_eq!(ints(&r), vec![0, 1, 0, 0, 1, 0]);
    }

    #[test]
    fn test_find_char_never_matches_number() {
        let r = find(&char_vec("1"), &ValueP::int_vector(&[1, 1])).unwrap();
        assert_eq!(ints(&r), vec![0, 0]);
    }

    #[test]
    fn test_find_int_float_compare_equal() {
        // 2⍷ vector holding 2.0 must match
        let a = ValueP::scalar_from(Cell::Int(2));
        let b =
            ValueP::from_parts(Shape::vector(2), vec![Cell::Float(2.0), Cell::Float(3.5)]).unwrap();
        assert_eq!(ints(&find(&a, &b).unwrap()), vec![1, 0]);
    }

    #[test]
    fn test_find_submatrix_in_matrix() {
        // pattern 2x2 of [[5 6] [8 9]] inside 3x3 ⍳9 reshaped:
        // 0 1 2 / 3 4 5 / 6 7 8  → looking for [[4 5] [7 8]] at origin (1,1)
        let a = ValueP::from_parts(
            Shape::matrix(2, 2),
            vec![Cell::Int(4), Cell::Int(5), Cell::Int(7), Cell::Int(8)],
        )
        .unwrap();
        let b = ValueP::from_parts(
            Shape::matrix(3, 3),
            (0..9).map(Cell::Int).collect::<Vec<_>>(),
        )
        .unwrap();
        let r = find(&a, &b).unwrap();
        // only origin (1,1) → linear index 4
        let mut expect = vec![0; 9];
        expect[4] = 1;
        assert_eq!(ints(&r), expect);
        assert_eq!(r.rank(), 2);
    }

    #[test]
    fn test_find_row_pattern_in_matrix_conforms_rank() {
        // a 2-element vector pattern against a matrix: conformed to 1x2, so it
        // matches horizontally within any row
        let a = ValueP::int_vector(&[3, 4]);
        let b = ValueP::from_parts(
            Shape::matrix(2, 3),
            vec![
                Cell::Int(1),
                Cell::Int(2),
                Cell::Int(3),
                Cell::Int(3),
                Cell::Int(4),
                Cell::Int(5),
            ],
        )
        .unwrap();
        let r = find(&a, &b).unwrap();
        // row 1 starts 3 4 → linear index 3
        let mut expect = vec![0; 6];
        expect[3] = 1;
        assert_eq!(ints(&r), expect);
    }
}
