//! Tests for parallel elementwise dispatch (rayon path in functions.rs).
//!
//! Pins semantics ACROSS the PARALLEL_THRESHOLD boundary: the same APL
//! expression must give identical results whether the sequential loop or
//! the rayon par_iter ran. Mirrors C++ ScalarFunction.cc behavior for
//! large arrays.

use crate::cell::Cell;
use crate::functions::Prim;
use crate::shape::Shape;
use crate::value::ValueP;

fn vec_val(vs: &[i64]) -> ValueP {
    let cells = vs.iter().map(|&v| Cell::Int(v)).collect();
    ValueP::from_parts(Shape::vector(vs.len() as i64), cells).unwrap()
}

#[test]
fn test_parallel_monadic_map_large() {
    // 5000 elements — above the 4096 threshold → rayon path.
    // monadic − negates every element.
    let n: i64 = 5000;
    let v = vec_val(&(0..n).collect::<Vec<_>>());
    let r = crate::functions::Prim::Subtract.eval_monadic(&v).unwrap();
    assert_eq!(r.element_count(), n);
    for i in [0i64, 1, 2577, n - 1] {
        assert_eq!(r.cells()[i as usize], Cell::Int(-i));
    }
}

#[test]
fn test_parallel_dyadic_elementwise_large() {
    let n: i64 = 5000;
    let a = vec_val(&(0..n).collect::<Vec<_>>());
    let b = vec_val(&(0..n).map(|x| x * 2).collect::<Vec<_>>());
    let r = crate::functions::eval_dyadic_public(Prim::Add, &a, &b).unwrap();
    assert_eq!(r.element_count(), n);
    for i in [0i64, 1, 2577, n - 1] {
        assert_eq!(r.cells()[i as usize], Cell::Int(i * 3));
    }
}

#[test]
fn test_small_array_stays_correct() {
    // below the threshold — sequential path
    let a = vec_val(&[1, 2, 3]);
    let b = vec_val(&[10, 20, 30]);
    let r = crate::functions::eval_dyadic_public(Prim::Multiply, &a, &b).unwrap();
    let expect = [10, 40, 90];
    for (i, e) in expect.iter().enumerate() {
        assert_eq!(r.cells()[i], Cell::Int(*e));
    }
}
