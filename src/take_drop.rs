//! Take `↑` and Drop `↓` (mirrors `src/Bif_F12_TAKE_DROP.cc`).
//!
//! Last-axis only for rank > 1 (applied per row); vectors and scalars
//! handled directly. Negative counts take/drop from the end.
//!
//! - `X↑B`: |X| > n → over-take padded with the prototype (0 or ' ')
//!   (negative X takes from the end, result still |X| long)
//! - `X↓B`: remove |X| items from the start (X<0) or end (X>0)

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::AplResult;
use crate::types::{ErrorCode, ShapeItem};
use crate::value::ValueP;

/// prototype cell of a value (mirrors the C++ `Value::get_cproto()`):
/// numeric → 0, character → ' ', nested → enclose of its prototype.
/// Uses ValueInner::proto() so EMPTY values still report their type.
fn prototype(b: &ValueP) -> Cell {
    let proto = b.inner.proto().clone();
    match proto {
        // normalize: any int → 0, any float → 0.0
        Cell::Int(_) => Cell::int(0),
        Cell::Float(_) => Cell::float(0.0),
        Cell::Char(_) => Cell::char(' ' as u32),
        Cell::Pointer(p) => {
            // prototype of a nested value is an enclosed prototype
            let inner_proto = prototype(&ValueP {
                inner: p.value.clone(),
            });
            Cell::pointer(std::sync::Arc::new(crate::value::ValueInner::new(
                Shape::scalar(),
                vec![inner_proto],
            )))
        }
        other => other,
    }
}

/// `X↑B` — take along the last axis.
pub fn take(x: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let count = single_int(x)?;
    let rank = b.rank();
    let n = axis_len(b);

    // how many from the front (>0) or back (<0); may exceed n (over-take)
    if rank <= 1 {
        let items = take_vec(b.cells(), count, &prototype(b));
        return ValueP::from_parts(Shape::vector(items.len() as ShapeItem), items);
    }

    // rank ≥ 2: apply per row along the last axis
    let outer = b.element_count() / n.max(1);
    let proto = prototype(b);
    let mut out = Vec::with_capacity((outer * count.abs()) as usize);
    let cells = b.cells();
    for row in 0..outer as usize {
        out.extend(take_vec(
            &cells[row * n as usize..(row + 1) * n as usize],
            count,
            &proto,
        ));
    }
    let mut dims: Vec<i64> = (0..rank as usize - 1)
        .map(|a| b.get_shape_item(a as i16))
        .collect();
    dims.push(count.abs());
    ValueP::from_parts(Shape::from_dims(&dims)?, out)
}

/// `X↓B` — drop along the last axis.
pub fn drop(x: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let count = single_int(x)?;
    let rank = b.rank();
    let n = axis_len(b);

    if rank <= 1 {
        let items = drop_vec(b.cells(), count, &prototype(b));
        return ValueP::from_parts(Shape::vector(items.len() as ShapeItem), items);
    }

    let outer = b.element_count() / n.max(1);
    let proto = prototype(b);
    let cells = b.cells();

    // dropped amount per row determines output width
    let keep = (n - count.abs()).max(0);
    let mut out = Vec::with_capacity((outer * keep) as usize);
    for row in 0..outer as usize {
        out.extend(drop_vec(
            &cells[row * n as usize..(row + 1) * n as usize],
            count,
            &proto,
        ));
    }
    let mut dims: Vec<i64> = (0..rank as usize - 1)
        .map(|a| b.get_shape_item(a as i16))
        .collect();
    dims.push(keep);
    ValueP::from_parts(Shape::from_dims(&dims)?, out)
}

// ---------------------------------------------------------------------------

fn axis_len(b: &ValueP) -> i64 {
    let rank = b.rank();
    if rank == 0 {
        1
    } else {
        b.get_shape_item(rank as i16 - 1)
    }
}

fn single_int(x: &ValueP) -> AplResult<i64> {
    if x.element_count() != 1 {
        return Err(ErrorCode::LengthError);
    }
    x.first_cell()
        .and_then(|c| c.get_int_value().ok())
        .ok_or(ErrorCode::DomainError)
}

/// take `count` items from a slice; pads with `proto` when over-taking.
/// negative count takes from the end.
fn take_vec(cells: &[Cell], count: i64, proto: &Cell) -> Vec<Cell> {
    let n = cells.len() as i64;
    let want = count.unsigned_abs() as usize;
    if count >= 0 {
        let take_n = want.min(cells.len());
        let mut out: Vec<Cell> = cells[..take_n].to_vec();
        out.resize(want, proto.clone());
        out
    } else {
        // take from the end; pad at the FRONT when over-taking
        let skip = (n - want as i64).max(0) as usize;
        let src = &cells[skip..];
        let mut out: Vec<Cell> = src.to_vec();
        while out.len() < want {
            out.insert(0, proto.clone());
        }
        out
    }
}

