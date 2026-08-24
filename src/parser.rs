//! Recursive-descent parser and evaluator for APL expressions.
//!
//! Mirrors `src/Parser.cc` + the prefix machine (simplified): handles
//! monadic/dyadic function application, parentheses, assignment, with
//! right-to-left (APL) evaluation order.

use std::collections::HashMap;

use crate::cell::Cell;
use crate::functions::Prim;
use crate::tokenizer::{tokenize, Tok};
use crate::types::AplResult;
use crate::types::ErrorCode;
use crate::value::ValueP;
use apl_ext::XValue;

/// A parsed expression.
#[derive(Clone, Debug)]
pub enum Expr {
    Num(f64),
    /// a numeric strand: adjacent literals `2 3 4`
    NumVec(Vec<f64>),
    /// a nested strand: adjacent parenthesized groups `(1 2)(3 4)` or
    /// mixed `(1)(2 3)` — each element is enclosed
    NestedVec(Vec<Expr>),
    Str(Vec<u32>),
    Var(String),
    Monadic(Prim, Box<Expr>),
    /// `LO/B` — reduce
    ReduceOp(Prim, Box<Expr>),
    /// `LO\B` — scan
    ScanOp(Prim, Box<Expr>),
    /// `LO⌿B` — first-axis reduce
    Reduce1Op(Prim, Box<Expr>),
    /// `LO⍀B` — first-axis scan
    Scan1Op(Prim, Box<Expr>),
    /// `F¨B` — each (monadic)
    EachOp(Prim, Box<Expr>),
    /// `A F¨B` — each (dyadic)
    EachDyad(Prim, Box<Expr>, Box<Expr>),
    /// `A ∘.f B` — outer product
    OuterProduct(Prim, Box<Expr>, Box<Expr>),
    /// `A f.g B` — inner product (f = reduction, g = pairwise function)
    InnerProduct(Prim, Prim, Box<Expr>, Box<Expr>),
    /// `A F[axis] B` — dyadic with explicit axis (take/drop/rotate)
    DyadicAxis(Prim, Box<Expr>, Box<Expr>, Box<Expr>),
    /// `⎕EA guarded ⋄ fallback` — evaluate guarded; on error, evaluate fallback
    ErrorGuard(Box<Expr>, Box<Expr>),
    /// `[apl_name ⎕NA decl]` — associate a native function (⎕NA).
    /// apl_name None means use the symbol name from the declaration.
    QuadNa(Option<Box<Expr>>, Box<Expr>),
    /// `⎕LOADSO 'path'` — load a Rust plugin cdylib; registers all its
    /// bindings into the function table.
    QuadLoadSo(Box<Expr>),
    /// `NAME[expr]` — bracket indexing
    Index(Box<Expr>, Box<Expr>),
    Dyadic(Prim, Box<Expr>, Box<Expr>),
    Assign(String, Box<Expr>),
    /// selective assignment: `NAME[idx] ← expr`
    AssignIndexed(String, Box<Expr>, Box<Expr>),
    /// selective pick assignment: `(A⊃NAME) ← expr`
    AssignPick(String, Box<Expr>, Box<Expr>),
    /// defined-function call: monadic `FN B` or ambivalent `FN`
    FuncCallMono(String, Option<Box<Expr>>),
    /// defined-function call: dyadic `A FN B`
    FuncCallDyad(String, Box<Expr>, Box<Expr>),
    /// a dfn `{...}` — evaluates to an anonymous function value that can
    /// be called immediately (`{...}B` / `A{...}B`) or assigned to a name.
    /// Evaluation strategy: the body compiles into an anonymous
    /// DefinedFunction with arg names ⍺/⍵; calling binds them through the
    /// ordinary shadowing mechanism (see eval of Dfn/DfnCallMono/DfnCallDyad).
    Dfn(Box<Expr>),
    /// immediate dfn call: `{BODY} ARG` — ⍵ bound to ARG's value
    DfnCallMono(Box<Expr>, Box<Expr>),
    /// immediate dfn call: `LARG {BODY} RARG` — ⍺/⍵ bound
    DfnCallDyad(Box<Expr>, Box<Expr>, Box<Expr>),
    /// dfn argument references: ⍺ (left arg) and ⍵ (right arg)
    Alpha,
    Omega,
    /// dfn self-call: ∇ B (monadic) or A ∇ B (dyadic) — calls the
    /// enclosing dfn recursively
    SelfCall(Box<Expr>),
    SelfCallDyad(Box<Expr>, Box<Expr>),
    /// dfn left operand function reference `⍺⍺`
    AlphaAlpha,
    /// dfn right operand function reference `⍵⍵`
    OmegaOmega,
    /// NAME ← {BODY} — named dfn definition
    AssignDfn(String, Box<Expr>),
    /// a sequence of expressions separated by `⋄` (diamond) — used for
    /// multi-statement dfn bodies: {e1 ⋄ e2 ⋄ e3}. Evaluates each in order,
    /// returns the last.
    DiamondList(Vec<Expr>),
    /// a sequence of expressions: e1 e2 e3 ... evaluates each, returns last.
    /// Used for multi-term dfn bodies and eval-time substitution results.
    Seq(Vec<Expr>),
    /// if-then-else: If(cond, then, else) — used for desugaring guarded
    /// expressions in dfns: {c1:e1 ⋄ c2:e2 ⋄ e3} → If(c1,e1,If(c2,e2,e3))
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    /// dop call: LO FN RO B (monadic) or A LO FN RO B (dyadic) — call dfn
    /// with operand primitives bound. left=None for monadic calls.
    DopCall(String, Prim, Prim, Option<Box<Expr>>, Box<Expr>),
    /// Apply an operand placeholder (⍺⍺/⍵⍵) to an argument.
    /// Produced by dfn body parsing when a multi-term statement starts with
    /// AlphaAlpha or OmegaOmega. substitute_dop resolves it to Monadic(p, arg)
    /// when the dop context is known; reaching eval unresolved is a SyntaxError.
    ApplyOp(Box<Expr>, Box<Expr>),
    /// Apply an operand placeholder (⍺⍺/⍵⍵) dyadically: `⍺ ⍺⍺ ⍵`.
    /// Produced by dfn body parsing for 3-term bodies where the middle term
    /// is AlphaAlpha or OmegaOmega. substitute_dop resolves to Dyadic(p, a, b).
    DyadicApply(Box<Expr>, Box<Expr>, Box<Expr>),
    /// compile-time function reference placeholder for dop bodies.
    /// When a dfn body references a function by name (not ⍺⍺/⍵⍵), this
    /// wraps the name so substitute_dop can pass it through unchanged.
    FuncRef(String),
}

/// compile a dfn body expression into an anonymous DefinedFunction whose
/// ⍺/⍵ args are wired through the standard call machinery.
///
/// The stored `result`/`source` use private-use markers that are NOT valid
/// APL; such functions are marked `no_save` so workspace save() skips them
/// (a dfn's real source text isn't retained — only named ∇-functions are).
fn dfn_to_function(body: &Expr) -> crate::functions_def::DefinedFunction {
    crate::functions_def::DefinedFunction {
        name: DFNS_PREFIX.to_string(),
        result: Some(DFN_RESULT.to_string()),
        arg_left: Some("⍺".to_string()),
        arg_right: Some("⍵".to_string()),
        body: vec![body.clone()],
        control: Vec::new(),
        leave_lines: Vec::new(),
        source: vec![DFN_BODY_MARK.to_string()],
        no_save: true,
        is_dfn: true,
        is_dop: false,
        dop_lo: None,
        dop_ro: None,
    }
}

/// substitute ⍺⍺ → dop_lo and ⍵⍵ → dop_ro throughout an expression
fn substitute_dop(
    e: &Expr,
    dop_lo: Option<crate::functions::Prim>,
    dop_ro: Option<crate::functions::Prim>,
) -> Expr {
    match e {
        Expr::AlphaAlpha => {
            if let Some(p) = dop_lo {
                Expr::Monadic(p, Box::new(Expr::Omega))
            } else {
                Expr::AlphaAlpha
            }
        }
        Expr::OmegaOmega => {
            if let Some(p) = dop_ro {
                Expr::Monadic(p, Box::new(Expr::Omega))
            } else {
                Expr::OmegaOmega
            }
        }
        Expr::Monadic(p, b) => Expr::Monadic(*p, Box::new(substitute_dop(b, dop_lo, dop_ro))),
        Expr::Dyadic(p, a, b) => Expr::Dyadic(
            *p,
            Box::new(substitute_dop(a, dop_lo, dop_ro)),
            Box::new(substitute_dop(b, dop_lo, dop_ro)),
        ),
        Expr::Dfn(body) => Expr::Dfn(Box::new(substitute_dop(body, dop_lo, dop_ro))),
        Expr::DfnCallMono(body, arg) => Expr::DfnCallMono(
            Box::new(substitute_dop(body, dop_lo, dop_ro)),
            Box::new(substitute_dop(arg, dop_lo, dop_ro)),
        ),
        Expr::DfnCallDyad(larg, body, rarg) => Expr::DfnCallDyad(
            Box::new(substitute_dop(larg, dop_lo, dop_ro)),
            Box::new(substitute_dop(body, dop_lo, dop_ro)),
            Box::new(substitute_dop(rarg, dop_lo, dop_ro)),
        ),
        Expr::If(c, t, e) => Expr::If(
            Box::new(substitute_dop(c, dop_lo, dop_ro)),
            Box::new(substitute_dop(t, dop_lo, dop_ro)),
            Box::new(substitute_dop(e, dop_lo, dop_ro)),
        ),
        Expr::DiamondList(exprs) => Expr::DiamondList(
            exprs
                .iter()
                .map(|e| substitute_dop(e, dop_lo, dop_ro))
                .collect(),
        ),
        Expr::Seq(exprs) => Expr::Seq(
            exprs
                .iter()
                .map(|e| substitute_dop(e, dop_lo, dop_ro))
                .collect(),
        ),
        Expr::SelfCall(arg) => Expr::SelfCall(Box::new(substitute_dop(arg, dop_lo, dop_ro))),
        Expr::SelfCallDyad(larg, rarg) => Expr::SelfCallDyad(
            Box::new(substitute_dop(larg, dop_lo, dop_ro)),
            Box::new(substitute_dop(rarg, dop_lo, dop_ro)),
        ),
        Expr::FuncCallMono(name, arg) => Expr::FuncCallMono(
            name.clone(),
            arg.as_ref()
                .map(|a| Box::new(substitute_dop(a, dop_lo, dop_ro))),
        ),
        Expr::FuncCallDyad(name, a, b) => Expr::FuncCallDyad(
            name.clone(),
            Box::new(substitute_dop(a, dop_lo, dop_ro)),
            Box::new(substitute_dop(b, dop_lo, dop_ro)),
        ),
        Expr::DopCall(name, lo, ro, left, rhs) => Expr::DopCall(
            name.clone(),
            *lo,
            *ro,
            left.as_ref()
                .map(|l| Box::new(substitute_dop(l, dop_lo, dop_ro))),
            Box::new(substitute_dop(rhs, dop_lo, dop_ro)),
        ),
        // ⍺⍺ arg → Monadic(dop_lo, arg) when dop_lo is known
        Expr::ApplyOp(f, arg) => {
            match f.as_ref() {
                Expr::AlphaAlpha if dop_lo.is_some() => Expr::Monadic(
                    dop_lo.unwrap(),
                    Box::new(substitute_dop(arg, dop_lo, dop_ro)),
                ),
                Expr::OmegaOmega if dop_ro.is_some() => Expr::Monadic(
                    dop_ro.unwrap(),
                    Box::new(substitute_dop(arg, dop_lo, dop_ro)),
                ),
                // any other func: just recurse
                _ => Expr::ApplyOp(
                    Box::new(substitute_dop(f, dop_lo, dop_ro)),
                    Box::new(substitute_dop(arg, dop_lo, dop_ro)),
                ),
            }
        }
        Expr::FuncRef(_) => e.clone(),
        Expr::DyadicApply(a, f, b) => {
            match f.as_ref() {
                Expr::AlphaAlpha if dop_lo.is_some() => Expr::Dyadic(
                    dop_lo.unwrap(),
                    Box::new(substitute_dop(a, dop_lo, dop_ro)),
                    Box::new(substitute_dop(b, dop_lo, dop_ro)),
                ),
                Expr::OmegaOmega if dop_ro.is_some() => Expr::Dyadic(
                    dop_ro.unwrap(),
                    Box::new(substitute_dop(a, dop_lo, dop_ro)),
                    Box::new(substitute_dop(b, dop_lo, dop_ro)),
                ),
                // any other func: just recurse
                _ => Expr::DyadicApply(
                    Box::new(substitute_dop(a, dop_lo, dop_ro)),
                    Box::new(substitute_dop(f, dop_lo, dop_ro)),
                    Box::new(substitute_dop(b, dop_lo, dop_ro)),
                ),
            }
        }
        other => other.clone(),
    }
}

/// internal markers for anonymous dfns (never valid APL names)
pub const DFNS_PREFIX: &str = "\u{f0000}dfn";
const DFN_RESULT: &str = "\u{f0000}r";
const DFN_BODY_MARK: &str = "\u{f0000}body";

/// A specification target on the left of ← (for future generalization).
#[derive(Clone, Debug)]
pub enum SpecTarget {
    /// NAME[idx] — ravel indexing
    Bracket(Box<Expr>),
    /// A⊃NAME — pick path
    Pick(Box<Expr>),
}

/// Parse a token slice into an Expr (APL right-to-left precedence).
pub fn parse(toks: &[Tok]) -> AplResult<(Expr, usize)> {
    parse_expr(toks)
}

