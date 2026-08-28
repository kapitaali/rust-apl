//! Over `⍥` — composes two functions before applying.
//!
//! Dyalog APL:
//! - (f⍥g)B — monadic over: f(g(B))
//! - A(f⍥g)B — dyadic over: f(g(A), g(B))

use crate::parser::Environment;
use crate::types::AplResult;
use crate::value::ValueP;

/// Evaluate (f⍥g)B — monadic over.
pub fn over_monadic(
    f: &crate::parser::Expr,
    g: &crate::parser::Expr,
    b: &crate::parser::Expr,
    env: &mut Environment,
) -> AplResult<ValueP> {
    // Apply g to B
    let gb = apply_func(g, b, env)?;
    // Apply f to g(B)
    apply_func_val(f, &gb, env)
}

/// Evaluate A(f⍥g)B — dyadic over.
pub fn over_dyad(
    f: &crate::parser::Expr,
    g: &crate::parser::Expr,
    a: &crate::parser::Expr,
    b: &crate::parser::Expr,
    env: &mut Environment,
) -> AplResult<ValueP> {
    // Apply g to A and B separately
    let ga = apply_func(g, a, env)?;
    let gb = apply_func(g, b, env)?;
    // Apply f to g(A) and g(B)
    apply_func_two_args(f, &ga, &gb, env)
}

/// Apply an expression (function) to an argument expression.
fn apply_func(
    func: &crate::parser::Expr,
    arg: &crate::parser::Expr,
    env: &mut Environment,
) -> AplResult<ValueP> {
    let arg_val = env.eval(arg)?;
    apply_func_val(func, &arg_val, env)
}

/// Apply an expression (function) to a pre-evaluated argument.
fn apply_func_val(
    func: &crate::parser::Expr,
    arg_val: &ValueP,
    env: &mut Environment,
) -> AplResult<ValueP> {
    match func {
        crate::parser::Expr::Monadic(p, _) => p.eval_monadic(arg_val),
        crate::parser::Expr::Dyadic(p, _, _) => crate::functions::eval_dyadic_public(*p, arg_val, arg_val),
        _ => Err(crate::types::ErrorCode::DomainError),
    }
}

/// Apply an expression (function) to two pre-evaluated arguments.
fn apply_func_two_args(
    func: &crate::parser::Expr,
    a: &ValueP,
    b: &ValueP,
    _env: &mut Environment,
) -> AplResult<ValueP> {
    match func {
        crate::parser::Expr::Dyadic(p, _, _) => crate::functions::eval_dyadic_public(*p, a, b),
        crate::parser::Expr::Monadic(p, _) => p.eval_monadic(a),
        _ => Err(crate::types::ErrorCode::DomainError),
    }
}
