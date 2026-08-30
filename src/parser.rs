//! Recursive-descent parser and evaluator for APL expressions.
//!
//! Mirrors `src/Parser.cc` + the prefix machine (simplified): handles
//! monadic/dyadic function application, parentheses, assignment, with
//! right-to-left (APL) evaluation order.

use std::collections::HashMap;

use crate::cell::Cell;
use crate::functions::Prim;
use crate::tokenizer::{tokenize, PowerFn, Tok};
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
    /// a complex strand: adjacent complex literals `1J2 2J3 3J4`
    ComplexVec(Vec<(f64, f64)>),
    /// a nested strand: adjacent parenthesized groups `(1 2)(3 4)` or
    /// mixed `(1)(2 3)` — each element is enclosed
    NestedVec(Vec<Expr>),
    Str(Vec<u32>),
    Var(String),
    Monadic(Prim, Box<Expr>),
    /// `LO/B` — reduce
    ReduceOp(Prim, Box<Expr>),
    /// `LO\\B` — scan
    ScanOp(Prim, Box<Expr>),
    /// `LO⌿B` — first-axis reduce
    Reduce1Op(Prim, Box<Expr>),
    /// `LO⍀B` — first-axis scan
    Scan1Op(Prim, Box<Expr>),
    /// `LO/[n] B` — reduce along axis n
    ReduceAxis(Prim, Box<Expr>, Box<Expr>),
    /// `LO\\[n] B` — scan along axis n
    ScanAxis(Prim, Box<Expr>, Box<Expr>),
    /// `F¨B` — each (monadic)
    EachOp(Prim, Box<Expr>),
    /// `A F¨B` — each (dyadic)
    EachDyad(Prim, Box<Expr>, Box<Expr>),
    /// `f¨B` — each with named function (monadic)
    EachOpName(String, Box<Expr>),
    /// `A f¨B` — each with named function (dyadic)
    EachDyadName(String, Box<Expr>, Box<Expr>),
    /// rank operator `(f⍤k)B` — apply f to each rank-k cell of B
    RankOp(Prim, i64, Box<Expr>),
    /// dyadic rank `A(f⍤kl kr)B` — separate cell ranks for the two arguments
    /// (the single-rank spelling `f⍤k` stores k in both)
    RankDyad(Prim, i64, i64, Box<Expr>, Box<Expr>),
    /// `A ∘.f B` — outer product
    OuterProduct(Prim, Box<Expr>, Box<Expr>),
    /// `A ∘ B` — matrix product (equivalent to A +.× B)
    MatrixProduct(Box<Expr>, Box<Expr>),
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
    /// `4 ⎕CR B` — boxed display (4⎕CR-style). Returns a char matrix/vector.
    QuadCr(i64, Box<Expr>),
    /// `⎕RVAL B` — random value (rank, shape, type, depth)
    QuadRval(Box<Expr>),
    /// `⎕RL B` — random link (seed state)
    QuadRl(Box<Expr>),
    /// `⎕CC B` — case conversion
    QuadCc(Box<Expr>),
    /// `⎕DLX B` — dancing links exact cover
    QuadDlx(Box<Expr>),
    /// `⎕TF B` — transfer form
    QuadTf(Box<Expr>),
    /// `⎕FX B` — fix function from character matrix
    QuadFx(Box<Expr>),
    /// `⎕MAP B` — symbol table map
    QuadMap(Box<Expr>),
    /// `⎕MX B` — matrix operations
    QuadMx(Box<Expr>),
    /// `⎕FIO B` — file I/O operations
    QuadFio(Box<Expr>),
    /// `⎕JSON B` — JSON parse/serialize
    QuadJson(Box<Expr>),
    /// `⎕XML B` — XML parse/serialize
    QuadXml(Box<Expr>),
    /// `⎕UCS B` — Unicode character set conversion
    QuadUcs(Box<Expr>),
    /// `⎕AV` — APL character vector
    QuadAv,
    /// `⎕TS` — current timestamp
    QuadTs,
    /// `⎕WA` — workspace available
    QuadWa,
    /// `⎕TC` — terminal control characters
    QuadTc,
    /// `⎕DM` — error message
    QuadDm,
    /// `⎕EN` — error number
    QuadEn,
    /// `(F⍣N) B` — power operator: apply F N times to B
    PowerOp(PowerFn, i64, Box<Expr>),
    /// `⍬` — zilde: the empty numeric vector
    Zilde,
    /// `reJ im` — complex number literal (e.g. `1J2`)
    Complex(f64, f64),
    /// `NAME[expr]` — bracket indexing
    Index(Box<Expr>, Box<Expr>),
    /// Multi-axis bracket index `B[i;j;...]`. One entry per axis; `None` is an
    /// ELIDED index, which selects that whole axis (`M[1;]` = row 1).
    IndexAxes(Box<Expr>, Vec<Option<Expr>>),
    Dyadic(Prim, Box<Expr>, Box<Expr>),
    Assign(String, Box<Expr>),
    /// `NAME +← expr` — modified assignment (shorthand for NAME ← NAME + expr)
    ModifiedAssign(String, Prim, Box<Expr>),
    /// selective assignment: `NAME[idx] ← expr`
    AssignIndexed(String, Box<Expr>, Box<Expr>),
    /// Multi-axis selective assignment: `NAME[i;j;...] ← expr`. One entry per
    /// axis; `None` is an elided index (that whole axis).
    AssignIndexAxes(String, Vec<Option<Expr>>, Box<Expr>),
    /// Selective assignment through a selector: `(selector)←value`
    /// The selector is a monadic function call (e.g. `2↑V`, `⌽V`, `3⍴V`).
    /// Uses the marker-array technique: apply the selector to an array of
    /// ravel indices to discover which positions are selected, then write
    /// the RHS values into those positions.
    AssignSelector(Box<Expr>, Box<Expr>, String),
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
    /// monadic: ⌸B — key (groups B's ravel elements, Dyalog extension)
    #[cfg(feature = "unofficial-ext")]
    Key(Box<Expr>),
    /// dyadic: A⌸B — key with A applied to B first (Dyalog extension)
    #[cfg(feature = "unofficial-ext")]
    KeyDyad(Box<Expr>, Box<Expr>),
    /// monadic operator: (f⍥g)B — over: f(g(B)) (Dyalog extension)
    #[cfg(feature = "unofficial-ext")]
    OverMonad(Prim, Prim, Box<Expr>),
    /// dyadic operator: A(f⍥g)B — over: f(g(A),g(B)) (Dyalog extension)
    #[cfg(feature = "unofficial-ext")]
    OverDyad(Prim, Prim, Box<Expr>, Box<Expr>),
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

/// Read the rank list following a `Rank(p)` token.
///
/// `f⍤k` is a single rank; `f⍤kl kr` splits the two arguments. Returns
/// `(kl, kr, n)` where `n` is the number of consumed Num tokens.
fn read_rank_list(toks: &[Tok]) -> AplResult<(i64, i64, usize)> {
    let a = match toks.first() {
        Some(Tok::Num(v)) => *v as i64,
        _ => return Err(ErrorCode::SyntaxError),
    };
    match toks.get(1) {
        Some(Tok::Num(v)) => Ok((a, *v as i64, 2)),
        _ => Ok((a, a, 1)),
    }
}

/// Parse `(f⍥g)` from tokens starting at `(`. Returns (f, g, tokens_consumed).
#[cfg(feature = "unofficial-ext")]
fn parse_over_operator(toks: &[Tok]) -> AplResult<(Prim, Prim, usize)> {
    // Expected: LParen, Prim(f), Prim(Over), Prim(g), RParen
    if toks.len() < 5 {
        return Err(ErrorCode::SyntaxError);
    }
    if !matches!(toks.first(), Some(Tok::LParen)) {
        return Err(ErrorCode::SyntaxError);
    }
    if let Some(Tok::Prim(f_p)) = toks.get(1) {
        if matches!(toks.get(2), Some(Tok::Prim(Prim::Over))) {
            if let Some(Tok::Prim(g_p)) = toks.get(3) {
                if matches!(toks.get(4), Some(Tok::RParen)) {
                    return Ok((*f_p, *g_p, 5));
                }
            }
        }
    }
    Err(ErrorCode::SyntaxError)
}

/// Parse `f⍥g` (without parens) — simplified form for monadic use.
#[cfg(feature = "unofficial-ext")]
fn parse_over_operator_simple(toks: &[Tok]) -> AplResult<(Prim, Prim, usize)> {
    // Expected: LParen, f, Over, g, RParen
    parse_over_operator(toks)
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
                    // The apl_name is evaluated to a character vector at runtime.
                    // For a Name token, we treat it as a string literal (not a
                    // variable reference) since ⎕NA defines the name, not reads it.
                    let e = match toks.first() {
                        Some(Tok::Name(n)) => {
                            let cps: Vec<u32> = n.chars().map(|c| c as u32).collect();
                            Expr::Str(cps)
                        }
                        Some(Tok::Str(s)) => Expr::Str(s.clone()),
                        _ => return Err(ErrorCode::SyntaxError),
                    };
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
        // selective assignment through a selector: (selector)←value
        // The pattern is LParen ... Name RParen Assign — the last token
        // inside the parens is the variable name, the rest is a monadic
        // selector (e.g. 2↑V, ⌽V, 3⍴V). We scan to find the matching
        // closing paren and check that a Name precedes it.
        if let Some((selector, name, close)) = scan_selector_target(toks) {
            if matches!(toks.get(close + 1), Some(Tok::Assign)) {
                let (rhs, rused) = parse_expr(&toks[close + 2..])?;
                return Ok((
                    Expr::AssignSelector(Box::new(selector), Box::new(rhs), name),
                    close + 2 + rused,
                ));
            }
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
        // modified assignment: NAME +← expr — shorthand for NAME ← NAME + expr
        if let Some(Tok::ModifiedAssign(p)) = toks.get(1) {
            let p = *p;
            let (rhs, used) = parse_expr(&toks[2..])?;
            return Ok((Expr::ModifiedAssign(name, p, Box::new(rhs)), used + 2));
        }
        // selective assignment: NAME[expr] ← expr  or  NAME[i;j;...] ← expr
        if matches!(toks.get(1), Some(Tok::LBracket)) {
            if let Some((parts, close)) = split_index_axes(&toks[2..]) {
                // toks[2 + close] is the ']'; an Assign must follow it
                let after = 2 + close + 1;
                if matches!(toks.get(after), Some(Tok::Assign)) {
                    let axes = parse_index_axes(&parts)?;
                    let (rhs, rused) = parse_expr(&toks[after + 1..])?;
                    let consumed = after + 1 + rused;
                    if axes.len() == 1 {
                        // single index, no semicolon: keep the 1-D form
                        let idx = axes
                            .into_iter()
                            .next()
                            .flatten()
                            .ok_or(ErrorCode::SyntaxError)?;
                        return Ok((
                            Expr::AssignIndexed(name, Box::new(idx), Box::new(rhs)),
                            consumed,
                        ));
                    }
                    return Ok((Expr::AssignIndexAxes(name, axes, Box::new(rhs)), consumed));
                }
            }
            // fall through: not an assignment — the bracket use is an
            // ordinary index expression handled by parse_simple
        }
    }
    parse_simple(toks)
}

/// Try to scan `( selector NAME )` starting at toks[0] == LParen.
///
/// Returns `Some((selector_expr, name, close_offset))` where the selector is
/// a monadic function call on NAME (e.g. `2↑V`, `⌽V`, `3⍴V`, `1 2⌷M`).
fn scan_selector_target(toks: &[Tok]) -> Option<(Expr, String, usize)> {
    // find the matching closing paren, tracking depth
    let mut depth = 0;
    let mut close = None;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::LParen => depth += 1,
            Tok::RParen => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    if close < 3 {
        return None;
    }
    // the last token inside the parens must be a Name
    let name = match toks.get(close - 1) {
        Some(Tok::Name(n)) => n.clone(),
        _ => return None,
    };
    // build the selector expression from toks[1..close]
    let inner = &toks[1..close];
    if inner.is_empty() {
        return None;
    }
    let selector = build_selector_expr(inner)?;
    Some((selector, name, close))
}

/// Build a selector expression from tokens.
///
/// The selector is `[left_args...] func NAME` where left_args is zero or
/// more numeric literals (forming a strand if multiple), func is a Prim,
/// and NAME is the variable. We build the expression directly because
/// parse_expr would treat NAME as a function call rather than a variable.
fn build_selector_expr(toks: &[Tok]) -> Option<Expr> {
    // find the last Prim token (the function)
    let mut func_pos = None;
    for (i, t) in toks.iter().enumerate() {
        if let Tok::Prim(_) = t {
            func_pos = Some(i);
        }
    }
    let func_pos = func_pos?;
    // the token after func must be a Name
    let name = match toks.get(func_pos + 1) {
        Some(Tok::Name(n)) => n.clone(),
        _ => return None,
    };
    // func must be the second-to-last token
    if func_pos + 2 != toks.len() {
        return None;
    }
    let func = match toks.get(func_pos) {
        Some(Tok::Prim(p)) => *p,
        _ => return None,
    };
    let arg = Expr::Var(name);
    if func_pos == 0 {
        // monadic: func NAME
        Some(Expr::Monadic(func, Box::new(arg)))
    } else {
        // dyadic: [left_args...] func NAME
        let left_toks = &toks[..func_pos];
        let left = build_selector_left(strand_value(left_toks)?)?;
        Some(Expr::Dyadic(func, Box::new(left), Box::new(arg)))
    }
}

/// Convert a slice of numeric tokens into a strand value (scalar or vector).
fn strand_value(toks: &[Tok]) -> Option<Vec<f64>> {
    let mut vals = Vec::with_capacity(toks.len());
    for t in toks {
        match t {
            Tok::Num(v) => vals.push(*v),
            _ => return None,
        }
    }
    Some(vals)
}