/// Invoke a plugin binding with one APL value (converted to XValue).
///
/// Unlike ⎕NA CAbi calls, plugin bindings declare their own arity and each
/// takes its arguments as ENCLOSED items: `FN ⊂arg1 ⊂arg2`. A single
/// non-enclosed value passes as-is.
fn call_plugin(pb: &crate::ffi::plugin::PluginBinding, arg_v: &ValueP) -> AplResult<ValueP> {
    let xargs: Vec<XValue> = if arg_v.is_scalar() {
        match arg_v.cells().first() {
            Some(Cell::Pointer(p)) => vec![crate::ffi::plugin::value_to_xvalue(&ValueP {
                inner: p.value.clone(),
            })?],
            _ => vec![crate::ffi::plugin::value_to_xvalue(arg_v)?],
        }
    } else {
        vec![crate::ffi::plugin::value_to_xvalue(arg_v)?]
    };

    let ctx = crate::ffi::plugin::make_context(0, 1e-13);
    let out = pb.call(&ctx, &xargs)?;
    crate::ffi::plugin::xvalue_to_value(&out)
}

/// expr := name '←' expr | name '[' expr ']' '←' expr
///       | '(' A⊃name ')' '←' expr | simple
fn parse_expr(toks: &[Tok]) -> AplResult<(Expr, usize)> {
    // ⎕NA association: [apl_name] ⎕NA decl_string
    // apl_name may be a Str literal ('div') or a Name (div)
    {
        let name_expr: Option<Box<Expr>> = match toks.first() {
            Some(Tok::Name(n)) if n == "⎕NA" => None,
            Some(Tok::Name(_)) | Some(Tok::Str(_)) => {
                let second_is_na = matches!(toks.get(1), Some(Tok::Name(m)) if m == "⎕NA");
                if second_is_na {
                    let (e, used) = parse_term(toks)?;
                    debug_assert_eq!(used, 1);
                    Some(Box::new(e))
                } else {
                    None
                }
            }
            _ => None,
        };
        let na_at = if name_expr.is_some() { 1 } else { 0 };
        let is_na = matches!(toks.get(na_at), Some(Tok::Name(m)) if m == "⎕NA");
        if is_na {
            let (decl, dused) = parse(&toks[na_at + 1..])?;
            return Ok((Expr::QuadNa(name_expr, Box::new(decl)), na_at + 1 + dused));
        }
    }
    // ⎕LOADSO 'path' — load a Rust plugin cdylib and register its bindings
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕LOADSO" {
            let (spec, sused) = parse(&toks[1..])?;
            return Ok((Expr::QuadLoadSo(Box::new(spec)), 1 + sused));
        }
    }
    // error guard: ⎕EA guarded ⋄ fallback
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕EA" {
            if let Some(diamond_pos) = toks.iter().position(|t| matches!(t, Tok::Diamond)) {
                // guard expr is toks[1..diamond]; fallback after the diamond.
                // Nested diamonds in the guard would need smarter splitting —
                // use the FIRST diamond (guards can't contain ⋄ for now).
                let guard_toks = &toks[1..diamond_pos];
                let (guard, _gused) = if guard_toks.is_empty() {
                    return Err(ErrorCode::SyntaxError);
                } else {
                    parse(guard_toks)?
                };
                let (fallback, fused) = parse(&toks[diamond_pos + 1..])?;
                if !matches!(
                    toks.get(diamond_pos + 1 + fused),
                    Some(Tok::End) | Some(Tok::Diamond)
                ) {
                    return Err(ErrorCode::SyntaxError);
                }
                return Ok((
                    Expr::ErrorGuard(Box::new(guard), Box::new(fallback)),
                    diamond_pos + 1 + fused,
                ));
            }
        }
    }
    // selective pick assignment: (A⊃NAME) ← expr — line starts with LParen
    if matches!(toks.first(), Some(Tok::LParen)) {
        if let Some((path, name, pused)) = try_parse_pick_target(toks)? {
            if matches!(toks.get(pused + 1), Some(Tok::Assign)) {
                let (rhs, rused) = parse_expr(&toks[pused + 2..])?;
                return Ok((
                    Expr::AssignPick(name, Box::new(path), Box::new(rhs)),
                    pused + 2 + rused,
                ));
            }
            // not followed by ←: fall through to normal parse; the
            // paren group is just a pick expression.
        }
    }
    // assignment: NAME ← expr
    if let Some(Tok::Name(name)) = toks.first() {
        let name = name.clone();
        if let Some(Tok::Assign) = toks.get(1) {
            // dfn definition: NAME ← {BODY} — compile into a named function
            if matches!(toks.get(2), Some(Tok::LBrace)) {
                let (rhs, used) = parse_expr(&toks[2..])?;
                if matches!(rhs, Expr::Dfn(_) | Expr::DfnCallMono(_, _)) {
                    return Ok((Expr::AssignDfn(name, Box::new(rhs)), used + 2));
                }
            }
            let (rhs, used) = parse_expr(&toks[2..])?;
            return Ok((Expr::Assign(name, Box::new(rhs)), used + 2));
        }
        // selective assignment: NAME[expr] ← expr
        if matches!(toks.get(1), Some(Tok::LBracket)) {
            let (idx, iused) = parse_expr(&toks[2..])?;
            if matches!(toks.get(iused + 2), Some(Tok::RBracket))
                && matches!(toks.get(iused + 3), Some(Tok::Assign))
            {
                let (rhs, rused) = parse_expr(&toks[iused + 4..])?;
                return Ok((
                    Expr::AssignIndexed(name, Box::new(idx), Box::new(rhs)),
                    iused + 4 + rused,
                ));
            }
            // fall through: not an assignment — the bracket use is an
            // ordinary index expression handled by parse_simple
        }
    }
    parse_simple(toks)
}

/// Try to parse `(A⊃NAME)` starting at toks[0] == LParen.
/// Returns Ok(None) if the pattern doesn't match (caller falls through).
fn try_parse_pick_target(toks: &[Tok]) -> AplResult<Option<(Expr, String, usize)>> {
    // inside the parens we expect: <index-expr> ⊃ NAME
    // toks[0] IS the opening paren of the target group; track inner depth.
    let mut depth = 0usize; // nesting INSIDE the target group
    let mut i = 1usize; // skip toks[0], the opening paren
    let mut disclose_at: Option<usize> = None;

    while let Some(t) = toks.get(i) {
        match t {
            Tok::LParen => depth += 1,
            Tok::RParen => {
                // closing paren of the pick target group
                if let (Some(pos), Some(Tok::Name(n))) = (disclose_at, toks.get(i - 1)) {
                    if depth == 0 && disclose_at == Some(i - 2) && i >= 3 {
                        // parse the index expression between '(' and '⊃'
                        let (idx_expr, _) = parse_expr(&toks[1..pos])?;
                        return Ok(Some((idx_expr, n.clone(), i)));
                    }
                }
                return Ok(None);
            }
            Tok::Prim(crate::functions::Prim::Disclose) if depth == 0 => {
                disclose_at = Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    Ok(None)
}

/// simple := term | term prim simple   (right-associative)
fn parse_simple(toks: &[Tok]) -> AplResult<(Expr, usize)> {
    // dop call: LO FN RO B — call a dfn that references ⍺⍺/⍵⍵,
    // binding LO to ⍺⍺ and RO to ⍵⍵. Detected by: Prim(f) Name(dop) Prim(g) rest
    // where rest is not another operator (which would be a regular dyadic chain).
    if let Some(Tok::Prim(lo_p)) = toks.first() {
        if let Some(Tok::Name(dop_name)) = toks.get(1) {
            if let Some(Tok::Prim(ro_p)) = toks.get(2) {
                let after_ro = 3;
                let is_dop = !matches!(
                    toks.get(after_ro),
                    Some(Tok::Prim(_))
                        | Some(Tok::Reduce(_))
                        | Some(Tok::Scan(_))
                        | Some(Tok::Each(_))
                        | Some(Tok::Commute)
                );
                if is_dop {
                    let lo = *lo_p;
                    let ro = *ro_p;
                    let (rhs, rused) = parse_simple(&toks[after_ro..])?;
                    return Ok((
                        Expr::DopCall(dop_name.clone(), lo, ro, None, Box::new(rhs)),
                        after_ro + rused,
                    ));
                }
            }
        }
    }

    let (lhs, mut used) = parse_term(toks)?;

    // commute operator: F⍨ after a value means the NEXT function is
    // commuted: `A F⍨ B` = B F A. We detect PRIM COMMUTE here.
    if let (Some(Tok::Prim(p)), Some(Tok::Commute)) = (toks.get(used), toks.get(used + 1)) {
        let p = *p;
        // the commuted derived function takes B (the whole rest)
        let (rhs, rused) = parse_simple(&toks[used + 2..])?;
        used += 2 + rused;
        return Ok((Expr::Dyadic(p, Box::new(rhs), Box::new(lhs)), used));
    }

    // dyadic each: A F¨ B — pair elements of A and B.
    // The tokenizer emits Each(F) directly (merging F+¨), so a bare
    // Tok::Each after the lhs means dyadic each.
    if let Some(Tok::Each(p)) = toks.get(used) {
        let p = *p;
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::EachDyad(p, Box::new(lhs), Box::new(rhs)), used));
    }

    // outer product: A ∘.f B
    if let Some(Tok::OuterDot(p)) = toks.get(used) {
        let p = *p;
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::OuterProduct(p, Box::new(lhs), Box::new(rhs)), used));
    }

    // inner product: A f.g B
    if let Some(Tok::InnerDot(f, g)) = toks.get(used) {
        let (f, g) = (*f, *g);
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::InnerProduct(f, g, Box::new(lhs), Box::new(rhs)), used));
    }

    // dfn call: LHS {BODY} — a value immediately followed by a brace group
    // is a DYADIC dfn call (LHS is ⍺). The brace group itself was consumed
    // as lhs (parse_term returns Dfn), so check what came BEFORE: if lhs is
    // a Dfn, an argument to its right makes it a call.
    if matches!(lhs, Expr::Dfn(_)) && !matches!(toks.get(used), Some(Tok::RBrace)) {
        // {BODY} ARG or {BODY} alone
        if !matches!(
            toks.get(used),
            None | Some(Tok::End) | Some(Tok::Diamond) | Some(Tok::RParen) | Some(Tok::Assign)
        ) && !matches!(toks.get(used), Some(Tok::LBrace))
        {
            if let Expr::Dfn(body) = &lhs {
                let body = body.clone();
                let (arg, aused) = parse_simple(&toks[used..])?;
                return Ok((Expr::DfnCallMono(body, Box::new(arg)), used + aused));
            }
        }
    }
    // value followed by a NEW brace group: A {…} B → dyadic dfn call with
    // A as ⍺. Only when the NEXT token is an LBrace.
    if let Some(Tok::LBrace) = toks.get(used) {
        // find matching close brace from here
        let mut depth = 0usize;
        let mut close = None;
        for (i, t) in toks[used..].iter().enumerate() {
            match t {
                Tok::LBrace => depth += 1,
                Tok::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(used + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close.ok_or(ErrorCode::SyntaxError)?;
        let (body, _bused) = parse_expr(&toks[used + 1..close])?;
        // the dfn's right arg is whatever follows the closing brace
        let after = close + 1;
        let (rarg, rused) = parse_simple(&toks[after..])?;
        return Ok((
            Expr::DfnCallDyad(Box::new(lhs), Box::new(body), Box::new(rarg)),
            after + rused,
        ));
    }

    // dyadic with axis: A F[n] B — e.g. 1↑[0]M (take along first axis).
    // MUST be checked before the plain dyadic arm below (which would
    // otherwise consume the Prim and leave the bracket dangling).
    if matches!(toks.get(used), Some(Tok::Prim(_)))
        && matches!(toks.get(used + 1), Some(Tok::LBracket))
    {
        if let Some(Tok::Prim(p)) = toks.get(used) {
            let p = *p;
            // axis functions we support
            if matches!(
                p,
                crate::functions::Prim::Take
                    | crate::functions::Prim::Drop
                    | crate::functions::Prim::Rotate
                    | crate::functions::Prim::Reverse
            ) {
                // parse the axis expression inside brackets
                if let Some(Tok::LBracket) = toks.get(used + 1) {
                    let (ax, aused) = parse_expr(&toks[used + 2..])?;
                    if !matches!(toks.get(used + 2 + aused), Some(Tok::RBracket)) {
                        return Err(ErrorCode::SyntaxError);
                    }
                    let after = used + 3 + aused;
                    let (rhs, rused) = parse_simple(&toks[after..])?;
                    return Ok((
                        Expr::DyadicAxis(p, Box::new(lhs), Box::new(ax), Box::new(rhs)),
                        after + rused,
                    ));
                }
            }
        }
    }

    // check for dyadic dop call: A LO FN RO B — lhs is ⍺, LO is ⍺⍺, RO is ⍵⍵, B is ⍵
    // MUST be checked before the plain dyadic arm below
    if let Some(Tok::Prim(lo_p)) = toks.get(used) {
        if let Some(Tok::Name(dop_name)) = toks.get(used + 1) {
            if let Some(Tok::Prim(ro_p)) = toks.get(used + 2) {
                let after_ro = used + 3;
                let is_dop = !matches!(
                    toks.get(after_ro),
                    Some(Tok::Prim(_))
                        | Some(Tok::Reduce(_))
                        | Some(Tok::Scan(_))
                        | Some(Tok::Each(_))
                        | Some(Tok::Commute)
                );
                if is_dop {
                    let lo = *lo_p;
                    let ro = *ro_p;
                    let (rhs, rused) = parse_simple(&toks[after_ro..])?;
                    return Ok((
                        Expr::DopCall(dop_name.clone(), lo, ro, Some(Box::new(lhs)), Box::new(rhs)),
                        after_ro + rused,
                    ));
                }
            }
        }
    }

    // check for dyadic function: lhs PRIM rest
    if let Some(Tok::Prim(p)) = toks.get(used) {
        let p = *p;
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::Dyadic(p, Box::new(lhs), Box::new(rhs)), used));
    }

    // dop call: LO FN RO B — call a dfn that references ⍺⍺/⍵⍵,
    // binding LO to ⍺⍺ and RO to ⍵⍵. Detected by: Prim(f) Name(dop) Prim(g) rest
    if let Some(Tok::Prim(lo_p)) = toks.get(used) {
        if let Some(Tok::Name(dop_name)) = toks.get(used + 1) {
            if let Some(Tok::Prim(ro_p)) = toks.get(used + 2) {
                // Check that the token after the second Prim is not another operator
                // (which would mean this is a regular dyadic chain, not a dop)
                let after_ro = used + 3;
                let is_dop = !matches!(
                    toks.get(after_ro),
                    Some(Tok::Prim(_))
                        | Some(Tok::Reduce(_))
                        | Some(Tok::Scan(_))
                        | Some(Tok::Each(_))
                        | Some(Tok::Commute)
                );
                if is_dop {
                    let lo = *lo_p;
                    let ro = *ro_p;
                    let (rhs, rused) = parse_simple(&toks[after_ro..])?;
                    return Ok((
                        Expr::DopCall(dop_name.clone(), lo, ro, None, Box::new(rhs)),
                        after_ro + rused,
                    ));
                }
            }
        }
    }

    // dyadic defined-function call: A FN B (Name in infix position)
    if let Some(Tok::Name(fname)) = toks.get(used) {
        // not an assignment (handled in parse_expr) and followed by an expr
        if !matches!(
            toks.get(used + 1),
            Some(Tok::Assign) | None | Some(Tok::End)
        ) {
            let fname = fname.clone();
            let (rhs, rused) = parse_simple(&toks[used + 1..])?;
            used += 1 + rused;
            return Ok((
                Expr::FuncCallDyad(fname, Box::new(lhs), Box::new(rhs)),
                used,
            ));
        }
    }

    Ok((lhs, used))
}

