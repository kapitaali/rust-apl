//! Primitive APL functions.
//!
//! Implements monadic and dyadic primitives operating on whole values,
//! mirroring `src/ScalarFunction.cc` and parts of `src/Bif_F12_*.cc`.

use crate::cell::{self, Cell};
use crate::shape::Shape;
use crate::types::ErrorCode;
use crate::value::ValueP;

/// Which primitive a token refers to (grows as functions are ported).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    Add,
    Subtract,
    Multiply,
    Divide,
    Neg,
    Factorial,
    Ceiling,
    Floor,
    Iota,
    Rho,
    Exponential,
    NatLog,
    Reciprocal,
    Magnitude,
    Direction,
    PiTimes,
    PiTimesInverse,
    Conjugate,
    Roll,
    Power,
    Take,
    Drop,
    Reverse,
    Rotate,
    GradeUp,
    GradeDown,
    Epsilon,
    Transpose,
    Enclose,
    Disclose,
    Depth,
    LessEq,
    Less,
    Equal,
    GreaterEq,
    Greater,
    NotEqual,
    Not,
    Branch,
    Replicate,
    Domino,
    And,
    Or,
    Comma,
    Encode,    // ⊤ — dyadic: A⊤B (representation)
    Decode,    // ⊥ — dyadic: A⊥B (base value)
    Without,   // ∼ — dyadic: A∼B (set difference)
    Union,     // ∪ — monadic: unique, dyadic: A∪B (union)
    Inter,     // ∩ — dyadic: A∩B (intersection)
    Comma1,    // ⍪ — monadic: table, dyadic: catenate first axis
    NotMatch,  // ≢ — monadic: tally, dyadic: not match
    Left,      // ⊣ — monadic: identity, dyadic: A
    Right,     // ⊢ — monadic: identity, dyadic: B
    Nand,      // ⍲ — dyadic: not and
    Nor,       // ⍱ — dyadic: not or
    Squad,     // ⌷ — dyadic: general index
    Rotate1,   // ⊖ — monadic: reverse first axis, dyadic: rotate first axis
    Format,    // ⍕ — monadic: format, dyadic: width/decimals format
    Where,     // ⍸ — monadic: indices of 1s in a boolean array
    Execute,   // ⍎ — monadic: evaluate a character vector as APL
    Find,      // ⍷ — dyadic: A⍷B locates occurrences of A within B
    Partition, // ⊆ — dyadic: A⊆B groups items of B into partitions
}

impl Prim {
    pub fn from_symbol(sym: &str) -> Option<Prim> {
        Some(match sym {
            "+" => Prim::Add,
            "-" => Prim::Subtract,
            "×" => Prim::Multiply,
            "÷" => Prim::Divide,
            "!" => Prim::Factorial,
            "⌈" => Prim::Ceiling,
            "⌊" => Prim::Floor,
            "⍳" => Prim::Iota,
            "⌹" => Prim::Domino,
            "∧" => Prim::And,
            "∨" => Prim::Or,
            "⍴" => Prim::Rho,
            "," | "¸" => Prim::Comma,
            "*" | "⋆" => Prim::Exponential,
            "○" => Prim::PiTimes,
            "⍟" => Prim::NatLog,
            "∣" | "|" => Prim::Magnitude,
            "?" => Prim::Roll,      // ? = roll
            "∼" => Prim::Without,   // ∼ = without (set difference)
            "∪" => Prim::Union,     // ∪ = union (unique / set union)
            "∩" => Prim::Inter,     // ∩ = intersection
            "⍪" => Prim::Comma1,    // ⍪ = table / catenate first axis
            "≢" => Prim::NotMatch,  // ≢ = tally / not match
            "⊣" => Prim::Left,      // ⊣ = left (identity / A)
            "⊢" => Prim::Right,     // ⊢ = right (identity / B)
            "⍲" => Prim::Nand,      // ⍲ = not and
            "⍱" => Prim::Nor,       // ⍱ = not or
            "⌷" => Prim::Squad,     // ⌷ = general index
            "⊖" => Prim::Rotate1,   // ⊖ = reverse/rotate first axis
            "⍕" => Prim::Format,    // ⍕ = format
            "⍸" => Prim::Where,     // ⍸ = where (indices of 1s)
            "⍎" => Prim::Execute,   // ⍎ = execute (evaluate char vector)
            "⍷" => Prim::Find,      // ⍷ = find (locate subarray)
            "⊆" => Prim::Partition, // ⊆ = partition (group items)
            "~" => Prim::Not,       // ~ (ASCII tilde) = logical not
            "↑" => Prim::Take,
            "↓" => Prim::Drop,
            "⌽" => Prim::Reverse,
            "⍋" => Prim::GradeUp,
            "∈" => Prim::Epsilon,
            "⍉" => Prim::Transpose,
            "⊂" => Prim::Enclose,
            "⊃" => Prim::Disclose,
            "⊤" => Prim::Encode,
            "⊥" => Prim::Decode,
            "≡" => Prim::Depth,
            "≤" => Prim::LessEq,
            "<" => Prim::Less,
            "=" => Prim::Equal,
            "≥" => Prim::GreaterEq,
            ">" => Prim::Greater,
            "≠" => Prim::NotEqual,
            "→" => Prim::Branch,
            _ => return None,
        })
    }

