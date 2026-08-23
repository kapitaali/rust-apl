//! Dyadic pick `A⊃B` (mirrors `src/Bif_F12_PARTITION_PICK.cc`,
//! `Bif_F12_PICK::pick()` / `pick_offset()`).
//!
//! A is a path of indices into nested B:
//! - simple scalar A: index into vector B → the picked cell as a value
//! - vector A of depth 1: `(1 0)⊃M` indexes a matrix M (row, col)
//! - deeper paths recurse into pointer cells: `((⊂2)(⊂1))⊃N` picks N[1][0]
//!   — in our port a plain multi-element vector A recurses into nested
//!   values when intermediate cells are pointers.
//!
//! The final cell, if a pointer, is disclosed (pick returns the value,
//! not the cell). Empty A returns B itself.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::{ValueInner, ValueP};

pub fn pick(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    if a.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    if a.element_count() == 0 {
        return Ok(b.clone());
    }

    // Each element of A addresses one level of B:
    // - a scalar element indexes into a vector-level B;
    //   if B is higher-rank, the scalar must be replaced by an enclosed
    //   coordinate vector — we tolerate scalars on rank-1 levels only.
    // - an enclosed (pointer) element carries all coordinates for its
    //   level: (⊂1 0)⊃M means M[1;0].
    let mut levels: Vec<LevelIndex> = Vec::new();
    for c in a.cells() {
        match c {
            Cell::Pointer(p) => {
                let mut coords = Vec::new();
                for ic in p.value.cells() {
                    coords.push(ic.get_near_int()?);
                }
                levels.push(LevelIndex::Coords(coords));
            }
            _ => levels.push(LevelIndex::Scalar(c.get_near_int()?)),
        }
    }

    pick_levels(&levels, b)
}

#[derive(Debug)]
enum LevelIndex {
    /// single index into a vector level
    Scalar(i64),
    /// full coordinate tuple for a multi-dimensional level
    Coords(Vec<i64>),
}

fn offsets_weights(shape: &[i64]) -> Vec<i64> {
    let mut w = vec![1i64; shape.len()];
    for i in (0..shape.len().saturating_sub(1)).rev() {
        w[i] = w[i + 1] * shape[i + 1];
    }
    w
}

/// walk B following `levels`, disclosing the final cell.
fn pick_levels(levels: &[LevelIndex], b: &ValueP) -> AplResult<ValueP> {
    let (first, rest) = match levels.split_first() {
        Some(x) => x,
        None => return Ok(b.clone()),
    };

    let dims: Vec<i64> = (0..b.rank() as usize)
        .map(|i| b.get_shape_item(i as i16))
        .collect();

    // compute ravel offset for this level
    let offset = match first {
        LevelIndex::Scalar(idx) => {
            if dims.len() != 1 {
                return Err(ErrorCode::RankError);
            }
            if *idx < 0 || *idx >= dims[0] {
                return Err(ErrorCode::IndexError);
            }
            *idx
        }
        LevelIndex::Coords(coords) => {
            if coords.len() != dims.len() {
                return Err(ErrorCode::RankError);
            }
            let weights = offsets_weights(&dims);
            let mut off = 0i64;
            for (k, &c) in coords.iter().enumerate() {
                if c < 0 || c >= dims[k] {
                    return Err(ErrorCode::IndexError);
                }
                off += c * weights[k];
            }
            off
        }
    };

    let cell = &b.cells()[offset as usize];
    let cell = match cell {
        // disclose pointers on the way AND at the end
        Cell::Pointer(p) => ValueP {
            inner: p.value.clone(),
        },
        other => ValueP {
            inner: std::sync::Arc::new(ValueInner::new(Shape::scalar(), vec![other.clone()])),
        },
    };

    if rest.is_empty() {
        Ok(cell)
    } else {
        pick_levels(rest, &cell)
    }
}

