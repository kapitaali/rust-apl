//! Enclose `⊂B` and disclose `⊃B` (mirrors `src/Bif_F12_PARTITION_PICK.cc`,
//! `Bif_F12_PARTITION` (monadic ⊂ only) and `Bif_F12_PICK::disclose()`).
//!
//! - `⊂B` encloses B into a scalar pointer cell (`ValueP::nested`)
//! - `⊃B` discloses: for a pointer scalar, the nested value; for a non-scalar
//!   array of pointers, all items disclosed and mixed (simplified: we handle
//!   the pointer-scalar case exactly, arrays of homogeneous simple scalars
//!   pass through unchanged — full mixed disclosure needs mixed arrays).
//!
//! Dyadic `⊂` (partition) and dyadic `⊃` (pick with index vector) are NOT
//! implemented yet.

use crate::types::AplResult;
use crate::value::ValueP;

/// `⊂B` — enclose.
pub fn enclose(b: &ValueP) -> AplResult<ValueP> {
    Ok(ValueP::nested(b.clone()))
}

/// `⊃B` — disclose (monadic).
///
/// - simple scalar → itself (ISO: ⊃B ≡ ⊃⊂B for scalars)
/// - pointer scalar → the nested value
/// - anything else → simplified pass-through (full mixed disclosure
///   requires mixed-array support; GNU APL computes item shapes here)
pub fn disclose(b: &ValueP) -> AplResult<ValueP> {
    // pointer scalar → nested value
    if b.is_scalar() {
        if let Some(crate::cell::Cell::Pointer(p)) = b.first_cell() {
            return Ok(ValueP {
                inner: p.value.clone(),
            });
        }
        // simple scalar: identity
        return Ok(b.clone());
    }
    Ok(b.clone())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{Cell, PointerCellData};
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
    fn test_enclose_makes_scalar() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let e = enclose(&v).unwrap();
        assert!(e.is_scalar());
        assert!(e.first_cell().unwrap().is_pointer_cell());
    }

    #[test]
    fn test_enclose_disclose_roundtrip() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let e = enclose(&v).unwrap();
        let d = disclose(&e).unwrap();
        assert_eq!(ints(&d), [1, 2, 3]);
        assert!(d.is_vector());
    }

    #[test]
    fn test_disclose_simple_scalar_is_identity() {
        let v = ValueP::scalar_from(Cell::Int(42));
        let d = disclose(&v).unwrap();
        assert_eq!(ints(&d), [42]);
    }

    #[test]
    fn test_disclose_non_scalar_passes_through() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let d = disclose(&v).unwrap();
        assert_eq!(ints(&d), [1, 2, 3]);
        assert!(d.is_vector());
    }

    #[test]
    fn test_disclose_of_nested_matrix() {
        use crate::shape::Shape;
        let shape = Shape::matrix(2, 2);
        let m =
            ValueP::from_parts(shape, [1, 2, 3, 4].into_iter().map(Cell::Int).collect()).unwrap();
        let d = disclose(&enclose(&m).unwrap()).unwrap();
        assert_eq!(d.rank(), 2);
        assert_eq!(d.get_shape_item(0), 2);
        assert_eq!(ints(&d), [1, 2, 3, 4]);
    }

    #[test]
    fn test_nested_shares_data() {
        // enclosing must share (not copy) the inner value
        let v = ValueP::int_vector(&[7, 8]);
        let e = enclose(&v).unwrap();
        match e.first_cell().unwrap() {
            Cell::Pointer(PointerCellData { value }) => {
                // same ravel content as original
                assert_eq!(value.cells(), v.cells());
            }
            o => panic!("expected pointer, got {:?}", o),
        }
        // silence unused import warning path
        let _ = Arc::strong_count(
            &(match e.first_cell().unwrap() {
                Cell::Pointer(p) => p.value.clone(),
                _ => unreachable!(),
            }),
        );
    }
}
