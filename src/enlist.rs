//! Enlist `∊B` (mirrors `Bif_F12_ELEMENT::do_eval_B()` in
//! `src/PrimitiveFunction.cc`).
//!
//! Recursively flattens B into a rank-1 vector of simple scalars:
//! - pointer cells recurse into their nested values
//! - simple cells are appended as-is
//! - empty argument: prototype decides the result — numeric → `0`,
//!   character → `' '`; an empty *nested* value recurses
//!
//! Result is always a vector (⍴⍴Z = 1).

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::AplResult;
use crate::value::{ValueInner, ValueP};

pub fn enlist(b: &ValueP) -> AplResult<ValueP> {
    let mut out = Vec::new();
    enlist_rec(b, &mut out);
    Ok(ValueP {
        inner: std::sync::Arc::new(ValueInner::new(Shape::vector(out.len() as i64), out)),
    })
}

fn enlist_rec(b: &ValueP, out: &mut Vec<Cell>) {
    if b.element_count() == 0 {
        // empty argument: recurse into a nested prototype, or use the
        // simple-cell default (0 / space)
        match b.first_cell() {
            Some(Cell::Pointer(p)) => {
                let nested = ValueP {
                    inner: p.value.clone(),
                };
                enlist_rec(&nested, out);
                // if the recursion produced nothing at all, fall back to 0
                if out.is_empty() {
                    out.push(Cell::Int(0));
                }
            }
            _ => {
                // prototype decides: char → space, numeric → 0
                if b.inner.proto().is_character_cell() {
                    out.push(Cell::char(' ' as u32));
                } else {
                    out.push(Cell::Int(0));
                }
            }
        }
        return;
    }

    for c in b.cells() {
        match c {
            Cell::Pointer(p) => {
                let nested = ValueP {
                    inner: p.value.clone(),
                };
                enlist_rec(&nested, out);
            }
            Cell::Lval(_) => out.push(Cell::Int(0)), // no lvals in this port yet
            simple => out.push(simple.clone()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::PointerCellData;
    use std::sync::Arc;

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
    fn test_enlist_simple_vector_is_identity() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(ints(&enlist(&v).unwrap()), [1, 2, 3]);
    }

    #[test]
    fn test_enlist_matrix_flattens() {
        // ⍴∊M is a vector even for a matrix
        let shape = Shape::matrix(2, 3);
        let m = ValueP::from_parts(
            shape,
            [1, 2, 3, 4, 5, 6].into_iter().map(Cell::Int).collect(),
        )
        .unwrap();
        let z = enlist(&m).unwrap();
        assert_eq!(z.rank(), 1);
        assert_eq!(ints(&z), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_enlist_nested() {
        // ∊(1 2)(3 4) → 1 2 3 4
        let n = ValueP::from_parts(
            Shape::vector(2),
            vec![
                Cell::Pointer(PointerCellData {
                    value: Arc::new(ValueInner::new(
                        Shape::vector(2),
                        vec![Cell::Int(1), Cell::Int(2)],
                    )),
                }),
                Cell::Pointer(PointerCellData {
                    value: Arc::new(ValueInner::new(
                        Shape::vector(2),
                        vec![Cell::Int(3), Cell::Int(4)],
                    )),
                }),
            ],
        )
        .unwrap();
        assert_eq!(ints(&enlist(&n).unwrap()), [1, 2, 3, 4]);
    }

    #[test]
    fn test_enlist_deeply_nested_scalars() {
        // ⊂⊂⊂5 enlists to ,5
        let inner = ValueP::scalar_from(Cell::Int(5));
        let l1 = ValueP::nested(inner);
        let l2 = ValueP::nested(l1);
        let l3 = ValueP::nested(l2);
        let z = enlist(&l3).unwrap();
        assert_eq!(z.rank(), 1);
        assert_eq!(ints(&z), [5]);
    }

    #[test]
    fn test_enlist_empty_numeric() {
        // ∊⍳0 → ,0
        let e = ValueP::int_vector(&[]);
        let z = enlist(&e).unwrap();
        assert_eq!(ints(&z), [0]);
    }

    #[test]
    fn test_enlist_empty_char() {
        // ∊'' → ,' '
        let e = ValueP::char_vector(&[]);
        let z = enlist(&e).unwrap();
        assert_eq!(z.cells(), &[Cell::char(' ' as u32)][..]);
    }

    #[test]
    fn test_enlist_empty_nested_reaches_inner() {
        // ∊(⊂⍳0) → the empty nested vector recurses; prototype of ⍳0 is
        // numeric so result is ,0
        let inner_empty = ValueP::int_vector(&[]);
        let enc = ValueP::nested(inner_empty);
        let n = ValueP::from_parts(
            Shape::vector(1),
            vec![Cell::Pointer(PointerCellData {
                value: enc.inner.clone(),
            })],
        )
        .unwrap();
        let z = enlist(&n).unwrap();
        assert_eq!(ints(&z), [0]);
    }

    #[test]
    fn test_enlist_mixed_types() {
        // chars and numbers flatten side by side
        let n = ValueP::from_parts(
            Shape::vector(2),
            vec![
                Cell::Pointer(PointerCellData {
                    value: Arc::new(ValueInner::new(
                        Shape::vector(2),
                        vec![Cell::Int(1), Cell::char('a' as u32)],
                    )),
                }),
                Cell::Pointer(PointerCellData {
                    value: Arc::new(ValueInner::new(Shape::vector(1), vec![Cell::Float(2.5)])),
                }),
            ],
        )
        .unwrap();
        let z = enlist(&n).unwrap();
        assert_eq!(z.cells().len(), 3); // 1 + 'a' + 2.5
        assert_eq!(z.cells()[0], Cell::Int(1));
        assert_eq!(z.cells()[1], Cell::char('a' as u32));
        assert!(matches!(z.cells()[2], Cell::Float(f) if f == 2.5));
    }
}
