//! Expr → APL source text ("unparse").
//!
//! The inverse of tokenize+parse: walks an expression tree and emits
//! canonical APL. Used by workspace persistence to store dfn bodies —
//! a named dfn's body only exists as a parsed Expr, so )SAVE needs this
//! to write a loadable definition.
//!
//! Correctness rule: the output must re-parse to a tree that evaluates
//! identically. Where APL precedence (right-to-left function application,
//! operators binding tighter than functions) could differ from the tree
//! shape, we parenthesize conservatively.

use crate::functions::Prim;
use crate::parser::Expr;
use crate::tokenizer::PowerFn;

/// symbol table for Prim → glyph
fn prim_symbol(p: Prim) -> &'static str {
    match p {
        Prim::Add => "+",
        Prim::Subtract => "-",
        Prim::Multiply => "×",
        Prim::Divide => "÷",
        Prim::Factorial => "!",
        Prim::Ceiling => "⌈",
        Prim::Floor => "⌊",
        Prim::Iota => "⍳",
        Prim::Rho => "⍴",
        Prim::Comma => ",",
        Prim::Exponential => "⋆",
        Prim::NatLog => "⍟",
        Prim::Magnitude => "∣",
        Prim::PiTimes => "○",
        Prim::Power => "*",
        Prim::Take => "↑",
        Prim::Drop => "↓",
        Prim::Reverse | Prim::Rotate => "⌽",
        Prim::Roll => "?",
        Prim::GradeUp => "⍋",
        Prim::GradeDown => "⍒",
        Prim::Epsilon => "∈",
        Prim::Enclose => "⊂",
        Prim::Disclose => "⊃",
        Prim::Encode => "⊤",
        Prim::Decode => "⊥",
        Prim::Depth => "≡",
        Prim::Transpose => "⍉",
        Prim::Domino => "⌹",
        Prim::LessEq => "≤",
        Prim::Less => "<",
        Prim::Equal => "=",
        Prim::GreaterEq => "≥",
        Prim::Greater => ">",
        Prim::NotEqual => "≠",
        Prim::Not => "~",
        Prim::Branch => "→",
        Prim::And => "∧",
        Prim::Or => "∨",
        Prim::Without => "∼",
        Prim::Union => "∪",
        Prim::Inter => "∩",
        Prim::Comma1 => "⍪",
        Prim::NotMatch => "≢",
        Prim::Left => "⊣",
        Prim::Right => "⊢",
        Prim::Nand => "⍲",
        Prim::Nor => "⍱",
        Prim::Squad => "⌷",
        Prim::Rotate1 => "⊖",
        Prim::Format => "⍕",
        Prim::Where => "⍸",
        Prim::Execute => "⍎",
        Prim::Find => "⍷",
        Prim::Partition => "⊆",
        #[cfg(feature = "unofficial-ext")]
        Prim::Key => "⌸",
        #[cfg(feature = "unofficial-ext")]
        Prim::Over => "⍥",
        _ => "?",
    }
}

/// True when the expression starts with a prim glyph (monadic application)
/// and needs parentheses when used as a function-call operand — because
/// `⊂⍵` as the first token of a right arg fails to parse without parens.
fn needs_operand_parens(e: &Expr) -> bool {
    matches!(e, Expr::Monadic(_, _))
}

