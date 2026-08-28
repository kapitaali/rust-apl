//! Over `⍥` — composes two functions before applying.
//!
//! Dyalog APL:
//! - (f⍥g)B — monadic over: f(g(B))
//! - A(f⍥g)B — dyadic over: f(g(A), g(B))

use crate::functions::Prim;
use crate::parser::Environment;
use crate::types::AplResult;
use crate::value::ValueP;

/// Evaluate (f⍥g)B — monadic over.
pub fn over_monadic(
    f: Prim,
    g: Prim,
    b: &crate::parser::Expr,
    env: &mut Environment,
) -> AplResult<ValueP> {
    // Apply g to B
    let gb = apply_prim_monadic(g, b, env)?;
    // Apply f to g(B)
    apply_prim_val(f, &gb)
}

/// Evaluate A(f⍥g)B — dyadic over.
pub fn over_dyad(
    f: Prim,
    g: Prim,
    a: &crate::parser::Expr,
    b: &crate::parser::Expr,
    env: &mut Environment,
) -> AplResult<ValueP> {
    // Apply g to A and B
    let ga = apply_prim_monadic(g, a, env)?;
    let gb = apply_prim_monadic(g, b, env)?;
    // Apply f to g(A), g(B)
    apply_prim_val_dyad(f, &ga, &gb)
}

fn apply_prim_monadic(
    p: Prim,
    operand: &crate::parser::Expr,
    env: &mut Environment,
) -> AplResult<ValueP> {
    let derived = crate::parser::Expr::Monadic(p, Box::new(operand.clone()));
    env.eval(&derived)
}

fn apply_prim_val(p: Prim, val: &ValueP) -> AplResult<ValueP> {
    p.eval_monadic(val)
}

fn apply_prim_val_dyad(p: Prim, lhs: &ValueP, rhs: &ValueP) -> AplResult<ValueP> {
    p.eval_dyadic(lhs, rhs)
}
