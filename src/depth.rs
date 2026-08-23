//! Depth `≡B` and match `A≡B` (mirrors `Bif_F12_EQUIV` in
//! `src/PrimitiveFunction.cc` and `Value::compute_depth()`).
//!
//! - `≡B` (monadic): nesting depth. Simple scalar = 0; a non-scalar is
//!   1 + max depth of its cells (pointer cells recurse). So a simple
//!   vector = 1, `(1 2)(3 4)` = 2, `⊂⊂5` = 3.
//! - `A≡B` (dyadic): match — same shape AND all cells equal (tolerant,
//!   recursive into pointers via Cell::equal).

use crate::cell::Cell;
use crate::types::AplResult;
use crate::value::ValueP;

/// monadic `≡B` — compute the nesting depth of B.
pub fn depth(b: &ValueP) -> AplResult<ValueP> {
    Ok(ValueP::scalar_from(Cell::Int(compute_depth(b))))
}

fn compute_depth(b: &ValueP) -> i64 {
    // scalar: pointer → 1 + nested depth; simple → 0
    if b.rank() == 0 {
        return match b.first_cell() {
            Some(Cell::Pointer(p)) => {
                1 + compute_depth(&ValueP {
                    inner: p.value.clone(),
                })
            }
            _ => 0,
        };
    }

    // non-scalar: 1 + max sub-depth of cells
    let mut sub = 0i64;
    for c in b.cells() {
        let d = match c {
            Cell::Pointer(p) => compute_depth(&ValueP {
                inner: p.value.clone(),
            }),
            _ => 0,
        };
        if d > sub {
            sub = d;
        }
    }
    sub + 1
}

/// dyadic `A≡B` — match: same shape, all corresponding cells equal.
pub fn equiv(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    Ok(ValueP::scalar_from(Cell::Int(if do_equiv(a, b) {
        1
    } else {
        0
    })))
}

fn do_equiv(a: &ValueP, b: &ValueP) -> bool {
    let (ra, rb) = (a.shape().get_rank(), b.shape().get_rank());
    if ra != rb {
        return false;
    }
    for axis in 0..ra as usize {
        if a.get_shape_item(axis as i16) != b.get_shape_item(axis as i16) {
            return false;
        }
    }
    let ac = a.cells();
    let bc = b.cells();
    if ac.len() != bc.len() {
        return false;
    }
    for (ca, cb) in ac.iter().zip(bc.iter()) {
        if !cells_equiv(ca, cb) {
            return false;
        }
    }
    true
}

/// deep cell equality: pointers compare structurally (with cycle-free recursion)
fn cells_equiv(a: &Cell, b: &Cell) -> bool {
    match (a, b) {
        (Cell::Pointer(pa), Cell::Pointer(pb)) => {
            let av = ValueP {
                inner: pa.value.clone(),
            };
            let bv = ValueP {
                inner: pb.value.clone(),
            };
            do_equiv(&av, &bv)
        }
        (Cell::Pointer(_), _) | (_, Cell::Pointer(_)) => false,
        (x, y) => x.equal(y, Cell::DEFAULT_CT),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::PointerCellData;
    use crate::shape::Shape;
    use crate::value::ValueInner;
    use std::sync::Arc;

    fn int_at(v: &ValueP) -> i64 {
        match v.first_cell().unwrap() {
            Cell::Int(i) => *i,
            o => panic!("expected int, got {:?}", o),
        }
    }

    #[test]
    fn test_depth_simple_scalar() {
        assert_eq!(
            int_at(&depth(&ValueP::scalar_from(Cell::Int(5))).unwrap()),
            0
        );
    }

    #[test]
    fn test_depth_simple_vector() {
        assert_eq!(int_at(&depth(&ValueP::int_vector(&[1, 2, 3])).unwrap()), 1);
    }

    #[test]
    fn test_depth_nested_vector() {
        // N ← (1 2)(3 4): each element is a vector → depth 2
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
        assert_eq!(int_at(&depth(&n).unwrap()), 2);
    }

    #[test]
    fn test_depth_enclosed_scalar_chain() {
        // ⊂⊂⊂5 → depth 3
        let l1 = ValueP::nested(ValueP::scalar_from(Cell::Int(5)));
        let l2 = ValueP::nested(l1);
        let l3 = ValueP::nested(l2);
        assert_eq!(int_at(&depth(&l3).unwrap()), 3);
    }

    #[test]
    fn test_match_equal() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(int_at(&equiv(&a, &b).unwrap()), 1);
    }

    #[test]
    fn test_match_shape_mismatch() {
        let a = ValueP::int_vector(&[1, 2]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(int_at(&equiv(&a, &b).unwrap()), 0);
    }

    #[test]
    fn test_match_nested_deep() {
        // ⊂1 2 ≡ ⊂1 2 → 1 (deep comparison through pointers)
        let enc =
            |v: ValueP| ValueP::scalar_from(Cell::Pointer(PointerCellData { value: v.inner }));
        let a = enc(ValueP::int_vector(&[1, 2]));
        let b = enc(ValueP::int_vector(&[1, 2]));
        assert_eq!(int_at(&equiv(&a, &b).unwrap()), 1);
        let c = enc(ValueP::int_vector(&[1, 9]));
        assert_eq!(int_at(&equiv(&a, &c).unwrap()), 0);
    }

    #[test]
    fn test_match_scalar_tolerance() {
        // tolerant float equality via ⎕CT
        let a = ValueP::scalar_from(Cell::Float(1.0));
        let b = ValueP::scalar_from(Cell::Int(1));
        assert_eq!(int_at(&equiv(&a, &b).unwrap()), 1);
    }
}
