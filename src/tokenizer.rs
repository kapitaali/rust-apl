//! Tokenizer — converts APL text into tokens.
//!
//! Mirrors `src/Tokenizer.cc` (simplified): numbers, names, strings,
//! primitive symbols, parentheses, and assignment.

use crate::functions::Prim;
use crate::types::ErrorCode;
use crate::types::{AplResult, Unicode};

/// One lexical token of an APL statement.
#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// numeric literal
    Num(f64),
    /// complex number literal reJim (e.g. 1J2)
    Complex(f64, f64),
    /// name (variable or function reference)
    Name(String),
    /// character string literal 'abc'
    Str(Vec<Unicode>),
    /// a primitive function symbol
    Prim(Prim),
    /// a reduced/derived operator: LO applied via reduce (`LO/B`)
    Reduce(Prim),
    /// first-axis reduce `LO⌿B`
    Reduce1(Prim),
    /// a scan operator: `LO\B`
    Scan(Prim),
    /// first-axis scan `LO⍀B`
    Scan1(Prim),
    /// the each operator: `F¨B`
    Each(Prim),
    /// the each operator with a named function: `f¨B`
    EachName(String),
    /// rank operator `f⍤k` — the operand prim; k follows as the next token
    Rank(Prim),
    /// a modified assignment `NAME +← expr` — shorthand for NAME ← NAME + expr
    ModifiedAssign(Prim),
    /// statement separator `⋄` (diamond)
    Diamond,
    /// outer product: `A ∘.f B`
    OuterDot(Prim),
    /// matrix product: `A ∘ B` — equivalent to `A +.× B`
    MatrixProduct,
    /// inner product: `A f.g B`
    InnerDot(Prim, Prim),
    /// commute operator `⍨`
    Commute,
    /// zilde `⍬` — the empty numeric vector
    Zilde,
    /// power operator `f⍣N` — apply f N times; the operand is either a
    /// primitive or a named function (resolved at parse time)
    PowerOp(PowerFn),
    /// assignment arrow ←
    Assign,
    /// left parenthesis
    LParen,
    /// right parenthesis
    RParen,
    /// left square bracket
    LBracket,
    /// right square bracket
    RBracket,
    /// left brace `{`
    LBrace,
    /// right brace `}`
    RBrace,
    /// dfn guard separator `:`
    Colon,
    /// index-axis separator `;` inside brackets: `M[i;j]`
    Semicolon,
    /// dfn left argument `⍺`
    Alpha,
    /// dfn right argument `⍵`
    Omega,
    /// dfn self-reference `∇`
    SelfRef,
    /// dfn left operand function `⍺⍺`
    AlphaAlpha,
    /// dfn right operand function `⍵⍵`
    OmegaOmega,
    /// end of input
    End,
}

/// The function operand of a power operator: either a primitive or a name.
#[derive(Clone, Debug, PartialEq)]
pub enum PowerFn {
    Prim(Prim),
    Name(String),
}

/// Primitive symbol table: single-char APL glyphs → Prim.
const PRIM_SYMBOLS: &[(&str, Prim)] = &[
    ("+", Prim::Add),
    ("-", Prim::Subtract),
    ("−", Prim::Subtract), // HIGH MINUS (U+2212) — alias for subtract/negate
    ("×", Prim::Multiply),
    ("÷", Prim::Divide),
    ("!", Prim::Factorial),
    ("?", Prim::Roll),
    ("⌈", Prim::Ceiling),
    ("⌊", Prim::Floor),
    ("⍳", Prim::Iota),
    ("⍴", Prim::Rho),
    (",", Prim::Comma),
    ("⋆", Prim::Exponential),
    ("○", Prim::PiTimes),
    ("⍟", Prim::NatLog),
    ("∣", Prim::Magnitude),
    ("|", Prim::Magnitude), // ASCII bar — GNU APL accepts both | and ∣
    ("∼", Prim::Without),
    ("∪", Prim::Union),
    ("∩", Prim::Inter),
    ("⍪", Prim::Comma1),
    ("≢", Prim::NotMatch),
    ("⊣", Prim::Left),
    ("⊢", Prim::Right),
    ("⍲", Prim::Nand),
    ("⍱", Prim::Nor),
    ("⌷", Prim::Squad),
    ("⊖", Prim::Rotate1),
    ("⍕", Prim::Format),
    ("⍸", Prim::Where),
    ("⍎", Prim::Execute),
    ("⍷", Prim::Find),
    ("⊆", Prim::Partition),
    #[cfg(feature = "unofficial-ext")]
    ("⌸", Prim::Key),
    ("↑", Prim::Take),
    ("↓", Prim::Drop),
    ("⌽", Prim::Reverse),
    ("⍋", Prim::GradeUp),
    ("⍒", Prim::GradeDown),
    ("∈", Prim::Epsilon),
    ("∊", Prim::Epsilon), // SMALL ELEMENT OF — alias used by GNU APL
    ("⊂", Prim::Enclose),
    ("⊃", Prim::Disclose),
    ("⊤", Prim::Encode),
    ("⊥", Prim::Decode),
    ("≡", Prim::Depth),
    ("≤", Prim::LessEq),
    ("<", Prim::Less),
    ("=", Prim::Equal),
    ("≥", Prim::GreaterEq),
    (">", Prim::Greater),
    ("≠", Prim::NotEqual),
    ("~", Prim::Not),
    ("→", Prim::Branch),
    ("⍉", Prim::Transpose),
    ("⌹", Prim::Domino),
    ("∧", Prim::And),
    ("∨", Prim::Or),
    ("*", Prim::Power), // APL power (dyadic) / exponential (monadic)
    ("⋆", Prim::Power), // STAR OPERATOR — alias for power (GNU APL accepts both)
];

