//! Inner product `A f.g B`.
//!
//! Mirrors `src/Bif_OPER2_INNER_PRODUCT.cc` (simplified): the contraction
//! axis is A's LAST axis and B's FIRST axis — they must agree in length
//! (LENGTH_ERROR otherwise). Scalars are promoted to 1-element vectors,
//! so `x +.× M` and `M +.× x` work like matrix-vector products. Each
//! result cell is `f/` applied to the elementwise pairing `g` over the
//! shared axis; the reduction reuses `operators::reduce`, so an empty
//! contraction axis falls back to `f`'s identity value.

use crate::functions::Prim;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

pub fn inner_product(a: &ValueP, f: Prim, g: Prim, b: &ValueP) -> AplResult<ValueP> {
    // Effective dimension lists; scalars promote to length-1 axes.
    let mut adims: Vec<i64> = (0..a.rank() as usize)
        .map(|k| a.get_shape_item(k as i16))
        .collect();
    let mut bdims: Vec<i64> = (0..b.rank() as usize)
        .map(|k| b.get_shape_item(k as i16))
        .collect();
    if adims.is_empty() {
        adims.push(1);
    }
    if bdims.is_empty() {
        bdims.push(1);
    }

    let n = *adims.last().expect("non-empty after promotion");
    if *bdims.first().expect("non-empty after promotion") != n {
        return Err(ErrorCode::LengthError);
    }

    // Frame sizes: everything before A's last axis / after B's first axis.
    let pa: usize = adims[..adims.len() - 1].iter().product::<i64>() as usize;
    let pb: usize = bdims[1..].iter().product::<i64>() as usize;

    let acells = a.cells();
    let bcells = b.cells();

    let mut out = Vec::with_capacity(pa * pb);
    for i in 0..pa {
        for j in 0..pb {
            // gather the shared-axis pairings and apply g elementwise
            let mut pair = Vec::with_capacity(n as usize);
            for k in 0..n as usize {
                let av =
                    ValueP::from_parts(Shape::scalar(), vec![acells[i * n as usize + k].clone()])
                        .map_err(|_| ErrorCode::DomainError)?;
                let bv = ValueP::from_parts(Shape::scalar(), vec![bcells[k * pb + j].clone()])
                    .map_err(|_| ErrorCode::DomainError)?;
                let r = g.eval_dyadic(&av, &bv)?;
                if r.is_scalar() {
                    pair.push(r.first_cell().expect("scalar has a cell").clone());
                } else {
                    // non-scalar g result cannot be contracted
                    return Err(ErrorCode::DomainError);
                }
            }
            let tmp = ValueP::from_parts(Shape::vector(n), pair)?;
            let red = crate::operators::reduce(f, &tmp)?;
            out.push(red.first_cell().ok_or(ErrorCode::DomainError)?.clone());
        }
    }

    // result shape: A's leading axes ++ B's trailing axes
    let mut rdims: Vec<i64> = adims[..adims.len() - 1].to_vec();
    rdims.extend_from_slice(&bdims[1..]);
    let shape = if rdims.is_empty() {
        Shape::scalar()
    } else {
        Shape::from_dims(&rdims)?
    };
    ValueP::from_parts(shape, out)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    fn vec_val(vs: &[i64]) -> ValueP {
        let cells = vs.iter().map(|&v| Cell::Int(v)).collect();
        ValueP::from_parts(crate::shape::Shape::vector(vs.len() as i64), cells).unwrap()
    }

    fn mat_val(rows: i64, cols: i64, vs: &[i64]) -> ValueP {
        let cells = vs.iter().map(|&v| Cell::Int(v)).collect();
        ValueP::from_parts(crate::shape::Shape::matrix(rows, cols), cells).unwrap()
    }

    #[test]
    fn test_dot_product() {
        // 1 2 3 +.× 10 20 30 → 140
        let a = vec_val(&[1, 2, 3]);
        let b = vec_val(&[10, 20, 30]);
        let r = inner_product(&a, Prim::Add, Prim::Multiply, &b).unwrap();
        assert!(r.is_scalar());
        assert_eq!(r.first_cell().unwrap(), &Cell::Int(140));
    }

    #[test]
    fn test_matrix_times_vector() {
        // [[1,2],[3,4]] +.× 5 6 → 17 39
        let m = mat_val(2, 2, &[1, 2, 3, 4]);
        let v = vec_val(&[5, 6]);
        let r = inner_product(&m, Prim::Add, Prim::Multiply, &v).unwrap();
        assert_eq!(r.rank(), 1);
        assert_eq!(r.get_shape_item(0), 2);
        assert_eq!(r.cells()[0], Cell::Int(17)); // 1·5 + 2·6
        assert_eq!(r.cells()[1], Cell::Int(39)); // 3·5 + 4·6
    }

    #[test]
    fn test_matrix_times_matrix() {
        // [[1,2],[3,4]] +.× [[5,6],[7,8]] → [[19,22],[43,50]]
        let a = mat_val(2, 2, &[1, 2, 3, 4]);
        let b = mat_val(2, 2, &[5, 6, 7, 8]);
        let r = inner_product(&a, Prim::Add, Prim::Multiply, &b).unwrap();
        assert_eq!(r.rank(), 2);
        let expect = [19, 22, 43, 50];
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(r.cells()[i], Cell::Int(*e));
        }
    }

    #[test]
    fn test_max_sum() {
        // 1 2 3 ⌈.+ 4 5 6 → ⌈/(1+4)(2+5)(3+6) = ⌈/5 7 9 = 9
        let a = vec_val(&[1, 2, 3]);
        let b = vec_val(&[4, 5, 6]);
        let r = inner_product(&a, Prim::Ceiling, Prim::Add, &b).unwrap();
        assert_eq!(r.first_cell().unwrap(), &Cell::Float(9.0));
    }

    #[test]
    fn test_length_error_on_mismatched_axes() {
        let a = vec_val(&[1, 2]);
        let b = vec_val(&[1, 2, 3]);
        assert_eq!(
            inner_product(&a, Prim::Add, Prim::Multiply, &b).unwrap_err(),
            ErrorCode::LengthError
        );
    }
}
