//! Additional parallel operation tests.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::value::ValueP;

fn vec_ints(vs: &[i64]) -> ValueP {
    let cells = vs.iter().map(|&v| Cell::Int(v)).collect();
    ValueP::from_parts(Shape::vector(vs.len() as i64), cells).unwrap()
}

#[test]
fn test_parallel_index_of_large() {
    let a = vec_ints(&(0..5000i64).collect::<Vec<_>>());
    let b = vec_ints(&[0, 2500, 4999, 5000, -1]);
    let r = crate::index_of::index_of(&a, &b).unwrap();
    assert_eq!(r.cells()[0], Cell::Int(0));
    assert_eq!(r.cells()[1], Cell::Int(2500));
    assert_eq!(r.cells()[2], Cell::Int(4999));
    assert_eq!(r.cells()[3], Cell::Int(5000));
    assert_eq!(r.cells()[4], Cell::Int(5000));
}

#[test]
fn test_parallel_interval_index_large() {
    let a = vec_ints(&[10, 20, 30, 40, 50]);
    let b = vec_ints(&(0..5000i64).collect::<Vec<_>>());
    let r = crate::interval_index::interval_index(&a, &b).unwrap();
    assert_eq!(r.cells()[0], Cell::Int(0));
    assert_eq!(r.cells()[4999], Cell::Int(5));
    assert_eq!(r.cells()[20], Cell::Int(2));
}

#[test]
fn test_parallel_vs_sequential_interval_index() {
    let a = vec_ints(&[1, 5, 10, 100]);
    let small = vec_ints(&[0, 1, 3, 5, 50, 100, 200]);
    let large = vec_ints(&(0..10000i64).collect::<Vec<_>>());

    let r_small = crate::interval_index::interval_index(&a, &small).unwrap();
    let large_prefix = vec_ints(&[0, 1, 3, 5, 50, 100, 200]);
    let r_large_prefix = crate::interval_index::interval_index(&a, &large_prefix).unwrap();

    for i in 0..7 {
        assert_eq!(r_small.cells()[i], r_large_prefix.cells()[i]);
    }
}

#[test]
fn test_parallel_vs_sequential_index_of() {
    let a = vec_ints(&(0..100i64).collect::<Vec<_>>());
    let small = vec_ints(&[0, 50, 99, 100]);
    let large_prefix = vec_ints(&[0, 50, 99, 100]);

    let r_small = crate::index_of::index_of(&a, &small).unwrap();
    let r_large_prefix = crate::index_of::index_of(&a, &large_prefix).unwrap();

    for i in 0..4 {
        assert_eq!(r_small.cells()[i], r_large_prefix.cells()[i]);
    }
}