    /// apply monadically: `f B`
    pub fn eval_monadic(self, b: &ValueP) -> Result<ValueP, ErrorCode> {
        match self {
            // ── arithmetic ────────────────────────────────────────────────
            Prim::Add => map_cells(b, cell::bif_conjugate),
            Prim::Subtract => map_cells(b, cell::bif_negative),
            Prim::Divide => map_cells(b, cell::bif_reciprocal),
            // ⋆B / *B — exponential (e to the B)
            Prim::Exponential => map_cells(b, cell::bif_exponential),
            // *B is tokenized as Power (it is dyadic-power's glyph), but
            // MONADIC * is exponential — without this arm `*1` was a SYNTAX
            // ERROR while `2*10` worked.
            Prim::Power => map_cells(b, cell::bif_exponential),
            Prim::NatLog => map_cells(b, cell::bif_nat_log),
            Prim::Ceiling => map_cells(b, cell::bif_ceiling),
            Prim::Floor => map_cells(b, cell::bif_floor),
            Prim::Magnitude => map_cells(b, cell::bif_magnitude),
            Prim::Direction => map_cells(b, cell::bif_direction),
            Prim::Conjugate => map_cells(b, cell::bif_conjugate),
            Prim::PiTimes => map_cells(b, cell::bif_pi_times),
            Prim::PiTimesInverse => map_cells(b, cell::bif_pi_times_inverse),
            Prim::Factorial => map_cells(b, cell::bif_factorial),

            // ×B = direction (signum) in APL
            Prim::Multiply => map_cells(b, cell::bif_direction),

            // ⌽B — reverse (last axis)
            Prim::Reverse => crate::rotate::reverse(b),

            // ⍉B — transpose
            Prim::Transpose => crate::transpose::transpose(b),

            // ⌹B — matrix inverse
            Prim::Domino => crate::domino::domino_monadic(b),

            // ~B — not (logical): 1−B elementwise
            Prim::Not => {
                let cells = b.cells();
                let out: Result<Vec<Cell>, ErrorCode> = cells
                    .iter()
                    .map(|c| match c {
                        Cell::Int(v) => Ok(Cell::Int(1 - v)),
                        Cell::Float(f) => Ok(Cell::Int(1 - *f as i64)),
                        _ => Err(ErrorCode::DomainError),
                    })
                    .collect();
                Ok(ValueP::from_ravel_like(b, out?))
            }

            // ⊂B — enclose
            Prim::Enclose => crate::enclose::enclose(b),

            // ∪B — unique (monadic)
            Prim::Union => crate::union::unique(b),

            // ⊃B — disclose
            Prim::Disclose => crate::enclose::disclose(b),

            // ∊B — enlist
            Prim::Epsilon => crate::enlist::enlist(b),

            // ≡B — depth
            Prim::Depth => crate::depth::depth(b),

            // ,B — ravel
            Prim::Comma => crate::comma::ravel(b),

            // ↑B — disclose (pick first/nested value out of a pointer scalar)
            Prim::Take if b.first_cell().map(|c| c.is_pointer_cell()).unwrap_or(false) => {
                Ok(b.disclose())
            }

            // ⍋B / ⍒B — grade up/down
            Prim::GradeUp => crate::sort::grade_up(b),
            Prim::GradeDown => crate::sort::grade_down(b),

            // ⍪B — table: turn B into a matrix (1-column for vectors)
            Prim::Comma1 => crate::comma1::table(b),

            // ≢B — tally: number of elements in B (rank-1 length)
            Prim::NotMatch => crate::comma1::tally(b),

            // ⊣B — left: identity (returns B unchanged)
            Prim::Left => Ok(b.clone()),

            // ⊢B — right: identity (returns B unchanged)
            Prim::Right => Ok(b.clone()),

            // ⊖B — reverse first axis
            Prim::Rotate1 => crate::squad::reverse_first(b),
            Prim::Format => crate::format::format(b),
            Prim::Where => crate::format::where_indices(b),
            // ⍎B needs &mut Environment to evaluate, so the parser's Monadic
            // arm intercepts it before reaching here. Hitting this arm means
            // ⍎ was used somewhere with no environment (e.g. inside an
            // operator's cell-level fold), which we cannot support.
            Prim::Execute => Err(ErrorCode::NonceError),

            // ⌷B — general index (monadic: identity-like, but rarely used alone)
            Prim::Squad => Ok(b.clone()),

            // ?B — roll: random integer from 0..B-1
            Prim::Roll => {
                use rand::Rng;
                let n = b.first_cell().map(|c| c.get_int_value()).unwrap_or(Ok(1))?;
                if n <= 0 {
                    return Err(ErrorCode::DomainError);
                }
                let val = rand::thread_rng().gen_range(0..n);
                Ok(ValueP::scalar_from(Cell::Int(val)))
            }

            // ── structural ────────────────────────────────────────────────
            // ⍴B → shape vector
            Prim::Rho => {
                let dims: Vec<_> = (0..b.rank() as usize)
                    .map(|i| b.get_shape_item(i as i16))
                    .collect();
                Ok(ValueP::int_vector(&dims))
            }
            // ⍳B → index generator
            Prim::Iota => {
                if !b.is_scalar() && !(b.is_vector() && b.element_count() == 1) {
                    return Err(ErrorCode::DomainError);
                }
                let n = b
                    .first_cell()
                    .and_then(|c| c.get_int_value().ok())
                    .ok_or(ErrorCode::DomainError)?;
                ValueP::iota(n)
            }
            _ => Err(ErrorCode::SyntaxError),
        }
    }

