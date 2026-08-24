//! ⎕NA declaration parser — Dyalog-compatible grammar → CAbiSpec.
//!
//! Grammar (subset accepted; matches Dyalog 19.0):
//!
//! ```text
//! decl      := [result] [pathname '|'] symbol arg*
//! result    := typespec          (optional; absent = shy nil result)
//! pathname  := path chars up to the LAST '|'
//! arg       := typespec
//! typespec  := [dir] [special] type [width] [array]
//! dir       := '<' | '>' | '='   (default: by-value)
//! special   := '0' | '#'         (NUL-terminated / byte-counted string)
//! type      := I|U|C|T|F|D|J|P|A|Z|∇ | UTF
//! width     := 1|2|4|8|16        (validated per type)
//! array     := '[' [int] ']'     ([] = length at call time)
//! structure := '{' typespec+ '}' [array]
//! count     := trailing '[n]' on an arg may declare "next N items use this"
//! ```

use crate::types::ErrorCode;

/// one leaf field of a declaration (scalar, or member of a struct)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafType {
    Int,
    UInt,
    Char,
    TransChar,
    Float,
    Decimal,
    Complex,
    /// uintptr_t — platform word size
    UintPtr,
    AplArray,
    AplArrayHeader,
    FuncPointer,
    Utf8,
    Utf16,
}

/// width in bytes after validation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    W1,
    W2,
    W4,
    W8,
    W16,
    /// type has no explicit width (P, A, Z, ∇)
    None,
}

