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

/// `X↑B` — take, ONE COUNT PER AXIS.
///
/// `X` must have exactly one element per axis of `B` (LENGTH ERROR otherwise,
/// matching the reference: `1↑2 3⍴⍳6` is an error, not a last-axis take).
/// Each count selects from the front when positive and from the end when
/// negative; over-taking pads with B's prototype. The result's shape is
/// `|X` — so `3 4↑2 3⍴⍳6` is a fully padded 3×4.
pub fn take(x: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let counts = axis_counts(x, b)?;
    if counts.len() == 1 {
        let items = take_vec(b.cells(), counts[0], &prototype(b));
        return ValueP::from_parts(Shape::vector(items.len() as ShapeItem), items);
    }
    let dims: Vec<i64> = counts.iter().map(|c| c.abs()).collect();
    let src_dims = shape_dims(b);
    let proto = prototype(b);
    let cells = b.cells();

    let total: i64 = dims.iter().product();
    let out: Vec<Cell> = if total as usize >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        (0..total as usize)
            .into_par_iter()
            .map(|idx| {
                // decode idx → result subscript
                let mut subs = vec![0i64; dims.len()];
                let mut rem = idx as i64;
                for ax in (0..dims.len()).rev() {
                    subs[ax] = rem % dims[ax];
                    rem /= dims[ax];
                }
                let mut inside = true;
                let mut src = Vec::with_capacity(subs.len());
                for (ax, &s) in subs.iter().enumerate() {
                    let off = if counts[ax] >= 0 {
                        s
                    } else {
                        src_dims[ax] + counts[ax] + s
                    };
                    if off < 0 || off >= src_dims[ax] {
                        inside = false;
                        break;
                    }
                    src.push(off);
                }
                if inside {
                    Ok(cells[encode(&src, &src_dims) as usize].clone())
                } else {
                    Ok(proto.clone())
                }
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut out = Vec::with_capacity(total as usize);
        let mut subs = vec![0i64; dims.len()];
        for _ in 0..total {
            let mut src = Vec::with_capacity(subs.len());
            let mut inside = true;
            for (ax, &s) in subs.iter().enumerate() {
                let off = if counts[ax] >= 0 {
                    s
                } else {
                    src_dims[ax] + counts[ax] + s
                };
                if off < 0 || off >= src_dims[ax] {
                    inside = false;
                    break;
                }
                src.push(off);
            }
            out.push(if inside {
                cells[encode(&src, &src_dims) as usize].clone()
            } else {
                proto.clone()
            });
            bump(&mut subs, &dims);
        }
        out
    };

    ValueP::from_parts(Shape::from_dims(&dims)?, out)
}

/// `X↓B` — drop, ONE COUNT PER AXIS.
///
/// Positive counts remove from the front of that axis, negative from the end.
/// Dropping at least as much as an axis holds leaves that axis empty (so
/// `5 5↓2 3⍴⍳6` has shape `0 0`).
pub fn drop(x: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let counts = axis_counts(x, b)?;
    if counts.len() == 1 {
        let items = drop_vec(b.cells(), counts[0], &prototype(b));
        return ValueP::from_parts(Shape::vector(items.len() as ShapeItem), items);
    }
    let src_dims = shape_dims(b);
    // remaining length per axis, and where the kept window starts
    let mut dims = Vec::with_capacity(counts.len());
    let mut starts = Vec::with_capacity(counts.len());
    for (ax, &c) in counts.iter().enumerate() {
        let keep = (src_dims[ax] - c.abs()).max(0);
        dims.push(keep);
        starts.push(if c > 0 { c } else { 0 });
    }
    let cells = b.cells();
    let total: i64 = dims.iter().product();
    let out: Vec<Cell> = if total as usize >= crate::functions::PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        (0..total as usize)
            .into_par_iter()
            .map(|idx| {
                let mut subs = vec![0i64; dims.len()];
                let mut rem = idx as i64;
                for ax in (0..dims.len()).rev() {
                    subs[ax] = rem % dims[ax];
                    rem /= dims[ax];
                }
                let src: Vec<i64> = subs
                    .iter()
                    .enumerate()
                    .map(|(ax, &s)| s + starts[ax])
                    .collect();
                Ok(cells[encode(&src, &src_dims) as usize].clone())
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut out = Vec::with_capacity(total.max(0) as usize);
        if total > 0 {
            let mut subs = vec![0i64; dims.len()];
            for _ in 0..total {
                let src: Vec<i64> = subs
                    .iter()
                    .enumerate()
                    .map(|(ax, &s)| s + starts[ax])
                    .collect();
                out.push(cells[encode(&src, &src_dims) as usize].clone());
                bump(&mut subs, &dims);
            }
        }
        out
    };

    ValueP::from_parts(Shape::from_dims(&dims)?, out)
}

// ---------------------------------------------------------------------------

/// Validate the left argument and return one integer count per axis of `B`.
///
/// A scalar B is treated as rank 1. GNU APL requires `≢X` to equal the rank
/// of B for rank ≥ 2; a 1-element X against a vector is the common case.
fn axis_counts(x: &ValueP, b: &ValueP) -> AplResult<Vec<i64>> {
    if x.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    let counts: Vec<i64> = x
        .cells()
        .iter()
        .map(|c| c.get_int_value())
        .collect::<Result<_, _>>()?;
    if counts.is_empty() {
        return Err(ErrorCode::LengthError);
    }
    let rank = b.rank().max(1) as usize;
    if counts.len() != rank {
        return Err(ErrorCode::LengthError);
    }
    Ok(counts)
}

/// dimensions of `b`, treating a scalar as a 1-element vector
fn shape_dims(b: &ValueP) -> Vec<i64> {
    if b.rank() == 0 {
        vec![1]
    } else {
        (0..b.rank()).map(|i| b.get_shape_item(i as i16)).collect()
    }
}

/// row-major subscripts → linear index
fn encode(subs: &[i64], dims: &[i64]) -> i64 {
    let mut lin = 0i64;
    for (ax, &s) in subs.iter().enumerate() {
        lin = lin * dims[ax].max(1) + s;
    }
    lin
}

/// increment a row-major subscript vector in place
fn bump(subs: &mut [i64], dims: &[i64]) {
    for ax in (0..subs.len()).rev() {
        subs[ax] += 1;
        if subs[ax] < dims[ax] {
            return;
        }
        subs[ax] = 0;
    }
}

/// single integer left argument — used by the AXIS forms `↑[a]` / `↓[a]`,
/// where exactly one count applies to the named axis. (The plain forms take
/// one count PER axis; see `axis_counts`.)
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

    let lines: Vec<(usize, Vec<(usize, Vec<Cell>)>)> =
        if pre.max(1) as usize * post.max(1) as usize >= crate::functions::PARALLEL_THRESHOLD {
            use rayon::prelude::*;
            (0..pre.max(1) as usize)
                .into_par_iter()
                .map(|p| {
                    let mut results = Vec::with_capacity(post.max(1) as usize);
                    for s in 0..post.max(1) as usize {
                        let line: Vec<Cell> = (0..n as usize)
                            .map(|k| cells[(p * n as usize + k) * post as usize + s].clone())
                            .collect();
                        let taken = take_vec(&line, count, &proto);
                        results.push((s, taken));
                    }
                    (p, results)
                })
                .collect()
        } else {
            let mut results = Vec::with_capacity(pre.max(1) as usize);
            for p in 0..pre.max(1) as usize {
                let mut p_results = Vec::with_capacity(post.max(1) as usize);
                for s in 0..post.max(1) as usize {
                    let line: Vec<Cell> = (0..n as usize)
                        .map(|k| cells[(p * n as usize + k) * post as usize + s].clone())
                        .collect();
                    let taken = take_vec(&line, count, &proto);
                    p_results.push((s, taken));
                }
                results.push((p, p_results));
            }
            results
        };

    for (p, p_results) in lines {
        for (s, taken) in p_results {
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

    let lines: Vec<(usize, Vec<(usize, Vec<Cell>)>)> =
        if pre.max(1) as usize * post.max(1) as usize >= crate::functions::PARALLEL_THRESHOLD {
            use rayon::prelude::*;
            (0..pre.max(1) as usize)
                .into_par_iter()
                .map(|p| {
                    let mut results = Vec::with_capacity(post.max(1) as usize);
                    for s in 0..post.max(1) as usize {
                        let line: Vec<Cell> = (0..n as usize)
                            .map(|k| cells[(p * n as usize + k) * post as usize + s].clone())
                            .collect();
                        let kept = drop_vec(&line, count, &prototype(b));
                        results.push((s, kept));
                    }
                    (p, results)
                })
                .collect()
        } else {
            let mut results = Vec::with_capacity(pre.max(1) as usize);
            for p in 0..pre.max(1) as usize {
                let mut p_results = Vec::with_capacity(post.max(1) as usize);
                for s in 0..post.max(1) as usize {
                    let line: Vec<Cell> = (0..n as usize)
                        .map(|k| cells[(p * n as usize + k) * post as usize + s].clone())
                        .collect();
                    let kept = drop_vec(&line, count, &prototype(b));
                    p_results.push((s, kept));
                }
                results.push((p, p_results));
            }
            results
        };

    for (p, p_results) in lines {
        for (s, kept) in p_results {
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
    fn test_take_axis1_equals_per_axis_take_of_full_rows() {
        // ↑[1] takes along the last axis only, so it is equivalent to a
        // per-axis take that keeps ALL rows: 2↑[1]M ≡ 2 2↑M for a 2-row M.
        // (Plain `2↑M` on a matrix is a LENGTH ERROR — the left argument
        // needs one count per axis. Verified against the reference.)
        let b = mat(&[0, 1, 2, 3, 4, 5], 2, 3);
        let z = take_axis(&ValueP::int_vector(&[2]), &b, 1).unwrap();
        let z_per_axis = take(&ValueP::int_vector(&[2, 2]), &b).unwrap();
        assert_eq!(ints(&z), ints(&z_per_axis));
    }

    #[test]
    fn test_take_scalar_left_on_matrix_is_length_error() {
        // one count per axis is required for rank ≥ 2
        let b = mat(&[0, 1, 2, 3, 4, 5], 2, 3);
        assert!(take(&ValueP::int_vector(&[2]), &b).is_err());
        assert!(drop(&ValueP::int_vector(&[1]), &b).is_err());
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
    fn test_take_matrix_per_axis() {
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
        // 2 2↑M keeps 2 rows and 2 columns → 1 2 / 4 5 (reference-verified)
        let z = take(&ValueP::int_vector(&[2, 2]), &b).unwrap();
        assert_eq!(z.get_shape_item(0), 2);
        assert_eq!(z.get_shape_item(1), 2);
        assert_eq!(ints(&z), vec![1, 2, 4, 5]);
    }
}