    /// apply dyadically: `A f B`
    pub fn eval_dyadic(self, a: &ValueP, b: &ValueP) -> Result<ValueP, ErrorCode> {
        match self {
            Prim::Add => elementwise(a, b, cell::bif_add),
            Prim::Subtract => elementwise(a, b, cell::bif_subtract),
            Prim::Multiply => elementwise(a, b, cell::bif_multiply),
            Prim::Divide => elementwise(a, b, cell::bif_divide),
            Prim::Factorial => elementwise(a, b, cell::bif_binomial_public),
            Prim::Ceiling => elementwise(a, b, cell::bif_maximum),
            Prim::Floor => elementwise(a, b, cell::bif_minimum),
            Prim::Magnitude => elementwise(a, b, cell::bif_residue),
            Prim::PiTimes => elementwise(a, b, |a, b| {
                // A ○ B = trigonometric circle function; only sin/cos basics
                let f = a.get_int_value()?;
                let x = b.get_real_value()?;
                Ok(match f {
                    1 => Cell::Float(x.sin()),
                    2 => Cell::Float(x.cos()),
                    3 => Cell::Float(x.tan()),
                    _ => return Err(ErrorCode::DomainError),
                })
            }),
            Prim::Power => elementwise(a, b, cell::bif_power),

            // comparisons — elementwise 0/1 via tolerant Cell::compare
            Prim::Less => elementwise(a, b, |x, y| {
                Ok(Cell::Int(match x.compare(y) {
                    crate::cell::CompResult::Lt => 1,
                    _ => 0,
                }))
            }),
            Prim::LessEq => elementwise(a, b, |x, y| {
                Ok(Cell::Int(match x.compare(y) {
                    crate::cell::CompResult::Gt => 0,
                    _ => 1,
                }))
            }),
            Prim::Equal => elementwise(a, b, |x, y| {
                Ok(Cell::Int(if x.equal(y, Cell::DEFAULT_CT) { 1 } else { 0 }))
            }),
            Prim::Greater => elementwise(a, b, |x, y| {
                Ok(Cell::Int(match x.compare(y) {
                    crate::cell::CompResult::Gt => 1,
                    _ => 0,
                }))
            }),
            Prim::GreaterEq => elementwise(a, b, |x, y| {
                Ok(Cell::Int(match x.compare(y) {
                    crate::cell::CompResult::Lt => 0,
                    _ => 1,
                }))
            }),
            Prim::NotEqual => elementwise(a, b, |x, y| {
                Ok(Cell::Int(if x.equal(y, Cell::DEFAULT_CT) { 0 } else { 1 }))
            }),

            // A↑B / A↓B — take/drop (last axis)
            Prim::Take => crate::take_drop::take(a, b),
            Prim::Drop => crate::take_drop::drop(a, b),

            // A⌽B — rotate (last axis)
            Prim::Reverse | Prim::Rotate => crate::rotate::rotate(a, b),

            // A⍳B — index of
            Prim::Iota => crate::index_of::index_of(a, b),

            // A∈B — membership
            Prim::Epsilon => crate::epsilon::epsilon(a, b),

            // A/B — replicate (compress): the guarded-branch idiom
            // →cond/line jumps only when cond=1; empty target = fall through.
            Prim::Replicate => crate::replicate::replicate(a, b),

            // A⍉B — axis permutation
            Prim::Transpose => crate::transpose::transpose_dyadic(a, b),

            // A⊃B — pick
            Prim::Disclose => crate::pick::pick(a, b),

            // A⌹B — matrix divide (solve B X = A)
            Prim::Domino => crate::domino::domino_dyadic(a, b),

            // A?B — deal: A unique random integers from 0..B-1
            Prim::Roll => {
                use rand::Rng;
                let n = b.first_cell().map(|c| c.get_int_value()).unwrap_or(Ok(1))?;
                let k = a.first_cell().map(|c| c.get_int_value()).unwrap_or(Ok(1))?;
                if n <= 0 || k < 0 || k > n {
                    return Err(ErrorCode::DomainError);
                }
                let mut rng = rand::thread_rng();
                let mut pool: Vec<i64> = (0..n).collect();
                for i in 0..k as usize {
                    let j = rng.gen_range(i..n as usize);
                    pool.swap(i, j);
                }
                pool.truncate(k as usize);
                Ok(ValueP::int_vector(&pool))
            }

            Prim::And => elementwise(a, b, cell::bif_and),
            Prim::Or => elementwise(a, b, cell::bif_or),

            // A≡B — match
            Prim::Depth => crate::depth::equiv(a, b),

            // A,B — catenate (last axis)
            Prim::Comma => crate::comma::catenate(a, b),

            // A⊂B — partition: split B into enclosed pieces where
            // consecutive elements of A have the same non-zero key.
            // Simplified: A is a boolean/int vector, B is a simple vector.
            Prim::Enclose => crate::partition::partition(a, b),

            // A⊤B — encode (representation)
            Prim::Encode => crate::encode_decode::encode(a, b),
            // A⊥B — decode (base value)
            Prim::Decode => crate::encode_decode::decode(a, b),
            // A∼B — without: elements of A not in B (set difference)
            Prim::Without => without(a, b),
            // A∪B — union: unique of A,B concatenated
            Prim::Union => crate::union::union(a, b),
            // A∩B — intersection: elements of A also in B
            Prim::Inter => crate::union::intersection(a, b),
            // A⍪B — catenate along first axis
            Prim::Comma1 => crate::comma1::catenate_first(a, b),
            // A≢B — not match: 0 if match, 1 if not
            Prim::NotMatch => crate::not_match::not_match(a, b),
            // A⊣B — left: returns A
            Prim::Left => Ok(a.clone()),
            // A⊢B — right: returns B
            Prim::Right => Ok(b.clone()),
            // A⍲B — nand
            Prim::Nand => elementwise(a, b, cell::bif_nand),
            // A⍱B — nor
            Prim::Nor => elementwise(a, b, cell::bif_nor),
            // A⊖B — rotate first axis
            Prim::Rotate1 => crate::squad::rotate_first(a, b),
            Prim::Format => crate::format::format_dyadic(a, b),
            // A⍷B — find occurrences of A within B
            Prim::Find => crate::find::find(a, b),
            // A⊆B — partition B into groups
            Prim::Partition => crate::partition::partition(a, b),
            // A⌷B — general index
            Prim::Squad => crate::squad::squad(a, b),
            // A⍟B — logarithm base A of B
            Prim::NatLog => elementwise(a, b, cell::bif_logarithm),

            // A ⍴ B → reshape
            Prim::Rho => {
                let dims = a
                    .cells()
                    .iter()
                    .map(|c| c.get_int_value())
                    .collect::<Result<Vec<_>, _>>()?;
                let shape = Shape::from_dims(&dims)?;
                reshape(&shape, b)
            }
            _ => Err(ErrorCode::SyntaxError),
        }
    }
}

