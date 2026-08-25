//! Dyadic partition: `A ⊂ B` — split B into enclosed pieces.
//!
//! APL partition: A is a vector of non-negative integers (keys).
//! B is split into pieces where consecutive elements have the same key.
//! Elements of B corresponding to key 0 are dropped. The result is a
//! vector of enclosed (nested) vectors, one per distinct non-zero key
//! value, in order of appearance.
//!
//! Simplified version: handles 1-D B (vector). Higher-rank B falls back
//! to enclosing the whole B (scalar enclosure).

use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

pub fn partition(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let a_cells = a.cells();
    let b_cells = b.cells();

    // If A is a scalar, just enclose B (treat as single partition).
    if a_cells.len() <= 1 {
        return Ok(ValueP::nested(b.clone()));
    }

    // Both must be 1-D vectors of the same length for the basic case.
    if b_cells.len() != a_cells.len() {
        return Err(ErrorCode::LengthError);
    }

    // Collect key values from A.
    let keys: Vec<i64> = a_cells
        .iter()
        .map(|c| match c {
            Cell::Int(i) => Ok(*i),
            Cell::Float(f) => Ok(*f as i64),
            _ => Err(ErrorCode::DomainError),
        })
        .collect::<Result<_, _>>()?;

    // Group B cells by consecutive equal non-zero keys.
    let mut pieces: Vec<Vec<Cell>> = Vec::new();
    let mut current: Vec<Cell> = Vec::new();
    let mut current_key: Option<i64> = None;

    for (i, &key) in keys.iter().enumerate() {
        if key == 0 {
            // key 0: drop this element. If we have an accumulated piece,
            // flush it.
            if !current.is_empty() {
                if let Some(k) = current_key {
                    if k != 0 {
                        pieces.push(std::mem::take(&mut current));
                    }
                }
            }
            current_key = Some(0);
            continue;
        }

        match current_key {
            Some(k) if k == key => {
                // same key group: accumulate
                current.push(b_cells[i].clone());
            }
            _ => {
                // key changed: flush previous piece
                if !current.is_empty() {
                    if let Some(k) = current_key {
                        if k != 0 {
                            pieces.push(std::mem::take(&mut current));
                        }
                    }
                }
                current_key = Some(key);
                current.push(b_cells[i].clone());
            }
        }
    }
    // flush last piece
    if !current.is_empty() {
        if let Some(k) = current_key {
            if k != 0 {
                pieces.push(current);
            }
        }
    }

    // Build result: vector of enclosed vectors.
    let item_count = pieces.len() as i64;
    let ravel: Vec<Cell> = pieces
        .iter()
        .map(|piece| {
            let inner = ValueP {
                inner: std::sync::Arc::new(crate::value::ValueInner::new(
                    crate::shape::Shape::vector(piece.len() as i64),
                    piece.clone(),
                )),
            };
            Cell::Pointer(crate::cell::PointerCellData { value: inner.inner })
        })
        .collect();

    Ok(ValueP {
        inner: std::sync::Arc::new(crate::value::ValueInner::new(
            crate::shape::Shape::vector(item_count),
            ravel,
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_basic() {
        // 1 1 2 2 1 ⊂ 'abcde' → ('ab') ('cd') ('e')
        let a = ValueP::int_vector(&[1, 1, 2, 2, 1]);
        let b = ValueP::char_vector(&"abcde".chars().map(|c| c as u32).collect::<Vec<_>>());
        let r = partition(&a, &b).unwrap();
        assert_eq!(r.element_count(), 3);
    }

    #[test]
    fn test_partition_with_zeros() {
        // 1 0 1 ⊂ 'abc' → ('a') ('c') — b dropped
        let a = ValueP::int_vector(&[1, 0, 1]);
        let b = ValueP::char_vector(&"abc".chars().map(|c| c as u32).collect::<Vec<_>>());
        let r = partition(&a, &b).unwrap();
        assert_eq!(r.element_count(), 2);
    }
}
