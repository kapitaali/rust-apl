//! Membership `A ∈ B` (mirrors `src/PrimitiveFunction.cc` `Bif_EPSILON`).
//!
//! For each element of A: 1 if it occurs anywhere in B, else 0.
//! Implemented via dyadic `⍳` semantics (find first occurrence).

use crate::cell::Cell;
use crate::types::AplResult;
use crate::value::ValueP;

/// `A ∈ B` — membership.
pub fn epsilon(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let cells_b = b.cells();
    let mut out = Vec::with_capacity(a.element_count() as usize);

    for ca in a.cells() {
        let found = cells_b.iter().any(|cb| ca.equal(cb, Cell::DEFAULT_CT));
        out.push(Cell::Int(i64::from(found)));
    }

    // result has A's shape with boolean ravel
    Ok(ValueP::from_ravel_like(a, out))
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
    fn test_epsilon_basic() {
        // 1 2 3 ∈ 2 4 6 8 → 0 1 0
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[2, 4, 6, 8]);
        assert_eq!(ints(&epsilon(&a, &b).unwrap()), [0, 1, 0]);
    }

    #[test]
    fn test_epsilon_all_found() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[9, 3, 1, 2]);
        assert_eq!(ints(&epsilon(&a, &b).unwrap()), [1, 1, 1]);
    }

    #[test]
    fn test_epsilon_empty_b() {
        let a = ValueP::int_vector(&[1, 2]);
        let b = ValueP::int_vector(&[]);
        assert_eq!(ints(&epsilon(&a, &b).unwrap()), [0, 0]);
    }

    #[test]
    fn test_epsilon_result_shape_follows_a() {
        use crate::shape::Shape;
        let shape = Shape::matrix(2, 2);
        let a = ValueP::from_parts(
            shape,
            vec![Cell::Int(1), Cell::Int(9), Cell::Int(3), Cell::Int(7)],
        )
        .unwrap();
        let b = ValueP::int_vector(&[1, 3]);
        let z = epsilon(&a, &b).unwrap();
        assert_eq!(z.get_shape_item(0), 2);
        assert_eq!(z.get_shape_item(1), 2);
        assert_eq!(ints(&z), [1, 0, 1, 0]);
    }

    #[test]
    fn test_epsilon_chars() {
        let a = ValueP::char_vector(&['h' as u32, 'x' as u32]);
        let b = ValueP::char_vector(&['h' as u32, 'i' as u32]);
        assert_eq!(ints(&epsilon(&a, &b).unwrap()), [1, 0]);
    }
}
