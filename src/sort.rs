//! Grade up `⍋` and grade down `⍒` (mirrors `src/Bif_F12_SORT.cc`).
//!
//! Returns the permutation vector that sorts B ascending (`⍋`) or
//! descending (`⍒`). Ties keep left-to-right order (stable), matching
//! GNU APL's grade semantics for simple arrays.
//!
//! The C++ uses `Heapsort<Cell>`; here we use a stable sort over indices
//! (simpler and stable; heapsort is only needed for in-place sorting).

use crate::types::AplResult;
use crate::value::ValueP;

/// `⍋B` — grade up: indices that sort B ascending.
///
/// Library-level helper with FIXED 0-based results. The interpreter never
/// calls this: the parser's Monadic arm intercepts ⍋/⍒ and routes through
/// `grade_io` with the live ⎕IO. Keep any new internal caller IO-consistent.
pub fn grade_up(b: &ValueP) -> AplResult<ValueP> {
    grade_io(b, false, 0)
}

/// `⍒B` — grade down: indices that sort B descending. See grade_up.
pub fn grade_down(b: &ValueP) -> AplResult<ValueP> {
    grade_io(b, true, 0)
}

/// ⎕IO-aware grade: results are io..io+n-1 instead of 0..n-1.
pub fn grade_io(b: &ValueP, descending: bool, io: i64) -> AplResult<ValueP> {
    let cells = b.cells();
    let n = cells.len();

    let mut idx: Vec<usize> = (0..n).collect();
    // stable sort keeps ties in original (left-to-right) order
    idx.sort_by(|&i, &j| {
        let ord = match cells[i].compare(&cells[j]) {
            crate::cell::CompResult::Lt => std::cmp::Ordering::Less,
            crate::cell::CompResult::Eq => std::cmp::Ordering::Equal,
            crate::cell::CompResult::Gt => std::cmp::Ordering::Greater,
        };
        if descending {
            ord.reverse()
        } else {
            ord
        }
    });

    Ok(ValueP::int_vector(
        &idx.into_iter().map(|i| i as i64 + io).collect::<Vec<_>>(),
    ))
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
    fn test_grade_up() {
        let b = ValueP::int_vector(&[30, 10, 20]);
        assert_eq!(ints(&grade_up(&b).unwrap()), [1, 2, 0]);
    }

    #[test]
    fn test_grade_down() {
        let b = ValueP::int_vector(&[30, 10, 20]);
        assert_eq!(ints(&grade_down(&b).unwrap()), [0, 2, 1]);
    }

    #[test]
    fn test_grade_up_sorted_roundtrip() {
        let b = ValueP::int_vector(&[5, 1, 4, 1, 3]);
        let g = grade_up(&b).unwrap();
        // apply the grade: B[⍋B] must be sorted
        let cells = b.cells();
        let sorted: Vec<i64> = g
            .cells()
            .iter()
            .map(|c| match c {
                Cell::Int(i) => match &cells[*i as usize] {
                    Cell::Int(v) => *v,
                    o => panic!("unexpected {:?}", o),
                },
                _ => panic!("expected int"),
            })
            .collect();
        assert_eq!(sorted, [1, 1, 3, 4, 5]);
    }

    #[test]
    fn test_grade_stable_ties() {
        // ties keep left-to-right order: both 1s grade as 1 then 3
        let b = ValueP::int_vector(&[10, 1, 20, 1]);
        assert_eq!(ints(&grade_up(&b).unwrap()), [1, 3, 0, 2]);
    }

    #[test]
    fn test_grade_floats() {
        let b = ValueP::from_ravel_like(
            &ValueP::vector(3),
            vec![Cell::Float(2.5), Cell::Float(1.5), Cell::Float(3.5)],
        );
        assert_eq!(ints(&grade_up(&b).unwrap()), [1, 0, 2]);
    }

    #[test]
    fn test_parser_intercepts_grade_so_io_applies() {
        // Locks the contract above: through the interpreter, ⍋ honors ⎕IO
        // even though the bare library helpers are fixed 0-based.
        let mut env = crate::parser::Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("B←30 10 20").unwrap();
        assert!(env.eval_line("⎕IO←1").unwrap().is_none());
        let r = eval_line_val(&mut env, "⍋B");
        assert_eq!(r.cells(), &[Cell::Int(2), Cell::Int(3), Cell::Int(1)]);
    }

    /// helper: evaluate a line that must produce a value
    fn eval_line_val(env: &mut crate::parser::Environment, line: &str) -> ValueP {
        env.eval_line(line)
            .expect("eval failed")
            .expect("expected a result")
    }
}