/// monadic `⍳B` honoring ⎕IO: generates io .. io+n-1
pub fn iota_monadic(b: &ValueP, io: i64) -> Result<ValueP, ErrorCode> {
    if !b.is_scalar() && !(b.is_vector() && b.element_count() == 1) {
        return Err(ErrorCode::DomainError);
    }
    let n = b
        .first_cell()
        .and_then(|c| c.get_int_value().ok())
        .ok_or(ErrorCode::DomainError)?;
    if n < 0 {
        return Ok(ValueP::int_vector(&[]));
    }
    Ok(ValueP::int_vector(&(io..io + n).collect::<Vec<_>>()))
}

/// public wrapper for dyadic primitive dispatch (used by parser for ⎕IO shifts)
pub fn eval_dyadic_public(p: Prim, a: &ValueP, b: &ValueP) -> Result<ValueP, ErrorCode> {
    // implicit disclosure: scalar Pointer args disclose before arithmetic
    // (Dyalog: `5+⊂8` is DOMAIN ERROR, but `x←⊂8 ⋄ 5+x[0]` must work — the
    // indexed scalar carries a Pointer cell that arithmetic cannot consume)
    let a2 = if a.is_scalar() {
        a.disclose()
    } else {
        a.clone()
    };
    let b2 = if b.is_scalar() {
        b.disclose()
    } else {
        b.clone()
    };
    p.eval_dyadic(&a2, &b2)
}