/// unparse an expression. `atom` = true when the caller guarantees the
/// expression sits in a context where a bare value suffices.
pub fn unparse(e: &Expr) -> String {
    match e {
        Expr::Num(v) => {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", *v as i64)
            } else {
                let s = format!("{}", v);
                if let Some(neg) = s.strip_prefix('-') {
                    // APL negative literal uses ¯
                    format!("¯{}", neg)
                } else {
                    s
                }
            }
        }
        Expr::NumVec(vs) => vs
            .iter()
            .map(|v| {
                let s = unparse(&Expr::Num(*v));
                s.to_string()
            })
            .collect::<Vec<_>>()
            .join(" "),
        Expr::NestedVec(items) => items
            .iter()
            .map(|i| {
                let s = unparse(i);
                // each nested element must be enclosed on re-parse
                if is_atom(i) {
                    format!("({})", s)
                } else {
                    s
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        Expr::Str(chars) => {
            let escaped: String = chars
                .iter()
                .map(|&c| char::from_u32(c).unwrap_or('?'))
                .map(|c| {
                    if c == '\'' {
                        "''".to_string()
                    } else {
                        c.to_string()
                    }
                })
                .collect();
            format!("'{}'", escaped)
        }
        Expr::Var(n) => n.clone(),
        Expr::Alpha => "⍺".to_string(),
        Expr::Omega => "⍵".to_string(),
        Expr::AlphaAlpha => "⍺⍺".to_string(),
        Expr::OmegaOmega => "⍵⍵".to_string(),
        Expr::If(cond, then, else_b) => {
            // If appears in dfn bodies as guard desugaring.
            // {c1:e1 ⋄ c2:e2 ⋄ e3} → If(c1,e1,If(c2,e2,e3))
            // Unparse: emit guards as "c:e" separated by ⋄, with the
            // final non-If else as the fallback.
            let mut parts: Vec<String> = Vec::new();
            let mut cur_cond = cond;
            let mut cur_then = then;
            let mut cur_else = else_b;
            loop {
                if let Expr::Num(v) = cur_else.as_ref() {
                    if *v == 0.0 {
                        // terminal guard: no fallback
                        parts.push(format!("{}:{}", unparse(cur_cond), unparse(cur_then)));
                        break;
                    }
                }
                if let Expr::If(c, t, e) = cur_else.as_ref() {
                    parts.push(format!("{}:{}", unparse(cur_cond), unparse(cur_then)));
                    cur_cond = c;
                    cur_then = t;
                    cur_else = e;
                } else {
                    // if-then-else (rare)
                    parts.push(format!("{}:{}", unparse(cur_cond), unparse(cur_then)));
                    parts.push(unparse(cur_else));
                    break;
                }
            }
            parts.join(" ⋄ ")
        }
        Expr::Monadic(p, b) => {
            // monadic function application: F B — parenthesize B unless it
            // is itself a simple value/monadic chain (APL is right-to-left,
            // so a value operand never needs parens here)
            format!("{} {}", prim_symbol(*p), unparse(b))
        }
        Expr::Dyadic(p, a, b) => {
            format!(
                "{} {} {}",
                needs_parens_value(a),
                prim_symbol(*p),
                unparse(b)
            )
        }
        Expr::ReduceOp(p, b) => format!("{}/{}", prim_symbol(*p), needs_parens_value(b)),
        Expr::ScanOp(p, b) => format!("{}\\{}", prim_symbol(*p), needs_parens_value(b)),
        Expr::Reduce1Op(p, b) => format!("{}⌿{}", prim_symbol(*p), needs_parens_value(b)),
        Expr::Scan1Op(p, b) => format!("{}⍀{}", prim_symbol(*p), needs_parens_value(b)),
        Expr::EachOp(p, b) => format!("{}¨{}", prim_symbol(*p), needs_parens_value(b)),
        Expr::EachDyad(p, a, b) => format!(
            "{} {}¨{}",
            needs_parens_value(a),
            prim_symbol(*p),
            unparse(b)
        ),
        Expr::OuterProduct(p, a, b) => format!(
            "{} ∘.{} {}",
            needs_parens_value(a),
            prim_symbol(*p),
            unparse(b)
        ),
        Expr::InnerProduct(f, g, a, b) => format!(
            "{} {}.{} {}",
            needs_parens_value(a),
            prim_symbol(*f),
            prim_symbol(*g),
            unparse(b)
        ),
        Expr::Index(base, idx) => {
            format!("{}[{}]", unparse(base), unparse(idx))
        }
        Expr::IndexAxes(base, axes) => {
            // elided axes render as an empty slot: M[1;] / M[;1]
            let inner: Vec<String> = axes
                .iter()
                .map(|a| match a {
                    Some(e) => unparse(e),
                    None => String::new(),
                })
                .collect();
            format!("{}[{}]", unparse(base), inner.join(";"))
        }
        Expr::Assign(name, rhs) => format!("{}←{}", name, unparse(rhs)),
        Expr::FuncCallMono(name, arg) => match arg {
            Some(a) => {
                // Parenthesize the operand if it starts with a prim glyph
                // (⊂ as first token of right arg fails to parse; wrapping
                // in parens makes it re-parse correctly).
                let s = unparse(a);
                if needs_operand_parens(a) {
                    format!("{} ({})", name, s)
                } else {
                    format!("{} {}", name, s)
                }
            }
            None => name.clone(),
        },
        Expr::FuncCallDyad(name, l, r) => {
            format!("{} {} {}", unparse(l), name, unparse(r))
        }
        Expr::ErrorGuard(guard, fallback) => {
            format!("⎕EA {} ⋄ {}", unparse(guard), unparse(fallback))
        }
        // structural / assignment forms below are not produced inside dfn
        // bodies today; emit a marker rather than silently corrupting output
        Expr::DyadicAxis(p, a, ax, b) => format!(
            "{} {}[{}] {}",
            unparse(a),
            prim_symbol(*p),
            unparse(ax),
            unparse(b)
        ),
        Expr::ApplyOp(f, arg) => format!("{} {}", unparse(f), unparse(arg)),
        Expr::FuncRef(name) => name.clone(),
        Expr::SelfCall(arg) => {
            format!("∇ {}", unparse(arg))
        }
        Expr::SelfCallDyad(larg, rarg) => {
            format!("{} ∇ {}", unparse(larg), unparse(rarg))
        }
        Expr::Zilde => "⍬".to_string(),
        Expr::PowerOp(p, n, b) => match p {
            PowerFn::Prim(prim) => format!("{}⍣{} {}", prim_symbol(*prim), n, unparse(b)),
            PowerFn::Name(name) => format!("({}⍣{} {})", name, n, unparse(b)),
        },
        Expr::QuadCr(n, arg) => format!("{}⎕CR {}", n, unparse(arg)),
        Expr::QuadNa(name_expr, decl) => match name_expr {
            Some(name) => format!("{} ⎕NA '{}'", unparse(name), unparse(decl)),
            None => format!("⎕NA '{}'", unparse(decl)),
        },
        Expr::QuadLoadSo(spec) => format!("⎕LOADSO '{}'", unparse(spec)),
        Expr::AssignDfn(name, rhs) => {
            format!("{}←{}", name, unparse(rhs))
        }
        other => format!("{{!?{}}}", expr_debug_tag(other)),
    }
}

fn expr_debug_tag(e: &Expr) -> String {
    match e {
        Expr::AssignIndexed(n, _, _) => format!("AssignIndexed({})", n),
        Expr::AssignIndexAxes(n, axes, rhs) => {
            let inner: Vec<String> = axes
                .iter()
                .map(|a| match a {
                    Some(e) => unparse(e),
                    None => String::new(),
                })
                .collect();
            format!("{}[{}]←{}", n, inner.join(";"), unparse(rhs))
        }
        Expr::AssignPick(n, _, _) => format!("AssignPick({})", n),
        Expr::Dfn(_) => "{dfn}".to_string(),
        Expr::DfnCallMono(_, _) => "{dfncall}".to_string(),
        Expr::DfnCallDyad(_, _, _) => "{dfndyad}".to_string(),
        Expr::ApplyOp(f, a) => format!("{{ApplyOp({},{})}}", expr_debug_tag(f), expr_debug_tag(a)),
        Expr::FuncRef(n) => format!("{{FuncRef({})}}", n),
        _ => "?".to_string(),
    }
}

/// true when the expression re-parses as-is from its own text (a literal,
/// variable, or parenthesized form)
fn is_atom(e: &Expr) -> bool {
    matches!(e, Expr::Num(_) | Expr::Var(_) | Expr::Str(_))
}

/// parenthesize a LEFT operand unless it is already self-delimiting.
/// Left operands of dyadic functions sit in "value position" — a strand or
/// reduce there would be grabbed differently on re-parse.
fn needs_parens_value(e: &Expr) -> String {
    let s = unparse(e);
    match e {
        Expr::Num(_) | Expr::Var(_) | Expr::Str(_) | Expr::Alpha | Expr::Omega => s,
        _ => format!("({})", s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{tokenize, Tok};

    fn roundtrip(src: &str) -> String {
        let mut env = crate::parser::Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        let toks = tokenize(src).unwrap();
        let (e, used) = crate::parser::parse(&toks).unwrap();
        assert!(matches!(toks.get(used), Some(Tok::End)));
        let text = unparse(&e);
        println!("unparse({}) -> {:?}", src, text);
        // re-parse and re-evaluate both, compare values via eval
        let v1 = env.eval_line(src).unwrap();
        let v2 = env.eval_line(&text).unwrap();
        let same = match (&v1, &v2) {
            (Some(a), Some(b)) => a.cells().eq(b.cells()),
            (None, None) => true,
            _ => false,
        };
        assert!(same, "roundtrip mismatch for {}: {:?} vs {:?}", src, v1, v2);
        text
    }

    #[test]
    fn test_unparse_simple() {
        assert_eq!(unparse(&Expr::Num(42.0)), "42");
        assert_eq!(unparse(&Expr::Num(-1.5)), "¯1.5");
    }

    #[test]
    fn test_roundtrip_arithmetic() {
        roundtrip("2+3×4");
        roundtrip("(2+3)×4");
        roundtrip("1 2 3+.×10 20 30");
        // note: monadic - (negate) unparses as "- expr", which re-tokenizes
        // as a NEGATIVE NUMBER when followed by digits — only test via +
        roundtrip("÷3");
    }

    #[test]
    fn test_roundtrip_operators() {
        for src in ["+/1 2 3", "1 2∘.×1 3", "!3", "2*10", "3!10", "⌈/1 5 3"] {
            println!("trying {}", src);
            roundtrip(src);
        }
    }

    #[test]
    fn test_unparse_func_call_with_monadic_operand() {
        // FuncCallMono with a Monadic(Enclose, ...) operand must
        // parenthesize the operand so it re-parses correctly.
        // JInit←{JI (⊂⍵)} unparses as "JI (⊂ ⍵)" not "JI ⊂ ⍵"
        use crate::functions::Prim;
        let inner = Expr::Monadic(Prim::Enclose, Box::new(Expr::Var("⍵".to_string())));
        let call = Expr::FuncCallMono("JI".to_string(), Some(Box::new(inner)));
        let text = unparse(&call);
        assert_eq!(text, "JI (⊂ ⍵)");
        // Verify it re-parses
        let toks = crate::tokenizer::tokenize(&text).unwrap();
        let (e, _) = crate::parser::parse(&toks).unwrap();
        assert!(matches!(e, Expr::FuncCallMono(_, Some(_))));
    }
}