/// drop `count` items; negative drops from the end.
fn drop_vec(cells: &[Cell], count: i64, _proto: &Cell) -> Vec<Cell> {
    let n = cells.len() as i64;
    let cut = count.unsigned_abs().min(n.unsigned_abs()) as usize;
    if count >= 0 {
        if cut >= cells.len() {
            Vec::new()
        } else {
            cells[cut..].to_vec()
        }
    } else {
        let keep = (n as usize).saturating_sub(cut);
        cells[..keep].to_vec()
    }
}

/// `X↑[a]B` — take along axis `a` (0-based).
pub fn take_axis(x: &ValueP, b: &ValueP, axis: i64) -> AplResult<ValueP> {
    let count = single_int(x)?;
    let rank = b.rank();
    if axis < 0 || axis >= rank as i64 {
        return Err(ErrorCode::RankError);
    }

    let n = b.get_shape_item(axis as i16);
    let pre: i64 = (0..axis).map(|k| b.get_shape_item(k as i16)).product();
    let post: i64 = ((axis + 1)..rank as i64)
        .map(|k| b.get_shape_item(k as i16))
        .product();

    let cells = b.cells();
    let proto = prototype(b);
    let out_len = (pre.max(1) * count.abs() * post.max(1)) as usize;
    let mut out = vec![proto.clone(); out_len];

    // process each line along the axis and write results back in place.
    // flat index = (p*n + k)*post + s for input; output has count.abs()
    // along the axis: out flat = (p*count + k')*post + s.
    for p in 0..pre.max(1) as usize {
        for s in 0..post.max(1) as usize {
            let line: Vec<Cell> = (0..n as usize)
                .map(|k| cells[(p * n as usize + k) * post as usize + s].clone())
                .collect();
            let taken = take_vec(&line, count, &proto);
            let w = count.unsigned_abs() as usize;
            for (k, c) in taken.into_iter().enumerate() {
                out[(p * w + k) * post as usize + s] = c;
            }
        }
    }
    ValueP::from_parts(
        {
            let mut dims: Vec<i64> = (0..rank as usize)
                .map(|k| b.get_shape_item(k as i16))
                .collect();
            dims[axis as usize] = count.abs();
            Shape::from_dims(&dims)?
        },
        out,
    )
}