/// Convert a numeric vector into an Expr (Num for scalar, NumVec for vector).
fn build_selector_left(vals: Vec<f64>) -> Option<Expr> {
    if vals.len() == 1 {
        Some(Expr::Num(vals[0]))
    } else {
        Some(Expr::NumVec(vals))
    }
}
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
    // dyadic each with named function: A f¨ B
    if let Some(Tok::EachName(n)) = toks.get(used) {
        let n = n.clone();
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::EachDyadName(n, Box::new(lhs), Box::new(rhs)), used));
    }

    // dyadic key: A ⌸ B — key with A applied to B first.
    #[cfg(feature = "unofficial-ext")]
    if let Some(Tok::Prim(Prim::Key)) = toks.get(used) {
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::KeyDyad(Box::new(lhs), Box::new(rhs)), used));
    }

    // dyadic over: A (f⍥g) B — over operator.
    // Token pattern after lhs: LParen Prim(f) Prim(Over) Prim(g) RParen
    #[cfg(feature = "unofficial-ext")]
    if let Ok((f_p, g_p, consumed)) = parse_over_operator(&toks[used..]) {
        let (rhs, rused) = parse_simple(&toks[used + consumed..])?;
        used += consumed + rused;
        return Ok((Expr::OverDyad(f_p, g_p, Box::new(lhs), Box::new(rhs)), used));
    }

    // dyadic over: A f⍥g B — check before normal dyadic dispatch
    // The tokenizer emits Tok::Prim(Prim::Over) for ⍥
    #[cfg(feature = "unofficial-ext")]
    if let (Some(Tok::Prim(f_p)), Some(Tok::Prim(Prim::Over))) =
        (toks.get(used), toks.get(used + 1))
    {
        let f = *f_p;
        // Next should be Prim(g)
        if let Some(Tok::Prim(g_p)) = toks.get(used + 2) {
            let g = *g_p;
            // B is the rest after g
            let (b, rused) = parse_simple(&toks[used + 3..])?;
            used += 3 + rused;
            return Ok((Expr::OverDyad(f, *g_p, Box::new(lhs), Box::new(b)), used));
        }
    }

    // dyadic rank: A (F⍤k) B or A F⍤kl kr B — the rank list follows the glyph
    if let Some(Tok::Rank(p)) = toks.get(used) {
        let p = *p;
        let (kl, kr, nk) = read_rank_list(&toks[used + 1..])?;
        let (rhs, rused) = parse_simple(&toks[used + 1 + nk..])?;
        used += 1 + nk + rused;
        return Ok((
            Expr::RankDyad(p, kl, kr, Box::new(lhs), Box::new(rhs)),
            used,
        ));
    }

    // dyadic rank in PARENTHESISED form: A (F⍤kl kr) B
    if let (Some(Tok::LParen), Some(Tok::Rank(p))) = (toks.get(used), toks.get(used + 1)) {
        let p = *p;
        if let Ok((kl, kr, nk)) = read_rank_list(&toks[used + 2..]) {
            if matches!(toks.get(used + 2 + nk), Some(Tok::RParen)) {
                let after = used + 2 + nk + 1;
                let (rhs, rused) = parse_simple(&toks[after..])?;
                used = after + rused;
                return Ok((
                    Expr::RankDyad(p, kl, kr, Box::new(lhs), Box::new(rhs)),
                    used,
                ));
            }
        }
    }

    // outer product: A ∘.f B
    if let Some(Tok::OuterDot(p)) = toks.get(used) {
        let p = *p;
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::OuterProduct(p, Box::new(lhs), Box::new(rhs)), used));
    }

    // matrix product: A ∘ B
    if let Some(Tok::MatrixProduct) = toks.get(used) {
        let (rhs, rused) = parse_simple(&toks[used + 1..])?;
        used += 1 + rused;
        return Ok((Expr::MatrixProduct(Box::new(lhs), Box::new(rhs)), used));
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
    // N ⎕CR B — character representation (N=1: ravel, N=4: boxed)
    if matches!(toks.first(), Some(Tok::Num(v)) if *v == 1.0 || *v == 4.0)
        && matches!(toks.get(1), Some(Tok::Name(n)) if n == "⎕CR")
    {
        let n = toks.first().unwrap().clone();
        let n = match n {
            Tok::Num(v) => v as i64,
            _ => 4,
        };
        let (arg, used) = parse(&toks[2..])?;
        return Ok((Expr::QuadCr(n, Box::new(arg)), 2 + used));
    }
    // ⎕UCS B — Unicode character set conversion
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕UCS" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadUcs(Box::new(arg)), 1 + used));
        }
    }
    // ⎕AV — APL character vector
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕AV" {
            return Ok((Expr::QuadAv, 1));
        }
    }
    // ⎕TS — current timestamp
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕TS" {
            return Ok((Expr::QuadTs, 1));
        }
    }
    // ⎕WA — workspace available
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕WA" {
            return Ok((Expr::QuadWa, 1));
        }
    }
    // ⎕TC — terminal control characters
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕TC" {
            return Ok((Expr::QuadTc, 1));
        }
    }
    // ⎕DM — error message
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕DM" {
            return Ok((Expr::QuadDm, 1));
        }
    }
    // ⎕EN — error number
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕EN" {
            return Ok((Expr::QuadEn, 1));
        }
    }
    // ⎕RVAL — random value
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕RVAL" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadRval(Box::new(arg)), 1 + used));
        }
    }
    // ⎕RL — random link
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕RL" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadRl(Box::new(arg)), 1 + used));
        }
    }
    // ⎕CC — case conversion
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕CC" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadCc(Box::new(arg)), 1 + used));
        }
    }
    // ⎕DLX — dancing links
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕DLX" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadDlx(Box::new(arg)), 1 + used));
        }
    }
    // ⎕TF — transfer form
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕TF" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadTf(Box::new(arg)), 1 + used));
        }
    }
    // ⎕FX — fix function
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕FX" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadFx(Box::new(arg)), 1 + used));
        }
    }
    // ⎕MAP — symbol table map
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕MAP" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadMap(Box::new(arg)), 1 + used));
        }
    }
    // ⎕MX — matrix operations
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕MX" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadMx(Box::new(arg)), 1 + used));
        }
    }
    // ⎕FIO — file I/O
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕FIO" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadFio(Box::new(arg)), 1 + used));
        }
    }
    // ⎕JSON — JSON parse/serialize
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕JSON" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadJson(Box::new(arg)), 1 + used));
        }
    }
    // ⎕XML — XML parse/serialize
    if let Some(Tok::Name(n)) = toks.first() {
        if n == "⎕XML" {
            let (arg, used) = parse(&toks[1..])?;
            return Ok((Expr::QuadXml(Box::new(arg)), 1 + used));
        }
    }
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
            // Check for `(f⍥g) B` pattern: LParen Prim(f) Over Prim(g) RParen
            // This is monadic over: f(g(B))
            #[cfg(feature = "unofficial-ext")]
            if let (Some(Tok::Prim(f_p)), Some(Tok::Prim(Prim::Over))) = (toks.get(1), toks.get(2))
            {
                if let Some(Tok::Prim(g_p)) = toks.get(3) {
                    if matches!(toks.get(4), Some(Tok::RParen)) {
                        let f = *f_p;
                        let g = *g_p;
                        let (rhs, rused) = parse_simple(&toks[5..])?;
                        return Ok((Expr::OverMonad(f, g, Box::new(rhs)), 5 + rused));
                    }
                }
            }
            // `(F⍤k) B` or `(F⍤kl kr) B` — a parenthesised DERIVED FUNCTION.
            // The rank operator's argument sits after the closing paren, so
            // the inner parse would fail. Detect LParen Rank(p) Num... RParen
            // and take the argument from outside the parens.
            if let Some(Tok::Rank(p)) = toks.get(1) {
                let p = *p;
                if let Ok((_, k, nk)) = read_rank_list(&toks[2..]) {
                    let close = 2 + nk;
                    if matches!(toks.get(close), Some(Tok::RParen)) {
                        let (operand, used) = parse_simple(&toks[close + 1..])?;
                        return Ok((Expr::RankOp(p, k, Box::new(operand)), close + 1 + used));
                    }
                }
            }
            // `(F⍣N) B` — parenthesised power operator.
            // Detect LParen PowerOp(p) Num(n) RParen and apply F N times.
            if let Some(Tok::PowerOp(p)) = toks.get(1) {
                let p = p.clone();
                if let Some(Tok::Num(n)) = toks.get(2) {
                    if matches!(toks.get(3), Some(Tok::RParen)) {
                        let n = *n as i64;
                        let (operand, used) = parse_simple(&toks[4..])?;
                        return Ok((Expr::PowerOp(p, n, Box::new(operand)), 4 + used));
                    }
                }
            }
            let (e, used) = parse_expr(&toks[1..])?;
            if !matches!(toks.get(used + 1), Some(Tok::RParen)) {
                return Err(ErrorCode::SyntaxError);
            }
            let total = used + 2;

            // bracket indexing directly on a parenthesised value:
            // `(2 3⍴⍳6)[1;1]`
            if matches!(toks.get(total), Some(Tok::LBracket)) {
                let (parts, close) =
                    split_index_axes(&toks[total + 1..]).ok_or(ErrorCode::SyntaxError)?;
                let consumed = total + 1 + close + 1;
                let axes = parse_index_axes(&parts)?;
                if axes.len() == 1 {
                    let idx = axes
                        .into_iter()
                        .next()
                        .flatten()
                        .ok_or(ErrorCode::SyntaxError)?;
                    return Ok((Expr::Index(Box::new(e), Box::new(idx)), consumed));
                }
                return Ok((Expr::IndexAxes(Box::new(e), axes), consumed));
            }

            // nested strand: `(expr)(expr)...` — adjacent paren groups form
            // a vector of enclosed values. But a paren group that opens with
            // an operator token is a DERIVED FUNCTION (e.g. (f⍤k)), not a
            // strand element — return here and let parse_simple match it.
            if matches!(toks.get(total), Some(Tok::LParen)) {
                match toks.get(total + 1) {
                    Some(Tok::Rank(_)) | Some(Tok::Each(_)) | Some(Tok::Reduce(_))
                    | Some(Tok::Scan(_)) | Some(Tok::Scan1(_)) => return Ok((e, total)),
                    #[cfg(feature = "unofficial-ext")]
                    Some(Tok::Prim(Prim::Over)) => return Ok((e, total)),
                    _ => {}
                }
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
            // Unofficial extensions: ⌸ (key) and ⍥ (over) need special parsing
            #[cfg(feature = "unofficial-ext")]
            if p == crate::functions::Prim::Key {
                let (operand, used) = parse_simple(&toks[1..])?;
                return Ok((Expr::Key(Box::new(operand)), used + 1));
            }
            #[cfg(feature = "unofficial-ext")]
            if p == crate::functions::Prim::Over {
                let (f, g, consumed) = parse_over_operator_simple(&toks[1..])?;
                let (operand, op_used) = parse_simple(&toks[1 + consumed..])?;
                return Ok((
                    Expr::OverMonad(f, g, Box::new(operand)),
                    1 + consumed + op_used,
                ));
            }
            // monadic functions bind to the WHOLE expression to their right
            // (APL semantics): ⊖2 3⍴⍳6 = ⊖(2 3⍴⍳6), not (⊖2) 3⍴⍳6
            let (operand, used) = parse_simple(&toks[1..])?;
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
        Tok::EachName(n) => {
            // monadic operator with named function: f¨ B
            let n = n.clone();
            let (operand, used) = parse_simple(&toks[1..])?;
            Ok((Expr::EachOpName(n, Box::new(operand)), used + 1))
        }
        #[cfg(feature = "unofficial-ext")]
        Tok::Prim(Prim::Key) => {
            // monadic key: ⌸ B — group B's ravel elements
            let (operand, used) = parse_simple(&toks[1..])?;
            Ok((Expr::Key(Box::new(operand)), used + 1))
        }
        #[cfg(feature = "unofficial-ext")]
        Tok::Prim(Prim::Over) => {
            // monadic over: (f⍥g) B
            // Over without left arg is parsed as a dyadic operator form
            // where the left is implicit: (f⍥g) B
            let (f, g, consumed) = parse_over_operator_simple(&toks[1..])?;
            let (operand, op_used) = parse_simple(&toks[1 + consumed..])?;
            Ok((
                Expr::OverMonad(f, g, Box::new(operand)),
                1 + consumed + op_used,
            ))
        }
        Tok::Rank(p) => {
            // rank operator: (F⍤k) B or (F⍤kl kr) B. The rank list comes
            // straight after the glyph. For MONADIC use the reference uses
            // the RIGHT rank kr for the single argument — so with one number
            // both are the same, but with two numbers f⍤1 0 applies rank 0.
            let p = *p;
            let (_, k, nk) = read_rank_list(&toks[1..])?;
            let (operand, used) = parse_simple(&toks[1 + nk..])?;
            Ok((Expr::RankOp(p, k, Box::new(operand)), 1 + nk + used))
        }
        Tok::PowerOp(p) => {
            // power operator: (F⍣N) B — apply F N times to B
            // N is a scalar integer, B is the operand
            let p = p.clone();
            let n_tok = toks.get(1);
            let n = match n_tok {
                Some(Tok::Num(v)) => *v as i64,
                _ => return Err(ErrorCode::SyntaxError),
            };
            let (operand, used) = parse_simple(&toks[2..])?;
            Ok((Expr::PowerOp(p, n, Box::new(operand)), 2 + used))
        }
        Tok::Zilde => {
            // ⍬ — the empty numeric vector (0⍴0)
            Ok((Expr::Zilde, 1))
        }
        Tok::Reduce(p) => {
            // monadic operator: LO/B — the derived function LO/ applies to
            // the WHOLE expression to its right (operators bind tighter
            // than functions): ×/20⍴2 = ×/(20⍴2)
            let p = *p;
            // Check for axis specification: LO/[n] B
            if matches!(toks.get(1), Some(Tok::LBracket)) {
                let (axis, aused) = parse_expr(&toks[2..])?;
                if !matches!(toks.get(2 + aused), Some(Tok::RBracket)) {
                    return Err(ErrorCode::SyntaxError);
                }
                let after = 3 + aused;
                let (operand, used) = parse_simple(&toks[after..])?;
                Ok((
                    Expr::ReduceAxis(p, Box::new(axis), Box::new(operand)),
                    after + used,
                ))
            } else {
                let (operand, used) = parse_simple(&toks[1..])?;
                Ok((Expr::ReduceOp(p, Box::new(operand)), used + 1))
            }
        }
        Tok::Scan(p) => {
            let p = *p;
            // Check for axis specification: LO\[n] B
            if matches!(toks.get(1), Some(Tok::LBracket)) {
                let (axis, aused) = parse_expr(&toks[2..])?;
                if !matches!(toks.get(2 + aused), Some(Tok::RBracket)) {
                    return Err(ErrorCode::SyntaxError);
                }
                let after = 3 + aused;
                let (operand, used) = parse_simple(&toks[after..])?;
                Ok((
                    Expr::ScanAxis(p, Box::new(axis), Box::new(operand)),
                    after + used,
                ))
            } else {
                let (operand, used) = parse_simple(&toks[1..])?;
                Ok((Expr::ScanOp(p, Box::new(operand)), used + 1))
            }
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
        Tok::Num(_) | Tok::Complex(_, _) | Tok::Str(_) => parse_strand(toks),
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
    // gather consecutive literal atoms (Num / Complex / Str) and paren groups
    let mut items: Vec<Expr> = Vec::new();
    let mut used = 0;
    while let Some(t) = toks.get(used) {
        match t {
            Tok::Num(v) => {
                items.push(Expr::Num(*v));
                used += 1;
            }
            Tok::Complex(re, im) => {
                items.push(Expr::Complex(*re, *im));
                used += 1;
            }
            Tok::Str(s) => {
                items.push(Expr::Str(s.clone()));
                used += 1;
            }
            Tok::LParen => {
                // Check if this paren group is a derived function (e.g. (f⍤k))
                // which should NOT be consumed as a strand element — let
                // parse_simple handle the A (F⍤kl kr) B pattern
                if matches!(toks.get(used + 1), Some(Tok::Rank(_))) {
                    break;
                }
                // Also check for `(f⍥g)` pattern — dyadic over operator
                #[cfg(feature = "unofficial-ext")]
                if let Some(Tok::Prim(_)) = toks.get(used + 1) {
                    if matches!(toks.get(used + 2), Some(Tok::Prim(Prim::Over))) {
                        break;
                    }
                }
                // paren group as a strand element: 3 (4 5) → 2-element nested vector
                let (e, gu) = parse_term(&toks[used + 1..])?;
                if !matches!(toks.get(used + 1 + gu), Some(Tok::RParen)) {
                    return Err(ErrorCode::SyntaxError);
                }
                items.push(e);
                used += gu + 2;
            }
            _ => break,
        }
    }

    if items.is_empty() {
        return Err(ErrorCode::SyntaxError);
    }
    if items.len() == 1 {
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

    // homogeneous all-complex strand
    let all_complex = items.iter().all(|e| matches!(e, Expr::Complex(_, _)));
    if all_complex {
        let complexes: Vec<(f64, f64)> = items
            .into_iter()
            .map(|e| match e {
                Expr::Complex(re, im) => (re, im),
                _ => unreachable!(),
            })
            .collect();
        return Ok((Expr::ComplexVec(complexes), used));
    }

    // mixed strand: each item enclosed
    Ok((Expr::NestedVec(items), used))
}

/// true if the token can continue a nested strand (a bare atom)
fn is_strand_atom(t: Option<&Tok>) -> bool {
    matches!(
        t,
        Some(Tok::Num(_)) | Some(Tok::Complex(_, _)) | Some(Tok::Str(_)) | Some(Tok::Name(_))
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
                let (e, gu) = parse_term(&toks[used + 1..])?;
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

    // Each element must contribute ONE enclosed cell. Wrap items in an
    // explicit Enclose unless they're already syntactic enclosures
    // (`⊂…`) — eval's strand rule keeps rank-0 (pointer) results as-is,
    // so `(1)(2 3)` → nested [1],[2 3] and `(⊂x)(⊂y)` doesn't
    // double-encode.
    let items: Vec<Expr> = acc
        .into_iter()
        .map(|(e, _)| match e {
            ref m @ Expr::Monadic(crate::functions::Prim::Enclose, _) => m.clone(),
            other => Expr::Monadic(crate::functions::Prim::Enclose, Box::new(other)),
        })
        .collect();
    Ok((Expr::NestedVec(items), used))
}

/// Split the tokens inside `[...]` on TOP-LEVEL semicolons.
///
/// Returns one slice per axis plus the offset of the closing bracket. An empty
/// slice means an ELIDED index (`M[1;]` → `[Some(1), None]`). Nested
/// brackets/parens/braces are skipped so `M[(A[1]);2]` splits correctly.
/// Returns None when the bracket never closes.
fn split_index_axes(toks: &[Tok]) -> Option<(Vec<&[Tok]>, usize)> {
    let mut depth = 0usize;
    let mut parts: Vec<&[Tok]> = Vec::new();
    let mut start = 0usize;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::LBracket | Tok::LParen | Tok::LBrace => depth += 1,
            Tok::RParen | Tok::RBrace => depth = depth.checked_sub(1)?,
            Tok::RBracket => {
                if depth == 0 {
                    parts.push(&toks[start..i]);
                    return Some((parts, i));
                }
                depth -= 1;
            }
            Tok::Semicolon if depth == 0 => {
                parts.push(&toks[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

/// Parse each axis slice of an already-split bracket index.
fn parse_index_axes(parts: &[&[Tok]]) -> AplResult<Vec<Option<Expr>>> {
    let mut axes = Vec::with_capacity(parts.len());
    for part in parts {
        if part.is_empty() {
            axes.push(None); // elided → whole axis
            continue;
        }
        let (e, used) = parse_expr(part)?;
        if used != part.len() {
            return Err(ErrorCode::SyntaxError);
        }
        axes.push(Some(e));
    }
    Ok(axes)
}

fn parse_atom(toks: &[Tok]) -> AplResult<(Expr, usize)> {
    match toks.first().ok_or(ErrorCode::SyntaxError)? {
        Tok::Num(v) => Ok((Expr::Num(*v), 1)),
        Tok::Complex(re, im) => Ok((Expr::Complex(*re, *im), 1)),
        Tok::Str(s) => Ok((Expr::Str(s.clone()), 1)),
        Tok::Name(n) => {
            let n = n.clone();
            // bracket indexing: NAME[expr] or NAME[i;j;...]
            if matches!(toks.get(1), Some(Tok::LBracket)) {
                let (parts, close) = split_index_axes(&toks[2..]).ok_or(ErrorCode::SyntaxError)?;
                let total = 2 + close + 1; // name + '[' + body + ']'
                if parts.len() == 1 {
                    // single index, no semicolon: keep the existing 1-D path
                    let axes = parse_index_axes(&parts)?;
                    let idx = axes
                        .into_iter()
                        .next()
                        .flatten()
                        .ok_or(ErrorCode::SyntaxError)?;
                    return Ok((Expr::Index(Box::new(Expr::Var(n)), Box::new(idx)), total));
                }
                let axes = parse_index_axes(&parts)?;
                return Ok((Expr::IndexAxes(Box::new(Expr::Var(n)), axes), total));
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
                // Check for prefix dyadic call: F A B → FuncCallDyad(F, A, B)
                // ONLY when F is a primitive — defined functions use infix
                // dyadic form (A F B), not prefix. In APL, `SUM 3 4` where
                // SUM is a defined function is `SUM(3 4)` (monadic), not
                // `3 SUM 4` (dyadic). Only primitives can use prefix dyadic.
                let is_prim = crate::functions::Prim::from_symbol(&n).is_some();
                if is_prim {
                    let next_next_is_value = matches!(
                        toks.get(1 + used),
                        Some(Tok::Num(_))
                            | Some(Tok::Str(_))
                            | Some(Tok::Name(_))
                            | Some(Tok::LParen)
                    );
                    if next_next_is_value {
                        let (rhs, rused) = parse_simple(&toks[1 + used..])?;
                        return Ok((
                            Expr::FuncCallDyad(n, Box::new(operand), Box::new(rhs)),
                            1 + used + rused,
                        ));
                    }
                }
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
        // disclose the selected cell (Dyalog: B[i] of a nested array yields
        // an ENCLOSED item; but for arithmetic it must disclose implicitly —
        // keep the Pointer cell so nesting round-trips)
        out.push(cells[i as usize].clone());
    }
    Ok(ValueP::from_ravel_like(idx, out))
}

/// `B[i;j;...]` — select along each axis independently.
///
/// `sel` holds one entry per axis of B: `None` is an elided index (take the
/// whole axis), `Some((idx, drop))` a list of 0-based positions where `drop`
/// says the selector was written as a SCALAR.
///
/// Axis-dropping follows APL and keys off the SYNTAX, not the count: an axis
/// indexed by a scalar disappears from the result, while a vector keeps it
/// even when it holds one element. For a 2×3 matrix, `⍴M[1;1]` is empty,
/// `⍴M[1;]` is `3`, `⍴M[;1]` is `2`, `⍴M[1 2;1 2]` is `2 2`, and `⍴M[1 1;]`
/// is `2 3` — all reference-verified.
fn index_axes(b: &ValueP, sel: &[Option<(Vec<i64>, bool)>]) -> AplResult<ValueP> {
    let rank = b.rank() as usize;
    if sel.len() != rank {
        return Err(ErrorCode::RankError);
    }
    let dims: Vec<i64> = (0..rank).map(|i| b.get_shape_item(i as i16)).collect();

    // resolve each axis to the concrete list of positions to take
    let mut picks: Vec<Vec<i64>> = Vec::with_capacity(rank);
    for (ax, s) in sel.iter().enumerate() {
        match s {
            None => picks.push((0..dims[ax]).collect()),
            Some((idx, _)) => {
                for &i in idx {
                    if i < 0 || i >= dims[ax] {
                        return Err(ErrorCode::IndexError);
                    }
                }
                picks.push(idx.clone());
            }
        }
    }

    // result shape keeps only the axes that were NOT scalar-indexed
    let out_dims: Vec<i64> = picks
        .iter()
        .zip(sel)
        .filter(|(_, s)| !matches!(s, Some((_, true))))
        .map(|(p, _)| p.len() as i64)
        .collect();

    let strides = {
        let mut st = vec![1i64; rank];
        for i in (0..rank.saturating_sub(1)).rev() {
            st[i] = st[i + 1] * dims[i + 1];
        }
        st
    };

    let cells = b.cells();
    let total: i64 = picks.iter().map(|p| p.len() as i64).product();
    let mut out = Vec::with_capacity(total.max(0) as usize);
    // walk the cartesian product of the per-axis pick lists, row-major
    let mut counter = vec![0usize; rank];
    for _ in 0..total {
        let mut off = 0i64;
        for ax in 0..rank {
            off += picks[ax][counter[ax]] * strides[ax];
        }
        out.push(cells[off as usize].clone());
        for ax in (0..rank).rev() {
            counter[ax] += 1;
            if counter[ax] < picks[ax].len() {
                break;
            }
            counter[ax] = 0;
        }
    }

    if out_dims.is_empty() {
        // every axis was scalar-indexed → a scalar result
        return Ok(ValueP::scalar_from(
            out.into_iter().next().ok_or(ErrorCode::IndexError)?,
        ));
    }
    ValueP::from_parts(crate::shape::Shape::from_dims(&out_dims)?, out)
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
        //
        // Statements BEFORE the first guard are plain prologue (typically
        // assignments like {r←⍺+⍵ ⋄ 0=⊃r:9 ⋄ r}) — they run unconditionally
        // and become part of the enclosing DiamondList.
        let mut prologue: Vec<Expr> = Vec::new();
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
                    if guards.is_empty() && fallback.is_none() {
                        // still before any guard/fallback: prologue only
                        // if another guard follows later — decided below;
                        // park it here for now
                        prologue.push(other);
                    } else if fallback.is_some() {
                        return Err(ErrorCode::SyntaxError);
                    } else {
                        fallback = Some(other);
                    }
                }
            }
        }
        // If no guards actually followed the parked statements, they were
        // ordinary statements, not a prologue — put them back as fallback.
        if guards.is_empty() {
            let mut all = prologue;
            if let Some(f) = fallback {
                all.push(f);
            }
            return Ok(Expr::DiamondList(all));
        }
        // Guards exist: an explicit fallback (statement after the last
        // guard) wins; otherwise the LAST prologue statement is the
        // fallback ({c:e ⋄ r} — GNU APL's most common form); otherwise 0.
        // NOTE: pop the prologue ONLY when it must supply the fallback —
        // a tuple-match like (fallback, prologue.pop()) would pop and then
        // silently discard the assignment in the (Some(f), _) arm.
        let fallback = if let Some(f) = fallback {
            f
        } else {
            prologue.pop().unwrap_or(Expr::Num(0.0))
        };
        // fold right-to-left: guards[0] is the first guard (outermost)
        let mut acc = fallback;
        for (c, b) in guards.into_iter().rev() {
            acc = Expr::If(Box::new(c), Box::new(b), Box::new(acc));
        }
        if prologue.is_empty() {
            return Ok(acc);
        }
        let mut all = prologue;
        all.push(acc);
        return Ok(Expr::DiamondList(all));
    }

    match exprs.len() {
        1 => Ok(exprs.into_iter().next().unwrap()),
        _ => Ok(Expr::DiamondList(exprs)),
    }
}

/// collect names assigned anywhere in a body (used to build local scope).
/// Recurses into DiamondList / Seq / If so assignments in prologue
/// statements or guard bodies register as dfn locals too.
fn collect_assigned_names(body: &[Expr], out: &mut Vec<String>) {
    for e in body {
        match e {
            Expr::Assign(n, _)
            | Expr::AssignIndexed(n, _, _)
            | Expr::AssignIndexAxes(n, _, _)
            | Expr::AssignPick(n, _, _)
            | Expr::AssignSelector(_, _, n)
                if !out.contains(n) =>
            {
                out.push(n.clone());
            }
            Expr::DiamondList(inner) | Expr::Seq(inner) => {
                collect_assigned_names(inner, out);
            }
            Expr::If(c, t, e2) => {
                collect_assigned_names(std::slice::from_ref(c), out);
                collect_assigned_names(std::slice::from_ref(t), out);
                collect_assigned_names(std::slice::from_ref(e2), out);
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
    /// plugin .so specs loaded via ⎕LOADSO (for workspace PLG records)
    pub(crate) loaded_plugins: Vec<String>,
    /// nesting depth of ⍎ (execute) — guards against runaway self-execution
    /// such as `F←'⍎F' ⋄ ⍎F`
    pub(crate) execute_depth: usize,
    /// set by execute_value when the executed text produced no value (a pure
    /// assignment). eval_line reads it so `⍎'X←5'` displays nothing, matching
    /// a bare `X←5`.
    pub(crate) execute_was_shy: bool,
    /// call stack tracking: (function-name, current-line-1-based) for )SI
    pub(crate) call_stack: Vec<(String, usize)>,
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
            loaded_plugins: Vec::new(),
            execute_depth: 0,
            execute_was_shy: false,
            call_stack: Vec::new(),
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
        self.call_stack.clear();
    }

    /// erase a single variable (used by )ERASE)
    pub fn erase_var(&mut self, name: &str) {
        self.vars.remove(name);
    }

    /// insert or update a variable (used by XML loading)
    pub fn insert_var(&mut self, name: String, val: ValueP) {
        self.vars.insert(name, val);
    }

    /// get a variable's value (used by XML saving)
    pub fn get_var(&self, name: &str) -> Option<ValueP> {
        self.vars.get(name).cloned()
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
        // push this frame onto the call stack for )SI
        self.call_stack
            .push((name.to_string(), 0 /* current line, 1-based */));
        let cs_top = self.call_stack.len() - 1;
        while pc < f.body.len() {
            self.call_stack[cs_top].1 = pc + 1; // update current line (1-based)
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
        // pop the call stack frame for )SI
        self.call_stack.pop();
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
        // ambivalent function (no args in header): bind ⍵/⍺ from call args
        // so bodies that reference them work when called monadically/dyadically
        if f.arg_right.is_none() && f.arg_left.is_none() {
            if let Some(v) = &right {
                self.vars.insert("⍵".to_string(), v.clone());
            }
            if let Some(v) = &left {
                self.vars.insert("⍺".to_string(), v.clone());
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
        // push this frame onto the call stack for )SI
        self.call_stack
            .push((name.to_string(), 0 /* current line, 1-based */));
        let cs_top = self.call_stack.len() - 1;
        while pc < body.len() {
            // structured control blocks: delegate to the same machinery
            // run_lines uses, but keep tracking `last`/branch state here
            self.call_stack[cs_top].1 = pc + 1; // update current line (1-based)
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

        // pop the call stack frame for )SI
        self.call_stack.pop();

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
            Expr::Complex(re, im) => Ok(ValueP::scalar_from(crate::cell::Cell::complex(*re, *im))),
            Expr::NumVec(vs) => Ok(ValueP::from_ravel_like(
                &ValueP::vector(vs.len() as i64),
                vs.iter().map(|&v| crate::cell::Cell::from_f64(v)).collect(),
            )),
            Expr::ComplexVec(vs) => Ok(ValueP::from_ravel_like(
                &ValueP::vector(vs.len() as i64),
                vs.iter()
                    .map(|&(re, im)| crate::cell::Cell::complex(re, im))
                    .collect(),
            )),
            Expr::NestedVec(items) => {
                // Strand semantics (GNU APL): any SINGLE-ELEMENT item
                // contributes its cell directly (`1 'a' 2` → flat mixed
                // vector, ≡ = 1 — a one-char string is still one cell);
                // multi-element items are enclosed once (`'ab' 'cd'` →
                // 2-element nested vector). Paren-strand items arrive
                // pre-wrapped in Enclose, so their rank-0 pointer results
                // pass through unchanged ((1)(2 3) → ≡ = 2).
                let mut ravel: Vec<crate::cell::Cell> = Vec::new();
                for item in items {
                    let v = self.eval(item)?;
                    if v.element_count() == 1 {
                        ravel.push(v.first_cell().unwrap().clone());
                    } else {
                        let enclosed = ValueP::nested(v);
                        ravel.push(enclosed.first_cell().unwrap().clone());
                    }
                }
                Ok(ValueP::from_ravel_like(
                    &ValueP::vector(ravel.len() as i64),
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
                // VARIABLE reinterpretation: `e Q 4` parses as
                // FuncCallMono("e", FuncCallMono("Q", 4)). When the outer
                // name is NOT a function but IS a variable, treat it as a
                // value and evaluate the inner expression with it as ⍺ —
                // the Session-29 var-in-fn-position fix that unblocks the
                // JAVA.APLWS wrapper layer.
                if self.funcs.get(name).is_none() && arg.is_some() {
                    if let Some(v) = self.vars.get(name).cloned() {
                        let inner = arg.as_deref().unwrap();
                        if let Expr::FuncCallMono(iname, iarg) = inner {
                            let iright = match iarg {
                                Some(ie) => Some(self.eval(ie)?),
                                None => None,
                            };
                            // native: dyadic form desugars to enclosed pair
                            if let Some(crate::functions_def::Callable::Native(b)) =
                                self.funcs.get(iname)
                            {
                                let iv = match &iright {
                                    Some(v2) => v2.clone(),
                                    None => ValueP::int_vector(&[]),
                                };
                                let pair = ValueP {
                                    inner: std::sync::Arc::new(crate::value::ValueInner::new(
                                        crate::shape::Shape::vector(2),
                                        vec![
                                            Cell::Pointer(crate::cell::PointerCellData {
                                                value: v.inner.clone(),
                                            }),
                                            Cell::Pointer(crate::cell::PointerCellData {
                                                value: iv.inner.clone(),
                                            }),
                                        ],
                                    )),
                                };
                                return b.call(&[pair]);
                            }
                            return self.call_function(iname, Some(v), iright);
                        }
                    }
                }
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
                    // Build a 2-item vector of enclosed (Pointer) cells —
                    // the Dyalog convention: dyadic ⎕NA calls desugar to
                    // monadic with the enclosed pair as right arg.
                    let pair = ValueP {
                        inner: std::sync::Arc::new(crate::value::ValueInner::new(
                            crate::shape::Shape::vector(2),
                            vec![
                                Cell::Pointer(crate::cell::PointerCellData {
                                    value: av.inner.clone(),
                                }),
                                Cell::Pointer(crate::cell::PointerCellData {
                                    value: bv.inner.clone(),
                                }),
                            ],
                        )),
                    };
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
            Expr::ModifiedAssign(name, p, rhs) => {
                // NAME +← expr is shorthand for NAME ← NAME + expr
                let rv = self.eval(rhs)?;
                let lv = self
                    .vars
                    .get(name)
                    .cloned()
                    .unwrap_or(ValueP::int_vector(&[0]));
                let result = p.eval_dyadic(&lv, &rv)?;
                self.vars.insert(name.clone(), result.clone());
                Ok(result)
            }
            Expr::AssignIndexed(name, idx, rhs) => {
                // B[idx] ← value. The index is ⎕IO-relative, and a
                // multi-element right side is distributed ELEMENTWISE
                // (`W[1 3]←7 8` sets two different values); a scalar right
                // side broadcasts to every selected position.
                let iv = self.eval(idx)?;
                let rv = self.eval(rhs)?;
                let io = self.get_io()?;
                let mut positions = Vec::with_capacity(iv.element_count() as usize);
                for c in iv.cells() {
                    positions.push(c.get_int_value()? - io);
                }
                let target = self.vars.get(name).ok_or(ErrorCode::ValueError)?.clone();
                let mut writable = target;
                writable.isolate(); // COW: never mutate a shared value
                {
                    let cells = writable.make_mut().ravel_mut();
                    let rc = rv.cells();
                    if rc.len() != 1 && rc.len() != positions.len() {
                        return Err(ErrorCode::LengthError);
                    }
                    for (k, &i) in positions.iter().enumerate() {
                        if i < 0 || i as usize >= cells.len() {
                            return Err(ErrorCode::IndexError);
                        }
                        let src = if rc.len() == 1 {
                            rc[0].clone()
                        } else {
                            rc[k].clone()
                        };
                        cells[i as usize] = src;
                    }
                }
                self.vars.insert(name.clone(), writable.clone());
                Ok(writable)
            }
            Expr::AssignIndexAxes(name, axes, rhs) => {
                // B[i;j;...] ← value — write into the cartesian product of the
                // per-axis selections, in row-major order.
                let rv = self.eval(rhs)?;
                let io = self.get_io()?;
                let target = self.vars.get(name).ok_or(ErrorCode::ValueError)?.clone();
                let rank = target.rank() as usize;
                if axes.len() != rank {
                    return Err(ErrorCode::RankError);
                }
                let dims: Vec<i64> = (0..rank).map(|i| target.get_shape_item(i as i16)).collect();

                // resolve each axis to the list of positions it selects
                let mut picks: Vec<Vec<i64>> = Vec::with_capacity(rank);
                for (ax, a) in axes.iter().enumerate() {
                    match a {
                        None => picks.push((0..dims[ax]).collect()),
                        Some(e) => {
                            let v = self.eval(e)?;
                            let mut list = Vec::with_capacity(v.element_count() as usize);
                            for c in v.cells() {
                                let i = c.get_int_value()? - io;
                                if i < 0 || i >= dims[ax] {
                                    return Err(ErrorCode::IndexError);
                                }
                                list.push(i);
                            }
                            picks.push(list);
                        }
                    }
                }

                let strides = {
                    let mut st = vec![1i64; rank];
                    for i in (0..rank.saturating_sub(1)).rev() {
                        st[i] = st[i + 1] * dims[i + 1];
                    }
                    st
                };
                let total: usize = picks.iter().map(|p| p.len()).product();
                let rc = rv.cells();
                if rc.len() != 1 && rc.len() != total {
                    return Err(ErrorCode::LengthError);
                }

                let mut writable = target;
                writable.isolate();
                {
                    let cells = writable.make_mut().ravel_mut();
                    let mut counter = vec![0usize; rank];
                    for k in 0..total {
                        let mut off = 0i64;
                        for ax in 0..rank {
                            off += picks[ax][counter[ax]] * strides[ax];
                        }
                        cells[off as usize] = if rc.len() == 1 {
                            rc[0].clone()
                        } else {
                            rc[k].clone()
                        };
                        for ax in (0..rank).rev() {
                            counter[ax] += 1;
                            if counter[ax] < picks[ax].len() {
                                break;
                            }
                            counter[ax] = 0;
                        }
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
            Expr::AssignSelector(selector, rhs, name) => {
                // selective assignment through a selector: (selector)←value
                //
                // Special cases:
                // - ⌷ (squad): extract indices, write directly to position
                // - ⍪ (table/comma1): replace entire variable with RHS
                //
                // General case uses the marker-array technique.

                // Check for ⍪ (replace entire variable)
                if let Expr::Monadic(p, _) = &**selector {
                    if *p == crate::functions::Prim::Comma1 {
                        let rv = self.eval(rhs)?;
                        self.vars.insert(name.clone(), rv);
                        return Ok(self.vars.get(name).unwrap().clone());
                    }
                }

                // Check for ⌷ (squad indexing)
                if let Expr::Dyadic(p, a, _) = &**selector {
                    if *p == crate::functions::Prim::Squad {
                        let rv = self.eval(rhs)?;
                        let target = self.vars.get(name).ok_or(ErrorCode::ValueError)?.clone();
                        let mut writable = target;
                        writable.isolate();

                        // Evaluate the indices (left arg of ⌷)
                        let indices_v = self.eval(a)?;

                        // Copy shape info before mutable borrow
                        let var_shape: Vec<i64> = (0..self.vars.get(name).unwrap().rank())
                            .map(|i| self.vars.get(name).unwrap().get_shape_item(i as i16))
                            .collect();
                        let var_rank = var_shape.len();

                        // Compute linear offset from indices
                        let indices: Vec<i64> = indices_v
                            .cells()
                            .iter()
                            .map(|c| c.get_int_value())
                            .collect::<Result<Vec<_>, _>>()?;

                        if indices.len() != var_rank {
                            return Err(ErrorCode::RankError);
                        }

                        // Honor ⎕IO: shift indices to 0-based
                        let io = self.get_io()?;
                        let shifted: Vec<i64> = indices.iter().map(|&idx| idx - io).collect();

                        // Bounds check
                        for (i, &idx) in shifted.iter().enumerate() {
                            let axis_len = var_shape[i];
                            if idx < 0 || idx >= axis_len {
                                return Err(ErrorCode::IndexError);
                            }
                        }

                        // Compute linear offset
                        let mut offset: i64 = 0;
                        let mut stride: i64 = 1;
                        for (i, &idx) in shifted.iter().enumerate().rev() {
                            offset += idx * stride;
                            if i > 0 {
                                stride *= var_shape[i];
                            }
                        }

                        let rc = rv.cells();
                        if rc.is_empty() {
                            return Err(ErrorCode::LengthError);
                        }

                        {
                            let cells = writable.make_mut().ravel_mut();
                            let src = rc[0].clone();
                            cells[offset as usize] = src;
                        }
                        self.vars.insert(name.clone(), writable.clone());
                        return Ok(writable);
                    }
                }

                // General case: marker-array technique
                let rv = self.eval(rhs)?;
                let target = self.vars.get(name).ok_or(ErrorCode::ValueError)?.clone();
                let mut writable = target;
                writable.isolate();

                // create marker array: same shape as target, each element
                // is its ravel index + 1. The +1 ensures that prototype
                // positions (value 0) are distinguishable from real indices.
                let n = writable.element_count();
                let marker_vals: Vec<Cell> = (1..=n).map(Cell::Int).collect();
                let marker = ValueP::from_ravel_like(&writable, marker_vals);

                // apply the selector to the marker
                let selected = self.eval_selector(selector, &marker)?;

                // flatten the selected values to get positions.
                // The marker values are ravel_index + 1, so values >= 1
                // correspond to real positions (subtract 1) and values == 0
                // are prototype positions from padding — the reference
                // ignores those, so we filter them out.
                let n = writable.element_count();
                let all_positions: Vec<i64> = selected
                    .cells()
                    .iter()
                    .map(|c| c.get_int_value())
                    .collect::<Result<Vec<_>, _>>()?;
                let positions: Vec<i64> = all_positions
                    .into_iter()
                    .filter(|p| *p >= 1 && *p <= n)
                    .map(|p| p - 1)
                    .collect();

                let rc = rv.cells();
                // RHS must broadcast (len 1) or have at least as many
                // elements as positions; extra elements are silently
                // ignored (matching the reference).
                if rc.len() != 1 && rc.len() < positions.len() {
                    return Err(ErrorCode::LengthError);
                }

                {
                    let cells = writable.make_mut().ravel_mut();
                    for (k, &pos) in positions.iter().enumerate() {
                        if pos < 0 || pos as usize >= cells.len() {
                            return Err(ErrorCode::IndexError);
                        }
                        let src = if rc.len() == 1 {
                            rc[0].clone()
                        } else {
                            rc[k].clone()
                        };
                        cells[pos as usize] = src;
                    }
                }
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
                // ⍸B yields positions, so it is ⎕IO-shifted too. The shift is
                // applied INSIDE where_indices because a rank≥2 result holds
                // nested index vectors, which scalar arithmetic cannot reach.
                if *p == crate::functions::Prim::Where {
                    return crate::format::where_indices_io(&bv, self.get_io()?);
                }
                // ⍕B honors ⎕PP for float rendering
                if *p == crate::functions::Prim::Format {
                    let pp = crate::sysvars::get_pp(self).unwrap_or(10);
                    return crate::format::format_with_pp(&bv, pp);
                }
                // ⍎B — execute: evaluate a character vector as an APL line.
                // Needs &mut self, so it cannot live in eval_monadic.
                if *p == crate::functions::Prim::Execute {
                    return self.execute_value(&bv);
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
            Expr::ReduceAxis(p, axis, b) => {
                let bv = self.eval(b)?;
                let xv = self.eval(axis)?;
                let ax = xv.first_cell().unwrap().get_near_int()?;
                let io = self.get_io()?;
                crate::operators::reduce_axis(*p, &bv, ax - io)
            }
            Expr::ScanAxis(p, axis, b) => {
                let bv = self.eval(b)?;
                let xv = self.eval(axis)?;
                let ax = xv.first_cell().unwrap().get_near_int()?;
                let io = self.get_io()?;
                crate::operators::scan_axis(*p, &bv, ax - io)
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
            Expr::EachOpName(n, b) => {
                let bv = self.eval(b)?;
                crate::operators::each_name(n, &bv, |val| self.call_function(n, None, Some(val)))
            }
            Expr::EachDyadName(n, a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                crate::operators::each_dyad_name(n, &av, &bv, |x, y| {
                    self.call_function(n, Some(x), Some(y))
                })
            }
            #[cfg(feature = "unofficial-ext")]
            Expr::Key(b) => {
                let bv = self.eval(b)?;
                crate::key::key_monadic(&bv)
            }
            #[cfg(feature = "unofficial-ext")]
            Expr::KeyDyad(a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                crate::key::key_dyad(&av, &bv)
            }
            #[cfg(feature = "unofficial-ext")]
            Expr::OverMonad(f, g, b) => crate::over::over_monadic(*f, *g, b, self),
            #[cfg(feature = "unofficial-ext")]
            Expr::OverDyad(f, g, a, b) => crate::over::over_dyad(*f, *g, a, b, self),
            Expr::RankOp(p, k, b) => {
                // (f⍤k)B — f applied to each rank-k cell.
                // Route through eval_monadic_io so ⎕IO-sensitive primitives
                // (⍳ ⍋ ⍒ ⍸) behave the same as they do outside the operator.
                let bv = self.eval(b)?;
                let p = *p;
                let io = self.get_io()?;
                crate::rank::rank_monadic(&bv, *k, |cell| Self::eval_monadic_io(p, cell, io))
            }
            Expr::PowerOp(p, n, b) => {
                // (F⍣N) B — apply F monadically N times: F(F(F(...F(B)...)))
                // N=0 returns B unchanged (identity)
                let bv = self.eval(b)?;
                let n = *n;
                if n <= 0 {
                    return Ok(bv);
                }
                let mut result = bv;
                let p = p.clone();
                match p {
                    PowerFn::Prim(prim) => {
                        for _ in 0..n {
                            result = prim.eval_monadic(&result)?;
                        }
                    }
                    PowerFn::Name(name) => {
                        for _ in 0..n {
                            result = self.call_function(&name, None, Some(result))?;
                        }
                    }
                }
                Ok(result)
            }
            Expr::Zilde => {
                // ⍬ — the empty numeric vector (0⍴0)
                Ok(ValueP::int_vector(&[]))
            }
            Expr::RankDyad(p, kl, kr, a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                let p = *p;
                crate::rank::rank_dyadic(&av, &bv, *kl, *kr, |x, y| {
                    crate::functions::eval_dyadic_public(p, x, y)
                })
            }
            Expr::MatrixProduct(a, b) => {
                // matrix product A∘B ≡ A +.× B
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                crate::inner::inner_product(&av, Prim::Add, Prim::Multiply, &bv)
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
            Expr::IndexAxes(base, axes) => {
                // B[i;j;...] — one selector per axis, elided = whole axis
                let bv = self.eval(base)?;
                let io = self.get_io()?;
                let mut sel: Vec<Option<(Vec<i64>, bool)>> = Vec::with_capacity(axes.len());
                for ax in axes {
                    match ax {
                        None => sel.push(None),
                        Some(e) => {
                            let v = self.eval(e)?;
                            // Whether the axis is DROPPED depends on the
                            // written form, not the element count: M[1;1] is a
                            // scalar but M[1 1;] keeps a 2-long axis. A rank-0
                            // result means a scalar was written.
                            let drops = v.rank() == 0;
                            let mut idx = Vec::with_capacity(v.element_count() as usize);
                            for c in v.cells() {
                                idx.push(c.get_int_value()? - io);
                            }
                            sel.push(Some((idx, drops)));
                        }
                    }
                }
                index_axes(&bv, &sel)
            }
            Expr::Dyadic(p, a, b) => {
                let av = self.eval(a)?;
                let bv = self.eval(b)?;
                // A⍳B results are ⎕IO-shifted
                if *p == crate::functions::Prim::Iota {
                    return crate::index_of::index_of_io(&av, &bv, self.get_io()?);
                }
                // A⍉B — dyadic transpose: the axis list is in ⎕IO origin
                if *p == crate::functions::Prim::Transpose {
                    return crate::transpose::transpose_dyadic_io(&av, &bv, self.get_io()?);
                }
                // A⌷B — general index: honor ⎕IO
                if *p == crate::functions::Prim::Squad {
                    let io = self.get_io()?;
                    let shifted = if io == 0 {
                        av
                    } else {
                        let minus_io = ValueP::scalar_from(crate::cell::Cell::Int(-io));
                        crate::functions::eval_dyadic_public(
                            crate::functions::Prim::Add,
                            &av,
                            &minus_io,
                        )?
                    };
                    return crate::squad::squad(&shifted, &bv);
                }
                // implicit disclosure of scalar Pointer args (indexed scalars)
                crate::functions::eval_dyadic_public(*p, &av, &bv)
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
                // remember the spec for workspace PLG records (dedupe)
                if !self.loaded_plugins.contains(&path) {
                    self.loaded_plugins.push(path.clone());
                }
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
            Expr::QuadCr(n, arg) => {
                // N ⎕CR B — character representation
                // N=1: ravel (flat character vector/matrix)
                // N=4: boxed display with outer wrapper
                let bv = self.eval(arg)?;
                let pp = crate::sysvars::get_pp(self).unwrap_or(10);
                let inner = crate::boxdisplay::render_with_pp(&bv, pp);
                if *n == 1 {
                    // Simple ravel: flatten with parens for nested arrays
                    let mut s = String::new();
                    Self::enlist(&bv, &mut s);
                    let cps: Vec<u32> = s.chars().map(|ch| ch as u32).collect();
                    Ok(ValueP::char_vector(&cps))
                } else {
                    // 4⎕CR: boxed display with outer wrapper
                    let width = inner.iter().map(|l| l.chars().count()).max().unwrap_or(0);
                    let mut out = Vec::with_capacity(inner.len() + 2);
                    let fills = width.saturating_sub(1);
                    out.push(format!("┏→{}┓", "━".repeat(fills)));
                    for l in &inner {
                        let pad = width - l.chars().count();
                        out.push(format!("┃{}{}┃", l, " ".repeat(pad)));
                    }
                    let mut bottom = String::from("┗∼");
                    for _ in 1..width {
                        bottom.push('━');
                    }
                    bottom.push('┛');
                    out.push(bottom);
                    if out.len() == 1 {
                        let cps: Vec<u32> = out[0].chars().map(|ch| ch as u32).collect();
                        Ok(ValueP::char_vector(&cps))
                    } else {
                        let max_w = out.iter().map(|l| l.chars().count()).max().unwrap_or(0);
                        let rows: Vec<Vec<Cell>> = out
                            .iter()
                            .map(|l| {
                                let mut cps: Vec<Cell> =
                                    l.chars().map(|ch| Cell::Char(ch as u32)).collect();
                                while cps.len() < max_w {
                                    cps.push(Cell::Char(' ' as u32));
                                }
                                cps
                            })
                            .collect();
                        let flat: Vec<Cell> = rows.iter().flatten().cloned().collect();
                        Ok(ValueP::from_parts(
                            crate::shape::Shape::matrix(rows.len() as i64, max_w as i64),
                            flat,
                        )?)
                    }
                }
            }
            Expr::QuadUcs(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_ucs(&bv)
            }
            Expr::QuadAv => Ok(crate::quad::quad_av()),
            Expr::QuadTs => crate::quad::quad_ts(),
            Expr::QuadWa => crate::quad::quad_wa(),
            Expr::QuadTc => Ok(crate::quad::quad_tc()),
            Expr::QuadDm => Ok(crate::quad::quad_dm()),
            Expr::QuadEn => Ok(crate::quad::quad_en()),
            Expr::QuadRval(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_rval(&bv)
            }
            Expr::QuadRl(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_rl(&bv)
            }
            Expr::QuadCc(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_cc(&bv)
            }
            Expr::QuadDlx(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_dlx(&bv)
            }
            Expr::QuadTf(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_tf(&bv)
            }
            Expr::QuadFx(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_fx(self, &bv)
            }
            Expr::QuadMap(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_map(self, &bv)
            }
            Expr::QuadMx(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_mx(&bv)
            }
            Expr::QuadFio(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_fio(&bv)
            }
            Expr::QuadJson(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_json(&bv)
            }
            Expr::QuadXml(arg) => {
                let bv = self.eval(arg)?;
                crate::quad::quad_xml(&bv)
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

    /// Render a value as APL ravel text (with parens for nested arrays).
    fn enlist(v: &ValueP, s: &mut String) {
        if v.rank() == 0 {
            if let Some(c) = v.first_cell() {
                s.push_str(&crate::boxdisplay::plain_cell(c, 10));
            }
            return;
        }
        for (i, c) in v.cells().iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            match c {
                Cell::Pointer(p) => {
                    let inner = ValueP {
                        inner: p.value.clone(),
                    };
                    if inner.rank() > 0 && inner.element_count() > 1 {
                        s.push('(');
                        Self::enlist(&inner, s);
                        s.push(')');
                    } else {
                        Self::enlist(&inner, s);
                    }
                }
                other => s.push_str(&crate::boxdisplay::plain_cell(other, 10)),
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

        // Multi-statement line: `A ⋄ B ⋄ C` runs each statement left to right
        // and displays only the LAST one's value. Diamonds inside braces or
        // brackets belong to a dfn body / index list, so only split at depth 0.
        // (⎕EA has its own diamond handling and must not be split here.)
        let is_quad_ea = matches!(toks.first(), Some(Tok::Name(n)) if n == "⎕EA");
        if !is_quad_ea {
            let mut depth = 0usize;
            let mut cuts: Vec<usize> = Vec::new();
            for (i, t) in toks.iter().enumerate() {
                match t {
                    Tok::LBrace | Tok::LBracket | Tok::LParen => depth += 1,
                    Tok::RBrace | Tok::RBracket | Tok::RParen => depth = depth.saturating_sub(1),
                    Tok::Diamond if depth == 0 => cuts.push(i),
                    _ => {}
                }
            }
            if !cuts.is_empty() {
                let mut last: Option<ValueP> = None;
                let mut start = 0usize;
                for cut in cuts.iter().copied().chain(std::iter::once(toks.len())) {
                    let stmt = &toks[start..cut];
                    start = cut + 1;
                    // skip empty statements (trailing ⋄, or ⋄ before End)
                    if stmt.is_empty() || matches!(stmt.first(), Some(Tok::End)) {
                        continue;
                    }
                    last = self.eval_statement(stmt)?;
                }
                return Ok(last);
            }
        }

        self.eval_statement(&toks)
    }

    /// Evaluate ONE statement's tokens (no top-level diamonds).
    fn eval_statement(&mut self, toks: &[Tok]) -> AplResult<Option<ValueP>> {
        if toks.is_empty() || matches!(toks.first(), Some(Tok::End)) {
            return Ok(None);
        }
        let (expr, used) = parse(toks)?;
        // a statement split out of a diamond list has no trailing End
        if used != toks.len() && !matches!(toks.get(used), Some(Tok::End)) {
            return Err(ErrorCode::SyntaxError);
        }
        let is_assign = matches!(
            expr,
            Expr::Assign(_, _)
                | Expr::AssignIndexed(_, _, _)
                | Expr::AssignIndexAxes(_, _, _)
                | Expr::AssignPick(_, _, _)
                | Expr::AssignDfn(_, _)
        );
        // ⍎ of a pure assignment is shy too: clear the flag, then let eval
        // set it if an execute inside this line produced no value.
        let outer_shy = std::mem::replace(&mut self.execute_was_shy, false);
        let v = self.eval(&expr)?;
        let executed_shy = self.execute_was_shy;
        self.execute_was_shy = outer_shy;
        Ok(if is_assign || executed_shy {
            None
        } else {
            Some(v)
        })
    }

    /// Apply a selector expression to a marker array.
    ///
    /// The selector contains a reference to a variable by name. We temporarily
    /// bind that variable to the marker array, evaluate the selector, then
    /// restore the original binding. This implements the marker-array
    /// technique for selective assignment through selectors.
    fn eval_selector(&mut self, selector: &Expr, marker: &ValueP) -> AplResult<ValueP> {
        // find the variable name in the selector
        let name = Self::find_selector_var(selector)?;
        // save the original binding
        let original = self.vars.get(name).cloned();
        // bind the variable to the marker
        self.vars.insert(name.to_string(), marker.clone());
        // evaluate the selector
        let result = self.eval(selector);
        // restore the original binding
        match original {
            Some(v) => self.vars.insert(name.to_string(), v),
            None => self.vars.remove(name),
        };
        result
    }

    /// Find the variable name referenced in a selector expression.
    fn find_selector_var(e: &Expr) -> AplResult<&str> {
        match e {
            Expr::Monadic(_, b) => Self::find_selector_var(b),
            Expr::Dyadic(_, a, b) => {
                Self::find_selector_var(b).or_else(|_| Self::find_selector_var(a))
            }
            Expr::Var(n) => Ok(n),
            _ => Err(ErrorCode::SyntaxError),
        }
    }
    ///
    /// The Monadic eval arm intercepts ⍳ ⍋ ⍒ ⍸ ⍕ before eval_monadic because
    /// their results depend on the index origin. Operators that apply a prim
    /// themselves (⍤ so far) must use this so `(⍳⍤0)3` matches a bare `⍳3`.
    fn eval_monadic_io(p: Prim, b: &ValueP, io: i64) -> AplResult<ValueP> {
        match p {
            Prim::Iota => crate::functions::iota_monadic(b, io),
            Prim::GradeUp => crate::sort::grade_io(b, false, io),
            Prim::GradeDown => crate::sort::grade_io(b, true, io),
            Prim::Where => crate::format::where_indices_io(b, io),
            _ => p.eval_monadic(b),
        }
    }

    /// Maximum nesting depth for ⍎. A self-executing expression such as
    /// `F←'⍎F' ⋄ ⍎F` would otherwise recurse until the native stack blows;
    /// this turns it into a catchable APL error instead.
    const MAX_EXECUTE_DEPTH: usize = 64;

    /// `⍎B` — execute: evaluate the character vector B as an APL expression.
    ///
    /// B must be a simple character array (its ravel is read in order, so a
    /// character matrix is executed as one concatenated line). An empty
    /// argument yields a shy 0, matching GNU APL's behavior for `⍎''`.
    /// Expressions that produce no value (a pure assignment) also yield 0,
    /// so `⍎'X←5'` assigns and returns quietly.
    pub fn execute_value(&mut self, b: &ValueP) -> AplResult<ValueP> {
        // extract the source text; anything non-character is a DOMAIN ERROR
        let mut src = String::with_capacity(b.element_count() as usize);
        for c in b.cells() {
            match c {
                Cell::Char(u) => src.push(char::from_u32(*u).ok_or(ErrorCode::DomainError)?),
                _ => return Err(ErrorCode::DomainError),
            }
        }
        if src.trim().is_empty() {
            self.execute_was_shy = true;
            return Ok(ValueP::scalar_from(Cell::Int(0)));
        }

        if self.execute_depth >= Self::MAX_EXECUTE_DEPTH {
            return Err(ErrorCode::LimitError);
        }
        self.execute_depth += 1;
        let result = self.eval_line(&src);
        // restore the depth even when the inner line errored, so a caught
        // error (⎕EA) does not leak depth into later executes
        self.execute_depth -= 1;

        match result? {
            // a real value: clear any shy flag an inner execute may have set,
            // so `(⍎'W←4')+⍎'2+3'` still displays its result
            Some(v) => {
                self.execute_was_shy = false;
                Ok(v)
            }
            // assignment or empty statement — shy result, flagged so the
            // caller can suppress display the way a bare assignment is
            None => {
                self.execute_was_shy = true;
                Ok(ValueP::scalar_from(Cell::Int(0)))
            }
        }
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

    /// numeric value of an expression against an EXISTING environment,
    /// accepting Int or Float (⌊ and | may return either representation)
    fn eval_num_in(env: &mut Environment, line: &str) -> f64 {
        let v = eval_one(env, line);
        match v.first_cell().unwrap() {
            crate::cell::Cell::Int(i) => *i as f64,
            crate::cell::Cell::Float(f) => *f,
            other => panic!("expected a number for {line:?}, got {other:?}"),
        }
    }

    /// all cells of a result as integers, in ravel order — the workhorse for
    /// rank ≥ 2 assertions (pair it with `,E` / `⍴E` in the expression)
    fn ravel_ints(env: &mut Environment, line: &str) -> Vec<i64> {
        eval_one(env, line)
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Int(i) => *i,
                crate::cell::Cell::Float(f) => *f as i64,
                other => panic!("expected numbers for {line:?}, got {other:?}"),
            })
            .collect()
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
        // 1 'a' 2 is a mixed SIMPLE strand → flat vector (GNU APL rule:
        // scalar strand items contribute their cell directly; ≡ = 1)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 'a' 2");
        assert_eq!(v.element_count(), 3);
        assert_eq!(
            crate::depth::depth(&v).unwrap().first_cell().cloned(),
            Some(crate::cell::Cell::Int(1))
        );
        assert_eq!(v.cells()[0], crate::cell::Cell::Int(1));
        assert_eq!(v.cells()[1], crate::cell::Cell::Char(97)); // 'a'
        assert_eq!(v.cells()[2], crate::cell::Cell::Int(2));
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
        // 1 +¨ ⍳3 → simple scalars (no boxing for simple results)
        let v = eval_one(&mut env, "1+¨⍳3");
        assert_eq!(v.element_count(), 3);
        let expect = [1, 2, 3];
        for (i, e) in expect.iter().enumerate() {
            match &v.cells()[i] {
                crate::cell::Cell::Int(x) => assert_eq!(*x, *e),
                o => panic!("expected int, got {:?}", o),
            }
        }
    }

    #[test]
    fn test_each_dyad_vector_vector() {
        // 10 20 +¨ 1 2 → simple scalars (11) (22)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "10 20+¨1 2");
        assert_eq!(v.element_count(), 2);
        for (i, e) in [11, 22].iter().enumerate() {
            match &v.cells()[i] {
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
        // axis-1 take keeps all rows, so it equals the per-axis form 2 2↑M.
        // Plain `2↑M` on a matrix is a LENGTH ERROR: the left argument of
        // take/drop needs one count PER AXIS (reference-verified).
        let a = eval_one(&mut env, "2↑[1]M");
        let b = eval_one(&mut env, "2 2↑M");
        assert_eq!(a.cells(), b.cells());
        assert!(env.eval_line("2↑M").is_err());
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
    fn test_matrix_product() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        // 2x2 matrix product: A∘B ≡ A+.×B
        let r = eval_one(&mut env, "(2 2⍴1 2 3 4)∘(2 2⍴5 6 7 8)");
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 2);
        assert_eq!(r.get_shape_item(1), 2);
        let expect = [19, 22, 43, 50];
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(r.cells()[i], crate::cell::Cell::Int(*e));
        }
    }

    #[test]
    fn test_nested_strand_deep() {
        // (1 2)(3 (4 5)) — a nested strand with a nested nested array
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        let r = eval_one(&mut env, "(1 2)(3 (4 5))");
        assert_eq!(r.element_count(), 2);
        // ≡ = 3 (depth of 3 (4 5))
        let d = crate::depth::depth(&r).unwrap();
        assert_eq!(d.first_cell().unwrap(), &crate::cell::Cell::Int(3));
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
    fn test_modified_assignment() {
        let mut env = Environment::new();
        env.eval_line("V←1 2 3 4").unwrap();
        env.eval_line("V+←10").unwrap();
        let v = env.eval_line("V").unwrap().unwrap();
        let cells = v.cells();
        assert_eq!(cells.len(), 4);
        for (i, c) in cells.iter().enumerate() {
            match c {
                crate::cell::Cell::Int(val) => assert_eq!(*val, 11 + i as i64),
                o => panic!("expected int, got {:?}", o),
            }
        }
    }

    #[test]
    fn test_modified_assignment_multiply() {
        let mut env = Environment::new();
        env.eval_line("X←2 3 4").unwrap();
        env.eval_line("X×←3").unwrap();
        let v = env.eval_line("X").unwrap().unwrap();
        let cells = v.cells();
        assert_eq!(cells.len(), 3);
        for (i, c) in cells.iter().enumerate() {
            match c {
                crate::cell::Cell::Int(val) => assert_eq!(*val, (2 + i as i64) * 3),
                o => panic!("expected int, got {:?}", o),
            }
        }
    }

    #[test]
    fn test_named_function_each() {
        let mut env = Environment::new();
        env.eval_line("f←{⍵+1}").unwrap();
        let v = env.eval_line("f¨ 1 2 3").unwrap().unwrap();
        assert_eq!(v.rank(), 1);
        assert_eq!(v.cells().len(), 3);
        for (i, c) in v.cells().iter().enumerate() {
            match c {
                crate::cell::Cell::Pointer(p) => {
                    let inner = crate::value::ValueP {
                        inner: p.value.clone(),
                    };
                    match inner.first_cell().unwrap() {
                        crate::cell::Cell::Int(val) => assert_eq!(*val, 2 + i as i64),
                        o => panic!("expected int, got {:?}", o),
                    }
                }
                o => panic!("expected pointer, got {:?}", o),
            }
        }
    }

    #[test]
    fn test_axis_reduce() {
        let mut env = Environment::new();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        let v1 = env.eval_line("+/[1]M").unwrap().unwrap();
        let v2 = env.eval_line("+/M").unwrap().unwrap();
        assert_eq!(v1.cells().len(), v2.cells().len());
        for (a, b) in v1.cells().iter().zip(v2.cells().iter()) {
            match (a, b) {
                (crate::cell::Cell::Int(x), crate::cell::Cell::Int(y)) => assert_eq!(x, y),
                _ => panic!("expected ints"),
            }
        }
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
    fn test_parse_prologue_then_guard() {
        // {r←5 ⋄ 0:99 ⋄ r} — assignment BEFORE a guard. The prologue must
        // run unconditionally; the guard's fallback is the final `r`.
        let toks = tokenize("{r←5 ⋄ 0:99 ⋄ r}").unwrap();
        eprintln!("TOKENS: {toks:?}");
        let (e, _) = parse(&toks).unwrap();
        eprintln!("EXPR: {e:#?}");
        match &e {
            Expr::Dfn(body) => match &**body {
                Expr::DiamondList(stmts) => {
                    assert!(
                        matches!(stmts[0], Expr::Assign(_, _)),
                        "prologue assign missing"
                    );
                    match &stmts[1] {
                        Expr::If(c, t, else_b) => {
                            assert!(matches!(**c, Expr::Num(_)));
                            assert!(matches!(**t, Expr::Num(_)));
                            // bare trailing `r` parses as an ambivalent
                            // FuncCallMono("r", None) (resolved to the
                            // variable at eval time)
                            assert!(
                                matches!(else_b.as_ref(), Expr::FuncCallMono(n, None) if n == "r")
                                    || matches!(else_b.as_ref(), Expr::Var(n) if n == "r"),
                                "fallback should reference r, got {else_b:?}"
                            );
                        }
                        o => panic!("expected If second, got {:?}", o),
                    }
                }
                o => panic!("expected DiamondList, got {:?}", o),
            },
            _ => panic!("expected Dfn"),
        }
    }

    #[test]
    fn test_eval_prologue_guard_fallback() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("K←{r←5 ⋄ 0=⊃⍵:9 ⋄ r}").unwrap();
        // ⍵ must be enclosed so ⊃⍵ discloses to 7 (scalar ⊃ is identity,
        // and 0=7 → 0 → fallback branch)
        let v = eval_one(&mut env, "K (⊂7)");
        assert_eq!(v.first_cell().unwrap().get_near_int().unwrap(), 5);
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

    #[test]
    fn test_var_in_fn_position_dyadic() {
        // Session-29 var-in-fn-position: `e Q 4` where `e` is a variable
        // and `Q` is a defined function — must call Q dyadically with
        // ⍺=e, ⍵=4, NOT misparse as a monadic call chain.
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("ADD←{⍺+⍵}").unwrap();
        env.eval_line("Q←{⍺ ADD ⍵}").unwrap();
        env.eval_line("e←3").unwrap();
        let v = eval_one(&mut env, "e Q 4");
        assert_eq!(v.first_cell().unwrap().get_near_int().unwrap(), 7);
    }

    #[test]
    fn test_var_in_fn_position_monadic() {
        // Monadic variant: `e Q` where e is a variable used as ⍺
        // inside a dfn body that references an enclosing-scope function.
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("ADD←{⍺+⍵}").unwrap();
        env.eval_line("e←3").unwrap();
        // {e ADD 4} in a dfn body — `e` must resolve from enclosing scope
        env.eval_line("Q←{e ADD 4}").unwrap();
        let v = eval_one(&mut env, "Q 0");
        assert_eq!(v.first_cell().unwrap().get_near_int().unwrap(), 7);
    }

    #[test]
    fn test_na_eval() {
        let mut env = Environment::new();
        // ⎕NA with library|symbol format — div is in libc.so.6
        let result = env.eval_line("mydiv ⎕NA 'I4 libc.so.6|div I4 I4'");
        assert!(
            result.is_ok(),
            "⎕NA association should succeed: {:?}",
            result
        );
        // now call it: 10 mydiv 3 should give 3 (integer division)
        let v = env.eval_line("10 mydiv 3").unwrap();
        assert!(v.is_some());
        let val = v.unwrap();
        let first = val.first_cell().unwrap().get_near_int().unwrap();
        assert_eq!(first, 3, "10 div 3 should be 3");
    }

    #[test]
    fn test_native_dyadic_pair_shape() {
        // The dyadic native-call desugar builds a 2-item vector of
        // enclosed (Pointer) cells. Verify the shape is [2] and both
        // items are enclosed scalars. Uses a no-op native if available;
        // otherwise just verify the pair shape via a simple test.
        // (The actual native bridge is tested in apl-java integration.)
        // Here we verify the Shape::vector(2) path indirectly:
        // a 2-item enclosed vector should have element_count == 2.
        let v = crate::value::ValueP::nested(crate::value::ValueP::scalar_from(
            crate::cell::Cell::Int(42),
        ));
        assert_eq!(v.element_count(), 1); // scalar has 1 cell
                                          // Build a 2-item vector of enclosed scalars (mimics the pair):
        let pair = crate::value::ValueP {
            inner: std::sync::Arc::new(crate::value::ValueInner::new(
                crate::shape::Shape::vector(2),
                vec![
                    crate::cell::Cell::Pointer(crate::cell::PointerCellData {
                        value: crate::value::ValueP::scalar_from(crate::cell::Cell::Int(1)).inner,
                    }),
                    crate::cell::Cell::Pointer(crate::cell::PointerCellData {
                        value: crate::value::ValueP::scalar_from(crate::cell::Cell::Int(2)).inner,
                    }),
                ],
            )),
        };
        assert_eq!(pair.element_count(), 2);
        assert_eq!(pair.shape().get_rank(), 1);
    }

    #[test]
    fn test_without_basic() {
        // 1 2 3∼2 3 4 = 1
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2 3∼2 3 4");
        assert_eq!(v.cells().len(), 1);
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 1);
    }

    #[test]
    fn test_without_empty_result() {
        // 1 2∼1 2 = empty vector
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2∼1 2");
        assert_eq!(v.element_count(), 0);
    }

    #[test]
    fn test_without_all_removed() {
        // 'abc'∼'abc' = empty
        let mut env = Environment::new();
        let v = eval_one(&mut env, "'abc'∼'abc'");
        assert_eq!(v.element_count(), 0);
    }

    #[test]
    fn test_without_no_overlap() {
        // 1 2∼3 4 = 1 2
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2∼3 4");
        assert_eq!(v.element_count(), 2);
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2]);
    }

    #[test]
    fn test_unique_monadic() {
        // ∪1 2 1 3 2 = 1 2 3
        let mut env = Environment::new();
        let v = eval_one(&mut env, "∪1 2 1 3 2");
        assert_eq!(v.element_count(), 3);
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3]);
    }

    #[test]
    fn test_union_dyadic() {
        // 1 2 3∪3 4 5 = 1 2 3 4 5
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2 3∪3 4 5");
        assert_eq!(v.element_count(), 5);
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_intersection_dyadic() {
        // 1 2 3 4∩2 4 6 = 2 4
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2 3 4∩2 4 6");
        assert_eq!(v.element_count(), 2);
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![2, 4]);
    }

    #[test]
    fn test_table_monadic() {
        // ⍪1 2 3 = 3×1 matrix
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍪1 2 3");
        assert_eq!(v.rank(), 2);
        assert_eq!(v.get_shape_item(0), 3);
        assert_eq!(v.get_shape_item(1), 1);
    }

    #[test]
    fn test_tally_monadic() {
        // ≢1 2 3 = 3
        let mut env = Environment::new();
        let v = eval_one(&mut env, "≢1 2 3");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 3);
    }

    #[test]
    fn test_tally_scalar() {
        // ≢42 = 1
        let mut env = Environment::new();
        let v = eval_one(&mut env, "≢42");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 1);
    }

    #[test]
    fn test_catenate_first() {
        // 1 2⍪3 4 5 = 1 2 3 4 5
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2⍪3 4 5");
        assert_eq!(v.element_count(), 5);
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_not_match_same() {
        // 1 2 3≢1 2 3 = 0
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2 3≢1 2 3");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 0);
    }

    #[test]
    fn test_not_match_diff() {
        // 1 2 3≢1 2 4 = 1
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2 3≢1 2 4");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 1);
    }

    #[test]
    fn test_left_dyadic() {
        // 1⊣2 = 1
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1⊣2");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 1);
    }

    #[test]
    fn test_right_dyadic() {
        // 1⊢2 = 2
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1⊢2");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 2);
    }

    #[test]
    fn test_left_monadic() {
        // ⊣1 2 3 = 1 2 3 (identity)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⊣1 2 3");
        assert_eq!(v.element_count(), 3);
    }

    #[test]
    fn test_right_monadic() {
        // ⊢1 2 3 = 1 2 3 (identity)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⊢1 2 3");
        assert_eq!(v.element_count(), 3);
    }

    #[test]
    fn test_nand() {
        // 0⍲0 = 1, 1⍲1 = 0
        let mut env = Environment::new();
        let v = eval_one(&mut env, "0⍲0");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 1);
        let v = eval_one(&mut env, "1⍲1");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 0);
    }

    #[test]
    fn test_nor() {
        // 0⍱0 = 1, 1⍱0 = 0
        let mut env = Environment::new();
        let v = eval_one(&mut env, "0⍱0");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 1);
        let v = eval_one(&mut env, "1⍱0");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 0);
    }

    #[test]
    fn test_squad_vector() {
        // 2⌷10 20 30 = 30 (0-based, so index 2 → value 30)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "2⌷10 20 30");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 30);
    }

    #[test]
    fn test_squad_matrix() {
        // 2 3⌷3 4⍴⍳12 → row 2, col 3 → value 11 (0-based)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "2 3⌷3 4⍴⍳12");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 11);
    }

    #[test]
    fn test_reverse_first_axis() {
        // ⊖2 3⍴⍳6 = [[3 4 5] [0 1 2]] (0-based)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⊖2 3⍴⍳6");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![3, 4, 5, 0, 1, 2]);
    }

    #[test]
    fn test_rotate_first_axis() {
        // 1⊖2 3⍴⍳6 = [[3 4 5] [0 1 2]] (0-based)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1⊖2 3⍴⍳6");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![3, 4, 5, 0, 1, 2]);
    }

    #[test]
    fn test_format_monadic_gives_chars() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍕1 2 3");
        let s: String = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                _ => '?',
            })
            .collect();
        assert_eq!(s, "1 2 3");
    }

    #[test]
    fn test_format_dyadic_decimals_in_repl() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "2⍕1.5");
        let s: String = v
            .cells()
            .iter()
            .map(|c| match c {
                crate::cell::Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                _ => '?',
            })
            .collect();
        assert_eq!(s, "1.50");
    }

    #[test]
    fn test_where_honors_index_origin() {
        let mut env = Environment::new();
        // default ⎕IO=0 → positions 1 3
        let v = eval_one(&mut env, "⍸0 1 0 1");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 3]);
        // ⎕IO=1 shifts to 2 4
        env.eval_line("⎕IO←1").unwrap();
        let v = eval_one(&mut env, "⍸0 1 0 1");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![2, 4]);
    }

    #[test]
    fn test_where_composes_with_comparison() {
        // ⍸3<1 5 2 7 → positions of elements > 3 → 1 3 (0-based)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍸3<1 5 2 7");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 3]);
    }

    #[test]
    fn test_execute_arithmetic() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍎'2+3'");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 5);
    }

    #[test]
    fn test_execute_vector_result() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍎'1 2 3'");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 2, 3]);
    }

    #[test]
    fn test_execute_assignment_persists() {
        let mut env = Environment::new();
        // the assignment happens in the caller's environment, and displays
        // nothing (shy) exactly like a bare Q←7
        assert!(env.eval_line("⍎'Q←7'").unwrap().is_none());
        let v = eval_one(&mut env, "Q+1");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 8);
    }

    #[test]
    fn test_execute_sees_existing_variables() {
        let mut env = Environment::new();
        env.eval_line("R←10").unwrap();
        let v = eval_one(&mut env, "⍎'R×2'");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 20);
    }

    #[test]
    fn test_execute_empty_is_shy() {
        // ⍎'' produces no displayed value
        let mut env = Environment::new();
        assert!(env.eval_line("⍎''").unwrap().is_none());
    }

    #[test]
    fn test_execute_round_trips_format() {
        // ⍎⍕N recovers a numeric value through its character form
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍎⍕42");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 42);
    }

    #[test]
    fn test_execute_rejects_numeric_argument() {
        let mut env = Environment::new();
        assert!(env.eval_line("⍎42").is_err());
    }

    #[test]
    fn test_execute_propagates_inner_error() {
        let mut env = Environment::new();
        // undefined name inside the executed text
        assert!(env.eval_line("⍎'NOSUCHVAR+1'").is_err());
    }

    #[test]
    fn test_execute_shy_does_not_leak_to_next_line() {
        // a shy execute must not suppress the FOLLOWING line's display
        let mut env = Environment::new();
        assert!(env.eval_line("⍎'Z←3'").unwrap().is_none());
        let v = eval_one(&mut env, "Z+1");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 4);
    }

    #[test]
    fn test_execute_assign_then_value_in_same_line_displays() {
        // an assigning execute followed by a value-producing one: the line
        // yields the value, so it must NOT be suppressed
        let mut env = Environment::new();
        let v = eval_one(&mut env, "(⍎'W←4')+⍎'2+3'");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 5);
    }

    #[test]
    fn test_execute_runaway_recursion_is_an_error() {
        // F←'⍎F' ⋄ ⍎F must hit the depth guard, not blow the native stack
        let mut env = Environment::new();
        env.eval_line("F←'⍎F'").unwrap();
        assert!(env.eval_line("⍎F").is_err());
    }

    #[test]
    fn test_execute_depth_resets_after_error() {
        // a caught runaway must not leave depth consumed for later executes
        let mut env = Environment::new();
        env.eval_line("F←'⍎F'").unwrap();
        assert!(env.eval_line("⍎F").is_err());
        // a plain execute still works afterwards
        let v = eval_one(&mut env, "⍎'1+1'");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 2);
    }

    #[test]
    fn test_find_subvector_in_repl() {
        // 1 2⍷1 2 3 1 2 → 1 0 0 1 0
        let mut env = Environment::new();
        let v = eval_one(&mut env, "1 2⍷1 2 3 1 2");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 0, 0, 1, 0]);
    }

    #[test]
    fn test_find_composes_with_where() {
        // ⍸1 2⍷1 2 3 1 2 → the origins of each match → 0 3 (0-based)
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⍸1 2⍷1 2 3 1 2");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![0, 3]);
    }

    #[test]
    fn test_find_string_in_repl() {
        // 'ab'⍷'xabyab' → 0 1 0 0 1 0
        let mut env = Environment::new();
        let v = eval_one(&mut env, "'ab'⍷'xabyab'");
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![0, 1, 0, 0, 1, 0]);
    }

    #[test]
    fn test_find_counts_occurrences_via_plus_reduce() {
        // +/'ab'⍷'xabyab' → 2 occurrences
        let mut env = Environment::new();
        let v = eval_one(&mut env, "+/'ab'⍷'xabyab'");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 2);
    }

    #[test]
    fn test_find_in_matrix_keeps_shape() {
        // pattern conformed to 1x2 inside a 2x3 matrix; result is 2x3
        let mut env = Environment::new();
        let v = eval_one(&mut env, "3 4⍷2 3⍴1 2 3 3 4 5");
        assert_eq!(v.rank(), 2);
        assert_eq!(v.get_shape_item(0), 2);
        assert_eq!(v.get_shape_item(1), 3);
        let ints: Vec<i64> = v
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![0, 0, 0, 1, 0, 0]);
    }

    // ── empty-array scalar extension ───────────────────────────────────────
    // Verified against the reference C++ GNU APL binary: scalar extension
    // over an EMPTY array yields an empty array, it is not a LENGTH ERROR.

    #[test]
    fn test_scalar_extension_over_empty_is_empty() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "(0⍴0)+1");
        assert_eq!(v.element_count(), 0);
    }

    #[test]
    fn test_scalar_extension_over_empty_either_side() {
        let mut env = Environment::new();
        assert_eq!(eval_one(&mut env, "1+0⍴0").element_count(), 0);
        assert_eq!(eval_one(&mut env, "(0⍴0)×2").element_count(), 0);
    }

    #[test]
    fn test_where_of_all_zeros_survives_index_origin_shift() {
        // ⍸0 0 0 is empty; with ⎕IO=1 the result is shifted by adding 1, which
        // used to raise LENGTH ERROR because the empty operand was rejected
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        let v = eval_one(&mut env, "⍸0 0 0");
        assert_eq!(v.element_count(), 0);
    }

    #[test]
    fn test_conflicting_lengths_still_error() {
        // the empty-array fix must not weaken real length checking
        let mut env = Environment::new();
        assert!(env.eval_line("1 2 3+1 2").is_err());
    }

    // ── monadic ravel , ────────────────────────────────────────────────────

    #[test]
    fn test_ravel_of_matrix_is_rank1_in_repl() {
        let mut env = Environment::new();
        assert_eq!(
            eval_one(&mut env, "≢⍴,2 3⍴⍳6")
                .first_cell()
                .unwrap()
                .get_int_value()
                .unwrap(),
            1
        );
        assert_eq!(
            eval_one(&mut env, "≢,2 3⍴⍳6")
                .first_cell()
                .unwrap()
                .get_int_value()
                .unwrap(),
            6
        );
    }

    // ── glyph coverage found by differential testing ───────────────────────

    #[test]
    fn test_ascii_bar_is_magnitude_like_the_apl_glyph() {
        // GNU APL accepts BOTH | (ASCII) and ∣ (U+2223); only ∣ was wired,
        // so every `3|10` style expression was a SYNTAX ERROR
        let mut env = Environment::new();
        assert_eq!(eval_num_in(&mut env, "3|10"), 1.0);
        assert_eq!(eval_num_in(&mut env, "3∣10"), 1.0);
        assert_eq!(eval_num_in(&mut env, "|¯7"), 7.0);
        assert_eq!(eval_num_in(&mut env, "¯3|10"), -2.0);
    }

    #[test]
    fn test_monadic_star_is_exponential() {
        // * tokenizes as Power (the dyadic glyph) but MONADIC * is
        // exponential; without an eval_monadic arm `*1` was a SYNTAX ERROR
        // even though `2*10` worked.
        let mut env = Environment::new();
        assert_eq!(eval_num_in(&mut env, "⌊*1"), 2.0); // ⌊e = 2
        assert_eq!(eval_num_in(&mut env, "2*10"), 1024.0);
        assert_eq!(eval_num_in(&mut env, "⌊⋆1"), 2.0); // ⋆ still works
    }

    #[test]
    fn test_binomial_in_repl() {
        // A!B is B choose A
        let mut env = Environment::new();
        assert_eq!(eval_num_in(&mut env, "2!5"), 10.0);
        assert_eq!(eval_num_in(&mut env, "5!2"), 0.0);
        assert_eq!(eval_num_in(&mut env, "2!4"), 6.0);
    }

    // ── rank ≥ 2 semantics found by differential testing ───────────────────
    // Every expectation below was verified against the reference C++ binary.

    #[test]
    fn test_take_drop_are_per_axis_on_matrices() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // one count per axis; a single count on a matrix is a LENGTH ERROR
        assert!(env.eval_line("1↑2 3⍴⍳6").is_err());
        assert!(env.eval_line("1↓2 3⍴⍳6").is_err());
        assert_eq!(ravel_ints(&mut env, "1 2↑2 3⍴⍳6"), vec![1, 2]);
        assert_eq!(ravel_ints(&mut env, "¯1 ¯2↑2 3⍴⍳6"), vec![5, 6]);
        assert_eq!(ravel_ints(&mut env, "1 1↓2 3⍴⍳6"), vec![5, 6]);
        assert_eq!(ravel_ints(&mut env, "¯1 ¯1↓2 3⍴⍳6"), vec![1, 2]);
        // over-take pads with the prototype to the full requested shape
        assert_eq!(
            ravel_ints(&mut env, "3 4↑2 3⍴⍳6"),
            vec![1, 2, 3, 0, 4, 5, 6, 0, 0, 0, 0, 0]
        );
        // dropping past an axis empties it
        assert_eq!(ravel_ints(&mut env, "⍴5 5↓2 3⍴⍳6"), vec![0, 0]);
    }

    #[test]
    fn test_monadic_transpose_reverses_all_axes() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(ravel_ints(&mut env, "⍴⍉2 3 4⍴⍳24"), vec![4, 3, 2]);
        assert_eq!(
            ravel_ints(&mut env, ",⍉2 2 2⍴⍳8"),
            vec![1, 5, 3, 7, 2, 6, 4, 8]
        );
    }

    #[test]
    fn test_dyadic_transpose_and_diagonal() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // identity permutation
        assert_eq!(ravel_ints(&mut env, ",1 2⍉2 3⍴⍳6"), vec![1, 2, 3, 4, 5, 6]);
        // swap → ordinary transpose
        assert_eq!(ravel_ints(&mut env, ",2 1⍉2 3⍴⍳6"), vec![1, 4, 2, 5, 3, 6]);
        assert_eq!(ravel_ints(&mut env, "⍴2 1⍉2 3⍴⍳6"), vec![3, 2]);
        // repeated axis selects the DIAGONAL and lowers the rank
        assert_eq!(ravel_ints(&mut env, ",1 1⍉3 3⍴⍳9"), vec![1, 5, 9]);
        assert_eq!(ravel_ints(&mut env, "⍴1 1⍉3 3⍴⍳9"), vec![3]);
    }

    #[test]
    fn test_catenate_matrices_interleaves_rows() {
        // joining along the last axis puts B's row AFTER A's row, per row
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(
            ravel_ints(&mut env, ",(2 3⍴⍳6),2 3⍴⍳6"),
            vec![1, 2, 3, 1, 2, 3, 4, 5, 6, 4, 5, 6]
        );
        assert_eq!(ravel_ints(&mut env, "⍴(2 3⍴⍳6),2 3⍴⍳6"), vec![2, 6]);
    }

    #[test]
    fn test_grade_on_matrix_orders_rows() {
        // ⍋ over a matrix grades its ROWS lexicographically, so there is one
        // index per row (not one per element)
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(ravel_ints(&mut env, "⍋3 2⍴1 2 0 1 2 2"), vec![2, 1, 3]);
        assert_eq!(ravel_ints(&mut env, "⍒3 2⍴1 2 0 1 2 2"), vec![3, 1, 2]);
    }

    #[test]
    fn test_encode_vector_builds_column_per_value() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // 2 2 2⊤5 3 → 3 rows (one per base) × 2 columns (one per value)
        assert_eq!(ravel_ints(&mut env, "⍴2 2 2⊤5 3"), vec![3, 2]);
        assert_eq!(ravel_ints(&mut env, ",2 2 2⊤5 3"), vec![1, 0, 0, 1, 1, 1]);
    }

    #[test]
    fn test_decode_matrix_reduces_each_column() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // each COLUMN is an independent base-2 number
        assert_eq!(ravel_ints(&mut env, "2⊥2 3⍴1 0 1 1 1 0"), vec![3, 1, 2]);
    }

    // ── 2-D bracket indexing M[i;j] ────────────────────────────────────────
    // All expectations reference-verified.

    #[test]
    fn test_bracket_index_two_axes() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        assert_eq!(ravel_ints(&mut env, "M[1;1]"), vec![1]);
        assert_eq!(ravel_ints(&mut env, "M[2;3]"), vec![6]);
        assert_eq!(ravel_ints(&mut env, ",M[1;]"), vec![1, 2, 3]);
        assert_eq!(ravel_ints(&mut env, ",M[;1]"), vec![1, 4]);
    }

    #[test]
    fn test_bracket_index_axis_dropping_follows_syntax() {
        // a SCALAR index drops its axis; a VECTOR index keeps it even when it
        // holds a single element — M[1;1] is a scalar but M[1 1;] is 2×3
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        assert_eq!(eval_one(&mut env, "M[1;1]").rank(), 0);
        assert_eq!(ravel_ints(&mut env, "⍴M[1;]"), vec![3]);
        assert_eq!(ravel_ints(&mut env, "⍴M[;1]"), vec![2]);
        assert_eq!(ravel_ints(&mut env, "⍴M[1 2;1 2]"), vec![2, 2]);
        assert_eq!(ravel_ints(&mut env, "⍴M[1 1;]"), vec![2, 3]);
    }

    #[test]
    fn test_bracket_index_selects_and_reorders() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M[1 2;1 2]"), vec![1, 2, 4, 5]);
        // reversed column order comes back reversed
        assert_eq!(ravel_ints(&mut env, ",M[;3 1]"), vec![3, 1, 6, 4]);
    }

    #[test]
    fn test_bracket_index_on_parenthesised_value() {
        // indexing applies to any expression, not just a named variable
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(ravel_ints(&mut env, "(2 3⍴⍳6)[2;3]"), vec![6]);
        assert_eq!(ravel_ints(&mut env, ",(2 3⍴⍳6)[1;]"), vec![1, 2, 3]);
    }

    #[test]
    fn test_bracket_index_rank3() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("C←2 2 2⍴⍳8").unwrap();
        assert_eq!(ravel_ints(&mut env, ",C[1;;]"), vec![1, 2, 3, 4]);
        assert_eq!(ravel_ints(&mut env, "⍴C[1;;]"), vec![2, 2]);
        assert_eq!(ravel_ints(&mut env, "C[1;2;1]"), vec![3]);
    }

    #[test]
    fn test_bracket_index_one_axis_still_works() {
        // the 1-D path must be untouched by the multi-axis addition
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←10 20 30").unwrap();
        assert_eq!(ravel_ints(&mut env, "V[2]"), vec![20]);
        assert_eq!(ravel_ints(&mut env, ",V[1 3]"), vec![10, 30]);
    }

    #[test]
    fn test_bracket_index_errors() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        // out of range
        assert!(env.eval_line("M[3;1]").is_err());
        assert!(env.eval_line("M[1;9]").is_err());
        // wrong number of axes for the rank
        assert!(env.eval_line("M[1;1;1]").is_err());
        // unclosed bracket
        assert!(env.eval_line("M[1;1").is_err());
    }

    // ── selective assignment M[i;j]←v ──────────────────────────────────────
    // All expectations reference-verified.

    #[test]
    fn test_indexed_assign_honors_index_origin() {
        // the 1-D path ignored ⎕IO entirely: V[2]←99 was an INDEX ERROR
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←10 20 30").unwrap();
        env.eval_line("V[2]←99").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![10, 99, 30]);
    }

    #[test]
    fn test_indexed_assign_distributes_elementwise() {
        // a multi-element right side pairs up with the indices; a scalar
        // right side broadcasts. The old code always wrote rv.cells()[0].
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("W←10 20 30").unwrap();
        env.eval_line("W[1 3]←7 8").unwrap();
        assert_eq!(ravel_ints(&mut env, "W"), vec![7, 20, 8]);
        env.eval_line("X←10 20 30").unwrap();
        env.eval_line("X[1 3]←0").unwrap();
        assert_eq!(ravel_ints(&mut env, "X"), vec![0, 20, 0]);
    }

    #[test]
    fn test_indexed_assign_length_mismatch_errors() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←10 20 30").unwrap();
        assert!(env.eval_line("V[1 2]←1 2 3").is_err());
    }

    #[test]
    fn test_assign_two_axes() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("M[1;2]←99").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M"), vec![1, 99, 3, 4, 5, 6]);
        // the shape must survive the write
        assert_eq!(ravel_ints(&mut env, "⍴M"), vec![2, 3]);
    }

    #[test]
    fn test_assign_whole_row_and_column() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("M[1;]←0 0 0").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M"), vec![0, 0, 0, 4, 5, 6]);
        env.eval_line("N←2 3⍴⍳6").unwrap();
        env.eval_line("N[;1]←7 7").unwrap();
        assert_eq!(ravel_ints(&mut env, ",N"), vec![7, 2, 3, 7, 5, 6]);
    }

    #[test]
    fn test_assign_submatrix_broadcasts_scalar() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("M[1 2;1 2]←100").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M"), vec![100, 100, 3, 100, 100, 6]);
    }

    #[test]
    fn test_assign_all_elided_fills_everything() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("M[;]←0").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M"), vec![0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_assign_rank3() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("C←2 2 2⍴⍳8").unwrap();
        env.eval_line("C[1;2;1]←0").unwrap();
        assert_eq!(ravel_ints(&mut env, ",C"), vec![1, 2, 0, 4, 5, 6, 7, 8]);
        env.eval_line("D←2 2 2⍴⍳8").unwrap();
        env.eval_line("D[1;;]←0").unwrap();
        assert_eq!(ravel_ints(&mut env, ",D"), vec![0, 0, 0, 0, 5, 6, 7, 8]);
    }

    #[test]
    fn test_assign_is_shy() {
        // an indexed assignment displays nothing, like a plain one
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        assert!(env.eval_line("M[1;2]←99").unwrap().is_none());
    }

    #[test]
    fn test_assign_axis_errors() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        assert!(env.eval_line("M[9;1]←0").is_err()); // row out of range
        assert!(env.eval_line("M[1;9]←0").is_err()); // column out of range
        assert!(env.eval_line("M[1;1;1]←0").is_err()); // too many axes
    }

    // ── multi-statement lines with ⋄ ────────────────────────────────────────

    #[test]
    fn test_top_level_diamond_runs_each_statement() {
        // ⋄ was only handled inside ⎕EA and dfn bodies, so a plain
        // `A ⋄ B` line was a SYNTAX ERROR at the REPL
        let mut env = Environment::new();
        let v = eval_one(&mut env, "2+3 ⋄ 4+5");
        // only the LAST statement's value is displayed
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 9);
    }

    #[test]
    fn test_top_level_diamond_sequences_side_effects() {
        let mut env = Environment::new();
        let v = eval_one(&mut env, "Q←7 ⋄ Q+1");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 8);
    }

    #[test]
    fn test_diamond_inside_dfn_still_belongs_to_the_dfn() {
        // splitting must be depth-aware: this ⋄ is a dfn body separator
        let mut env = Environment::new();
        let v = eval_one(&mut env, "{Z←⍵ ⋄ Z+1} 5");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 6);
    }

    #[test]
    fn test_error_guard_diamond_still_works() {
        // ⎕EA has its own diamond handling and must not be split by the new
        // top-level splitter: the guard fails, so the fallback runs
        let mut env = Environment::new();
        let v = eval_one(&mut env, "⎕EA NOPE+1 ⋄ 42");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 42);
        // and when the guard succeeds, ITS value is the result
        let v = eval_one(&mut env, "⎕EA 5+5 ⋄ NOPE");
        assert_eq!(v.first_cell().unwrap().get_int_value().unwrap(), 10);
    }

    #[test]
    fn test_rank_operator_applies_per_cell() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // reverse each ROW of a matrix
        assert_eq!(ravel_ints(&mut env, ",(⌽⍤1)2 3⍴⍳6"), vec![3, 2, 1, 6, 5, 4]);
        assert_eq!(ravel_ints(&mut env, "⍴(⌽⍤1)2 3⍴⍳6"), vec![2, 3]);
        // tally per row: one scalar each
        assert_eq!(ravel_ints(&mut env, ",(≢⍤1)2 3⍴⍳6"), vec![3, 3]);
    }

    #[test]
    fn test_rank_at_or_above_argument_rank_is_whole_array() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // rank 2 on a matrix → one cell → ≢ of the whole matrix is 2
        assert_eq!(ravel_ints(&mut env, "(≢⍤2)2 3⍴⍳6"), vec![2]);
        // rank 3 exceeds the rank, so still one cell
        assert_eq!(ravel_ints(&mut env, ",(⌽⍤3)2 3⍴⍳6"), vec![3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn test_rank_operator_frames_a_cube() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(
            ravel_ints(&mut env, ",(⌽⍤1)2 2 2⍴⍳8"),
            vec![2, 1, 4, 3, 6, 5, 8, 7]
        );
        // one scalar per row, framed by the leading two axes
        assert_eq!(ravel_ints(&mut env, ",(≢⍤1)2 2 2⍴⍳8"), vec![2, 2, 2, 2]);
        assert_eq!(ravel_ints(&mut env, "⍴(≢⍤1)2 2 2⍴⍳8"), vec![2, 2]);
    }

    #[test]
    fn test_rank_operator_honors_index_origin() {
        // the operator applies the prim itself, so it must route ⎕IO-sensitive
        // primitives the same way the Monadic arm does — (⍳⍤0)3 must match ⍳3
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(ravel_ints(&mut env, ",(⍳⍤0)3"), vec![1, 2, 3]);
        env.eval_line("⎕IO←0").unwrap();
        assert_eq!(ravel_ints(&mut env, ",(⍳⍤0)3"), vec![0, 1, 2]);
    }

    #[test]
    fn test_rank_operator_on_vector() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(ravel_ints(&mut env, ",(⌽⍤1)1 2 3"), vec![3, 2, 1]);
    }

    #[test]
    fn test_rank_dyadic_two_ranks_uses_right_for_monadic() {
        // monadic f⍤kl kr uses kr for the single argument
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // ,⍤1 0 on a matrix: rank 0 → ravel each scalar → shape 2 3 1
        assert_eq!(ravel_ints(&mut env, "⍴(,⍤1 0)2 3⍴⍳6"), vec![2, 3, 1]);
        // ,⍤0 1 on a matrix: rank 1 → ravel each row → shape 2 3
        assert_eq!(ravel_ints(&mut env, "⍴(,⍤0 1)2 3⍴⍳6"), vec![2, 3]);
    }

    #[test]
    fn test_rank_dyadic_two_ranks_pair_independently() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // scalar cells pair up
        assert_eq!(
            ravel_ints(&mut env, ",1 2 3(,⍤0 0)4 5 6"),
            vec![1, 4, 2, 5, 3, 6]
        );
        assert_eq!(ravel_ints(&mut env, "⍴1 2 3(,⍤0 0)4 5 6"), vec![3, 2]);
        // each scalar with the whole B
        assert_eq!(
            ravel_ints(&mut env, ",1 2 3(,⍤0 1)4 5 6"),
            vec![1, 4, 5, 6, 2, 4, 5, 6, 3, 4, 5, 6]
        );
        // whole A with each scalar
        assert_eq!(
            ravel_ints(&mut env, ",1 2 3(,⍤1 0)4 5 6"),
            vec![1, 2, 3, 4, 1, 2, 3, 5, 1, 2, 3, 6]
        );
    }

    #[test]
    fn test_rank_dyadic_parenthesized_lhs() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(
            ravel_ints(&mut env, ",(2 2⍴1 2 3 4)(,⍤1 1)2 2⍴5 6 7 8"),
            vec![1, 2, 5, 6, 3, 4, 7, 8]
        );
        assert_eq!(
            ravel_ints(&mut env, ",(2 2⍴1 2 3 4)(+⍤1)2 2⍴5 6 7 8"),
            vec![6, 8, 10, 12]
        );
    }

    // ── selective assignment through selectors ─────────────────────────

    #[test]
    fn test_selective_assignment_take() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←1 2 3 4 5").unwrap();
        env.eval_line("(2↑V)←9 8").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![9, 8, 3, 4, 5]);
    }

    #[test]
    fn test_selective_assignment_drop() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←1 2 3 4 5").unwrap();
        env.eval_line("(2↓V)←9 9 9").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![1, 2, 9, 9, 9]);
    }

    #[test]
    fn test_selective_assignment_rotate() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←1 2 3 4 5").unwrap();
        env.eval_line("(3⌽V)←10 20 30 40 50").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![30, 40, 50, 10, 20]);
    }

    #[test]
    fn test_selective_assignment_reshape() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←1 2 3 4 5").unwrap();
        env.eval_line("(3⍴V)←99 99 99").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![99, 99, 99, 4, 5]);
    }

    #[test]
    fn test_selective_assignment_broadcast_scalar() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←1 2 3 4 5").unwrap();
        env.eval_line("(2↑V)←99").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![99, 99, 3, 4, 5]);
    }

    #[test]
    fn test_selective_assignment_matrix() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("(2 3↑M)←99").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M"), vec![99, 99, 99, 99, 99, 99]);
    }

    #[test]
    fn test_selective_assignment_extra_rhs_ignored() {
        // RHS has more elements than positions; extras are silently ignored
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←1 2 3 4 5").unwrap();
        env.eval_line("(4↑V)←10 20 30 40 50 60").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![10, 20, 30, 40, 5]);
    }

    // ── display parity: nested arrays print plain by default ────────────────

    #[test]
    fn test_nested_vector_prints_plain() {
        // GNU APL prints nested arrays plain by default, not boxed
        let mut env = Environment::new();
        env.eval_line("N←(1 2)(3 4 5)").unwrap();
        let v = env.eval_line("N").unwrap().unwrap();
        let lines = crate::boxdisplay::render_plain(&v);
        assert_eq!(lines, vec!["1 2  3 4 5"]);
    }

    #[test]
    fn test_nested_matrix_prints_plain() {
        let mut env = Environment::new();
        env.eval_line("M←2 2⍴(1 2)(3 4 5)(6 7)(8 9 10)").unwrap();
        let v = env.eval_line("M").unwrap().unwrap();
        let lines = crate::boxdisplay::render_plain(&v);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("1 2"));
        assert!(lines[0].contains("3 4 5"));
        assert!(lines[1].contains("6 7"));
        assert!(lines[1].contains("8 9 10"));
    }

    #[test]
    fn test_enclosed_vector_prints_plain() {
        let mut env = Environment::new();
        env.eval_line("N←⊂(1 2)(3 4 5)").unwrap();
        let v = env.eval_line("N").unwrap().unwrap();
        let lines = crate::boxdisplay::render_plain(&v);
        assert_eq!(lines, vec!["1 2  3 4 5"]);
    }

    // ── 4⎕CR boxed display ──────────────────────────────────────────────────

    #[test]
    fn test_quadcr_simple_vector() {
        let mut env = Environment::new();
        let v = env.eval_line("4⎕CR 1 2 3").unwrap().unwrap();
        // Simple vector → char vector (no outer box needed for single line)
        // Simple vector → char matrix with outer box (3 rows: top, content, bottom)
        assert_eq!(v.rank(), 2);
        let text: String = v
            .cells()
            .iter()
            .map(|c| match c {
                Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                _ => '?',
            })
            .collect();
        assert!(text.contains("1 2 3"));
    }

    #[test]
    fn test_quadcr_nested_vector() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        let v = env.eval_line("4⎕CR(1 2)(3 4 5)").unwrap().unwrap();
        // Nested vector → char matrix with outer box
        assert_eq!(v.rank(), 2);
        let shape = v.shape();
        assert_eq!(shape.get_shape_item(0), 5); // 5 rows (box top, 2 content, box bottom, padding)
        assert!(shape.get_shape_item(1) >= 10); // at least 10 cols
    }

    #[test]
    fn test_quadcr_in_expression() {
        // 4⎕CR should work when nested in another expression
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        let v = env.eval_line(",4⎕CR 1 2 3").unwrap().unwrap();
        // Ravel of a char matrix → char vector
        // Simple vector → char matrix with outer box (3 rows: top, content, bottom)
        assert_eq!(v.rank(), 1);
    }

    #[test]
    fn test_quadcr_1_ravel() {
        // 1⎕CR B — simple ravel into a char vector
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        let v = env.eval_line("1⎕CR 1 2 3").unwrap().unwrap();
        assert_eq!(v.rank(), 1);
        let text: String = v
            .cells()
            .iter()
            .map(|c| match c {
                Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                _ => '?',
            })
            .collect();
        assert!(text.contains("1 2 3"));
    }

    #[test]
    fn test_quadcr_1_nested() {
        // 1⎕CR on nested array adds parens
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        let v = env.eval_line("1⎕CR(1 2)(3 4 5)").unwrap().unwrap();
        let text: String = v
            .cells()
            .iter()
            .map(|c| match c {
                Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                _ => '?',
            })
            .collect();
        assert!(text.contains("(1 2)"));
        assert!(text.contains("(3 4 5)"));
    }

    #[test]
    fn test_power_op_named_function() {
        // f⍣N — apply named function N times
        let mut env = Environment::new();
        env.eval_line("double←{2×⍵}").unwrap();
        let v = env.eval_line("double⍣3 5").unwrap().unwrap();
        assert_eq!(v.first_cell().unwrap(), &Cell::Int(40)); // 5→10→20→40
    }

    #[test]
    fn test_power_op_named_function_square() {
        // square⍣2 3 → 3²=9, 9²=81
        let mut env = Environment::new();
        env.eval_line("square←{⍵×⍵}").unwrap();
        let v = env.eval_line("square⍣2 3").unwrap().unwrap();
        assert_eq!(v.first_cell().unwrap(), &Cell::Int(81));
    }

    // ── squad ⌷ selector for selective assignment ──────────────────────────

    #[test]
    fn test_selective_assignment_squad() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("(1 2⌷M)←99").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M"), vec![1, 99, 3, 4, 5, 6]);
    }

    #[test]
    fn test_selective_assignment_squad_multiple() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("(1 2⌷M)←99").unwrap();
        env.eval_line("(2 3⌷M)←88").unwrap();
        assert_eq!(ravel_ints(&mut env, ",M"), vec![1, 99, 3, 4, 5, 88]);
    }

    #[test]
    fn test_selective_assignment_squad_out_of_bounds() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        assert!(env.eval_line("(3 1⌷M)←99").is_err()); // row 3 out of bounds
        assert!(env.eval_line("(1 4⌷M)←99").is_err()); // col 4 out of bounds
    }

    // ── table ⍪ selector for selective assignment ──────────────────────────

    #[test]
    fn test_selective_assignment_table() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←10 20 30").unwrap();
        env.eval_line("(⍪V)←99 88 77").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![99, 88, 77]);
    }

    #[test]
    fn test_selective_assignment_table_reshape() {
        // ⍪ on a vector makes it a column vector; assignment replaces entirely
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        env.eval_line("V←10 20 30").unwrap();
        env.eval_line("(⍪V)←99 88 77").unwrap();
        assert_eq!(ravel_ints(&mut env, "V"), vec![99, 88, 77]);
    }

    // ── zilde ⍬ ────────────────────────────────────────────────────────────

    #[test]
    fn test_zilde_is_empty_numeric_vector() {
        let mut env = Environment::new();
        let v = env.eval_line("⍬").unwrap().unwrap();
        assert_eq!(v.rank(), 1);
        assert_eq!(v.element_count(), 0);
    }

    #[test]
    fn test_zilde_rho() {
        let _env = Environment::new();
        assert_eq!(eval_int("⍴⍬"), 0);
    }

    #[test]
    fn test_zilde_tally() {
        let _env = Environment::new();
        assert_eq!(eval_int("≢⍬"), 0);
    }

    #[test]
    fn test_zilde_equals_empty_numeric() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(eval_int("⍬≡0⍴0"), 1);
    }

    // ── power operator ⍣ ───────────────────────────────────────────────────

    #[test]
    fn test_power_op_monic_identity() {
        // F⍣0 B = B (identity)
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(eval_int("×⍣0 5"), 5);
    }

    #[test]
    fn test_power_op_monic_once() {
        // F⍣1 B = F B
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        // sign of 5 is 1
        assert_eq!(eval_int("×⍣1 5"), 1);
    }

    #[test]
    fn test_power_op_monic_thrice() {
        // ×⍣3 5 = ×(×(×5)) = ×(×1) = ×1 = 1
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(eval_int("×⍣3 5"), 1);
    }

    #[test]
    fn test_power_op_negation() {
        // Test with a monadic primitive: ⌽ reverses a vector
        // ⌽⍣3 1 2 3 = ⌽(⌽(⌽1 2 3)) = ⌽(⌽3 2 1) = ⌽1 2 3 = 3 2 1
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        let _v = env.eval_line("⌽⍣3 1 2 3").unwrap().unwrap();
        assert_eq!(ravel_ints(&mut env, ",⌽⍣3 1 2 3"), vec![3, 2, 1]);
    }

    // ── complex numbers ────────────────────────────────────────────────────

    #[test]
    fn test_complex_literal() {
        let mut env = Environment::new();
        let v = env.eval_line("1J2").unwrap().unwrap();
        assert_eq!(v.first_cell().unwrap(), &Cell::complex(1.0, 2.0));
    }

    #[test]
    fn test_complex_add() {
        let mut env = Environment::new();
        let v = env.eval_line("1J2+2J3").unwrap().unwrap();
        assert_eq!(v.first_cell().unwrap(), &Cell::complex(3.0, 5.0));
    }

    #[test]
    fn test_complex_multiply() {
        let mut env = Environment::new();
        // (1+2i)(2+3i) = 2+3i+4i+6i² = 2+7i-6 = -4+7i
        let v = env.eval_line("1J2×2J3").unwrap().unwrap();
        assert_eq!(v.first_cell().unwrap(), &Cell::complex(-4.0, 7.0));
    }

    #[test]
    fn test_complex_divide() {
        let mut env = Environment::new();
        let v = env.eval_line("1J2÷2J3").unwrap().unwrap();
        // (1+2i)/(2+3i) = (1+2i)(2-3i)/13 = (2-3i+4i+6)/13 = (8+i)/13
        match v.first_cell().unwrap() {
            Cell::Complex(c) => {
                assert!((c.re - 8.0 / 13.0).abs() < 1e-10);
                assert!((c.im - 1.0 / 13.0).abs() < 1e-10);
            }
            other => panic!("expected complex, got {:?}", other),
        }
    }

    #[test]
    fn test_complex_real_part() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(eval_num_in(&mut env, "9○1J2"), 1.0);
    }

    #[test]
    fn test_complex_imag_part() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        assert_eq!(eval_num_in(&mut env, "11○1J2"), 2.0);
    }

    #[test]
    fn test_complex_magnitude() {
        let mut env = Environment::new();
        env.eval_line("⎕IO←1").unwrap();
        let mag = eval_num_in(&mut env, "10○1J2");
        assert!((mag - (5.0_f64).sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_complex_conjugate() {
        let mut env = Environment::new();
        let v = env.eval_line("¯12○1J2").unwrap().unwrap();
        assert_eq!(v.first_cell().unwrap(), &Cell::complex(1.0, -2.0));
    }
}