impl Width {
    pub fn bytes(self) -> Option<u64> {
        match self {
            Width::W1 => Some(1),
            Width::W2 => Some(2),
            Width::W4 => Some(4),
            Width::W8 => Some(8),
            Width::W16 => Some(16),
            Width::None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// by-value scalar (no direction marker)
    Value,
    In,
    Out,
    InOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    None,
    /// '0' — null-terminated
    NulTerm,
    /// '#' — byte-counted
    ByteCounted,
}

/// one complete argument/result descriptor
#[derive(Clone, Debug, PartialEq)]
pub struct TypeSpec {
    pub dir: Direction,
    pub special: Special,
    pub leaf: LeafType,
    pub width: Width,
    /// Some(n) fixed array length; Some(0)... no — [] parses to None-with-
    /// is_array; we use: array: false => scalar, array_len: Some(n) => fixed,
    /// array_open => [] runtime-length
    pub array_len: Option<u64>,
    pub array_open: bool,
    /// struct members when leaf is a composite (see ArgSpec below)
    pub members: Vec<TypeSpec>,
    pub is_struct: bool,
}

impl TypeSpec {
    fn plain_scalar() -> TypeSpec {
        TypeSpec {
            dir: Direction::Value,
            special: Special::None,
            leaf: LeafType::Int,
            width: Width::W4,
            array_len: None,
            array_open: false,
            members: Vec::new(),
            is_struct: false,
        }
    }
}

/// a full ⎕NA declaration
#[derive(Clone, Debug, PartialEq)]
pub struct CAbiSpec {
    /// None = void/shy-nil result
    pub result: Option<TypeSpec>,
    /// library path (before the last '|'); empty = OS search order
    pub library: String,
    /// symbol name inside the library
    pub symbol: String,
    /// positional arguments
    pub args: Vec<TypeSpec>,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

type PResult<T> = Result<T, ErrorCode>;

impl<'a> P<'a> {
    fn new(s: &'a str) -> Self {
        P {
            s: s.as_bytes(),
            i: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t')) {
            self.i += 1;
        }
    }

    fn at_end(&self) -> bool {
        self.i >= self.s.len()
    }

    /// parse an unsigned integer literal
    fn uint(&mut self) -> PResult<Option<u64>> {
        let start = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if start == self.i {
            return Ok(None);
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()
            .and_then(|t| t.parse().ok())
            .map(Some)
            .ok_or(ErrorCode::SyntaxError)
    }

    /// parse one typespec (leaf or struct), used for args and results
    fn typespec(&mut self, allow_result_only: bool) -> PResult<TypeSpec> {
        let mut ts = TypeSpec::plain_scalar();
        // direction
        match self.peek() {
            Some(b'<') => {
                ts.dir = Direction::In;
                self.bump();
            }
            Some(b'>') => {
                ts.dir = Direction::Out;
                self.bump();
            }
            Some(b'=') => {
                ts.dir = Direction::InOut;
                self.bump();
            }
            _ => {}
        }
        // special
        match self.peek() {
            Some(b'0') => {
                ts.special = Special::NulTerm;
                self.bump();
            }
            Some(b'#') => {
                ts.special = Special::ByteCounted;
                self.bump();
            }
            _ => {}
        }
        // struct?
        if self.peek() == Some(b'{') {
            self.bump();
            ts.is_struct = true;
            ts.leaf = LeafType::Int; // unused marker
            loop {
                self.skip_ws();
                if self.peek() == Some(b'}') {
                    self.bump();
                    break;
                }
                if self.at_end() {
                    return Err(ErrorCode::SyntaxError);
                }
                let m = self.typespec(false)?;
                ts.members.push(m);
            }
        } else {
            // leaf type name (longest first: UTF)
            let rest = &self.s[self.i..];
            if rest.starts_with(b"UTF") {
                self.i += 3;
                match self.peek() {
                    Some(b'8') => {
                        ts.leaf = LeafType::Utf8;
                        ts.width = Width::None;
                        self.bump();
                    }
                    Some(b'1') if rest.starts_with(b"UTF16") => {
                        ts.leaf = LeafType::Utf16;
                        ts.width = Width::None;
                        self.i += 2;
                    }
                    _ => return Err(ErrorCode::SyntaxError),
                }
            } else {
                let c = self.bump().ok_or(ErrorCode::SyntaxError)?;
                ts.leaf = match c.to_ascii_uppercase() {
                    b'I' => LeafType::Int,
                    b'U' => LeafType::UInt,
                    b'C' => LeafType::Char,
                    b'T' => LeafType::TransChar,
                    b'F' => LeafType::Float,
                    b'D' => LeafType::Decimal,
                    b'J' => LeafType::Complex,
                    b'P' => LeafType::UintPtr,
                    b'A' => LeafType::AplArray,
                    b'Z' => LeafType::AplArrayHeader,
                    // ∇ arrives as UTF-8 E2 88 87 — first byte 0xE2
                    0xE2 => LeafType::FuncPointer,
                    _ => {
                        // ∇ is 3 bytes E2 88 87 — back up and try again
                        if self.s[self.i - 1..].starts_with(&[0xE2, 0x88, 0x87][..]) {
                            self.i += 2;
                            LeafType::FuncPointer
                        } else {
                            return Err(ErrorCode::SyntaxError);
                        }
                    }
                };
                // width
                if let Some(w) = self.uint()? {
                    ts.width = match w {
                        1 => Width::W1,
                        2 => Width::W2,
                        4 => Width::W4,
                        8 => Width::W8,
                        16 => Width::W16,
                        _ => return Err(ErrorCode::SyntaxError),
                    };
                } else {
                    let leaf = ts.leaf;
                    ts.width = default_width(leaf);
                }
            }
        }
        // array suffix
        if self.peek() == Some(b'[') {
            self.bump();
            match self.uint()? {
                Some(n) => ts.array_len = Some(n),
                None => ts.array_open = true,
            }
            if self.bump() != Some(b']') {
                return Err(ErrorCode::SyntaxError);
            }
        }
        validate(&ts, allow_result_only)?;
        Ok(ts)
    }

    /// parse a whole declaration string
    fn decl(&mut self) -> PResult<CAbiSpec> {
        let mut spec = CAbiSpec {
            result: None,
            library: String::new(),
            symbol: String::new(),
            args: Vec::new(),
        };

        // Tokenize into whitespace-separated words, but keep '{...}' groups
        // intact (structs contain spaces).
        let words = self.split_words()?;
        if words.is_empty() {
            return Err(ErrorCode::SyntaxError);
        }

        let mut wi = 0usize;

        // optional leading result: a word that is NOT followed-by/containing
        // '|' and whose parse succeeds as a typespec... ambiguity! Dyalog's
        // rule: the FIRST word is the result if there are ≥2 words AND the
        // word containing '|' comes later OR the first word parses as a pure
        // typespec and the next word contains '|'. We follow: find the word
        // containing '|'; everything before it that isn't part of it is
        // result (if exactly one such word) else error.
        let pipe_pos = words.iter().position(|w| w.contains('|'));

        match pipe_pos {
            None => {
                // no library — symbol is word[0] unless word count suggests
                // result+symbol+args. With no pipe: word[0]=symbol? No —
                // Dyalog allows omitting the library entirely only via bare
                // symbol when a previous association exists; we require
                // format: [result] symbol args*. If words[0] parses as a
                // typespec AND words.len()>1 AND words[0] has no lowercase
                // letters beyond type codes... simplest robust rule:
                // if words.len() >= 2 and words[0] is not a valid symbol
                // (contains type-code-only chars) treat as result.
                if words.len() >= 2 && looks_like_typespec(&words[0]) {
                    let mut p = P::new(&words[0]);
                    spec.result = Some(p.typespec(true)?);
                    p.finish()?;
                    wi = 1;
                }
                if wi < words.len() {
                    spec.symbol = words[wi].clone();
                    wi += 1;
                } else {
                    return Err(ErrorCode::SyntaxError);
                }
            }
            Some(pp) => {
                // split the piped word into lib|symbol (last '|' wins per doc)
                let w = &words[pp];
                let cut = w.rfind('|').unwrap();
                let lib_part = &w[..cut];
                let sym_part = &w[cut + 1..];
                if sym_part.is_empty() {
                    return Err(ErrorCode::SyntaxError);
                }
                spec.library = lib_part.to_string();
                spec.symbol = sym_part.to_string();
                // any words BEFORE pp form the result (must be exactly ≤1)
                if pp == 1 {
                    let mut p = P::new(&words[0]);
                    spec.result = Some(p.typespec(true)?);
                    p.finish()?;
                } else if pp > 1 {
                    return Err(ErrorCode::SyntaxError);
                }
                wi = pp + 1;
            }
        }

        // no-pipe form may still carry a dotted symbol: lib.symbol
        if spec.library.is_empty() {
            if let Some(dot) = spec.symbol.rfind('.') {
                let lib = spec.symbol[..dot].to_string();
                let sym = spec.symbol[dot + 1..].to_string();
                spec.library = lib;
                spec.symbol = sym;
            }
        }

        // remaining words are args (a word may be a struct group)
        while wi < words.len() {
            let mut p = P::new(&words[wi]);
            let ts = p.typespec(false)?;
            p.finish()?;
            spec.args.push(ts);
            wi += 1;
        }

        // sanity: max 12 args (LIMIT ERROR territory; keep simple SyntaxError here)
        if spec.args.len() > 12 {
            return Err(ErrorCode::SyntaxError);
        }
        Ok(spec)
    }

    fn finish(&self) -> PResult<()> {
        if !self.at_end() {
            return Err(ErrorCode::SyntaxError);
        }
        Ok(())
    }

    /// split on whitespace but keep {...} groups (which may contain spaces)
    /// as single words.
    fn split_words(&mut self) -> PResult<Vec<String>> {
        let mut words = Vec::new();
        let mut cur = String::new();
        let mut depth = 0i32;
        let bytes = self.s;
        let mut k = 0usize;
        while k < bytes.len() {
            let c = bytes[k];
            match c {
                b'{' => {
                    depth += 1;
                    cur.push(c as char);
                }
                b'}' => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(ErrorCode::SyntaxError);
                    }
                    cur.push(c as char);
                }
                b' ' | b'\t' if depth == 0 => {
                    if !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                    }
                }
                _ => {
                    // multi-byte UTF-8 (∇) — push raw bytes losslessly
                    if c >= 0x80 {
                        let start = k;
                        let len = utf8_len(c);
                        let end = (start + len).min(bytes.len());
                        cur.push_str(&String::from_utf8_lossy(&bytes[start..end]));
                        k = end - 1;
                    } else {
                        cur.push(c as char);
                    }
                }
            }
            k += 1;
        }
        if depth != 0 {
            return Err(ErrorCode::SyntaxError);
        }
        if !cur.is_empty() {
            words.push(cur);
        }
        self.i = bytes.len();
        Ok(words)
    }
}

fn utf8_len(first: u8) -> usize {
    if first >= 0xF0 {
        4
    } else if first >= 0xE0 {
        3
    } else if first >= 0xC0 {
        2
    } else {
        1
    }
}

fn looks_like_typespec(w: &str) -> bool {
    // a result typespec: optional dir/special, type letter(s), digits, [] —
    // never contains '|' and never contains characters illegal in symbols
    // beyond the typespec alphabet. Conservative: all chars in the set.
    !w.contains('|')
        && w.chars().all(|c| {
            matches!(
                c,
                '<' | '>'
                    | '='
                    | '0'
                    | '#'
                    | '['
                    | ']'
                    | 'I'
                    | 'U'
                    | 'C'
                    | 'T'
                    | 'F'
                    | 'D'
                    | 'J'
                    | 'P'
                    | 'A'
                    | 'Z'
                    | 'i'
                    | 'u'
                    | 'c'
                    | 't'
                    | 'f'
                    | 'd'
                    | 'j'
                    | 'p'
                    | 'a'
                    | 'z'
                    | '0'..='9'
            )
        })
}

fn default_width(leaf: LeafType) -> Width {
    match leaf {
        LeafType::Int | LeafType::UInt => Width::W4,
        LeafType::Char => Width::W1,
        LeafType::TransChar => Width::W4, // wide char on Linux
        LeafType::Float => Width::W8,
        LeafType::Decimal | LeafType::Complex => Width::W16,
        LeafType::UintPtr
        | LeafType::AplArray
        | LeafType::AplArrayHeader
        | LeafType::FuncPointer => Width::None,
        LeafType::Utf8 | LeafType::Utf16 => Width::None,
    }
}

/// post-parse validation: legal type/width/direction combos
fn validate(ts: &TypeSpec, allow_result_only: bool) -> PResult<()> {
    if ts.is_struct {
        if ts.special != Special::None {
            return Err(ErrorCode::DomainError);
        }
        for m in &ts.members {
            validate(m, false)?;
        }
        return Ok(());
    }
    // D16 unsupported in v1
    if ts.leaf == LeafType::Decimal {
        return Err(ErrorCode::DomainError);
    }
    // ∇ callbacks deferred to v2
    if ts.leaf == LeafType::FuncPointer {
        return Err(ErrorCode::DomainError);
    }
    // results may not be Out
    if allow_result_only && ts.dir == Direction::Out {
        return Err(ErrorCode::DomainError);
    }
    // width legality per type
    let legal: &[Width] = match ts.leaf {
        LeafType::Int | LeafType::UInt => &[Width::W1, Width::W2, Width::W4, Width::W8],
        LeafType::Char => &[Width::W1, Width::W2, Width::W4],
        LeafType::TransChar => &[Width::W1, Width::W2, Width::W4],
        LeafType::Float => &[Width::W4, Width::W8],
        LeafType::Complex => &[Width::W16],
        LeafType::UintPtr
        | LeafType::AplArray
        | LeafType::AplArrayHeader
        | LeafType::FuncPointer => &[Width::None],
        LeafType::Utf8 => &[Width::None],
        LeafType::Utf16 => &[Width::None],
        LeafType::Decimal => unreachable!("rejected above"),
    };
    if !legal.contains(&ts.width) {
        return Err(ErrorCode::DomainError);
    }
    // specials only make sense with pointer-ish directions
    if ts.special != Special::None && ts.dir == Direction::Value {
        return Err(ErrorCode::DomainError);
    }
    Ok(())
}

/// Public entry: parse a full ⎕NA right-argument string.
pub fn parse_na_decl(src: &str) -> Result<CAbiSpec, ErrorCode> {
    let mut p = P::new(src.trim());
    let spec = p.decl()?;
    Ok(spec)
}