/// term := '(' expr ')' | PRIM term | OP1 term | strand | atom | '{' dfn
fn parse_term(toks: &[Tok]) -> AplResult<(Expr, usize)> {
    match toks.first().ok_or(ErrorCode::SyntaxError)? {
        Tok::LBrace => {
            // dfn: `{ BODY }` — body is one or more expressions separated by
            // `⋄` (diamond) at the top level; nesting braces don't count.
            // ⍺/⍵ reference the eventual arguments. Split on top-level
            // diamonds, desugar guards (`c:e`) to If-then-else.
            let mut depth = 0usize;
            let mut close = None;
            for (i, t) in toks.iter().enumerate() {
                match t {
                    Tok::LBrace => depth += 1,
                    Tok::RBrace => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let close = close.ok_or(ErrorCode::SyntaxError)?;
            let body_expr = parse_dfn_body_expr(&toks[1..close])?;
            Ok((Expr::Dfn(Box::new(body_expr)), close + 1))
        }
        Tok::Alpha => Ok((Expr::Alpha, 1)),
        Tok::Omega => Ok((Expr::Omega, 1)),
        Tok::AlphaAlpha => Ok((Expr::AlphaAlpha, 1)),
        Tok::OmegaOmega => Ok((Expr::OmegaOmega, 1)),
        Tok::SelfRef => {
            // ∇ is an ambivalent self-call: ∇ B (monadic) or A ∇ B (dyadic)
            // monadic case: ∇ followed by a value
            let next_is_value = matches!(
                toks.get(1),
                Some(Tok::Num(_))
                    | Some(Tok::Str(_))
                    | Some(Tok::Name(_))
                    | Some(Tok::LParen)
                    | Some(Tok::LBrace)
                    | Some(Tok::Alpha)
                    | Some(Tok::Omega)
                    | Some(Tok::SelfRef)
            );
            if next_is_value {
                let (operand, used) = parse_simple(&toks[1..])?;
                Ok((Expr::SelfCall(Box::new(operand)), used + 1))
            } else {
                Err(ErrorCode::SyntaxError)
            }
        }
        Tok::LParen => {
            let (e, used) = parse_expr(&toks[1..])?;
            if !matches!(toks.get(used + 1), Some(Tok::RParen)) {
                return Err(ErrorCode::SyntaxError);
            }
            let total = used + 2;

            // nested strand: `(expr)(expr)...` — adjacent paren groups form
            // a vector of enclosed values
            if matches!(toks.get(total), Some(Tok::LParen)) {
                return parse_nested_strand_from(toks, vec![(e, total)]);
            }
            // single group followed by another atom also strands: (1) 2
            if is_strand_atom(toks.get(total)) {
                return parse_nested_strand_from(toks, vec![(e, total)]);
            }
            Ok((e, total))
        }
        Tok::Prim(p) => {
            let p = *p;
            // branch arrow consumes the WHOLE expression to its right
            // (like reduce/scan operators): →A×B is →(A×B)
            if p == crate::functions::Prim::Branch {
                let (operand, used) = parse_simple(&toks[1..])?;
                return Ok((Expr::Monadic(p, Box::new(operand)), used + 1));
            }
            let (operand, used) = parse_term(&toks[1..])?;
            Ok((Expr::Monadic(p, Box::new(operand)), used + 1))
        }
        Tok::Commute => {
            // ⍨ must follow a function: F⍨ ... (syntax error otherwise)
            Err(ErrorCode::SyntaxError)
        }
        Tok::Each(p) => {
            // monadic operator: F¨ B — apply F to each ravel element of B,
            // nesting each result. Binds the whole expression to its right.
            let p = *p;
            let (operand, used) = parse_simple(&toks[1..])?;
            Ok((Expr::EachOp(p, Box::new(operand)), used + 1))
        }
        Tok::Reduce(p) => {
            // monadic operator: LO/B — the derived function LO/ applies to
            // the WHOLE expression to its right (operators bind tighter
            // than functions): ×/20⍴2 = ×/(20⍴2)
            let p = *p;
            let (operand, used) = parse_simple(&toks[1..])?;
            Ok((Expr::ReduceOp(p, Box::new(operand)), used + 1))
        }
        Tok::Scan(p) => {
            let p = *p;
            let (operand, used) = parse_simple(&toks[1..])?;
            Ok((Expr::ScanOp(p, Box::new(operand)), used + 1))
        }
        Tok::Reduce1(p) => {
            let p = *p;
            let (operand, used) = parse_simple(&toks[1..])?;
            Ok((Expr::Reduce1Op(p, Box::new(operand)), used + 1))
        }
        Tok::Scan1(p) => {
            let p = *p;
            let (operand, used) = parse_simple(&toks[1..])?;
            Ok((Expr::Scan1Op(p, Box::new(operand)), used + 1))
        }
        Tok::Num(_) => parse_strand(toks),
        _ => parse_atom(toks),
    }
}

/// strand := atom atom atom ...   (adjacent literals form a vector)
///
/// In APL, `2 3 4` is a 3-element vector (numeric strand) and `1 'a' 2`
/// is a mixed strand (each item becomes an enclosed element when types
/// differ). This handles the left argument of reshape (`2 3⍴⍳6`) and
/// similar constructs.
fn parse_strand(toks: &[Tok]) -> AplResult<(Expr, usize)> {
    // gather consecutive literal atoms (Num / Str)
    let mut items: Vec<Expr> = Vec::new();
    let mut used = 0;
    while let Some(t) = toks.get(used) {
        match t {
            Tok::Num(v) => {
                items.push(Expr::Num(*v));
                used += 1;
            }
            Tok::Str(s) => {
                items.push(Expr::Str(s.clone()));
                used += 1;
            }
            _ => break,
        }
    }

    if items.is_empty() {
        return Err(ErrorCode::SyntaxError);
    }
    if items.len() == 1 {
        // single literal — return it directly
        return Ok((items.pop().unwrap(), used));
    }

    // homogeneous all-number strand stays a flat numeric vector
    let all_nums = items.iter().all(|e| matches!(e, Expr::Num(_)));
    if all_nums {
        let nums: Vec<f64> = items
            .into_iter()
            .map(|e| match e {
                Expr::Num(v) => v,
                _ => unreachable!(),
            })
            .collect();
        return Ok((Expr::NumVec(nums), used));
    }

    // mixed strand: each item enclosed
    Ok((Expr::NestedVec(items), used))
}

/// true if the token can continue a nested strand (a bare atom)
fn is_strand_atom(t: Option<&Tok>) -> bool {
    matches!(
        t,
        Some(Tok::Num(_)) | Some(Tok::Str(_)) | Some(Tok::Name(_))
    )
}

/// Continue a nested strand starting after the first paren-group has been
/// consumed. `acc` holds already-parsed (expr, tokens-consumed) pairs.
///
/// Grammar: strand_item := '(' expr ')' | atom
///         nested_strand := strand_item strand_item ...
fn parse_nested_strand_from(toks: &[Tok], mut acc: Vec<(Expr, usize)>) -> AplResult<(Expr, usize)> {
    let mut used = acc.last().map(|(_, u)| *u).unwrap_or(0);

    loop {
        match toks.get(used) {
            // another paren group
            Some(Tok::LParen) => {
                let (e, gu) = parse_expr(&toks[used + 1..])?;
                if !matches!(toks.get(used + gu + 1), Some(Tok::RParen)) {
                    return Err(ErrorCode::SyntaxError);
                }
                acc.push((e, used + gu + 2));
                used += gu + 2;
            }
            // bare atoms strand too: (1) 2 3 → 3-element nested vector
            t @ (Some(Tok::Num(_)) | Some(Tok::Str(_))) => {
                let (e, au) = parse_atom(&[t.unwrap().clone()])?;
                acc.push((e, used + au));
                used += au;
            }
            _ => break,
        }
    }

    // each element evaluates to an enclosed value; build NestedVec
    let items: Vec<Expr> = acc.into_iter().map(|(e, _)| e).collect();
    Ok((Expr::NestedVec(items), used))
}

fn parse_atom(toks: &[Tok]) -> AplResult<(Expr, usize)> {
    match toks.first().ok_or(ErrorCode::SyntaxError)? {
        Tok::Num(v) => Ok((Expr::Num(*v), 1)),
        Tok::Str(s) => Ok((Expr::Str(s.clone()), 1)),
        Tok::Name(n) => {
            let n = n.clone();
            // bracket indexing: NAME[expr] (selective assignment not yet supported)
            if matches!(toks.get(1), Some(Tok::LBracket)) {
                let (idx, used) = parse_expr(&toks[2..])?;
                match toks.get(used + 2) {
                    Some(Tok::RBracket) => {
                        return Ok((Expr::Index(Box::new(Expr::Var(n)), Box::new(idx)), used + 3))
                    }
                    _ => return Err(ErrorCode::SyntaxError),
                }
            }
            // monadic defined-function call: NAME <value> (name in function
            // position). Only when the next token starts a VALUE — a Prim
            // after a name is much more likely `X+1` (variable + prim) than
            // a monadic call with a prim operand. Resolved at eval time.
            let next_is_value = matches!(
                toks.get(1),
                Some(Tok::Num(_)) | Some(Tok::Str(_)) | Some(Tok::Name(_)) | Some(Tok::LParen)
            );
            if next_is_value {
                let (operand, used) = parse_simple(&toks[1..])?;
                return Ok((Expr::FuncCallMono(n, Some(Box::new(operand))), used + 1));
            }
            // bare name — ambivalent call FN (no args) or a variable reference;
            // resolved at eval time.
            Ok((Expr::FuncCallMono(n, None), 1))
        }
        _ => Err(ErrorCode::SyntaxError),
    }
}

/// bracket indexing: `B[idx]` — pick ravel elements by index vector.
/// Indices are 0-based (matching our `⍳` which generates 0..n).
fn index_value(b: &ValueP, idx: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    let mut out = Vec::with_capacity(idx.element_count() as usize);
    for c in idx.cells() {
        let i = c.get_int_value()?;
        if i < 0 || i as usize >= cells.len() {
            return Err(ErrorCode::IndexError);
        }
        out.push(cells[i as usize].clone());
    }
    Ok(ValueP::from_ravel_like(idx, out))
}

/// Parse a dfn body (tokens between `{` and `}`) into a single Expr.
/// Handles:
/// - single expression: `{e}` → e
/// - multi-statement: `{e1 ⋄ e2}` → DiamondList[e1, e2]
/// - guarded expressions: `{c1:e1 ⋄ c2:e2 ⋄ e3}` → If(c1,e1,If(c2,e2,e3))
///
/// The last expression (or the last guard's "else" branch) is the fallback.
fn parse_dfn_body_expr(toks: &[Tok]) -> AplResult<Expr> {
    // split on top-level diamonds (depth 0)
    let mut stmts: Vec<Vec<Tok>> = Vec::new();
    let mut cur: Vec<Tok> = Vec::new();
    let mut depth = 0usize;
    for t in toks.iter().cloned() {
        match t {
            Tok::LBrace => {
                depth += 1;
                cur.push(t);
            }
            Tok::RBrace => {
                depth -= 1;
                cur.push(t);
            }
            Tok::Diamond if depth == 0 => {
                stmts.push(cur);
                cur = Vec::new();
            }
            _ => cur.push(t),
        }
    }
    stmts.push(cur);

    // parse each statement (skip empty)
    let exprs: Vec<Expr> = stmts
        .into_iter()
        .filter(|s| !s.is_empty() && !matches!(s[0], Tok::End))
        .map(|s| {
            // check for guard: first colon (at depth 0) splits into cond:body
            let mut colon_pos = None;
            let mut d = 0usize;
            for (i, t) in s.iter().enumerate() {
                match t {
                    Tok::LBrace => d += 1,
                    Tok::RBrace => d -= 1,
                    Tok::Colon if d == 0 && colon_pos.is_none() => {
                        colon_pos = Some(i);
                    }
                    _ => {}
                }
            }
            if let Some(cp) = colon_pos {
                let (cond, cused) = parse_expr(&s[..cp])?;
                // after the condition, the next token must be the colon
                if !matches!(s.get(cused), Some(Tok::Colon)) {
                    return Err(ErrorCode::SyntaxError);
                }
                // body is everything after the colon
                let (body, bused) = parse_expr(&s[cp + 1..])?;
                // body must consume the rest of the statement (no trailing End in dfn bodies)
                if cp + 1 + bused != s.len() {
                    return Err(ErrorCode::SyntaxError);
                }
                Ok(Expr::If(
                    Box::new(cond),
                    Box::new(body),
                    Box::new(Expr::Num(0.0)),
                ))
            } else {
                // parse all terms in the statement; single term stays as-is,
                // multiple terms become a Seq (evaluated left-to-right, last wins)
                let mut exprs = Vec::new();
                let mut pos = 0;
                while pos < s.len() && !matches!(s.get(pos), Some(Tok::End)) {
                    let (e, used) = parse_expr(&s[pos..])?;
                    exprs.push(e);
                    pos += used;
                }
                match exprs.len() {
                    0 => Err(ErrorCode::SyntaxError),
                    1 => Ok(exprs.into_iter().next().unwrap()),
                    _ => {
                        // fold multiple terms:
                        // 2 terms: F X → ApplyOp(F, X)  (monadic application)
                        // 3 terms: A F B → DyadicApply(A, F, B) (dyadic application)
                        //   where F is AlphaAlpha or OmegaOmega
                        if exprs.len() == 3 {
                            let a = &exprs[0];
                            let f = &exprs[1];
                            let b = &exprs[2];
                            if matches!(f, Expr::AlphaAlpha | Expr::OmegaOmega) {
                                return Ok(Expr::DyadicApply(
                                    Box::new(a.clone()),
                                    Box::new(f.clone()),
                                    Box::new(b.clone()),
                                ));
                            }
                        }
                        // otherwise fold right-to-left into monadic application
                        let mut acc = exprs.pop().unwrap();
                        for e in exprs.into_iter().rev() {
                            match e {
                                Expr::AlphaAlpha | Expr::OmegaOmega => {
                                    acc = Expr::ApplyOp(Box::new(e), Box::new(acc));
                                }
                                _ => {
                                    // non-prim function: can't fold into Monadic,
                                    // so emit Seq (evaluates each, returns last)
                                    return Ok(Expr::Seq(vec![e, acc]));
                                }
                            }
                        }
                        Ok(acc)
                    }
                }
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if exprs.is_empty() {
        return Err(ErrorCode::SyntaxError);
    }

    // If the last statement is an If (from a guard with no else), its
    // "else" becomes the preceding guard. This is wrong — need to restructure.
    // Actually, the simplest correct approach: if we have guard statements,
    // fold them right-to-left into nested Ifs.
    //
    // {c1:e1 ⋄ c2:e2 ⋄ e3} → [If(c1,e1,0), If(c2,e2,0), e3]
    // fold: If(c2,e2,e3), then If(c1,e1,If(c2,e2,e3))
    //
    // Detect guards: if any stmt is an If (from guard), fold.
    let has_guards = exprs.iter().any(|e| matches!(e, Expr::If(_, _, _)));

    if has_guards {
        // In GNU APL, a dfn body with guards has the form:
        //   {c1:e1 ⋄ c2:e2 ⋄ ... ⋄ en}
        // where c1, c2, ... are guard conditions and en is the fallback.
        // Evaluate: if c1 then e1 else if c2 then e2 else ... else en.
        let mut guards: Vec<(Expr, Expr)> = Vec::new();
        let mut fallback: Option<Expr> = None;
        for e in exprs {
            match e {
                Expr::If(c, b, _) => {
                    if fallback.is_some() {
                        return Err(ErrorCode::SyntaxError);
                    }
                    guards.push((*c, *b));
                }
                other => {
                    if fallback.is_some() {
                        return Err(ErrorCode::SyntaxError);
                    }
                    fallback = Some(other);
                }
            }
        }
        let fallback = fallback.unwrap_or(Expr::Num(0.0));
        // fold right-to-left: guards[0] is the first guard (outermost)
        let mut acc = fallback;
        for (c, b) in guards.into_iter().rev() {
            acc = Expr::If(Box::new(c), Box::new(b), Box::new(acc));
        }
        return Ok(acc);
    }

    match exprs.len() {
        1 => Ok(exprs.into_iter().next().unwrap()),
        _ => Ok(Expr::DiamondList(exprs)),
    }
}

/// collect names assigned anywhere in a body (used to build local scope)
fn collect_assigned_names(body: &[Expr], out: &mut Vec<String>) {
    for e in body {
        match e {
            Expr::Assign(n, _) | Expr::AssignIndexed(n, _, _) | Expr::AssignPick(n, _, _)
                if !out.contains(n) =>
            {
                out.push(n.clone());
            }
            _ => {}
        }
    }
}

/// The evaluator environment: variable bindings + function table.
#[derive(Default)]
pub struct Environment {
    vars: HashMap<String, ValueP>,
    pub funcs: crate::functions_def::FunctionTable,
    /// stack of pending branch targets, one slot per active call frame.
    /// `→N` pushes N (0 = exit, empty target = no-op); call_function's body
    /// loop pops the top. The stack keeps recursive frames independent.
    branch_stack: Vec<Option<i64>>,
    /// counter for unique anonymous dfn names
    dfn_counter: usize,
    /// (fn-name, alpha-arg-name) for named dfns that reference ⍺ — used by
    /// call_function to bind ⍺ on dyadic calls
    pub(crate) dfn_alpha_names: Vec<(String, String)>,
    /// name of the function currently executing (for ∇ self-reference)
    current_fn_name: Option<String>,
    /// dlopen handle cache (libraries stay loaded for process lifetime)
    pub(crate) lib_cache: crate::ffi::loader::LibraryCache,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            funcs: crate::functions_def::FunctionTable::new(),
            branch_stack: Vec::new(),
            dfn_counter: 0,
            dfn_alpha_names: Vec::new(),
            current_fn_name: None,
            lib_cache: crate::ffi::loader::LibraryCache::new(),
        }
    }

    /// install an anonymous dfn body and return its unique name
    fn install_dfn(&mut self, body: &Expr) -> String {
        let fname = format!("{}{}", DFNS_PREFIX, self.dfn_counter);
        self.dfn_counter += 1;
        let mut f = dfn_to_function(body);
        f.name = fname.clone();
        self.funcs.insert(f);
        fname
    }

    pub fn get(&self, name: &str) -> Option<&ValueP> {
        self.vars.get(name)
    }

    pub fn set(&mut self, name: &str, val: ValueP) {
        self.vars.insert(name.to_string(), val);
    }

    /// all variable names (including system ⎕ vars)
    pub fn var_names(&self) -> Vec<String> {
        self.vars.keys().cloned().collect()
    }

    /// wipe all variables and functions (system command )CLEAR)
    pub fn clear_workspace(&mut self) {
        self.vars.clear();
        self.funcs.clear();
    }

    /// read ⎕IO (index origin; 0 if unset)
    pub fn get_io(&self) -> AplResult<i64> {
        crate::sysvars::get_io(self)
    }

    /// true if body line pc is the :Leave control marker
    fn is_leave_line(&self, f: &crate::functions_def::DefinedFunction, pc: usize) -> bool {
        // :Leave lines occupy a body slot as Expr::Num(0.0) no-ops (they are
        // control markers), so match on the raw source kept out-of-band.
        f.leave_lines.contains(&pc)
    }

    /// consume a pending :Leave signal from branch_stack (if any).
    /// Returns true when the innermost enclosing loop should stop.
    fn consume_leave(&mut self) -> AplResult<bool> {
        if let Some(Some(t)) = self.branch_stack.last() {
            if *t == crate::functions_def::LEAVE_SENTINEL {
                self.branch_stack.pop();
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// execute body lines [from..to), honoring nested control blocks that
    /// start within the range. Used by :If/:While branch execution.
    ///
    /// A `:Leave` inside a loop body pushes LEAVE_SENTINEL onto
    /// branch_stack; the innermost While/Repeat arm consumes it and breaks.
    fn run_lines(
        &mut self,
        f: &crate::functions_def::DefinedFunction,
        from: usize,
        to: usize,
    ) -> AplResult<()> {
        let mut pc = from;
        while pc < to {
            // :Leave marker line — signal the enclosing loop to stop
            if self.is_leave_line(f, pc) {
                self.branch_stack
                    .push(Some(crate::functions_def::LEAVE_SENTINEL));
                return Ok(());
            }
            // find a control block starting at this line; capture its end
            let block_end = f.control.iter().find_map(|b| match b {
                crate::functions_def::ControlBlock::If { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                crate::functions_def::ControlBlock::While { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                crate::functions_def::ControlBlock::Repeat { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                _ => None,
            });
            if let Some(block_end) = block_end {
                let block = f
                    .control
                    .iter()
                    .find(|b| match b {
                        crate::functions_def::ControlBlock::If { start, .. } => *start == pc,
                        crate::functions_def::ControlBlock::While { start, .. } => *start == pc,
                        crate::functions_def::ControlBlock::Repeat { start, .. } => *start == pc,
                    })
                    .cloned()
                    .unwrap();
                match block {
                    crate::functions_def::ControlBlock::If {
                        cond, else_start, ..
                    } => {
                        let c = self.eval(&cond)?;
                        let truthy = c.first_cell().unwrap().get_int_value()? != 0;
                        if truthy {
                            let stop = else_start.unwrap_or(block_end - 1);
                            self.run_lines(f, pc + 1, stop)?;
                        } else if let Some(es) = else_start {
                            self.run_lines(f, es + 1, block_end - 1)?;
                        }
                    }
                    crate::functions_def::ControlBlock::While { start, cond, .. } => loop {
                        let c = self.eval(&cond)?;
                        if c.first_cell().unwrap().get_int_value()? == 0 {
                            break;
                        }
                        self.run_lines(f, start + 1, block_end - 1)?;
                        // :Leave inside the body → exit this loop
                        if self.consume_leave()? {
                            break;
                        }
                    },
                    crate::functions_def::ControlBlock::Repeat {
                        start,
                        until_pos,
                        ref until_cond,
                        ..
                    } => loop {
                        let stop = until_pos.unwrap_or(block_end - 1);
                        self.run_lines(f, start + 1, stop)?;
                        // :Leave inside the body → exit this loop
                        // (before :Until is checked)
                        if self.consume_leave()? {
                            break;
                        }
                        if let Some(uc) = until_cond {
                            let c = self.eval(uc)?;
                            // :Until cond → repeat while cond is FALSE
                            if c.first_cell().unwrap().get_int_value()? != 0 {
                                break;
                            }
                        }
                    },
                }
                pc = block_end; // jump past this block
                continue;
            }
            self.eval(&f.body[pc])?;
            pc += 1;
        }
        Ok(())
    }

    /// call a defined function by name with dop primitives (⍺⍺/⍵⍵) bound
    pub fn call_function_dop(
        &mut self,
        name: &str,
        dop_lo: crate::functions::Prim,
        dop_ro: crate::functions::Prim,
        left: Option<ValueP>,
        right: Option<ValueP>,
    ) -> AplResult<ValueP> {
        let f = match self.funcs.get(name).and_then(|c| c.interpreted()) {
            Some(f) => {
                let mut f = f.clone();
                f.body = f
                    .body
                    .iter()
                    .map(|e| substitute_dop(e, Some(dop_lo), Some(dop_ro)))
                    .collect();
                f
            }
            None => return Err(ErrorCode::ValueError),
        };
        let provided = left.is_some() as u8 + right.is_some() as u8;
        if f.arity() != 0 && provided == 0 || f.arity() == 2 && provided != 2 {
            return Err(ErrorCode::SyntaxError);
        }
        let mut shadowed: Vec<(String, Option<ValueP>)> = Vec::new();
        for local in [&f.arg_left, &f.arg_right, &f.result].into_iter().flatten() {
            shadowed.push((local.clone(), self.vars.get(local).cloned()));
        }
        // For named dfns, arg_left may have been moved to dfn_alpha_names
        // during AssignDfn. Look it up and shadow+bind ⍺ if a left arg was provided.
        let alpha_name = f.arg_left.clone().or_else(|| {
            self.dfn_alpha_names
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, a)| a.clone())
        });
        if let Some(n) = &alpha_name {
            shadowed.push((n.clone(), self.vars.get(n).cloned()));
        }
        let mut body_locals: Vec<String> = Vec::new();
        collect_assigned_names(&f.body, &mut body_locals);
        for l in &body_locals {
            shadowed.push((l.clone(), self.vars.get(l).cloned()));
        }
        let caller_fn_name = self.current_fn_name.clone();
        self.current_fn_name = Some(name.to_string());
        // For named dfns, arg_left may have been moved to dfn_alpha_names
        // during AssignDfn. Look it up and bind ⍺ if a left arg was provided.
        let alpha_name = f.arg_left.clone().or_else(|| {
            self.dfn_alpha_names
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, a)| a.clone())
        });
        if let Some(n) = &alpha_name {
            if let Some(v) = &left {
                self.vars.insert(n.clone(), v.clone());
            }
        }
        if let Some(n) = &f.arg_right {
            if let Some(v) = &right {
                self.vars.insert(n.clone(), v.clone());
            }
        }
        let mut last: Option<ValueP> = None;
        let mut err = None;
        let mut pc = 0usize;
        self.branch_stack.push(None);
        let frame_base = self.branch_stack.len() - 1;
        while pc < f.body.len() {
            let block_end = f.control.iter().find_map(|b| match b {
                crate::functions_def::ControlBlock::If { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                crate::functions_def::ControlBlock::While { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                crate::functions_def::ControlBlock::Repeat { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                _ => None,
            });
            if let Some(block_end) = block_end {
                self.run_lines(&f, pc, block_end)?;
                pc = block_end;
                continue;
            }
            match self.eval(&f.body[pc]) {
                Ok(v) => {
                    if self.branch_stack.len() > frame_base + 1 {
                        if let Some(Some(t)) = self.branch_stack.pop() {
                            if t == 0 {
                                self.branch_stack.truncate(frame_base);
                                break;
                            }
                            pc = (t - 1) as usize;
                            continue;
                        }
                    }
                    last = Some(v);
                }
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
            pc += 1;
        }
        self.branch_stack.truncate(frame_base);
        self.current_fn_name = caller_fn_name;
        let explicit_result: Option<ValueP> =
            f.result.as_ref().and_then(|rn| self.vars.get(rn).cloned());
        for (name, old) in shadowed {
            match old {
                Some(v) => {
                    self.vars.insert(name, v);
                }
                None => {
                    self.vars.remove(&name);
                }
            }
        }
        if let Some(e) = err {
            return Err(e);
        }
        match (explicit_result, last) {
            (Some(v), _) => Ok(v),
            (None, Some(v)) => Ok(v),
            (None, None) => Err(ErrorCode::SyntaxError),
        }
    }

    /// call a defined function by name (monadic or dyadic).
    /// Creates a child scope with the args bound; recursion works because
    /// the function table is shared.
    pub fn call_function(
        &mut self,
        name: &str,
        left: Option<ValueP>,
        right: Option<ValueP>,
    ) -> AplResult<ValueP> {
        let f = match self.funcs.get(name).and_then(|c| c.interpreted()) {
            Some(f) => f.clone(),
            None => return Err(ErrorCode::ValueError),
        };

        // arity check
        let provided = left.is_some() as u8 + right.is_some() as u8;
        if f.arity() != 0 && provided == 0 || f.arity() == 2 && provided != 2 {
            return Err(ErrorCode::SyntaxError);
        }

        // save/restore shadowed locals (simple dynamic scoping)
        let mut shadowed: Vec<(String, Option<ValueP>)> = Vec::new();
        for local in [&f.arg_left, &f.arg_right, &f.result].into_iter().flatten() {
            shadowed.push((local.clone(), self.vars.get(local).cloned()));
        }
        // also shadow every name assigned in the body (locals)
        let mut body_locals: Vec<String> = Vec::new();
        collect_assigned_names(&f.body, &mut body_locals);
        for l in &body_locals {
            shadowed.push((l.clone(), self.vars.get(l).cloned()));
        }

        // dop: if this function references ⍺⍺/⍵⍵, substitute bound primitives
        let body = if f.dop_lo.is_some() || f.dop_ro.is_some() {
            f.body
                .iter()
                .map(|e| substitute_dop(e, f.dop_lo, f.dop_ro))
                .collect()
        } else {
            f.body.clone()
        };

        // save the caller's current function name so ∇ can reference this one
        let caller_fn_name = self.current_fn_name.clone();
        self.current_fn_name = Some(name.to_string());

        // bind args
        if let Some(n) = &f.arg_left {
            if let Some(v) = &left {
                self.vars.insert(n.clone(), v.clone());
            }
        }
        if let Some(n) = &f.arg_right {
            if let Some(v) = &right {
                self.vars.insert(n.clone(), v.clone());
            }
        }
        // named dfns: bind ⍺ on dyadic calls even though arg_left was
        // dropped from the arity signature (dfns are ambivalent)
        if let Some(alpha) = self
            .dfn_alpha_names
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, a)| a.clone())
        {
            match (&left, &right) {
                (Some(v), Some(_)) => {
                    self.vars.insert(alpha, v.clone());
                }
                (None, Some(_)) => {
                    // monadic call: shadow ⍺ so an accidental reference
                    // doesn't see a STALE outer value
                    self.vars.remove(&alpha);
                }
                _ => {}
            }
        }

        // run the body; result = last line's value.
        // A line that is a bare `→expr` branch: target 0 exits, N jumps
        // to line N (1-based), empty = fall through. Branch targets go on
        // a per-frame stack so nested (recursive) calls don't interfere:
        // we push a None sentinel for THIS frame and only react to targets
        // pushed after it (i.e. by our own body, not by inner calls).
        let mut last: Option<ValueP> = None;
        let mut err = None;
        let mut pc = 0usize;
        self.branch_stack.push(None); // frame sentinel
        let frame_base = self.branch_stack.len() - 1;
        while pc < body.len() {
            // structured control blocks: delegate to the same machinery
            // run_lines uses, but keep tracking `last`/branch state here
            let block_end = f.control.iter().find_map(|b| match b {
                crate::functions_def::ControlBlock::If { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                crate::functions_def::ControlBlock::While { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                crate::functions_def::ControlBlock::Repeat { start, end, .. } if *start == pc => {
                    Some(*end)
                }
                _ => None,
            });
            if let Some(block_end) = block_end {
                self.run_lines(&f, pc, block_end)?;
                pc = block_end;
                continue;
            }
            match self.eval(&body[pc]) {
                Ok(v) => {
                    // consume any targets this line pushed (inner calls may
                    // have pushed+consumed their own already)
                    if self.branch_stack.len() > frame_base + 1 {
                        // a target pushed by OUR line (not an inner call —
                        // inner calls pop their own). Take the top one.
                        if let Some(Some(t)) = self.branch_stack.pop() {
                            if t == 0 {
                                // →0 exits THIS frame: drop the sentinel
                                self.branch_stack.truncate(frame_base);
                                break;
                            }
                            pc = (t - 1) as usize; // 1-based → 0-based
                            continue;
                        }
                    }
                    last = Some(v);
                }
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
            pc += 1;
        }
        // make sure the sentinel is gone even on error paths
        self.branch_stack.truncate(frame_base);

        // restore current function name to the caller's
        self.current_fn_name = caller_fn_name;

        // capture explicit result var BEFORE restoring shadowed names
        let explicit_result: Option<ValueP> =
            f.result.as_ref().and_then(|rn| self.vars.get(rn).cloned());

        // restore shadowed names
        for (name, old) in shadowed {
            match old {
                Some(v) => {
                    self.vars.insert(name, v);
                }
                None => {
                    self.vars.remove(&name);
                }
            }
        }

        if let Some(e) = err {
            return Err(e);
        }

        // explicit result var takes precedence over last-line value
        match (explicit_result, last) {
            (Some(v), _) => Ok(v),
            (None, Some(v)) => Ok(v),
            (None, None) => Err(ErrorCode::SyntaxError),
        }
    }

    /// evaluate an expression in this environment.
    pub fn eval(&mut self, e: &Expr) -> AplResult<ValueP> {
        match e {
            Expr::Num(v) => Ok(ValueP::scalar_from(crate::cell::Cell::from_f64(*v))),
            Expr::NumVec(vs) => Ok(ValueP::from_ravel_like(
                &ValueP::vector(vs.len() as i64),
                vs.iter().map(|&v| crate::cell::Cell::from_f64(v)).collect(),
            )),
            Expr::NestedVec(items) => {
                // evaluate each element and enclose it
                let mut ravel = Vec::with_capacity(items.len());
                for item in items {
                    let v = self.eval(item)?;
                    let enclosed = ValueP::nested(v);
                    ravel.push(enclosed.first_cell().unwrap().clone());
                }
                Ok(ValueP::from_ravel_like(
                    &ValueP::vector(items.len() as i64),
                    ravel,
                ))
            }
            Expr::Str(s) => Ok(ValueP::char_vector(s)),
            Expr::Var(name) => self.vars.get(name).cloned().ok_or(ErrorCode::ValueError),
            Expr::Alpha => self.vars.get("⍺").cloned().ok_or(ErrorCode::ValueError),
            Expr::Omega => self.vars.get("⍵").cloned().ok_or(ErrorCode::ValueError),
            Expr::Dfn(body) => {
                // a bare dfn evaluates to... itself as a function value. We
                // don't have first-class function values yet, so represent
                // it by installing an anonymous entry in the function table
                // under a unique name and returning that name as a Var-like
                // reference is overkill: instead evaluate the body with
                // ⍺/⍵ UNBOUND — only valid if the body doesn't use them.
                let f = dfn_to_function(body);
                let fname = format!("{}{}", DFNS_PREFIX, self.dfn_counter);
                self.dfn_counter += 1;
                let mut f = f;
                f.name = fname.clone();
                self.funcs.insert(f);
                Ok(ValueP::scalar_from(crate::cell::Cell::Char(
                    fname.chars().next().unwrap() as u32,
                )))
            }
            Expr::If(cond, then_b, else_b) => {
                let cv = self.eval(cond)?;
                if cv.first_cell().is_some_and(|c| c.get_near_int() == Ok(0)) {
                    self.eval(else_b)
                } else {
                    self.eval(then_b)
                }
            }
            Expr::AlphaAlpha => Err(ErrorCode::SyntaxError),
            Expr::OmegaOmega => Err(ErrorCode::SyntaxError),
            Expr::ApplyOp(func, arg) => {
                // unresolved — should have been substituted via substitute_dop
                let _ = (func, arg);
                Err(ErrorCode::SyntaxError)
            }
            Expr::DyadicApply(a, f, b) => {
                // unresolved — should have been substituted via substitute_dop
                let _ = (a, f, b);
                Err(ErrorCode::SyntaxError)
            }
            Expr::FuncRef(_) => Err(ErrorCode::SyntaxError),
            Expr::Seq(exprs) => {
                let mut last = None;
                for e in exprs {
                    last = Some(self.eval(e)?);
                }
                last.ok_or(ErrorCode::SyntaxError)
            }
            Expr::DopCall(dop_name, lo, ro, left, rhs) => {
                // LO DOP RO B (monadic) or A LO DOP RO B (dyadic)
                let fname = dop_name.clone();
                let rv = self.eval(rhs)?;
                let lv = match left {
                    Some(l) => Some(self.eval(l)?),
                    None => None,
                };
                self.call_function_dop(&fname, *lo, *ro, lv, Some(rv))
            }
            Expr::SelfCall(arg) => {
                // ∇ B — monadic self-call
                let fname = self.current_fn_name.clone().ok_or(ErrorCode::SyntaxError)?;
                let av = self.eval(arg)?;
                self.call_function(&fname, None, Some(av))
            }
            Expr::SelfCallDyad(larg, rarg) => {
                // A ∇ B — dyadic self-call
                let fname = self.current_fn_name.clone().ok_or(ErrorCode::SyntaxError)?;
                let bv = self.eval(rarg)?;
                let av = self.eval(larg)?;
                self.call_function(&fname, Some(av), Some(bv))
            }
            Expr::DiamondList(exprs) => {
                let mut last = None;
                for e in exprs {
                    last = Some(self.eval(e)?);
                }
                last.ok_or(ErrorCode::SyntaxError)
            }
            Expr::DfnCallMono(body, arg) => {
                // install + immediately call with ⍵ = arg (⍺ unbound).
                // Dfns are AMBIVALENT: strip arg_left so the arity check
                // passes; an unbound ⍺ reference then raises VALUE ERROR
                // naturally if the body actually uses it.
                let fname = self.install_dfn(body);
                self.funcs.get_mut(&fname).unwrap().arg_left = None;
                let av = self.eval(arg)?;
                self.call_function(&fname, None, Some(av))
            }
            Expr::DfnCallDyad(larg, body, rarg) => {
                // APL evaluates ⍵ then ⍺
                let fname = self.install_dfn(body);
                let bv = self.eval(rarg)?;
                let av = self.eval(larg)?;
                self.call_function(&fname, Some(av), Some(bv))
            }
            Expr::AssignDfn(name, rhs) => {
                // NAME ← {BODY} — a named dfn. Dfns are AMBIVALENT: keep
                // arg_right (⍵) as the declared argument but drop arg_left
                // from the ARITY computation by marking it optional; an
                // unbound ⍺ reference raises VALUE ERROR naturally.
                match &**rhs {
                    Expr::Dfn(body) => {
                        let f = dfn_to_function(body);
                        let mut f = f;
                        f.name = name.clone();
                        // arity-2 with only-right-provided must not fail:
                        // store ⍺ as a non-arity-affecting local instead
                        let alpha_local = f.arg_left.take();
                        self.funcs.insert(f);
                        // remember that ⍺ exists for dyadic calls
                        if let Some(a) = alpha_local {
                            self.dfn_alpha_names.push((name.clone(), a));
                        }
                        Ok(ValueP::scalar_from(crate::cell::Cell::Int(0)))
                    }
                    _ => Err(ErrorCode::SyntaxError),
                }
            }
            Expr::FuncCallMono(name, arg) => {
                let right = match arg {
                    Some(e) => Some(self.eval(e)?),
                    None => None,
                };
                // native ⎕NA binding takes precedence over everything:
                // always monadic, right arg is a (possibly nested) vector
                if let Some(crate::functions_def::Callable::Native(b)) = self.funcs.get(name) {
                    let args: Vec<ValueP> = match &right {
                        Some(v) => vec![v.clone()],
                        None => vec![ValueP::int_vector(&[])],
                    };
                    return b.call(&args);
                }
                // plugin binding: typed XValue path with panic containment
                if let Some(crate::functions_def::Callable::Plugin(pb)) = self.funcs.get(name) {
                    let arg_v = right.unwrap_or_else(|| ValueP::int_vector(&[]));
                    return call_plugin(pb, &arg_v);
                }
                // defined function takes precedence; a bare name with no
                // argument falls back to a variable reference
                if self.funcs.get(name).is_some() {
                    return self.call_function(name, None, right);
                }
                match (arg, right) {
                    (None, _) => self.vars.get(name).cloned().ok_or(ErrorCode::ValueError),
                    (Some(_), r) => self.call_function(name, None, r),
                }
            }
            Expr::FuncCallDyad(name, a, b) => {
                // dyadic call on a native binding: desugar to monadic with
                // the enclosed pair (Dyalog rule — ⎕NA fns are never dyadic)
                let is_native = matches!(
                    self.funcs.get(name),
                    Some(crate::functions_def::Callable::Native(_))
                );
                if is_native {
                    let av = self.eval(a)?;
                    let bv = self.eval(b)?;
                    let pair = ValueP::from_ravel_like(
                        &ValueP::int_vector(&[]),
                        vec![
                            Cell::Pointer(crate::cell::PointerCellData {
                                value: av.clone_inner_arc(),
                            }),
                            Cell::Pointer(crate::cell::PointerCellData {
                                value: bv.clone_inner_arc(),
                            }),
                        ],
                    );
                    if let Some(crate::functions_def::Callable::Native(nb)) = self.funcs.get(name) {
                        return nb.call(&[pair]);
                    }
                    unreachable!("checked above");
                }
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                self.call_function(name, Some(av), Some(bv))
            }
            Expr::Assign(name, rhs) => {
                let v = self.eval(rhs)?;
                self.vars.insert(name.clone(), v.clone());
                Ok(v)
            }
            Expr::AssignIndexed(name, idx, rhs) => {
                // selective assignment: B[idx] ← value (mutates B in place)
                let iv = self.eval(idx)?;
                let rv = self.eval(rhs)?;
                let target = self
                    .vars
                    .get_mut(name)
                    .ok_or(ErrorCode::ValueError)?
                    .clone();
                let mut writable = target;
                writable.isolate(); // COW: never mutate a shared value
                {
                    let cells = writable.make_mut().ravel_mut();
                    for c in iv.cells() {
                        let i = c.get_int_value()?;
                        if i < 0 || i as usize >= cells.len() {
                            return Err(ErrorCode::IndexError);
                        }
                        let src = rv.cells()[0].clone();
                        cells[i as usize] = src;
                    }
                }
                self.vars.insert(name.clone(), writable.clone());
                Ok(writable)
            }
            Expr::AssignPick(name, path, rhs) => {
                // selective pick assignment: (A⊃B) ← value
                let pv = self.eval(path)?;
                let rv = self.eval(rhs)?;
                let target = self
                    .vars
                    .get_mut(name)
                    .ok_or(ErrorCode::ValueError)?
                    .clone();
                let mut writable = target;
                writable.isolate();

                // build index path like pick() does
                let mut levels: Vec<i64> = Vec::new();
                for c in pv.cells() {
                    match c {
                        crate::cell::Cell::Pointer(p) => {
                            for ic in p.value.cells() {
                                levels.push(ic.get_int_value()?);
                            }
                        }
                        _ => levels.push(c.get_near_int()?),
                    }
                }
                if levels.is_empty() {
                    return Err(ErrorCode::IndexError);
                }

                crate::pick::pick_assign(&mut writable, &levels, rv.first_cell().unwrap())?;
                self.vars.insert(name.clone(), writable.clone());
                Ok(writable)
            }
            Expr::Monadic(p, b) => {
                // branch arrow: →expr — evaluate the target; push it for
                // call_function's body loop. An EMPTY target = no jump
                // (fall through); 0 = exit function; N = jump to line N.
                if *p == crate::functions::Prim::Branch {
                    let bv = self.eval(b)?;
                    match bv.first_cell() {
                        None => self.branch_stack.push(None), // no jump
                        Some(c) => {
                            let t = c.get_near_int()?;
                            self.branch_stack.push(Some(t));
                        }
                    }
                    return Ok(ValueP::scalar_from(crate::cell::Cell::Int(0)));
                }
                let bv = self.eval(b)?;
                // ⍳B generates ⎕IO .. ⎕IO+B-1
                if *p == crate::functions::Prim::Iota {
                    return crate::functions::iota_monadic(&bv, self.get_io()?);
                }
                // ⍋/⍒ results are also ⎕IO-shifted
                if *p == crate::functions::Prim::GradeUp || *p == crate::functions::Prim::GradeDown
                {
                    return crate::sort::grade_io(
                        &bv,
                        *p == crate::functions::Prim::GradeDown,
                        self.get_io()?,
                    );
                }
                p.eval_monadic(&bv)
            }
            Expr::ReduceOp(p, b) => {
                let bv = self.eval(b)?;
                crate::operators::reduce(*p, &bv)
            }
            Expr::ScanOp(p, b) => {
                let bv = self.eval(b)?;
                crate::operators::scan(*p, &bv)
            }
            Expr::Reduce1Op(p, b) => {
                let bv = self.eval(b)?;
                crate::operators::reduce_first(*p, &bv)
            }
            Expr::Scan1Op(p, b) => {
                let bv = self.eval(b)?;
                crate::operators::scan_first(*p, &bv)
            }
            Expr::EachOp(p, b) => {
                let bv = self.eval(b)?;
                crate::operators::each(*p, &bv)
            }
            Expr::EachDyad(p, a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                crate::operators::each_dyad(*p, &av, &bv)
            }
            Expr::OuterProduct(p, a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                crate::outer::outer_product(&av, *p, &bv)
            }
            Expr::InnerProduct(f, g, a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                crate::inner::inner_product(&av, *f, *g, &bv)
            }
            Expr::Index(base, idx) => {
                let bv = self.eval(base)?;
                let iv = self.eval(idx)?;
                // bracket indexing honors ⎕IO: subtract it to get 0-based
                let io = self.get_io()?;
                let shifted = if io == 0 {
                    iv
                } else {
                    let minus_io = ValueP::scalar_from(crate::cell::Cell::Int(-io));
                    crate::functions::eval_dyadic_public(
                        crate::functions::Prim::Add,
                        &iv,
                        &minus_io,
                    )?
                };
                index_value(&bv, &shifted)
            }
            Expr::Dyadic(p, a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                // A⍳B results are ⎕IO-shifted
                if *p == crate::functions::Prim::Iota {
                    return crate::index_of::index_of_io(&av, &bv, self.get_io()?);
                }
                p.eval_dyadic(&av, &bv)
            }
            Expr::ErrorGuard(guard, fallback) => {
                // ⎕EA guarded ⋄ fallback — run guard; on ANY error, run fallback
                match self.eval(guard) {
                    Ok(v) => Ok(v),
                    Err(_) => self.eval(fallback),
                }
            }
            Expr::QuadNa(apl_name, decl) => {
                // 'name' ⎕NA 'F8 lib|sym I4 I4' — associate native fn
                let decl_v = self.eval(decl)?;
                let decl_str = decl_v
                    .cells()
                    .iter()
                    .map(|c| match c {
                        Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                        other => panic!("⎕NA: non-char in declaration {:?}", other),
                    })
                    .collect::<String>();
                let spec = crate::ffi::nadecl::parse_na_decl(&decl_str).map_err(|e| {
                    let _ = e;
                    ErrorCode::SyntaxError
                })?;
                let default_name = spec.symbol.clone();
                let binding = crate::ffi::cabi::CAbiBinding::associate(&mut self.lib_cache, spec)
                    .map_err(|e| match e {
                    crate::ffi::cabi::CablError::Load(_) => ErrorCode::FileError,
                    crate::ffi::cabi::CablError::Symbol(_)
                    | crate::ffi::cabi::CablError::Domain(_) => ErrorCode::ValueError,
                    crate::ffi::cabi::CablError::Syntax => ErrorCode::SyntaxError,
                })?;
                let name = match apl_name {
                    Some(e) => {
                        let nv = self.eval(e)?;
                        nv.cells()
                            .iter()
                            .map(|c| match c {
                                Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                                _ => '?',
                            })
                            .collect::<String>()
                    }
                    None => default_name,
                };
                self.funcs.insert_native(&name, binding);
                // shy result: the name that was fixed
                let cps: Vec<u32> = name.chars().map(|ch| ch as u32).collect();
                Ok(ValueP::char_vector(&cps))
            }
            Expr::QuadLoadSo(spec) => {
                // ⎕LOADSO 'path' — load plugin, register all its bindings
                let spec_v = self.eval(spec)?;
                let path = spec_v
                    .cells()
                    .iter()
                    .map(|c| match c {
                        Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                        _ => '?',
                    })
                    .collect::<String>();
                let loaded = crate::ffi::plugin::load_plugin(&mut self.lib_cache, &path)
                    .map_err(|_| ErrorCode::FileError)?;
                let mut names: Vec<String> = Vec::new();
                for b in loaded.bindings {
                    names.push(b.apl_name.clone());
                    let name = b.apl_name.clone();
                    self.funcs.insert_plugin(&name, b);
                }
                names.sort();
                // result: vector of registered APL names
                let flat: Vec<u32> = names.join(" ").chars().map(|ch| ch as u32).collect();
                Ok(ValueP::char_vector(&flat))
            }
            Expr::DyadicAxis(p, a, axis, b) => {
                let av = self.eval(a)?;
                let xv = self.eval(axis)?;
                let bv = self.eval(b)?;
                let ax = xv.first_cell().unwrap().get_near_int()?;
                // axis numbers follow ⎕IO: under IO=1, [1] means first axis = 0
                let io = self.get_io()?;
                let ax0 = ax - io;
                match p {
                    crate::functions::Prim::Take => crate::take_drop::take_axis(&av, &bv, ax0),
                    crate::functions::Prim::Drop => crate::take_drop::drop_axis(&av, &bv, ax0),
                    crate::functions::Prim::Rotate | crate::functions::Prim::Reverse => {
                        crate::rotate::rotate_axis(&av, &bv, ax0)
                    }
                    _ => Err(ErrorCode::SyntaxError),
                }
            }
        }
    }

    /// tokenize + parse + evaluate one line. Returns the result value
    /// (None if the line was a pure assignment with no displayed value —
    /// but in APL assignments DO display nothing; we return None then).
    pub fn eval_line(&mut self, line: &str) -> AplResult<Option<ValueP>> {
        let toks = tokenize(line)?;
        if matches!(toks.first(), Some(Tok::End)) || toks.len() < 2 {
            return Ok(None); // empty line
        }
        let (expr, used) = parse(&toks)?;
        if !matches!(toks.get(used), Some(Tok::End)) {
            return Err(ErrorCode::SyntaxError);
        }
        let is_assign = matches!(
            expr,
            Expr::Assign(_, _)
                | Expr::AssignIndexed(_, _, _)
                | Expr::AssignPick(_, _, _)
                | Expr::AssignDfn(_, _)
        );
        let v = self.eval(&expr)?;
        Ok(if is_assign { None } else { Some(v) })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn eval_one(env: &mut Environment, line: &str) -> ValueP {
        env.eval_line(line)
            .expect("eval failed")
            .expect("expected a result")
    }

    fn eval_int(line: &str) -> i64 {
        let mut env = Environment::new();
        let v = eval_one(&mut env, line);
        match v.first_cell().unwrap() {
            crate::cell::Cell::Int(i) => *i,
            other => panic!("expected int scalar, got {:?}", other),
        }
    }

    #[test]
    fn test_right_to_left() {
        // 2×3+4 must be 2×(3+4)=14, not (2×3)+4=10
        assert_eq!(eval_int("2×3+4"), 14);
        // 5-1-2 must be 5-(1-2)=6, not (5-1)-2=2
        assert_eq!(eval_int("5-1-2"), 6);
    }

    #[test]
    fn test_parentheses_override() {
        assert_eq!(eval_int("(2×3)+4"), 10);
    }

    #[test]
    fn test_assignment() {
        let mut env = Environment::new();
        // assignments produce no displayed result...
        assert!(env.eval_line("X←42").unwrap().is_none());
        // ...but bind the name
        assert_eq!(eval_int_env(&mut env, "X"), 42);
        // and the bound name works in later expressions
        assert_eq!(eval_int_env(&mut env, "X+1"), 43);
    }

    fn eval_int_env(env: &mut Environment, line: &str) -> i64 {
        match eval_one(env, line).first_cell().unwrap() {
            crate::cell::Cell::Int(i) => *i,
            other => panic!("expected int scalar, got {:?}", other),
        }
    }

    #[test]
    fn test_reduce_works() {
        // reduce was implemented: +/⍳5 = 10
        let mut env = Environment::new();
        assert_eq!(eval_int_env(&mut env, "+/⍳5"), 10);
    }

    #[test]
    fn test_monadic_iota_vector() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍳5");
        let cells: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_scalar_extension() {
        let mut env = Environment::new();
        env.eval_line("V←⍳4").unwrap();
        let v = eval_one(&mut env, "100+V");
        let cells: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![100, 101, 102, 103]);
    }

    #[test]
    fn test_syntax_error() {
        let mut env = Environment::new();
        assert!(env.eval_line("2+").is_err());
        assert!(env.eval_line("(2+3").is_err());
        assert!(env.eval_line("$5").is_err());
    }

    #[test]
    fn test_reshape() {
        let mut env = Environment::new();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        let m = env.get("M").expect("M not set");
        assert_eq!(m.rank(), 2);
        assert_eq!(m.element_count(), 6);
        let cells: Vec<i64> = m
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_shape_of() {
        let mut env = Environment::new();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        let s = eval_one(&mut env, "⍴M");
        let cells: Vec<i64> = s
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![2, 3]);
    }

    #[test]
    fn test_commute_swap() {
        // A F⍨ B = B F A: 10 -⍨ 3 = 3-10 = ¯7
        assert_eq!(eval_int("10 -⍨ 3"), -7);
        // ÷ commute: 6 ÷⍨ 2 = 2÷6
        let mut env = Environment::new();
        let v = eval_one(&mut env, "6÷⍨2");
        match v.first_cell().unwrap() {
            crate::cell::Cell::Float(f) => assert!((f - (2.0 / 6.0)).abs() < 1e-13),
            crate::cell::Cell::Int(i) => {
                let f = *i as f64;
                assert!((f - (2.0 / 6.0)).abs() < 1e-13)
            }
            o => panic!("unexpected {:?}", o),
        }
    }

    #[test]
    fn test_commute_stray_is_syntax_error() {
        let mut env = Environment::new();
        assert!(env.eval_line("⍨5").is_err());
    }

    #[test]
    fn test_nested_literal_strand() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "(1 2)(3 4 5)");
        assert_eq!(v.element_count(), 2);
        assert!(v.is_vector());
        // each element is a pointer to the enclosed vector
        for c in v.cells() {
            match c {
                crate::cell::Cell::Pointer(p) => {
                    assert_eq!(p.value.shape().get_rank(), 1);
                }
                o => panic!("expected pointer, got {:?}", o),
            }
        }
        // disclose both and check contents
        let first = match &v.cells()[0] {
            crate::cell::Cell::Pointer(p) => p.value.clone(),
            _ => panic!(),
        };
        let cells: Vec<i64> = first
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![1, 2]);

        let second = match &v.cells()[1] {
            crate::cell::Cell::Pointer(p) => p.value.clone(),
            _ => panic!(),
        };
        let cells: Vec<i64> = second
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![3, 4, 5]);
    }

    #[test]
    fn test_nested_literal_mixed_lengths() {
        // (1)(2 3) — a scalar item and a vector item in one nested vector
        let mut env = Environment::new();
        let v = eval_one(&mut env, "(1)(2 3)");
        assert_eq!(v.element_count(), 2);
        let lens: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Pointer(p) => p.value.element_count(),
                _ => panic!("expected pointers"),
            })
            .collect();
        assert_eq!(lens, vec![1, 2]);
    }

    #[test]
    fn test_nested_literal_with_exprs() {
        // elements can be arbitrary expressions: (2+3)(⍳4)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "(2+3)(⍳4)");
        assert_eq!(v.element_count(), 2);
        let second = match &v.cells()[1] {
            crate::cell::Cell::Pointer(p) => p.value.clone(),
            _ => panic!(),
        };
        assert_eq!(second.element_count(), 4);
    }

    #[test]
    fn test_mixed_strand() {
        // 1 'a' 2 is a mixed strand → nested vector of enclosed scalars
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 'a' 2");
        assert_eq!(v.element_count(), 3);
        for c in v.cells() {
            assert!(
                matches!(c, crate::cell::Cell::Pointer(_)),
                "expected pointers in a mixed strand"
            );
        }
        // disclose: 1 'a' 2
        let first = match &v.cells()[0] {
            crate::cell::Cell::Pointer(p) => p.value.clone(),
            _ => panic!(),
        };
        assert_eq!(first.cells(), &[crate::cell::Cell::Int(1)][..]);
        let second = match &v.cells()[1] {
            crate::cell::Cell::Pointer(p) => p.value.clone(),
            _ => panic!(),
        };
        assert_eq!(second.cells(), &[crate::cell::Cell::Char(97)][..]); // 'a'
    }

    #[test]
    fn test_mixed_reshape() {
        let mut env = Environment::new();
        assert!(env.eval_line("M←2 2⍴1 'a' 2 'b'").is_ok());
        let m = eval_one(&mut env, "M");
        assert_eq!(m.rank(), 2);
        assert_eq!(m.element_count(), 4);
        // pick M[1] — the nested 'a' (0-based ravel index 1)
        let picked = eval_one(&mut env, "⊃M[1]");
        assert_eq!(picked.cells(), &[crate::cell::Cell::Char(97)][..]);
    }

    #[test]
    fn test_numeric_strand_still_flat() {
        // all-number strands must remain FLAT vectors (not nested)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "2 3⍴1 2 3 4 5 6");
        assert_eq!(v.rank(), 2);
        for c in v.cells() {
            assert!(
                matches!(c, crate::cell::Cell::Int(_)),
                "numeric strand must stay flat, got {:?}",
                c
            );
        }
    }

    #[test]
    fn test_each_dyad() {
        let mut env = Environment::new();
        // 1 +¨ ⍳3 → nested scalars (1) (2) (3)
        let v = eval_one(&mut env, "1+¨⍳3");
        assert_eq!(v.element_count(), 3);
        for c in v.cells() {
            assert!(matches!(c, crate::cell::Cell::Pointer(_)));
        }
        // disclose and check values
        let expect = [1, 2, 3];
        for (i, e) in expect.iter().enumerate() {
            let d = match &v.cells()[i] {
                crate::cell::Cell::Pointer(p) => p.value.clone(),
                _ => panic!(),
            };
            match d.cells().first().unwrap() {
                crate::cell::Cell::Int(x) => assert_eq!(*x, *e),
                o => panic!("expected int, got {:?}", o),
            }
        }
    }

    #[test]
    fn test_each_dyad_vector_vector() {
        // 10 20 +¨ 1 2 → (11) (22)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "10 20+¨1 2");
        assert_eq!(v.element_count(), 2);
        for (i, e) in [11, 22].iter().enumerate() {
            let d = match &v.cells()[i] {
                crate::cell::Cell::Pointer(p) => p.value.clone(),
                _ => panic!(),
            };
            match d.cells().first().unwrap() {
                crate::cell::Cell::Int(x) => assert_eq!(*x, *e),
                o => panic!("expected int, got {:?}", o),
            }
        }
    }

    #[test]
    fn test_each_dyad_length_error() {
        let mut env = Environment::new();
        assert!(env.eval_line("10 20 30+¨1 2").is_err());
    }

    #[test]
    fn test_pick_assignment() {
        let mut env = Environment::new();
        assert!(env.eval_line("N←(10 20)(30 40)").is_ok());
        // (0 1⊃N)←99 replaces N[0][1]
        assert!(env.eval_line("(0 1⊃N)←99").unwrap().is_none());
        // verify: ∊N → 10 99 30 40
        let z = eval_one(&mut env, "∊N");
        assert_eq!(
            z.cells(),
            &[
                crate::cell::Cell::Int(10),
                crate::cell::Cell::Int(99),
                crate::cell::Cell::Int(30),
                crate::cell::Cell::Int(40)
            ][..]
        );
    }

    #[test]
    fn test_pick_assignment_cow_safety() {
        let mut env = Environment::new();
        assert!(env.eval_line("A←(1 2)(3 4)").is_ok());
        assert!(env.eval_line("B←A").unwrap().is_none());
        assert!(env.eval_line("(0 0⊃B)←77").unwrap().is_none());
        // A must be untouched
        let a = eval_one(&mut env, "∊A");
        assert_eq!(
            a.cells(),
            &[
                crate::cell::Cell::Int(1),
                crate::cell::Cell::Int(2),
                crate::cell::Cell::Int(3),
                crate::cell::Cell::Int(4)
            ][..]
        );
        let b = eval_one(&mut env, "∊B");
        assert_eq!(
            b.cells(),
            &[
                crate::cell::Cell::Int(77),
                crate::cell::Cell::Int(2),
                crate::cell::Cell::Int(3),
                crate::cell::Cell::Int(4)
            ][..]
        );
    }

    #[test]
    fn test_defined_function_call() {
        let mut env = Environment::new();
        crate::functions_def::define_function(&mut env.funcs, "DOUBLE X", &["X+X".to_string()])
            .unwrap();
        // monadic call: DOUBLE 21 → 42
        assert_eq!(eval_int_env(&mut env, "DOUBLE 21"), 42);
        // inside expressions
        assert_eq!(eval_int_env(&mut env, "1+DOUBLE 20"), 41);
    }

    #[test]
    fn test_defined_function_dyadic() {
        let mut env = Environment::new();
        crate::functions_def::define_function(&mut env.funcs, "R←ADD A B", &["R←A+B".to_string()])
            .unwrap();
        assert_eq!(eval_int_env(&mut env, "3 ADD 4"), 7);
    }

    #[test]
    fn test_defined_function_recursion() {
        let mut env = Environment::new();
        // classic guarded recursion via compress-branch:
        //   line 1: →(N≤1)/4     ⍝ N≤1: jump to line 4; else empty = fall through
        //   line 2: R←N×FAC N-1  ⍝ recursion
        //   line 3: →0           ⍝ exit
        //   line 4: R←1          ⍝ base result
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←FAC N",
            &[
                "→(N≤1)/4".to_string(),
                "R←N×FAC N-1".to_string(),
                "→0".to_string(),
                "R←1".to_string(),
            ],
        )
        .unwrap();
        for (n, want) in [(0, 1), (1, 1), (3, 6), (5, 120), (6, 720)] {
            let v = eval_one(&mut env, &format!("FAC {}", n));
            assert_eq!(
                v.first_cell().unwrap().get_int_value().unwrap(),
                want,
                "FAC {} wrong",
                n
            );
        }
    }

    #[test]
    fn test_local_shadowing_restored() {
        let mut env = Environment::new();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←F X",
            ["T←X*2".to_string(), "R←T+1".to_string()].as_slice(),
        )
        .unwrap();
        env.eval_line("T←100").unwrap();
        assert_eq!(eval_int_env(&mut env, "F 4"), 17); // T local = 16, +1
        assert_eq!(eval_int_env(&mut env, "T"), 100); // global T untouched
    }

    #[test]
    fn test_ambivalent_call() {
        let mut env = Environment::new();
        crate::functions_def::define_function(&mut env.funcs, "HELLO", &["42".to_string()])
            .unwrap();
        assert_eq!(eval_int_env(&mut env, "HELLO"), 42);
    }

    #[test]
    fn test_quad_io_iota_and_indexing() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // default ⎕IO=0
        assert_eq!(eval_int_env(&mut env, "⍳3"), 0);
        // switch to 1-based: ⍳3 → 1 2 3 and B[1] reads the FIRST element
        env.eval_line("⎕IO←1").unwrap();
        let v = eval_one(&mut env, "⍳3");
        assert_eq!(
            v.cells(),
            &[
                crate::cell::Cell::Int(1),
                crate::cell::Cell::Int(2),
                crate::cell::Cell::Int(3)
            ][..]
        );
        env.eval_line("B←10 20 30").unwrap();
        assert_eq!(eval_int_env(&mut env, "B[1]"), 10);
        assert_eq!(eval_int_env(&mut env, "B[3]"), 30);
        // back to 0
        env.eval_line("⎕IO←0").unwrap();
        assert_eq!(eval_int_env(&mut env, "B[0]"), 10);
    }

    #[test]
    fn test_quad_io_index_of() {
        // index-of results are ⎕IO-shifted: under ⎕IO=1 the first position
        // is 1 and "not found" is len+1.
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("A←10 20 30").unwrap();
        // ⎕IO=0: 20 is at 0-based position 1; 99 not found → len=3
        assert_eq!(eval_int_env(&mut env, "A⍳20"), 1);
        assert_eq!(eval_int_env(&mut env, "A⍳99"), 3);
        // ⎕IO=1: positions shift up
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(eval_int_env(&mut env, "A⍳20"), 2);
        assert_eq!(eval_int_env(&mut env, "A⍳99"), 4);
        // vector result: shape follows B
        let v = eval_one(&mut env, "A⍳20 30");
        assert_eq!(
            v.cells(),
            &[crate::cell::Cell::Int(2), crate::cell::Cell::Int(3)][..]
        );
    }

    #[test]
    fn test_quad_io_grade() {
        // grade results are ⎕IO-shifted too
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("B←30 10 20").unwrap();
        // ⎕IO=0: ⍋B → 1 2 0 (10,20,30)
        assert_eq!(eval_int_env(&mut env, "⍋B"), 1);
        // ⎕IO=1: shifted to 2 3 1
        env.eval_line("⎕IO←1").unwrap();
        let g = eval_one(&mut env, "⍋B");
        assert_eq!(
            g.cells(),
            &[
                crate::cell::Cell::Int(2),
                crate::cell::Cell::Int(3),
                crate::cell::Cell::Int(1)
            ][..]
        );
        // B[⍋B] still sorts correctly under IO=1
        let sorted = eval_one(&mut env, "B[⍋B]");
        assert_eq!(
            sorted.cells(),
            &[
                crate::cell::Cell::Int(10),
                crate::cell::Cell::Int(20),
                crate::cell::Cell::Int(30)
            ][..]
        );
    }

    #[test]
    fn test_dyadic_axis_syntax() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("M←2 3⍴⍳6").unwrap();
        // 1↑[0]M takes the first ROW (axis 0)
        let z = eval_one(&mut env, "1↑[0]M");
        assert_eq!(z.rank(), 2);
        assert_eq!(
            z.cells(),
            &[
                crate::cell::Cell::Int(0),
                crate::cell::Cell::Int(1),
                crate::cell::Cell::Int(2)
            ][..]
        );
        // 1↓[0]M drops the first ROW
        let d = eval_one(&mut env, "1↓[0]M");
        assert_eq!(
            d.cells(),
            &[
                crate::cell::Cell::Int(3),
                crate::cell::Cell::Int(4),
                crate::cell::Cell::Int(5)
            ][..]
        );
        // axis-1 take equals plain last-axis take
        let a = eval_one(&mut env, "2↑[1]M");
        let b = eval_one(&mut env, "2↑M");
        assert_eq!(a.cells(), b.cells());
        // rotate along axis 0 (columns rotate vertically): 1⌽[0]M
        // rows [0 1 2],[3 4 5] → each COLUMN rotates: col0: 0,3→3,0; col1: 1,4→4,1; col2: 2,5→5,2
        let r = eval_one(&mut env, "1⌽[0]M");
        assert_eq!(
            r.cells(),
            &[
                crate::cell::Cell::Int(3),
                crate::cell::Cell::Int(4),
                crate::cell::Cell::Int(5),
                crate::cell::Cell::Int(0),
                crate::cell::Cell::Int(1),
                crate::cell::Cell::Int(2)
            ][..]
        );
    }

    #[test]
    fn test_error_guard() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // NOTE: ÷0 gives inf in GNU APL (IEEE), NOT an error — use real
        // erroring guards: DOMAIN ERROR on bad shapes, VALUE ERROR on
        // undefined names.
        // guard fails (undefined name → VALUE ERROR) → fallback runs
        assert_eq!(eval_int_env(&mut env, "⎕EA NOPE+1 ⋄ 99"), 99);
        // guard succeeds → fallback NOT evaluated (would also fail)
        assert_eq!(eval_int_env(&mut env, "⎕EA 5+5 ⋄ NOPE"), 10);
        // index error: B[99] on a 3-element vector errors; fallback runs
        env.eval_line("B←10 20 30").unwrap();
        assert_eq!(eval_int_env(&mut env, "⎕EA B[99] ⋄ 42"), 42);
        // valid index returns normally
        assert_eq!(eval_int_env(&mut env, "⎕EA B[0] ⋄ 42"), 10);
    }

    #[test]
    fn test_control_if() {
        let mut env = Environment::new();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←SIGN N",
            &[
                ":If 0≤N".to_string(),
                "R←1".to_string(),
                ":EndIf".to_string(),
                ":If N<0".to_string(),
                "R←¯1".to_string(),
                ":EndIf".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(eval_int_env(&mut env, "SIGN 5"), 1);
        assert_eq!(eval_int_env(&mut env, "SIGN ¯5"), -1);
    }

    #[test]
    fn test_control_while() {
        let mut env = Environment::new();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←SUMTO N",
            &[
                "R←0".to_string(),
                "I←1".to_string(),
                ":While I≤N".to_string(),
                "R←R+I".to_string(),
                "I←I+1".to_string(),
                ":EndWhile".to_string(),
            ],
        )
        .unwrap();
        // sum 1..=4 = 10
        assert_eq!(eval_int_env(&mut env, "SUMTO 4"), 10);
        assert_eq!(eval_int_env(&mut env, "SUMTO 0"), 0);
    }

    #[test]
    fn test_control_if_else() {
        let mut env = Environment::new();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←CLASSIFY N",
            &[
                ":If N<0".to_string(),
                "R←¯1".to_string(),
                ":Else".to_string(),
                ":If N=0".to_string(),
                "R←0".to_string(),
                ":Else".to_string(),
                "R←1".to_string(),
                ":EndIf".to_string(),
                ":EndIf".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(eval_int_env(&mut env, "CLASSIFY ¯7"), -1);
        assert_eq!(eval_int_env(&mut env, "CLASSIFY 0"), 0);
        assert_eq!(eval_int_env(&mut env, "CLASSIFY 9"), 1);
    }

    #[test]
    fn test_control_repeat_until() {
        let mut env = Environment::new();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←COUNT N",
            &[
                "R←0".to_string(),
                "I←1".to_string(),
                ":Repeat".to_string(),
                "R←R+I".to_string(),
                "I←I+1".to_string(),
                ":Until I>N".to_string(),
                ":EndRepeat".to_string(),
            ],
        )
        .unwrap();
        // sum 1..=4 = 10; body runs, then checks :Until I>N
        assert_eq!(eval_int_env(&mut env, "COUNT 4"), 10);
        // N=0: body still runs once (until-check is at the end) → R=1
        assert_eq!(eval_int_env(&mut env, "COUNT 0"), 1);
    }

    #[test]
    fn test_control_leave_in_while() {
        let mut env = Environment::new();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←FINDSTOP N",
            &[
                "R←0".to_string(),
                "I←1".to_string(),
                ":While 1".to_string(),
                "R←R+I".to_string(),
                ":If I≥N".to_string(),
                ":Leave".to_string(),
                ":EndIf".to_string(),
                "I←I+1".to_string(),
                ":EndWhile".to_string(),
            ],
        )
        .unwrap();
        // infinite :While 1 loop, exited by :Leave when I reaches N: sum 1..=3 = 6
        assert_eq!(eval_int_env(&mut env, "FINDSTOP 3"), 6);
    }

    #[test]
    fn test_control_leave_in_repeat() {
        let mut env = Environment::new();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←LEAVECOUNT N",
            &[
                "R←0".to_string(),
                "I←1".to_string(),
                ":Repeat".to_string(),
                "R←R+1".to_string(),
                ":If I≥N".to_string(),
                ":Leave".to_string(),
                ":EndIf".to_string(),
                "I←I+1".to_string(),
                ":EndRepeat".to_string(),
            ],
        )
        .unwrap();
        // no :Until — only :Leave terminates. N passes → R counts N iterations
        assert_eq!(eval_int_env(&mut env, "LEAVECOUNT 5"), 5);
    }

    #[test]
    fn test_outer_product_syntax() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // 1 2 ∘.× 1 3 → 2×2 matrix 1 3 / 2 6
        let r = eval_one(&mut env, "1 2∘.×1 3");
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 2);
        assert_eq!(r.get_shape_item(1), 2);
        let expect = [1, 3, 2, 6];
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(r.cells()[i], crate::cell::Cell::Int(*e));
        }
    }

    #[test]
    fn test_inner_product_bool_and_equal() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // 1 0 1 ∧.= 1 1 1 → ∧/(1=1)(0=1)(1=1) = ∧/1 0 1 = 0
        let r = eval_one(&mut env, "1 0 1∧.=1 1 1");
        assert_eq!(r.first_cell().unwrap(), &crate::cell::Cell::Int(0));
    }

    #[test]
    fn test_dfn_immediate_calls() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // monadic immediate: {⍵+1} 5 → 6
        assert_eq!(
            eval_one(&mut env, "{⍵+1} 5").first_cell().unwrap(),
            &crate::cell::Cell::Int(6)
        );
        // dyadic immediate: 2 {⍺×⍵} 3 → 6
        assert_eq!(
            eval_one(&mut env, "2 {⍺×⍵} 3").first_cell().unwrap(),
            &crate::cell::Cell::Int(6)
        );
        // monadic call on a ⍺-using body → VALUE ERROR (⍺ unbound)
        assert!(env.eval_line("{⍺+⍵} 3 4").is_err());
    }

    #[test]
    fn test_dfn_named_definitions() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // definition produces no output
        assert!(env.eval_line("DOUBLE←{⍵×2}").unwrap().is_none());
        // and the function is callable
        assert_eq!(
            eval_one(&mut env, "DOUBLE 21").first_cell().unwrap(),
            &crate::cell::Cell::Int(42)
        );
        // dyadic named dfn
        assert!(env.eval_line("SUM←{⍺+⍵}").unwrap().is_none());
        assert_eq!(
            eval_one(&mut env, "5 SUM 7").first_cell().unwrap(),
            &crate::cell::Cell::Int(12)
        );
        // dfn visible via )FNS
        assert!(env.funcs.get("DOUBLE").is_some());
        assert!(env.funcs.get("SUM").is_some());
    }

    #[test]
    fn test_inner_product_syntax_dot_product() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // 1 2 3 +.× 10 20 30 → 140
        let r = eval_one(&mut env, "1 2 3+.×10 20 30");
        assert_eq!(r.first_cell().unwrap(), &crate::cell::Cell::Int(140));
    }

    #[test]
    fn test_inner_product_syntax_matrix_times_vector() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // M←2 3⍴⍳6; M +.× 5 6 7 → (0·5+1·6+2·7)(3·5+4·6+5·7) = 20 74
        env.eval_line("M←2 3⍴⍳6").unwrap();
        let r = eval_one(&mut env, "M+.×5 6 7");
        // ⎕IO=0 → ⍳6 is 0..5, so rows are (0 1 2) and (3 4 5)
        let expect = [20, 74];
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(r.cells()[i], crate::cell::Cell::Int(*e));
        }
    }

    #[test]
    fn test_inner_product_length_error() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        assert!(env.eval_line("1 2+.×1 2 3").is_err());
    }

    #[test]
    fn test_each_monadic() {
        let mut env = Environment::new();
        env.eval_line("V←⍳4").unwrap(); // 0 1 2 3
        let v = eval_one(&mut env, "-¨V"); // negate each → nested scalars
        assert_eq!(v.element_count(), 4);
        // every result cell is a pointer to a scalar
        for c in v.cells() {
            match c {
                crate::cell::Cell::Pointer(p) => {
                    assert!(p.value.is_scalar_shape());
                }
                o => panic!("expected pointer, got {:?}", o),
            }
        }
        // check the values via disclose semantics: second should be ¯1
        let first = match &v.cells()[1] {
            crate::cell::Cell::Pointer(p) => p.value.clone(),
            _ => panic!(),
        };
        match first.cells().first().unwrap() {
            crate::cell::Cell::Int(i) => assert_eq!(*i, -1),
            o => panic!("expected int, got {:?}", o),
        }
    }

    #[test]
    fn test_each_stray_is_syntax_error() {
        let mut env = Environment::new();
        assert!(env.eval_line("¨5").is_err());
    }

    #[test]
    fn test_selective_assignment() {
        let mut env = Environment::new();
        env.eval_line("B←10 20 30").unwrap();
        // assignments produce no output
        assert!(env.eval_line("B[1]←99").unwrap().is_none());
        assert_eq!(eval_int_env(&mut env, "B[1]"), 99);
        // other elements untouched
        assert_eq!(eval_int_env(&mut env, "B[0]"), 10);
        assert_eq!(eval_int_env(&mut env, "B[2]"), 30);
    }

    #[test]
    fn test_selective_assignment_cow_safety() {
        let mut env = Environment::new();
        env.eval_line("A←1 2 3").unwrap();
        env.eval_line("B←A").unwrap(); // B shares with A
        env.eval_line("B[0]←99").unwrap();
        // A must be unchanged (COW isolation)
        assert_eq!(eval_int_env(&mut env, "A[0]"), 1);
        assert_eq!(eval_int_env(&mut env, "B[0]"), 99);
    }

    #[test]
    fn test_selective_assignment_index_vector() {
        let mut env = Environment::new();
        env.eval_line("V←0 0 0 0 0").unwrap();
        env.eval_line("V[1 3]←7").unwrap();
        let v = eval_one(&mut env, "V");
        let cells: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![0, 7, 0, 7, 0]);
    }

    #[test]
    fn test_selective_assignment_out_of_range() {
        let mut env = Environment::new();
        env.eval_line("B←10 20 30").unwrap();
        assert!(env.eval_line("B[99]←1").is_err());
    }

    #[test]
    fn test_bracket_indexing() {
        let mut env = Environment::new();
        env.eval_line("B←30 10 20").unwrap();
        assert_eq!(eval_int_env(&mut env, "B[1]"), 10);
        assert_eq!(eval_int_env(&mut env, "B[0]"), 30);
        assert_eq!(eval_int_env(&mut env, "B[2]"), 20);
    }

    #[test]
    fn test_index_out_of_range() {
        let mut env = Environment::new();
        env.eval_line("B←30 10 20").unwrap();
        assert!(env.eval_line("B[99]").is_err());
    }

    #[test]
    fn test_grade_sort_roundtrip() {
        let mut env = Environment::new();
        // B[⍋B] must be the sorted version of B
        env.eval_line("B←42 7 19 3").unwrap();
        let v = eval_one(&mut env, "B[⍋B]");
        let cells: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![3, 7, 19, 42]);

        let v = eval_one(&mut env, "B[⍒B]");
        let cells: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                _ => panic!("expected ints"),
            })
            .collect();
        assert_eq!(cells, vec![42, 19, 7, 3]);
    }

    #[test]
    fn test_parse_guard() {
        let toks = tokenize("{⍵<0:(-⍵) ⋄ ⍵}").unwrap();
        let (e, used) = parse(&toks).unwrap();
        assert_eq!(used, toks.len() - 1);
        match &e {
            Expr::Dfn(body) => match &**body {
                Expr::If(c, t, _) => {
                    assert!(matches!(**c, Expr::Dyadic(_, _, _)));
                    assert!(matches!(**t, Expr::Monadic(_, _)));
                }
                _ => panic!("expected If in dfn body, got {:?}", body),
            },
            _ => panic!("expected Dfn, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_multi_guard() {
        let toks = tokenize("{⍵=0:(1) ⋄ ⍵×∇ ⍵-1}").unwrap();
        let (e, used) = parse(&toks).unwrap();
        assert_eq!(used, toks.len() - 1);
        // FAC: outer If has condition ⍵=0, then-branch (1), else-branch
        // is the body (⍵×∇⍵-1)
        match &e {
            Expr::Dfn(body) => match &**body {
                Expr::If(_, _, else_b) => {
                    assert!(matches!(**else_b, Expr::Dyadic(_, _, _)));
                }
                _ => panic!("expected If, got {:?}", body),
            },
            _ => panic!("expected Dfn, got {:?}", e),
        }
    }

    #[test]
    fn test_dop_simple() {
        // DOP←{⍺⍺ ⍵} — applies ⍺⍺ monadically to ⍵
        // + DOP × 5 → body {⍺⍺ ⍵} with ⍺⍺=+, ⍵=5 → + 5 = 5
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("DOP←{⍺⍺ ⍵}").unwrap();
        // mark as dop
        if let Some(f) = env.funcs.get_mut("DOP") {
            f.is_dop = true;
        }
        let result = env.eval_line("+ DOP × 5").unwrap();
        assert!(result.is_some());
        let v = result.unwrap();
        // + DOP × 5 → dop call: ⍺⍺=+, ⍵⍵=×, ⍵=5 → body {+ 5} = 5
        assert_eq!(v.first_cell().unwrap().get_near_int().unwrap(), 5);
    }

    #[test]
    fn test_dop_dyadic() {
        // DOP←{⍺ ⍺⍺ ⍵} — applies ⍺⍺ dyadically to ⍺ and ⍵
        // 2 + DOP × 5 → ⍺=2, ⍺⍺=+, ⍵⍵=×, ⍵=5 → body {2 + 5} = 7
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("DOP←{⍺ ⍺⍺ ⍵}").unwrap();
        if let Some(f) = env.funcs.get_mut("DOP") {
            f.is_dop = true;
        }
        let result = env.eval_line("2 + DOP × 5").unwrap();
        assert!(result.is_some());
        let v = result.unwrap();
        // 2 + 5 = 7
        assert_eq!(v.first_cell().unwrap().get_near_int().unwrap(), 7);
    }
}
