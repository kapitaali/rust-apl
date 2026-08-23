use apl::cell::Cell;
use apl::functions::Prim;
use apl::value::ValueP;
fn main() {
    let mk = |vs: &[i64]| {
        ValueP::from_parts(
            apl::shape::Shape::vector(vs.len() as i64),
            vs.iter().map(|&v| Cell::Int(v)).collect(),
        )
        .unwrap()
    };
    let a = mk(&[1, 2]);
    let b = mk(&[10, 20, 30]);
    match apl::outer::outer_product(&a, Prim::Multiply, &b) {
        Ok(r) => println!("rank={} cells={:?}", r.rank(), r.cells()),
        Err(e) => println!("ERR {:?}", e),
    }
}
