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
    /// statement separator `⋄` (diamond)
    Diamond,
    /// outer product: `A ∘.f B`
    OuterDot(Prim),
    /// inner product: `A f.g B`
    InnerDot(Prim, Prim),
    /// commute operator `⍨`
    Commute,
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
    /// dfn left brace `{`
    LBrace,
    /// dfn right brace `}`
    RBrace,
    /// dfn left argument `⍺`
    Alpha,
    /// dfn right argument `⍵`
    Omega,
    /// end of input
    End,
}

/// Primitive symbol table: single-char APL glyphs → Prim.
const PRIM_SYMBOLS: &[(&str, Prim)] = &[
    ("+", Prim::Add),
    ("-", Prim::Subtract),
    ("×", Prim::Multiply),
    ("÷", Prim::Divide),
    ("!", Prim::Factorial),
    ("⌈", Prim::Ceiling),
    ("⌊", Prim::Floor),
    ("⍳", Prim::Iota),
    ("⍴", Prim::Rho),
    ("⋆", Prim::Exponential),
    ("○", Prim::PiTimes),
    ("∣", Prim::Magnitude),
    ("↑", Prim::Take),
    ("↓", Prim::Drop),
    ("⌽", Prim::Reverse),
    ("⍋", Prim::GradeUp),
    ("⍒", Prim::GradeDown),
    ("∈", Prim::Epsilon),
    ("∊", Prim::Epsilon), // SMALL ELEMENT OF — alias used by GNU APL
    ("⊂", Prim::Enclose),
    ("⊃", Prim::Disclose),
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
            '{' => {
                toks.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                toks.push(Tok::RBrace);
                i += 1;
            }
            '⍺' => {
                toks.push(Tok::Alpha);
                i += 1;
            }
            '⍵' => {
                toks.push(Tok::Omega);
                i += 1;
            }
            '←' => {
                toks.push(Tok::Assign);
                i += 1;
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
                return Err(ErrorCode::SyntaxError);
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
            '¯' if i == 0 || !matches!(chars[i - 1], c if c.is_ascii_digit() || c == '.') => {
                // leading negative sign on a number
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
                }
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
                            _ => return Err(ErrorCode::SyntaxError),
                        }
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

    if chars[0] == '¯' {
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
            && !num.contains('e')
            && !num.contains('E')
            && i + 1 < chars.len()
        {
            let next = chars[i + 1];
            if next.is_ascii_digit() || next == '¯' {
                num.push('e');
                i += 1;
                if next == '¯' {
                    num.push('-');
                    i += 1;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    let v: f64 = num.parse().map_err(|_| ErrorCode::SyntaxError)?;
    Ok((Some(Tok::Num(v)), i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inner_dot_tokenize() {
        let toks = tokenize("1 0 1∧.=1 1 1").unwrap();
        println!("{:?}", toks);
        assert!(toks.contains(&Tok::InnerDot(
            crate::functions::Prim::And,
            crate::functions::Prim::Equal
        )));
    }
}