/// Selective pick assignment: `(A⊃B) ← value`.
///
/// Walks B following `levels`, isolating (COW) each nested level on the
/// way down, then replaces the final cell with `value` (enclosed if the
/// surrounding structure is nested).
pub fn pick_assign(b: &mut ValueP, levels: &[i64], value: &Cell) -> AplResult<()> {
    let (first, rest) = match levels.split_first() {
        Some(x) => x,
        None => return Err(ErrorCode::IndexError),
    };

    b.isolate();
    let idx = *first;
    if idx < 0 || idx as usize >= b.cells().len() {
        return Err(ErrorCode::IndexError);
    }

    if rest.is_empty() {
        // final level: replace the cell
        let cells = b.make_mut().ravel_mut();
        cells[idx as usize] = value.clone();
        return Ok(());
    }

    // deeper: descend into the pointer cell, recurse, write back
    let inner = match &b.cells()[idx as usize] {
        Cell::Pointer(p) => ValueP {
            inner: p.value.clone(),
        },
        _ => return Err(ErrorCode::RankError),
    };
    let mut inner_mut = inner;
    pick_assign(&mut inner_mut, rest, value)?;
    let cells = b.make_mut().ravel_mut();
    cells[idx as usize] = Cell::Pointer(crate::cell::PointerCellData {
        value: inner_mut.inner.clone(),
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::PointerCellData;
    use crate::shape::Shape;

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
    fn test_pick_vector_scalar_index() {
        // 2 ⊃ 10 20 30 → 30
        let b = ValueP::int_vector(&[10, 20, 30]);
        let a = ValueP::scalar_from(Cell::Int(2));
        assert_eq!(ints(&pick(&a, &b).unwrap()), [30]);
    }

    #[test]
    fn test_pick_empty_a_returns_b() {
        let b = ValueP::int_vector(&[10, 20]);
        let a = ValueP::int_vector(&[]);
        assert_eq!(ints(&pick(&a, &b).unwrap()), [10, 20]);
    }

    #[test]
    fn test_pick_matrix_coords_then_vector() {
        // mirrors Pick.tc: S←2 3⍴'AB' 'CD' 'EF' 'GH' 'IJ' 'KL'; (1 3) 2⊃S → 'F'
        // level 1: coords (row=1, col=3-⎕IO... we are 0-based: (1,2)) into S
        // GNU APL used ⎕IO←1 there; our port is 0-based.
        let shape = Shape::matrix(2, 3);
        let s = ValueP::from_parts(
            shape,
            vec![
                Cell::char('A' as u32),
                Cell::char('B' as u32),
                Cell::char('C' as u32),
                Cell::char('D' as u32),
                Cell::char('E' as u32),
                Cell::char('F' as u32),
            ],
        )
        .unwrap();
        // 0-based coords for row 1 col 2 = 'F', then index... it's a scalar
        // so a second Scalar level would RANK ERROR. Use just Coords:
        let a = ValueP::from_parts(
            Shape::vector(1),
            vec![Cell::Pointer(PointerCellData {
                value: std::sync::Arc::new(ValueInner::new(
                    Shape::vector(2),
                    vec![Cell::Int(1), Cell::Int(2)],
                )),
            })],
        )
        .unwrap();
        let z = pick(&a, &s).unwrap();
        assert_eq!(z.cells(), &[Cell::char('F' as u32)][..]);
    }

    #[test]
    fn test_pick_into_nested() {
        // N ← (10 20)(30 40); (0 1)⊃N should give N[0][1] = 20
        let n = crate::parser::Environment::new();
        drop(n);
        let inner0 = rc_value(vec![10, 20]);
        let inner1 = rc_value(vec![30, 40]);
        let n = ValueP::from_parts(
            Shape::vector(2),
            vec![
                Cell::Pointer(PointerCellData { value: inner0 }),
                Cell::Pointer(PointerCellData { value: inner1 }),
            ],
        )
        .unwrap();
        // A = (⊂0)(⊂1) is expressed as a flat path [0, 1] in our port:
        // first level: N[0] = (10 20), second level: [1] of it = 20
        let a = ValueP::int_vector(&[0, 1]);
        assert_eq!(ints(&pick(&a, &n).unwrap()), [20]);

        let a = ValueP::int_vector(&[1, 0]);
        assert_eq!(ints(&pick(&a, &n).unwrap()), [30]);
    }

    #[test]
    fn test_pick_index_out_of_range() {
        let b = ValueP::int_vector(&[10, 20]);
        let a = ValueP::scalar_from(Cell::Int(9));
        assert!(pick(&a, &b).is_err());
    }

    #[test]
    fn test_pick_rank_error_on_matrix_a() {
        let b = ValueP::int_vector(&[10, 20]);
        let shape = Shape::matrix(1, 1);
        let a = ValueP::from_parts(shape, vec![Cell::Int(0)]).unwrap();
        assert!(matches!(pick(&a, &b), Err(ErrorCode::RankError)));
    }

    /// helper: build an Arc<ValueInner> int vector
    fn rc_value(vals: Vec<i64>) -> std::sync::Arc<ValueInner> {
        std::sync::Arc::new(ValueInner::new(
            Shape::vector(vals.len() as i64),
            vals.into_iter().map(Cell::Int).collect(),
        ))
    }
}
