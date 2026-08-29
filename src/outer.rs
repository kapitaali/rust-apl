//! Outer product `A ∘.f B`.
//!
//! Mirrors `src/Bif_OPER2_OUTER.cc`: result shape is (⍴A, ⍴B) — every
//! combination of an A-element (as LEFT arg) and a B-element (as RIGHT
//! arg). Nested results are boxed.

use crate::cell::Cell;
use crate::functions::Prim;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

pub fn outer_product(a: &ValueP, p: Prim, b: &ValueP) -> AplResult<ValueP> {
    let prim = p;
    let na = a.element_count() as usize;
    let nb = b.element_count() as usize;
    let total = na * nb;

    let out: Vec<Cell> = if total >= crate::functions::PARALLEL_THRESHOLD {
        // Large outer products: parallelize over result cells
        use rayon::prelude::*;
        let acells = a.cells();
        let bcells = b.cells();
        (0..total)
            .into_par_iter()
            .map(|idx| {
                let i = idx / nb;
                let j = idx % nb;
                let av = ValueP::from_parts(crate::shape::Shape::scalar(), vec![acells[i].clone()])
                    .map_err(|_| ErrorCode::DomainError)?;
                let bv = ValueP::from_parts(crate::shape::Shape::scalar(), vec![bcells[j].clone()])
                    .map_err(|_| ErrorCode::DomainError)?;
                let v = prim.eval_dyadic(&av, &bv)?;
                if v.is_scalar() {
                    Ok(v.first_cell().unwrap().clone())
                } else {
                    Ok(Cell::pointer(v.inner.clone()))
                }
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut cells = Vec::with_capacity(total);
        for ac in a.cells() {
            for bc in b.cells() {
                let av = ValueP::from_parts(crate::shape::Shape::scalar(), vec![ac.clone()])
                    .map_err(|_| ErrorCode::DomainError)?;
                let bv = ValueP::from_parts(crate::shape::Shape::scalar(), vec![bc.clone()])
                    .map_err(|_| ErrorCode::DomainError)?;
                let v = prim.eval_dyadic(&av, &bv)?;
                if v.is_scalar() {
                    cells.push(v.first_cell().unwrap().clone());
                } else {
                    cells.push(Cell::pointer(v.inner.clone()));
                }
            }
        }
        cells
    };

    // result shape: (⍴A) ++ (⍴B)
    let mut dims: Vec<i64> = Vec::new();
    for k in 0..a.rank() as usize {
        dims.push(a.get_shape_item(k as i16));
    }
    for k in 0..b.rank() as usize {
        dims.push(b.get_shape_item(k as i16));
    }
    let shape = crate::shape::Shape::from_dims(&dims)?;
    ValueP::from_parts(shape, out)
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_val(vs: &[i64]) -> ValueP {
        let cells = vs.iter().map(|&v| Cell::Int(v)).collect();
        ValueP::from_parts(crate::shape::Shape::vector(vs.len() as i64), cells).unwrap()
    }

    #[test]
    fn test_outer_times() {
        // 1 2 ∘.× 10 20 30 → 2×3 matrix 10 20 30 / 20 40 60
        let a = vec_val(&[1, 2]);
        let b = vec_val(&[10, 20, 30]);
        let r = outer_product(&a, Prim::Multiply, &b).unwrap();
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 2);
        assert_eq!(r.get_shape_item(1), 3);
        let expect = [10, 20, 30, 20, 40, 60];
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(r.cells()[i], Cell::Int(*e));
        }
    }

    #[test]
    fn test_outer_parallel_path() {
        // Large enough to trigger the parallel path (≥4096 result cells)
        let n = 64;
        let a: Vec<i64> = (0..n).collect();
        let b: Vec<i64> = (0..n).collect();
        let av = vec_val(&a);
        let bv = vec_val(&b);
        let r = outer_product(&av, Prim::Add, &bv).unwrap();
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), n);
        assert_eq!(r.get_shape_item(1), n);
        // spot check: 0+0=0, 1+1=2, 63+63=126
        assert_eq!(r.cells()[0], Cell::Int(0));
        assert_eq!(r.cells()[n as usize + 1], Cell::Int(2));
        assert_eq!(r.cells()[(n * n - 1) as usize], Cell::Int(126));
    }

    #[test]
    fn test_outer_le() {
        // 1 2 ∘.≤ 1 2 → rows: 1≤1 1≤2 = 1 1 ; 2≤1 2≤2 = 0 1
        let a = vec_val(&[1, 2]);
        let b = vec_val(&[1, 2]);
        let r = outer_product(&a, Prim::LessEq, &b).unwrap();
        assert_eq!(r.rank(), 2);
        let expect = [1, 1, 0, 1];
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(r.cells()[i], Cell::Int(*e));
        }
    }
}