/// Apply a monadic cell function over every ravel element of `b`.
fn map_cells(
    b: &ValueP,
    f: impl Fn(&Cell) -> Result<Cell, ErrorCode> + Sync + Send,
) -> Result<ValueP, ErrorCode> {
    let cells = b.cells();
    // large arrays: elementwise work is embarrassingly parallel
    let out: Vec<Cell> = if cells.len() >= PARALLEL_THRESHOLD {
        use rayon::prelude::*;
        cells.par_iter().map(f).collect::<Result<Vec<_>, _>>()?
    } else {
        cells.iter().map(f).collect::<Result<Vec<_>, _>>()?
    };
    Ok(ValueP::from_ravel_like(b, out))
}

/// below this many elements, parallel dispatch costs more than it saves
pub const PARALLEL_THRESHOLD: usize = 4096;

/// Apply a dyadic cell function element-wise with scalar extension
/// (mirrors C++ ScalarFunction broadcast rules).
pub fn elementwise(
    a: &ValueP,
    b: &ValueP,
    f: impl Fn(&Cell, &Cell) -> Result<Cell, ErrorCode> + Sync + Send,
) -> Result<ValueP, ErrorCode> {
    let ac = a.element_count();
    let bc = b.element_count();
    let len = ac.max(bc);

    if ac != bc && ac != 1 && bc != 1 {
        return Err(ErrorCode::LengthError);
    }
    // Empty operands: scalar extension over an empty array yields an EMPTY
    // array, not an error — `(0⍴0)+1` is empty in GNU APL, and that path is
    // reached by any ⎕IO shift applied to an empty index result (⍸0 0 0).
    // Only a length CONFLICT between two non-unit lengths is an error, and
    // that was already caught above.
    if ac == 0 || bc == 0 {
        // the empty operand dictates the result shape; if both are empty
        // they already agree in length (checked above)
        let empty = if ac == 0 { a } else { b };
        let dims: Vec<i64> = (0..empty.rank())
            .map(|i| empty.get_shape_item(i as i16))
            .collect();
        let shape = if dims.is_empty() {
            crate::shape::Shape::vector(0)
        } else {
            crate::shape::Shape::from_dims(&dims)?
        };
        return ValueP::from_parts(shape, Vec::new());
    }

    let mut out = Vec::with_capacity(len as usize);
    if len as usize >= PARALLEL_THRESHOLD && ac == bc {
        // same-shape arrays: each output cell is independent — parallelize
        use rayon::prelude::*;
        let a_cells = a.cells();
        let b_cells = b.cells();
        let par: Result<Vec<Cell>, ErrorCode> = (0..len as usize)
            .into_par_iter()
            .map(|i| f(&a_cells[i], &b_cells[i]))
            .collect();
        out = par?;
    } else {
        for i in 0..len as usize {
            let ca = a.cells()[i % ac as usize].clone();
            let cb = b.cells()[i % bc as usize].clone();
            out.push(f(&ca, &cb)?);
        }
    }
    Ok(ValueP::from_ravel_like(if ac > 1 { a } else { b }, out))
}