/// `X↓[a]B` — drop along axis `a` (0-based).
pub fn drop_axis(x: &ValueP, b: &ValueP, axis: i64) -> AplResult<ValueP> {
    let count = single_int(x)?;
    let rank = b.rank();
    if axis < 0 || axis >= rank as i64 {
        return Err(ErrorCode::RankError);
    }
    let n = b.get_shape_item(axis as i16);
    let pre: i64 = (0..axis).map(|k| b.get_shape_item(k as i16)).product();
    let post: i64 = ((axis + 1)..rank as i64)
        .map(|k| b.get_shape_item(k as i16))
        .product();

    let cells = b.cells();
    let keep = (n - count.abs()).max(0);
    let out_len = (pre.max(1) * keep * post.max(1)) as usize;
    let mut out = vec![prototype(b); out_len];

    // same in-place writeback: input (p*n + k)*post + s; output has `keep`
    // along the axis, and dropped items shift remaining ones forward.
    for p in 0..pre.max(1) as usize {
        for s in 0..post.max(1) as usize {
            let line: Vec<Cell> = (0..n as usize)
                .map(|k| cells[(p * n as usize + k) * post as usize + s].clone())
                .collect();
            let kept = drop_vec(&line, count, &prototype(b));
            for (k, c) in kept.into_iter().enumerate() {
                out[(p * keep as usize + k) * post as usize + s] = c;
            }
        }
    }

    let mut dims: Vec<i64> = (0..rank as usize)
        .map(|k| b.get_shape_item(k as i16))
        .collect();
    dims[axis as usize] = keep;
    ValueP::from_parts(Shape::from_dims(&dims)?, out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
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

    fn mat(data: &[i64], rows: i64, cols: i64) -> ValueP {
        ValueP::from_parts(
            Shape::matrix(rows, cols),
            data.iter().map(|&i| Cell::Int(i)).collect(),
        )
        .unwrap()
    }

    #[test]
    fn test_take_axis0_matrix() {
        // 1↑[0] of a 2×3 matrix takes the first ROW
        let b = mat(&[0, 1, 2, 3, 4, 5], 2, 3);
        let x = ValueP::int_vector(&[1]);
        let z = take_axis(&x, &b, 0).unwrap();
        assert_eq!(z.rank(), 2);
        assert_eq!(ints(&z), [0, 1, 2]);
    }

    #[test]
    fn test_take_axis1_equals_last_axis() {
        // ↑[1] on a matrix == plain last-axis take
        let b = mat(&[0, 1, 2, 3, 4, 5], 2, 3);
        let x = ValueP::int_vector(&[2]);
        let z = take_axis(&x, &b, 1).unwrap();
        let z_last = take(&x, &b).unwrap();
        assert_eq!(ints(&z), ints(&z_last));
    }

    #[test]
    fn test_drop_axis0_matrix() {
        // 1↓[0] drops the first ROW: [3 4 5]
        let b = mat(&[0, 1, 2, 3, 4, 5], 2, 3);
        let x = ValueP::int_vector(&[1]);
        let z = drop_axis(&x, &b, 0).unwrap();
        assert_eq!(z.get_shape_item(0), 1);
        assert_eq!(ints(&z), [3, 4, 5]);
    }

    #[test]
    fn test_take_axis_over_takes_with_proto() {
        // 3↑[0] over-takes rows (pads with zeros)
        let b = mat(&[1, 2, 3, 4], 2, 2);
        let x = ValueP::int_vector(&[3]);
        let z = take_axis(&x, &b, 0).unwrap();
        assert_eq!(z.get_shape_item(0), 3);
        assert_eq!(ints(&z), [1, 2, 3, 4, 0, 0]);
    }

    #[test]
    fn test_take_axis_rank_error() {
        let b = ValueP::int_vector(&[1, 2]);
        let x = ValueP::int_vector(&[1]);
        assert!(take_axis(&x, &b, 5).is_err());
    }

    #[test]
    fn test_take_basic() {
        let b = ValueP::int_vector(&[1, 2, 3, 4, 5]);
        assert_eq!(
            ints(&take(&ValueP::int_vector(&[3]), &b).unwrap()),
            [1, 2, 3]
        );
        assert_eq!(
            ints(&take(&ValueP::int_vector(&[-3]), &b).unwrap()),
            [3, 4, 5]
        );
    }

    #[test]
    fn test_take_over_pad_zeros() {
        let b = ValueP::int_vector(&[1, 2, 3]);
        assert_eq!(
            ints(&take(&ValueP::int_vector(&[5]), &b).unwrap()),
            [1, 2, 3, 0, 0]
        );
        // negative over-take pads at the FRONT
        assert_eq!(
            ints(&take(&ValueP::int_vector(&[-5]), &b).unwrap()),
            [0, 0, 1, 2, 3]
        );
    }

    #[test]
    fn test_take_char_pads_space() {
        let b = ValueP::char_vector(&['a' as u32, 'b' as u32]);
        let z = take(&ValueP::int_vector(&[4]), &b).unwrap();
        match z.cells() {
            [Cell::Char(a), Cell::Char(bch), Cell::Char(c), Cell::Char(d)] => {
                assert_eq!(*a, 'a' as u32);
                assert_eq!(*bch, 'b' as u32);
                assert_eq!(*c, ' ' as u32);
                assert_eq!(*d, ' ' as u32);
            }
            _ => panic!("expected chars"),
        }
    }

    #[test]
    fn test_drop_basic() {
        let b = ValueP::int_vector(&[1, 2, 3, 4, 5]);
        assert_eq!(
            ints(&drop(&ValueP::int_vector(&[2]), &b).unwrap()),
            [3, 4, 5]
        );
        assert_eq!(
            ints(&drop(&ValueP::int_vector(&[-2]), &b).unwrap()),
            [1, 2, 3]
        );
        assert!(drop(&ValueP::int_vector(&[9]), &b).unwrap().is_empty());
    }

    #[test]
    fn test_over_take_empty_char_value() {
        // 3↑'' must pad with SPACES (prototype), not zeros.
        // Before the proto fix, an empty value had no first cell to infer from.
        let b = ValueP::char_vector(&[]);
        let z = take(&ValueP::int_vector(&[3]), &b).unwrap();
        assert_eq!(z.element_count(), 3);
        for c in z.cells() {
            assert_eq!(*c, Cell::char(' ' as u32), "pad must be space");
        }
    }

    #[test]
    fn test_over_take_empty_numeric_value() {
        let b = ValueP::int_vector(&[]);
        let z = take(&ValueP::int_vector(&[2]), &b).unwrap();
        assert_eq!(ints(&z), [0, 0]);
    }

    #[test]
    fn test_float_prototype_pads_with_zero() {
        // float values pad with 0 (displayed as plain zero)
        let b =
            ValueP::from_ravel_like(&ValueP::vector(2), vec![Cell::Float(1.5), Cell::Float(2.5)]);
        let z = take(&ValueP::int_vector(&[4]), &b).unwrap();
        match z.cells() {
            [Cell::Float(1.5), Cell::Float(2.5), Cell::Float(a), Cell::Float(c)] => {
                assert_eq!(*a, 0.0);
                assert_eq!(*c, 0.0);
            }
            o => panic!("expected floats, got {:?}", o),
        }
    }

    #[test]
    fn test_take_matrix_rows() {
        let shape = Shape::matrix(2, 3);
        let b = ValueP::from_parts(
            shape,
            vec![
                Cell::Int(1),
                Cell::Int(2),
                Cell::Int(3),
                Cell::Int(4),
                Cell::Int(5),
                Cell::Int(6),
            ],
        )
        .unwrap();
        let z = take(&ValueP::int_vector(&[2]), &b).unwrap();
        assert_eq!(z.get_shape_item(0), 2);
        assert_eq!(z.get_shape_item(1), 2);
        assert_eq!(ints(&z), vec![1, 2, 4, 5]);
    }
}
