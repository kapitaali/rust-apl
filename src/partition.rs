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

        // A new partition opens only when the key INCREASES. A key that
        // stays the same or DECREASES continues the partition already open —
        // verified against GNU APL: 2 2 1 1⊂'abcd' is one piece 'abcd',
        // not 'ab' 'cd'. (Only 0 excludes an element, handled above.)
        match current_key {
            Some(k) if key <= k => {
                // same or lower key: keep accumulating into the open piece
                current.push(b_cells[i].clone());
            }
            _ => {
                // key increased (or this is the first piece): flush and open
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

    fn char_vec(s: &str) -> ValueP {
        ValueP::char_vector(&s.chars().map(|c| c as u32).collect::<Vec<_>>())
    }

    /// read a partition result back as the list of its pieces
    fn pieces_of(v: &ValueP) -> Vec<String> {
        v.cells()
            .iter()
            .map(|c| match c {
                Cell::Pointer(p) => {
                    let inner = ValueP {
                        inner: p.value.clone(),
                    };
                    inner
                        .cells()
                        .iter()
                        .map(|ic| match ic {
                            Cell::Char(u) => char::from_u32(*u).unwrap_or('?'),
                            _ => '?',
                        })
                        .collect()
                }
                other => panic!("expected nested piece, got {other:?}"),
            })
            .collect()
    }

    // All expectations below were verified against the reference C++ GNU APL
    // binary (~/Apps/apl-2.0/src/apl --script).

    #[test]
    fn test_partition_basic() {
        // 1 1 2 2 1⊂'abcde' → 'ab' 'cde'
        // The trailing 1 is a DECREASE from 2, so it continues that piece
        // rather than opening a third one.
        let a = ValueP::int_vector(&[1, 1, 2, 2, 1]);
        let r = partition(&a, &char_vec("abcde")).unwrap();
        assert_eq!(pieces_of(&r), vec!["ab", "cde"]);
    }

    #[test]
    fn test_partition_with_zeros() {
        // 1 0 1⊂'abc' → 'a' 'c' — b dropped, and c opens a new piece
        let a = ValueP::int_vector(&[1, 0, 1]);
        let r = partition(&a, &char_vec("abc")).unwrap();
        assert_eq!(pieces_of(&r), vec!["a", "c"]);
    }

    #[test]
    fn test_partition_increase_splits() {
        // 1 1 2 2⊂'abcd' → 'ab' 'cd'
        let a = ValueP::int_vector(&[1, 1, 2, 2]);
        let r = partition(&a, &char_vec("abcd")).unwrap();
        assert_eq!(pieces_of(&r), vec!["ab", "cd"]);
    }

    #[test]
    fn test_partition_decrease_does_not_split() {
        // 2 2 1 1⊂'abcd' → 'abcd' (one piece) — this is the case the old
        // implementation got wrong by splitting on any key CHANGE
        let a = ValueP::int_vector(&[2, 2, 1, 1]);
        let r = partition(&a, &char_vec("abcd")).unwrap();
        assert_eq!(pieces_of(&r), vec!["abcd"]);
    }

    #[test]
    fn test_partition_single_decrease_mid_vector() {
        // 1 2 1⊂'abc' → 'a' 'bc'
        let a = ValueP::int_vector(&[1, 2, 1]);
        let r = partition(&a, &char_vec("abc")).unwrap();
        assert_eq!(pieces_of(&r), vec!["a", "bc"]);
    }

    #[test]
    fn test_partition_every_increase_splits_each_item() {
        // 1 2 3⊂'abc' → 'a' 'b' 'c'
        let a = ValueP::int_vector(&[1, 2, 3]);
        let r = partition(&a, &char_vec("abc")).unwrap();
        assert_eq!(pieces_of(&r), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_partition_leading_zeros_skipped() {
        // 0 0 1 1⊂'abcd' → 'cd'
        let a = ValueP::int_vector(&[0, 0, 1, 1]);
        let r = partition(&a, &char_vec("abcd")).unwrap();
        assert_eq!(pieces_of(&r), vec!["cd"]);
    }

    #[test]
    fn test_partition_words_by_blanks_idiom() {
        // the classic split-on-spaces: 1 1 0 1 1⊂'ab cd' → 'ab' 'cd'
        let a = ValueP::int_vector(&[1, 1, 0, 1, 1]);
        let r = partition(&a, &char_vec("ab cd")).unwrap();
        assert_eq!(pieces_of(&r), vec!["ab", "cd"]);
    }

    #[test]
    fn test_partition_all_zeros_is_empty() {
        let a = ValueP::int_vector(&[0, 0, 0]);
        let r = partition(&a, &char_vec("abc")).unwrap();
        assert_eq!(r.element_count(), 0);
    }

    #[test]
    fn test_partition_length_mismatch_errors() {
        let a = ValueP::int_vector(&[1, 1]);
        assert!(partition(&a, &char_vec("abc")).is_err());
    }
}
