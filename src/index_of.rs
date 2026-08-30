//! Dyadic `⍳` — index of (mirrors `src/Bif_F12_INDEX_OF.cc`).
//!
//! `A⍳B` returns, for every ravel element of B, the index of its first
//! occurrence in A (0-based, matching our monadic `⍳`), or `len(A)` when
//! not found (the classic "1 + last index" convention).
//!
//! The C++ adds a binary-search path for large A; we use a simple linear
//! scan with tolerant equality via ⎕CT (correctness first).

use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

pub fn index_of(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    index_of_io(a, b, 0)
}

/// ⎕IO-aware variant: results are shifted by io (real APL returns 1-based
/// positions under ⎕IO=1; "not found" becomes len(A)+io).
pub fn index_of_io(a: &ValueP, b: &ValueP, io: i64) -> AplResult<ValueP> {
    if a.element_count() == 0 {
        // empty A: everything is "not found" → all len_A + io = io
        let n = b.element_count();
        return Ok(ValueP::from_ravel_like(b, vec![Cell::Int(io); n as usize]));
    }

    let cells_a = a.cells();
    let len_a = cells_a.len() as i64;
    let use_parallel = b.element_count() as usize >= crate::functions::PARALLEL_THRESHOLD;

    let out: Vec<Cell> = if use_parallel {
        use rayon::prelude::*;
        b.cells()
            .par_iter()
            .map(|cb| {
                let mut found = len_a + io;
                for (i, ca) in cells_a.iter().enumerate() {
                    if cell_eq(ca, cb) {
                        found = i as i64 + io;
                        break;
                    }
                }
                Ok(Cell::Int(found))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut out = Vec::with_capacity(b.element_count() as usize);
        for cb in b.cells() {
            let mut found = len_a + io;
            for (i, ca) in cells_a.iter().enumerate() {
                if cell_eq(ca, cb) {
                    found = i as i64 + io;
                    break;
                }
            }
            out.push(Cell::Int(found));
        }
        out
    };

    // result shape follows B; ravel length == volume holds by construction
    ValueP::from_parts(*b.shape(), out).map_err(|_| ErrorCode::LengthError)
}

/// tolerant equality wrapper (uses Cell::equal with default ⎕CT)
fn cell_eq(a: &Cell, b: &Cell) -> bool {
    a.equal(b, Cell::DEFAULT_CT)
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
    fn test_index_of_basic() {
        // 10 20 30 ⍳ 20 → 1
        let a = ValueP::int_vector(&[10, 20, 30]);
        let b = ValueP::int_vector(&[20]);
        assert_eq!(ints(&index_of(&a, &b).unwrap()), [1]);
    }

    #[test]
    fn test_index_of_first_occurrence() {
        // 5 1 5 2 ⍳ 5 → 0 (first occurrence wins)
        let a = ValueP::int_vector(&[5, 1, 5, 2]);
        let b = ValueP::int_vector(&[5]);
        assert_eq!(ints(&index_of(&a, &b).unwrap()), [0]);
    }

    #[test]
    fn test_index_of_not_found() {
        // 10 20 30 ⍳ 99 → 3 (= len A)
        let a = ValueP::int_vector(&[10, 20, 30]);
        let b = ValueP::int_vector(&[99]);
        assert_eq!(ints(&index_of(&a, &b).unwrap()), [3]);
    }

    #[test]
    fn test_index_of_vector_b() {
        // 10 20 30 ⍳ 30 10 99 → 2 0 3
        let a = ValueP::int_vector(&[10, 20, 30]);
        let b = ValueP::int_vector(&[30, 10, 99]);
        assert_eq!(ints(&index_of(&a, &b).unwrap()), [2, 0, 3]);
    }

    #[test]
    fn test_index_of_result_has_shape_of_b() {
        // result shape follows B's shape
        let a = ValueP::int_vector(&[1, 2, 3]);
        let shape = crate::shape::Shape::matrix(2, 2);
        let b = ValueP::from_parts(
            shape,
            vec![Cell::Int(1), Cell::Int(9), Cell::Int(3), Cell::Int(7)],
        )
        .unwrap();
        let z = index_of(&a, &b).unwrap();
        assert_eq!(z.get_shape_item(0), 2);
        assert_eq!(z.get_shape_item(1), 2);
        assert_eq!(ints(&z), [0, 3, 2, 3]);
    }

    #[test]
    fn test_index_of_chars() {
        let a = ValueP::char_vector(&['a' as u32, 'b' as u32, 'c' as u32]);
        let b = ValueP::char_vector(&['b' as u32, 'z' as u32]);
        assert_eq!(ints(&index_of(&a, &b).unwrap()), [1, 3]);
    }

    #[test]
    fn test_index_of_empty_a() {
        // (⍳0) ⍳ 1 2 → 0 0  (len_A = 0 ⇒ everything maps to 0)
        let a = ValueP::int_vector(&[]);
        let b = ValueP::int_vector(&[1, 2]);
        assert_eq!(ints(&index_of(&a, &b).unwrap()), [0, 0]);
    }
}