/// Tokenize an APL source line.
pub fn tokenize(line: &str) -> AplResult<Vec<Tok>> {
    let mut toks = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    'outer: while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => {
                i += 1;
            }
            '⍝' | '#' => break, // comment: rest of line ignored
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '[' => {
                toks.push(Tok::LBracket);
                i += 1;
            }
            ']' => {
                toks.push(Tok::RBracket);
                i += 1;
            }
            ';' => {
                toks.push(Tok::Semicolon);
                i += 1;
            }
            '{' => {
                toks.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                toks.push(Tok::RBrace);
                i += 1;
            }
            ':' => {
                toks.push(Tok::Colon);
                i += 1;
            }
            '⍺' => {
                if chars.get(i + 1) == Some(&'⍺') {
                    toks.push(Tok::AlphaAlpha);
                    i += 2;
                } else {
                    toks.push(Tok::Alpha);
                    i += 1;
                }
            }
            '⍵' => {
                if chars.get(i + 1) == Some(&'⍵') {
                    toks.push(Tok::OmegaOmega);
                    i += 2;
                } else {
                    toks.push(Tok::Omega);
                    i += 1;
                }
            }
            '∇' => {
                toks.push(Tok::SelfRef);
                i += 1;
            }
            '←' => {
                // modified assignment: NAME +← expr — detect Prim Assign
                // and convert to ModifiedAssign(Prim)
                if let Some(Tok::Prim(p)) = toks.last() {
                    let p = *p;
                    toks.pop();
                    toks.push(Tok::ModifiedAssign(p));
                    i += 1;
                } else {
                    toks.push(Tok::Assign);
                    i += 1;
                }
            }
            '⍨' => {
                toks.push(Tok::Commute);
                i += 1;
            }
            '∘' => {
                // outer product: ∘.f — expect '.' then a prim symbol
                if chars.get(i + 1) == Some(&'.') {
                    // try to match a prim starting at i+2
                    let rest: String = chars[i + 2..].iter().collect();
                    let mut matched = false;
                    for (sym, prim) in PRIM_SYMBOLS {
                        if rest.starts_with(sym) {
                            toks.push(Tok::OuterDot(*prim));
                            i += 2 + sym.chars().count();
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        continue;
                    }
                }
                // matrix product: A∘B — equivalent to A+.×B
                toks.push(Tok::MatrixProduct);
                i += 1;
            }
            '⋄' => {
                toks.push(Tok::Diamond);
                i += 1;
            }
            '\'' => {
                // string literal; '' escapes a quote
                let mut s = Vec::new();
                i += 1;
                loop {
                    if i >= chars.len() {
                        return Err(ErrorCode::SyntaxError);
                    }
                    if chars[i] == '\'' {
                        if i + 1 < chars.len() && chars[i + 1] == '\'' {
                            s.push('\'' as Unicode);
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    s.push(chars[i] as Unicode);
                    i += 1;
                }
                i += 1; // closing quote
                toks.push(Tok::Str(s));
            }
            '¯' | '−' if i == 0 || !matches!(chars[i - 1], c if c.is_ascii_digit() || c == '.') => {
                // leading negative sign on a number (both ¯ and − are accepted)
                let (tok, len) = scan_number(&chars[i..])?;
                debug_assert!(len > 0);
                toks.push(match tok {
                    Some(t) => t,
                    None => return Err(ErrorCode::SyntaxError),
                });
                if matches!(toks.last(), Some(Tok::Prim(_))) || true {
                    // fallthrough — scan_number handles ¯ prefix
                }
                i += len;
            }
            c if c.is_ascii_digit() => {
                let (tok, len) = scan_number(&chars[i..])?;
                match tok {
                    Some(t) => toks.push(t),
                    None => return Err(ErrorCode::SyntaxError),
                };
                i += len;
            }
            '⎕' => {
                // quad name: ⎕IO, ⎕CT, ... — tokenized as a Name so it lives
                // in the ordinary vars table
                let mut name = String::from("⎕");
                i += 1;
                while i < chars.len()
                    && (chars[i].is_alphanumeric()
                        || chars[i] == '_'
                        || chars[i] == '.'
                        || chars[i] == '∆'
                        || chars[i] == '⍙')
                {
                    name.push(chars[i]);
                    i += 1;
                }
                toks.push(Tok::Name(name));
            }
            c if c.is_alphabetic() || c == '_' || c == '∆' || c == '⍙' => {
                // APL name: letters, digits, _, ∆, ⍙
                let mut name = String::new();
                while i < chars.len()
                    && (chars[i].is_alphanumeric()
                        || chars[i] == '_'
                        || chars[i] == '.'
                        || chars[i] == '∆'
                        || chars[i] == '⍙')
                {
                    name.push(chars[i]);
                    i += 1;
                }
                toks.push(Tok::Name(name));
            }
            _ => {
                // primitive symbol?
                let rest: String = chars[i..].iter().collect();
                let mut matched = false;
                for (sym, prim) in PRIM_SYMBOLS {
                    if rest.starts_with(sym) {
                        toks.push(Tok::Prim(*prim));
                        i += sym.chars().count();
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    // not a primitive itself — but might be `/` or `\`
                    // following an LO (reduce/scan operator syntax).
                    // A `/` after a VALUE (Num/Str/Name/RParen) is instead
                    // dyadic replicate (compress): 1/2 → 2, 0/2 → empty.
                    let after_value = matches!(
                        toks.last(),
                        Some(Tok::Num(_))
                            | Some(Tok::Str(_))
                            | Some(Tok::Name(_))
                            | Some(Tok::RParen)
                    );
                    if rest.starts_with('/') && !after_value {
                        match toks.last() {
                            Some(Tok::Prim(p)) => {
                                let p = *p;
                                toks.pop();
                                toks.push(Tok::Reduce(p));
                                i += 1;
                                continue;
                            }
                            _ => return Err(ErrorCode::SyntaxError),
                        }
                    }
                    if rest.starts_with('/') && after_value {
                        // dyadic replicate: push Replicate prim, consume '/'
                        toks.push(Tok::Prim(crate::functions::Prim::Replicate));
                        i += 1;
                        continue;
                    }
                    if rest.starts_with('⌿') {
                        match toks.last() {
                            Some(Tok::Prim(p)) => {
                                let p = *p;
                                toks.pop();
                                toks.push(Tok::Reduce1(p));
                                i += 1;
                                continue;
                            }
                            _ => return Err(ErrorCode::SyntaxError),
                        }
                    }
                    if rest.starts_with('\\') {
                        match toks.last() {
                            Some(Tok::Prim(p)) => {
                                let p = *p;
                                toks.pop();
                                toks.push(Tok::Scan(p));
                                i += 1;
                                continue;
                            }
                            _ => return Err(ErrorCode::SyntaxError),
                        }
                    }
                    if rest.starts_with('⍀') {
                        match toks.last() {
                            Some(Tok::Prim(p)) => {
                                let p = *p;
                                toks.pop();
                                toks.push(Tok::Scan1(p));
                                i += 1;
                                continue;
                            }
                            _ => return Err(ErrorCode::SyntaxError),
                        }
                    }
                    if rest.starts_with('.') {
                        // inner product f.g: PRIM '.' PRIM — e.g. +.×
                        if let Some(Tok::Prim(f)) = toks.last() {
                            let f = *f;
                            let rest2: String = chars[i + 1..].iter().collect();
                            for (sym, g) in PRIM_SYMBOLS {
                                if rest2.starts_with(sym) {
                                    toks.pop();
                                    toks.push(Tok::InnerDot(f, *g));
                                    i += 1 + sym.chars().count();
                                    continue 'outer;
                                }
                            }
                        }
                    }
                    if rest.starts_with('¨') {
                        match toks.last() {
                            Some(Tok::Prim(p)) => {
                                let p = *p;
                                toks.pop();
                                toks.push(Tok::Each(p));
                                i += 1;
                                continue;
                            }
                            Some(Tok::Name(n)) => {
                                let n = n.clone();
                                toks.pop();
                                toks.push(Tok::EachName(n));
                                i += 1;
                                continue;
                            }
                            _ => return Err(ErrorCode::SyntaxError),
                        }
                    }
                    // rank operator: PRIM⍤k — unlike ¨ it carries a numeric
                    // right operand, which the parser reads as the next token.
                    if rest.starts_with('⍤') {
                        match toks.last() {
                            Some(Tok::Prim(p)) => {
                                let p = *p;
                                toks.pop();
                                toks.push(Tok::Rank(p));
                                i += 1;
                                continue;
                            }
                            _ => return Err(ErrorCode::SyntaxError),
                        }
                    }
                    // power operator: PRIM⍣N — apply function N times
                    // Also handles named functions: NAME⍣N
                    if rest.starts_with('⍣') {
                        match toks.last() {
                            Some(Tok::Prim(p)) => {
                                let p = *p;
                                toks.pop();
                                toks.push(Tok::PowerOp(PowerFn::Prim(p)));
                                i += 1;
                                continue;
                            }
                            Some(Tok::Name(n)) => {
                                let n = n.clone();
                                toks.pop();
                                toks.push(Tok::PowerOp(PowerFn::Name(n)));
                                i += 1;
                                continue;
                            }
                            _ => return Err(ErrorCode::SyntaxError),
                        }
                    }
                    // zilde: ⍬ — the empty numeric vector
                    if rest.starts_with('⍬') {
                        toks.push(Tok::Zilde);
                        i += 1;
                        continue;
                    }
                    // over: ⍥ — Dyalog "over" operator (f⍥g)
                    #[cfg(feature = "unofficial-ext")]
                    if rest.starts_with('⍥') {
                        toks.push(Tok::Prim(crate::functions::Prim::Over));
                        i += 1;
                        continue;
                    }
                    return Err(ErrorCode::SyntaxError);
                }
            }
        }
    }

    toks.push(Tok::End);
    Ok(toks)
}

/// Scan a numeric literal starting at `chars[0]`.
///
/// Accepts `123`, `12.5`, `1e3`, `¯4` (negative), and exponent signs like
/// `1e¯3`. Returns `(token, chars_consumed)`.
fn scan_number(chars: &[char]) -> AplResult<(Option<Tok>, usize)> {
    let mut num = String::new();
    let mut i = 0;

    if chars[0] == '¯' || chars[0] == '−' {
        num.push('-');
        i += 1;
        if i >= chars.len() || !(chars[i].is_ascii_digit()) {
            return Ok((None, i));
        }
    }

    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            i += 1;
        } else if (c == 'e' || c == 'E')
            && i + 1 < chars.len()
            && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '¯' || chars[i + 1] == '−' || chars[i + 1] == '-')
        {
            num.push('e');
            i += 1;
            if chars[i] == '¯' || chars[i] == '−' || chars[i] == '-' {
                num.push('-');
                i += 1;
            }
        } else {
            break;
        }
    }

    // Complex number: reJim (e.g. 1J2, ¯3J¯4, −3J−4)
    if i < chars.len() && chars[i] == 'J' {
        let re_str = num.clone();
        let mut im_str = String::new();
        i += 1; // skip 'J'
        if i < chars.len() && (chars[i] == '¯' || chars[i] == '−') {
            im_str.push('-');
            i += 1;
        }
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() || c == '.' {
                im_str.push(c);
                i += 1;
            } else {
                break;
            }
        }
        if im_str.is_empty() || im_str == "-" {
            return Err(ErrorCode::SyntaxError);
        }
        let re = re_str.parse::<f64>().map_err(|_| ErrorCode::SyntaxError)?;
        let im = im_str.parse::<f64>().map_err(|_| ErrorCode::SyntaxError)?;
        return Ok((Some(Tok::Complex(re, im)), i));
    }

    if num.is_empty() {
        return Ok((None, 0));
    }

    match num.parse::<f64>() {
        Ok(v) => Ok((Some(Tok::Num(v)), i)),
        Err(_) => Err(ErrorCode::SyntaxError),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numbers() {
        let toks = tokenize("1 2 3").unwrap();
        assert!(toks.contains(&Tok::Num(1.0)));
        assert!(toks.contains(&Tok::Num(2.0)));
        assert!(toks.contains(&Tok::Num(3.0)));
    }

    #[test]
    fn test_negative_numbers() {
        let toks = tokenize("¯1 ¯2").unwrap();
        assert!(toks.contains(&Tok::Num(-1.0)));
        assert!(toks.contains(&Tok::Num(-2.0)));
    }

    #[test]
    fn test_float_exponent() {
        let toks = tokenize("1e3 1e¯3").unwrap();
        assert!(toks.contains(&Tok::Num(1000.0)));
        assert!(toks.contains(&Tok::Num(0.001)));
    }

    #[test]
    fn test_names() {
        let toks = tokenize("X Y Z").unwrap();
        assert!(toks.contains(&Tok::Name("X".to_string())));
        assert!(toks.contains(&Tok::Name("Y".to_string())));
        assert!(toks.contains(&Tok::Name("Z".to_string())));
    }

    #[test]
    fn test_delta_names() {
        let toks = tokenize("∆x ⍙y").unwrap();
        assert!(toks.contains(&Tok::Name("∆x".to_string())));
        assert!(toks.contains(&Tok::Name("⍙y".to_string())));
    }

    #[test]
    fn test_strings() {
        let toks = tokenize("'hello' 'world'").unwrap();
        assert!(toks.contains(&Tok::Str(vec![
            'h' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 'o' as u32
        ])));
    }

    #[test]
    fn test_escaped_quote() {
        let toks = tokenize("'it''s'").unwrap();
        assert!(toks.contains(&Tok::Str(vec![
            'i' as u32,
            't' as u32,
            '\'' as u32,
            's' as u32
        ])));
    }

    #[test]
    fn test_primitives() {
        let toks = tokenize("+ - × ÷").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Add)));
        assert!(toks.contains(&Tok::Prim(Prim::Subtract)));
        assert!(toks.contains(&Tok::Prim(Prim::Multiply)));
        assert!(toks.contains(&Tok::Prim(Prim::Divide)));
    }

    #[test]
    fn test_zilde() {
        let toks = tokenize("⍬").unwrap();
        assert!(toks.contains(&Tok::Zilde));
    }

    #[test]
    fn test_power_op() {
        // The power operator needs a function (primitive) before it
        let toks = tokenize("×⍣3").unwrap();
        assert!(toks.contains(&Tok::PowerOp(PowerFn::Prim(Prim::Multiply))));
    }

    #[test]
    fn test_comments() {
        let toks = tokenize("1 2 ⍝ this is a comment").unwrap();
        assert_eq!(toks.len(), 3); // Num(1), Num(2), End
    }

    #[test]
    fn test_quad_names() {
        let toks = tokenize("⎕IO ⎕CT ⎕PP").unwrap();
        assert!(toks.contains(&Tok::Name("⎕IO".to_string())));
        assert!(toks.contains(&Tok::Name("⎕CT".to_string())));
        assert!(toks.contains(&Tok::Name("⎕PP".to_string())));
    }

    #[test]
    fn test_na_tokenize() {
        let toks = tokenize("mydiv ⎕NA 'F8 libm.so.6|cdiv I4 I4'").unwrap();
        assert!(toks.contains(&Tok::Name("⎕NA".to_string())));
        // must have Name, Name(⎕NA), Str, End
        assert!(toks.len() >= 3);
    }

    #[test]
    fn test_na_parse() {
        let line = "mydiv ⎕NA 'F8 libm.so.6|cdiv I4 I4'";
        let toks = tokenize(line).unwrap();
        let (expr, used) = crate::parser::parse(&toks).unwrap();
        assert!(matches!(expr, crate::parser::Expr::QuadNa(Some(_), _)));
        assert_eq!(used, 3);
    }

    #[test]
    fn test_rank_op() {
        let toks = tokenize("⌽⍤1").unwrap();
        assert!(toks.contains(&Tok::Rank(Prim::Reverse)));
    }

    #[test]
    fn test_each_op() {
        // Each needs a preceding prim
        let toks = tokenize("×¨").unwrap();
        assert!(toks.contains(&Tok::Each(Prim::Multiply)));
    }

    #[test]
    fn test_each_op_named() {
        // Each with a named function
        let toks = tokenize("f¨").unwrap();
        assert!(toks.contains(&Tok::EachName("f".to_string())));
    }

    #[test]
    fn test_modified_assignment_token() {
        // V +← 10: the +← should become ModifiedAssign(Add)
        let toks = tokenize("V+←10").unwrap();
        assert!(toks.contains(&Tok::ModifiedAssign(Prim::Add)));
    }

    #[test]
    fn test_scan_op() {
        let toks = tokenize("+\\").unwrap();
        assert!(toks.contains(&Tok::Scan(Prim::Add)));
    }

    #[test]
    fn test_reduce_op() {
        let toks = tokenize("+/").unwrap();
        assert!(toks.contains(&Tok::Reduce(Prim::Add)));
    }

    #[test]
    fn test_inner_product() {
        let toks = tokenize("+.×").unwrap();
        assert!(toks.contains(&Tok::InnerDot(Prim::Add, Prim::Multiply)));
    }

    #[test]
    fn test_outer_product() {
        let toks = tokenize("∘.×").unwrap();
        assert!(toks.contains(&Tok::OuterDot(Prim::Multiply)));
    }

    #[test]
    fn test_diamond() {
        let toks = tokenize("⋄").unwrap();
        assert!(toks.contains(&Tok::Diamond));
    }

    #[test]
    fn test_commute() {
        let toks = tokenize("⍨").unwrap();
        assert!(toks.contains(&Tok::Commute));
    }

    #[test]
    fn test_assign() {
        let toks = tokenize("X←5").unwrap();
        assert!(toks.contains(&Tok::Name("X".to_string())));
        assert!(toks.contains(&Tok::Assign));
        assert!(toks.contains(&Tok::Num(5.0)));
    }

    #[test]
    fn test_parens() {
        let toks = tokenize("(1+2)").unwrap();
        assert!(toks.contains(&Tok::LParen));
        assert!(toks.contains(&Tok::RParen));
    }

    #[test]
    fn test_brackets() {
        let toks = tokenize("M[1]").unwrap();
        assert!(toks.contains(&Tok::LBracket));
        assert!(toks.contains(&Tok::RBracket));
    }

    #[test]
    fn test_semicolon() {
        let toks = tokenize("M[1;2]").unwrap();
        assert!(toks.contains(&Tok::Semicolon));
    }

    #[test]
    fn test_alpha_omega() {
        let toks = tokenize("⍺ ⍵").unwrap();
        assert!(toks.contains(&Tok::Alpha));
        assert!(toks.contains(&Tok::Omega));
    }

    #[test]
    fn test_alphaalpha_omegaomega() {
        let toks = tokenize("⍺⍺ ⍵⍵").unwrap();
        assert!(toks.contains(&Tok::AlphaAlpha));
        assert!(toks.contains(&Tok::OmegaOmega));
    }

    #[test]
    fn test_selfref() {
        let toks = tokenize("∇").unwrap();
        assert!(toks.contains(&Tok::SelfRef));
    }

    #[test]
    fn test_natlog() {
        let toks = tokenize("⍟").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::NatLog)));
    }

    #[test]
    fn test_without() {
        let toks = tokenize("∼").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Without)));
    }

    #[test]
    fn test_union() {
        let toks = tokenize("∪").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Union)));
    }

    #[test]
    fn test_inter() {
        let toks = tokenize("∩").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Inter)));
    }

    #[test]
    fn test_comma1() {
        let toks = tokenize("⍪").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Comma1)));
    }

    #[test]
    fn test_notmatch() {
        let toks = tokenize("≢").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::NotMatch)));
    }

    #[test]
    fn test_left_right() {
        let toks = tokenize("⊣ ⊢").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Left)));
        assert!(toks.contains(&Tok::Prim(Prim::Right)));
    }

    #[test]
    fn test_nand_nor() {
        let toks = tokenize("⍲ ⍱").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Nand)));
        assert!(toks.contains(&Tok::Prim(Prim::Nor)));
    }

    #[test]
    fn test_squad() {
        let toks = tokenize("⌷").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Squad)));
    }

    #[test]
    fn test_rotate1() {
        let toks = tokenize("⊖").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Rotate1)));
    }

    #[test]
    fn test_format() {
        let toks = tokenize("⍕").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Format)));
    }

    #[test]
    fn test_where() {
        let toks = tokenize("⍸").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Where)));
    }

    #[test]
    fn test_execute() {
        let toks = tokenize("⍎").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Execute)));
    }

    #[test]
    fn test_find() {
        let toks = tokenize("⍷").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Find)));
    }

    #[test]
    fn test_partition() {
        let toks = tokenize("⊆").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Partition)));
    }

    #[test]
    fn test_take_drop() {
        let toks = tokenize("↑ ↓").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Take)));
        assert!(toks.contains(&Tok::Prim(Prim::Drop)));
    }

    #[test]
    fn test_reverse() {
        let toks = tokenize("⌽").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Reverse)));
    }

    #[test]
    fn test_grade() {
        let toks = tokenize("⍋ ⍒").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::GradeUp)));
        assert!(toks.contains(&Tok::Prim(Prim::GradeDown)));
    }

    #[test]
    fn test_epsilon() {
        let toks = tokenize("∈ ∊").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Epsilon)));
    }

    #[test]
    fn test_enclose_disclose() {
        let toks = tokenize("⊂ ⊃").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Enclose)));
        assert!(toks.contains(&Tok::Prim(Prim::Disclose)));
    }

    #[test]
    fn test_encode_decode() {
        let toks = tokenize("⊤ ⊥").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Encode)));
        assert!(toks.contains(&Tok::Prim(Prim::Decode)));
    }

    #[test]
    fn test_depth() {
        let toks = tokenize("≡").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Depth)));
    }

    #[test]
    fn test_comparison() {
        let toks = tokenize("< ≤ = ≥ > ≠").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Less)));
        assert!(toks.contains(&Tok::Prim(Prim::LessEq)));
        assert!(toks.contains(&Tok::Prim(Prim::Equal)));
        assert!(toks.contains(&Tok::Prim(Prim::GreaterEq)));
        assert!(toks.contains(&Tok::Prim(Prim::Greater)));
        assert!(toks.contains(&Tok::Prim(Prim::NotEqual)));
    }

    #[test]
    fn test_not() {
        let toks = tokenize("~").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Not)));
    }

    #[test]
    fn test_branch() {
        let toks = tokenize("→").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Branch)));
    }

    #[test]
    fn test_transpose() {
        let toks = tokenize("⍉").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Transpose)));
    }

    #[test]
    fn test_domino() {
        let toks = tokenize("⌹").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Domino)));
    }

    #[test]
    fn test_and_or() {
        let toks = tokenize("∧ ∨").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::And)));
        assert!(toks.contains(&Tok::Prim(Prim::Or)));
    }

    #[test]
    fn test_power() {
        let toks = tokenize("*").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Power)));
    }

    #[test]
    fn test_exponential() {
        let toks = tokenize("⋆").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Exponential)));
    }

    #[test]
    fn test_pitimes() {
        let toks = tokenize("○").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::PiTimes)));
    }

    #[test]
    fn test_magnitude() {
        let toks = tokenize("∣").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Magnitude)));
    }

    #[test]
    fn test_roll() {
        let toks = tokenize("?").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Roll)));
    }

    #[test]
    fn test_factorial() {
        let toks = tokenize("!").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Factorial)));
    }

    #[test]
    fn test_ceiling_floor() {
        let toks = tokenize("⌈ ⌊").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Ceiling)));
        assert!(toks.contains(&Tok::Prim(Prim::Floor)));
    }

    #[test]
    fn test_iota() {
        let toks = tokenize("⍳").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Iota)));
    }

    #[test]
    fn test_rho() {
        let toks = tokenize("⍴").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Rho)));
    }

    #[test]
    fn test_comma() {
        let toks = tokenize(",").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Comma)));
    }

    #[test]
    fn test_add() {
        let toks = tokenize("+").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Add)));
    }

    #[test]
    fn test_subtract() {
        let toks = tokenize("-").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Subtract)));
    }

    #[test]
    fn test_multiply() {
        let toks = tokenize("×").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Multiply)));
    }

    #[test]
    fn test_divide() {
        let toks = tokenize("÷").unwrap();
        assert!(toks.contains(&Tok::Prim(Prim::Divide)));
    }
}