/// A∼B — without: elements of A not found in B (set difference).
/// Both A and B are raveled. Result is a flat vector.
/// Mirrors `Bif_F12_WITHOUT::eval_AB` in C++ (simplified: rank ≤ 1, no axis).
fn without(a: &ValueP, b: &ValueP) -> Result<ValueP, ErrorCode> {
    if a.rank() > 1 || b.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    let acells = a.cells();
    let bcells = b.cells();
    let qct = Cell::DEFAULT_CT;
    let mut out = Vec::new();
    for ca in acells {
        let found = bcells.iter().any(|cb| ca.equal(cb, qct));
        if !found {
            out.push(ca.clone());
        }
    }
    // result is always a vector (possibly empty)
    let shape = if out.is_empty() {
        Shape::vector(0)
    } else {
        Shape::vector(out.len() as i64)
    };
    ValueP::from_parts(shape, out)
}

/// reshape: `A ⍴ B`
fn reshape(shape: &Shape, b: &ValueP) -> Result<ValueP, ErrorCode> {
    let count = shape.get_volume();
    if count < 0 {
        return Err(ErrorCode::LimitError);
    }
    let src = b.cells();
    if src.is_empty() {
        return Err(ErrorCode::DomainError);
    }
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        out.push(src[i % src.len()].clone());
    }
    Ok(ValueP {
        inner: std::sync::Arc::new(crate::value::ValueInner::new(*shape, out)),
    })
}
