//! The Cell hierarchy — one item of an APL ravel.
//!
//! Mirrors the C++ `Cell` class hierarchy (`Cell.hh`, `CharCell.hh`,
//! `IntCell.hh`, `FloatCell.hh`, `ComplexCell.hh`, `PointerCell.hh`,
//! `LvalCell.hh`) as a single Rust enum with exhaustive matching.

use crate::types::{APLComplex, APLFloat, APLInteger, AplResult, CellType, ErrorCode, Unicode};

/// Ordering convention used throughout (mirrors C++ `Cell::greater()`):
/// PointerCell > NumericCell > CharCell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompResult {
    Lt,
    Eq,
    Gt,
}

/// A cell pointing to a (nested) APL value.
///
/// In the C++ original this holds a `Value *` with reference counting via
/// the Value's owner_count. In Rust we hold a direct `Arc` handle to the
/// nested value data — the type system tracks ownership for us, and
/// `Arc::strong_count` plays the role of owner_count.
#[derive(Clone)]
pub struct PointerCellData {
    pub value: std::sync::Arc<crate::value::ValueInner>,
}

impl PartialEq for PointerCellData {
    /// pointer cells compare equal only if they point at the SAME value
    /// (identity, not deep equality — mirrors C++ handle comparison).
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.value, &other.value)
    }
}

impl std::fmt::Debug for PointerCellData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pointer(rank={})", self.value.shape().get_rank())
    }
}

/// A cell pointing to another cell (used for selective assignment).
#[derive(Clone, Debug, PartialEq)]
pub struct LvalCellData {
    /// Offset of the target cell within the owner's ravel.
    pub offset: usize,
}

/// One item of an APL ravel.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Char(Unicode),
    Int(APLInteger),
    Float(APLFloat),
    Complex(APLComplex),
    Pointer(PointerCellData),
    Lval(LvalCellData),
}

impl Cell {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    #[inline]
    pub fn char(v: Unicode) -> Cell {
        Cell::Char(v)
    }

    #[inline]
    pub fn int(v: APLInteger) -> Cell {
        Cell::Int(v)
    }

    #[inline]
    pub fn float(v: APLFloat) -> Cell {
        Cell::Float(v)
    }

    /// build the most natural numeric cell for an f64
    /// (int if integral, else float) — mirrors `NumericCell::zV()`.
    pub fn from_f64(v: APLFloat) -> Cell {
        if v.fract() == 0.0 && v.abs() < 9.0e18 {
            Cell::Int(v as APLInteger)
        } else {
            Cell::Float(v)
        }
    }

    #[inline]
    pub fn complex(re: APLFloat, im: APLFloat) -> Cell {
        Cell::Complex(APLComplex::new(re, im))
    }

    #[inline]
    pub fn pointer(v: std::sync::Arc<crate::value::ValueInner>) -> Cell {
        Cell::Pointer(PointerCellData { value: v })
    }

    /// get the nested value of a PointerCell (None for other cell types)
    pub fn get_nested_value(&self) -> Option<&std::sync::Arc<crate::value::ValueInner>> {
        match self {
            Cell::Pointer(p) => Some(&p.value),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Type tests
    // -----------------------------------------------------------------------

    #[inline]
    pub fn cell_type(&self) -> CellType {
        match self {
            Cell::Char(_) => CellType::CHAR,
            Cell::Int(_) => CellType::INT,
            Cell::Float(_) => CellType::FLOAT,
            Cell::Complex(_) => CellType::COMPLEX,
            Cell::Pointer(_) => CellType::POINTER,
            Cell::Lval(_) => CellType::CELLREF,
        }
    }

    #[inline]
    pub fn is_integer_cell(&self) -> bool {
        matches!(self, Cell::Int(_))
    }

    #[inline]
    pub fn is_float_cell(&self) -> bool {
        matches!(self, Cell::Float(_))
    }

    #[inline]
    pub fn is_character_cell(&self) -> bool {
        matches!(self, Cell::Char(_))
    }

    #[inline]
    pub fn is_pointer_cell(&self) -> bool {
        matches!(self, Cell::Pointer(_))
    }

    #[inline]
    pub fn is_lval_cell(&self) -> bool {
        matches!(self, Cell::Lval(_))
    }

    #[inline]
    pub fn is_numeric(&self) -> bool {
        CellType::NUMERIC.contains(self.cell_type())
    }

    #[inline]
    pub fn is_simple_cell(&self) -> bool {
        CellType::SIMPLE.contains(self.cell_type())
    }

    // -----------------------------------------------------------------------
    // Value accessors (mirror C++ get_*_value() family)
    // -----------------------------------------------------------------------

    pub fn get_char_value(&self) -> AplResult<Unicode> {
        match self {
            Cell::Char(v) => Ok(*v),
            _ => Err(ErrorCode::DomainError),
        }
    }

    pub fn get_int_value(&self) -> AplResult<APLInteger> {
        match self {
            Cell::Int(v) => Ok(*v),
            _ => Err(ErrorCode::DomainError),
        }
    }

    pub fn get_real_value(&self) -> AplResult<APLFloat> {
        match self {
            Cell::Int(v) => Ok(*v as APLFloat),
            Cell::Float(v) => Ok(*v),
            _ => Err(ErrorCode::DomainError),
        }
    }

    pub fn get_imag_value(&self) -> AplResult<APLFloat> {
        match self {
            Cell::Complex(c) => Ok(c.im),
            Cell::Int(_) | Cell::Float(_) => Ok(0.0),
            _ => Err(ErrorCode::DomainError),
        }
    }

    pub fn get_complex_value(&self) -> AplResult<APLComplex> {
        match self {
            Cell::Int(v) => Ok(APLComplex::new(*v as APLFloat, 0.0)),
            Cell::Float(v) => Ok(APLComplex::new(*v, 0.0)),
            Cell::Complex(c) => Ok(*c),
            _ => Err(ErrorCode::DomainError),
        }
    }

    // -----------------------------------------------------------------------
    // Near-X tests (mirror C++ is_near_* family)
    // -----------------------------------------------------------------------

    /// Default comparison tolerance (mirrors `DEFAULT_Quad_CT` = 1e-13).
    pub const DEFAULT_CT: APLFloat = 1e-13;

    /// Maximum comparison tolerance (mirrors `MAX_Quad_CT` = 1e-9).
    pub const MAX_CT: APLFloat = 1e-9;

    #[inline]
    pub fn is_near_zero(&self) -> bool {
        match self {
            Cell::Int(v) => *v == 0,
            Cell::Float(v) => v.abs() < Self::DEFAULT_CT,
            Cell::Complex(c) => c.re.abs() < Self::DEFAULT_CT && c.im.abs() < Self::DEFAULT_CT,
            _ => false,
        }
    }

    #[inline]
    pub fn is_near_one(&self) -> bool {
        match self {
            Cell::Int(v) => *v == 1,
            Cell::Float(v) => (v - 1.0).abs() < Self::DEFAULT_CT,
            Cell::Complex(c) => {
                (c.re - 1.0).abs() < Self::DEFAULT_CT && c.im.abs() < Self::DEFAULT_CT
            }
            _ => false,
        }
    }

    #[inline]
    pub fn is_near_bool(&self) -> bool {
        self.is_near_zero() || self.is_near_one()
    }

    #[inline]
    pub fn is_near_int(&self) -> bool {
        match self {
            Cell::Int(_) => true,
            Cell::Float(v) => (*v - v.round()).abs() < Self::DEFAULT_CT,
            Cell::Complex(c) => {
                c.im.abs() < Self::DEFAULT_CT && (c.re - c.re.round()).abs() < Self::DEFAULT_CT
            }
            _ => false,
        }
    }

    pub fn get_near_int(&self) -> AplResult<APLInteger> {
        match self {
            Cell::Int(v) => Ok(*v),
            Cell::Float(v) => {
                let r = v.round();
                if (*v - r).abs() < Self::DEFAULT_CT {
                    Ok(r as APLInteger)
                } else {
                    Err(ErrorCode::DomainError)
                }
            }
            Cell::Complex(c) => {
                if c.im.abs() < Self::DEFAULT_CT {
                    let r = c.re.round();
                    if (c.re - r).abs() < Self::DEFAULT_CT {
                        return Ok(r as APLInteger);
                    }
                }
                Err(ErrorCode::DomainError)
            }
            _ => Err(ErrorCode::DomainError),
        }
    }

    // -----------------------------------------------------------------------
    // Comparison
    // -----------------------------------------------------------------------

    /// ISO p. 19: tolerant equality within `qct`.
    #[inline]
    pub fn tolerantly_equal(a: APLFloat, b: APLFloat, qct: APLFloat) -> bool {
        (a - b).abs() <= qct * (a.abs().max(b.abs()))
    }

    /// ISO p. 15: A and B are on the same half-plane.
    #[inline]
    pub fn same_half_plane(a: APLComplex, b: APLComplex) -> bool {
        // cross product of the two vectors (as 2D vectors) is >= 0
        a.re * b.im - a.im * b.re >= 0.0
    }

    /// ISO p. 19: A is integral (close to a Gaussian integer) within `qct`.
    #[inline]
    pub fn integral_within(a: APLFloat, qct: APLFloat) -> bool {
        (a - a.round()).abs() <= qct * a.abs().max(1.0)
    }

    /// Tolerant equality of two cells (mirrors `Cell::equal()`).
    pub fn equal(&self, other: &Cell, qct: APLFloat) -> bool {
        match (self, other) {
            (Cell::Char(a), Cell::Char(b)) => a == b,
            (Cell::Int(a), Cell::Int(b)) => a == b,
            (Cell::Int(a), Cell::Float(b)) => Self::tolerantly_equal(*a as APLFloat, *b, qct),
            (Cell::Float(a), Cell::Int(b)) => Self::tolerantly_equal(*a, *b as APLFloat, qct),
            (Cell::Float(a), Cell::Float(b)) => Self::tolerantly_equal(*a, *b, qct),
            (Cell::Complex(a), Cell::Complex(b)) => {
                Self::tolerantly_equal(a.re, b.re, qct) && Self::tolerantly_equal(a.im, b.im, qct)
            }
            // mixed real/complex: promote the real side
            (Cell::Int(_) | Cell::Float(_), Cell::Complex(_)) => {
                let a = match self.get_complex_value() {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                let b = match other.get_complex_value() {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                Self::tolerantly_equal(a.re, b.re, qct) && Self::tolerantly_equal(a.im, b.im, qct)
            }
            (Cell::Complex(_), Cell::Int(_) | Cell::Float(_)) => {
                let a = match self.get_complex_value() {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                let b = match other.get_complex_value() {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                Self::tolerantly_equal(a.re, b.re, qct) && Self::tolerantly_equal(a.im, b.im, qct)
            }
            (Cell::Pointer(a), Cell::Pointer(b)) => {
                // identity comparison (same object ⇒ equal); deep equality
                // would require recursion with cycle protection.
                std::sync::Arc::ptr_eq(&a.value, &b.value)
                    || a.value.cells() == b.value.cells() && a.value.shape() == b.value.shape()
            }
            _ => false,
        }
    }

    /// Ordering (mirrors `Cell::greater()`): PointerCell > Numeric > Char.
    pub fn greater(&self, other: &Cell) -> bool {
        use Cell::*;
        match (self, other) {
            // Pointer > everything else
            (Pointer(_), Pointer(_)) => false, // same handle rank; deep compare in Value layer
            (Pointer(_), _) => true,
            (_, Pointer(_)) => false,
            // Numeric > Char
            (Char(_), Char(_)) => false,
            (Char(_), _) => false,
            (_, Char(_)) => true,
            // Numeric comparisons
            (Int(a), Int(b)) => a > b,
            (Int(a), Float(b)) => (*a as APLFloat) > *b,
            (Float(a), Int(b)) => *a > (*b as APLFloat),
            (Float(a), Float(b)) => a > b,
            // Complex: compare real first, then imag (mirrors C++)
            (Complex(a), Complex(b)) => {
                if a.re != b.re {
                    a.re > b.re
                } else {
                    a.im > b.im
                }
            }
            (Int(_) | Float(_), Complex(_)) => {
                let a = match self.get_complex_value() {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if a.re != other.get_complex_value().map(|c| c.re).unwrap_or(0.0) {
                    a.re > other.get_complex_value().map(|c| c.re).unwrap_or(0.0)
                } else {
                    a.im > other.get_complex_value().map(|c| c.im).unwrap_or(0.0)
                }
            }
            (Complex(_), Int(_) | Float(_)) => {
                let b = match other.get_complex_value() {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                let a = self.get_complex_value().unwrap_or_default();
                if a.re != b.re {
                    a.re > b.re
                } else {
                    a.im > b.im
                }
            }
            (Lval(_), _) | (_, Lval(_)) => false,
        }
    }

    /// Three-way compare (mirrors `Cell::compare()`).
    pub fn compare(&self, other: &Cell) -> CompResult {
        if self == other {
            return CompResult::Eq;
        }
        if self.greater(other) {
            CompResult::Gt
        } else {
            CompResult::Lt
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar primitive (bif_*) operations
// ---------------------------------------------------------------------------
//
// These mirror the C++ `bif_add`, `bif_subtract`, ... family. In C++ they are
// virtual methods on Cell; in Rust they are free functions over Cell pairs
// returning the result cell (or an error).

/// Monadic operations: `f B`
pub fn bif_negative(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => Cell::Int(-v),
        Cell::Float(v) => Cell::Float(-v),
        Cell::Complex(c) => Cell::Complex(-*c),
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_reciprocal(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => {
            if *v == 0 {
                Cell::Float(f64::INFINITY)
            } else {
                Cell::Float(1.0 / (*v as APLFloat))
            }
        }
        Cell::Float(v) => Cell::Float(1.0 / v),
        Cell::Complex(c) => Cell::Complex(APLComplex::new(1.0, 0.0) / *c),
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_magnitude(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => Cell::Int(v.abs()),
        Cell::Float(v) => Cell::Float(v.abs()),
        Cell::Complex(c) => Cell::Float((c.re * c.re + c.im * c.im).sqrt()),
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_conjugate(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => Cell::Int(*v),
        Cell::Float(v) => Cell::Float(*v),
        Cell::Complex(c) => Cell::Complex(APLComplex::new(c.re, -c.im)),
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_exponential(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => Cell::Float((*v as APLFloat).exp()),
        Cell::Float(v) => Cell::Float(v.exp()),
        Cell::Complex(c) => {
            let m = c.re.exp();
            Cell::Complex(APLComplex::new(m * c.im.cos(), m * c.im.sin()))
        }
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_nat_log(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => {
            if *v <= 0 {
                return Err(ErrorCode::DomainError);
            }
            Cell::Float((*v as APLFloat).ln())
        }
        Cell::Float(v) => {
            if *v <= 0.0 {
                return Err(ErrorCode::DomainError);
            }
            Cell::Float(v.ln())
        }
        Cell::Complex(c) => Cell::Complex(APLComplex::new(c.re.abs().ln(), c.im.atan2(c.re))),
        _ => return Err(ErrorCode::DomainError),
    })
}

/// Dyadic logarithm: A⍟B = log_A(B) = ln(B) / ln(A)
pub fn bif_logarithm(a: &Cell, b: &Cell) -> AplResult<Cell> {
    // For complex or mixed, compute via complex logs
    let ca = a.get_complex_value()?;
    let cb = b.get_complex_value()?;
    let ln_a = APLComplex::new(ca.re.abs().ln(), ca.im.atan2(ca.re));
    let ln_b = APLComplex::new(cb.re.abs().ln(), cb.im.atan2(cb.re));
    let result = ln_b / ln_a;
    // If both inputs were real and result is real, return Float
    if ca.im == 0.0 && cb.im == 0.0 && result.im == 0.0 {
        Ok(Cell::Float(result.re))
    } else {
        Ok(Cell::Complex(result))
    }
}

pub fn bif_floor(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => Cell::Int(*v),
        Cell::Float(v) => {
            let f = v.floor();
            if Cell::integral_within(*v - f, Cell::DEFAULT_CT) {
                Cell::Int(f as APLInteger)
            } else {
                Cell::Float(f)
            }
        }
        Cell::Complex(c) => Cell::Complex(APLComplex::new(c.re.floor(), c.im.floor())),
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_ceiling(b: &Cell) -> AplResult<Cell> {
    Ok(match b {
        Cell::Int(v) => Cell::Int(*v),
        Cell::Float(v) => {
            let c = v.ceil();
            if Cell::integral_within(*v - c, Cell::DEFAULT_CT) {
                Cell::Int(c as APLInteger)
            } else {
                Cell::Float(c)
            }
        }
        Cell::Complex(c) => Cell::Complex(APLComplex::new(c.re.ceil(), c.im.ceil())),
        _ => return Err(ErrorCode::DomainError),
    })
}

/// Dyadic operations: `A f B`
pub fn bif_add(a: &Cell, b: &Cell) -> AplResult<Cell> {
    use Cell::*;
    Ok(match (a, b) {
        (Int(x), Int(y)) => Int(x.wrapping_add(*y)),
        (Int(x), Float(y)) => Float(*x as APLFloat + y),
        (Float(x), Int(y)) => Float(x + *y as APLFloat),
        (Float(x), Float(y)) => Float(x + y),
        (_, Complex(_)) | (Complex(_), _) => {
            let x = a.get_complex_value()?;
            let y = b.get_complex_value()?;
            Complex(x + y)
        }
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_subtract(a: &Cell, b: &Cell) -> AplResult<Cell> {
    use Cell::*;
    Ok(match (a, b) {
        (Int(x), Int(y)) => Int(x.wrapping_sub(*y)),
        (Int(x), Float(y)) => Float(*x as APLFloat - y),
        (Float(x), Int(y)) => Float(x - *y as APLFloat),
        (Float(x), Float(y)) => Float(x - y),
        (_, Complex(_)) | (Complex(_), _) => {
            let x = a.get_complex_value()?;
            let y = b.get_complex_value()?;
            Complex(x - y)
        }
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_multiply(a: &Cell, b: &Cell) -> AplResult<Cell> {
    use Cell::*;
    Ok(match (a, b) {
        (Int(x), Int(y)) => Int(x.wrapping_mul(*y)),
        (Int(x), Float(y)) => Float(*x as APLFloat * y),
        (Float(x), Int(y)) => Float(x * *y as APLFloat),
        (Float(x), Float(y)) => Float(x * y),
        (_, Complex(_)) | (Complex(_), _) => {
            let x = a.get_complex_value()?;
            let y = b.get_complex_value()?;
            Complex(x * y)
        }
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_divide(a: &Cell, b: &Cell) -> AplResult<Cell> {
    use Cell::*;
    Ok(match (a, b) {
        (_, Int(y)) if *y == 0 => {
            // division by zero → infinity (mirrors C++ behavior)
            let x = a.get_real_value()?;
            Float(x / 0.0)
        }
        (Int(x), Int(y)) => {
            if x % y == 0 {
                Int(x / y)
            } else {
                Float(*x as APLFloat / *y as APLFloat)
            }
        }
        (Int(x), Float(y)) => Float(*x as APLFloat / y),
        (Float(x), Int(y)) => Float(x / *y as APLFloat),
        (Float(x), Float(y)) => Float(x / y),
        (_, Complex(_)) | (Complex(_), _) => {
            let x = a.get_complex_value()?;
            let y = b.get_complex_value()?;
            Complex(x / y)
        }
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_power(a: &Cell, b: &Cell) -> AplResult<Cell> {
    use Cell::*;
    Ok(match (a, b) {
        (Int(x), Int(y)) if *y >= 0 => {
            // integer power
            let mut result: APLInteger = 1;
            for _ in 0..*y {
                result = result.wrapping_mul(*x);
            }
            Int(result)
        }
        (_, Complex(_)) | (Complex(_), _) => {
            let x = a.get_complex_value()?;
            let y = b.get_complex_value()?;
            // complex power via polar form: x^y = exp(y * ln x)
            let ln_x = APLComplex::new(x.re.abs().ln(), x.im.atan2(x.re));
            let yln = y * ln_x;
            let m = yln.re.exp();
            Complex(APLComplex::new(m * yln.im.cos(), m * yln.im.sin()))
        }
        _ => {
            let x = a.get_real_value()?;
            let y = b.get_real_value()?;
            Float(x.powf(y))
        }
    })
}

pub fn bif_maximum(a: &Cell, b: &Cell) -> AplResult<Cell> {
    let x = a.get_real_value()?;
    let y = b.get_real_value()?;
    Ok(Cell::Float(x.max(y)))
}

pub fn bif_minimum(a: &Cell, b: &Cell) -> AplResult<Cell> {
    let x = a.get_real_value()?;
    let y = b.get_real_value()?;
    Ok(Cell::Float(x.min(y)))
}

pub fn bif_residue(a: &Cell, b: &Cell) -> AplResult<Cell> {
    // A | B  =  B - (⌊B÷A) × A
    let x = a.get_real_value()?;
    let y = b.get_real_value()?;
    if x == 0.0 {
        return Ok(b.clone());
    }
    let q = (y / x).floor();
    Ok(Cell::Float(y - q * x))
}

/// Dyadic `∧` — logical AND, generalized to LCM (mirrors `NumericCell::bif_and`).
///
/// Booleans give the classical result (Int 0/1); near-zero args short-circuit
/// to 0; integers give the least common multiple; anything else (floats,
/// complex, chars) is a DOMAIN ERROR in this simplified port.
pub fn bif_and(a: &Cell, b: &Cell) -> AplResult<Cell> {
    let x = cell_as_f64(a)?;
    let y = cell_as_f64(b)?;
    if x == 0.0 || y == 0.0 {
        return Ok(Cell::Int(0));
    }
    if x == 1.0 && y == 1.0 {
        return Ok(Cell::Int(1));
    }
    match (a, b) {
        (Cell::Int(p), Cell::Int(q)) => Ok(Cell::Int(lcm(*p, *q)?)),
        _ => Err(ErrorCode::DomainError),
    }
}

/// Dyadic `∨` — logical OR, generalized to GCD (mirrors `NumericCell::bif_or`).
///
/// Booleans give the classical result; integers give the greatest common
/// divisor; anything else is a DOMAIN ERROR.
pub fn bif_or(a: &Cell, b: &Cell) -> AplResult<Cell> {
    match (a, b) {
        (Cell::Int(p), Cell::Int(q)) => {
            if (*p == 0 || *p == 1) && (*q == 0 || *q == 1) {
                Ok(Cell::Int(i64::from(*p != 0 || *q != 0)))
            } else {
                Ok(Cell::Int(gcd(*p, *q)))
            }
        }
        _ => Err(ErrorCode::DomainError),
    }
}

/// greatest common divisor of non-negative magnitudes
fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// least common multiple via gcd; errors on overflow-sized results
fn lcm(a: i64, b: i64) -> AplResult<i64> {
    if a == 0 || b == 0 {
        return Ok(0);
    }
    let g = gcd(a, b);
    let r = (a / g).checked_mul(b.abs());
    match r {
        Some(v) => Ok(v),
        None => Err(ErrorCode::DomainError),
    }
}

/// numeric view of a cell for the logical functions; chars/pointers error
fn cell_as_f64(c: &Cell) -> AplResult<APLFloat> {
    match c {
        Cell::Int(v) => Ok(*v as APLFloat),
        Cell::Float(v) => Ok(*v),
        _ => Err(ErrorCode::DomainError),
    }
}

/// cell-level boolean comparisons used by reduce/scan (mirrors functions.rs)
pub fn bif_equal(a: &Cell, b: &Cell) -> AplResult<Cell> {
    Ok(Cell::Int(if a.equal(b, Cell::DEFAULT_CT) { 1 } else { 0 }))
}

pub fn bif_not_equal(a: &Cell, b: &Cell) -> AplResult<Cell> {
    Ok(Cell::Int(if a.equal(b, Cell::DEFAULT_CT) { 0 } else { 1 }))
}

pub fn bif_less(a: &Cell, b: &Cell) -> AplResult<Cell> {
    Ok(Cell::Int(match a.compare(b) {
        CompResult::Lt => 1,
        _ => 0,
    }))
}

pub fn bif_less_eq(a: &Cell, b: &Cell) -> AplResult<Cell> {
    Ok(Cell::Int(match a.compare(b) {
        CompResult::Gt => 0,
        _ => 1,
    }))
}

pub fn bif_greater(a: &Cell, b: &Cell) -> AplResult<Cell> {
    Ok(Cell::Int(match a.compare(b) {
        CompResult::Gt => 1,
        _ => 0,
    }))
}

pub fn bif_greater_eq(a: &Cell, b: &Cell) -> AplResult<Cell> {
    Ok(Cell::Int(match a.compare(b) {
        CompResult::Lt => 0,
        _ => 1,
    }))
}

/// Dyadic `!` — binomial coefficient (public entry used by functions.rs).
///
/// A!B = N over K for integer arguments, generalized via gamma otherwise.
pub fn bif_binomial_public(a: &Cell, b: &Cell) -> AplResult<Cell> {
    use Cell::*;
    match (a, b) {
        (Int(n), Int(k)) => {
            if *k < 0 || *k > *n {
                return Ok(Int(0));
            }
            let mut result: APLInteger = 1;
            for i in 0..*k {
                result = result.wrapping_mul(*n - i).wrapping_div(i + 1);
            }
            Ok(Int(result))
        }
        _ => {
            let n = a.get_real_value()?;
            let k = b.get_real_value()?;
            Ok(Float(
                tgamma(k + 1.0) / (tgamma(n + 1.0) * tgamma(k - n + 1.0)),
            ))
        }
    }
}

pub fn bif_direction(b: &Cell) -> AplResult<Cell> {
    // ×B: signum
    Ok(match b {
        Cell::Int(v) => Cell::Int(i64::from(*v > 0) - i64::from(*v < 0)),
        Cell::Float(v) => Cell::Float(v.signum()),
        Cell::Complex(c) => {
            let mag = (c.re * c.re + c.im * c.im).sqrt();
            if mag == 0.0 {
                Cell::Complex(APLComplex::new(0.0, 0.0))
            } else {
                Cell::Complex(APLComplex::new(c.re / mag, c.im / mag))
            }
        }
        _ => return Err(ErrorCode::DomainError),
    })
}

pub fn bif_pi_times(b: &Cell) -> AplResult<Cell> {
    let v = b.get_real_value()?;
    Ok(Cell::Float(std::f64::consts::PI * v))
}

pub fn bif_pi_times_inverse(b: &Cell) -> AplResult<Cell> {
    let v = b.get_real_value()?;
    Ok(Cell::Float(v / std::f64::consts::PI))
}

pub fn bif_factorial(b: &Cell) -> AplResult<Cell> {
    // !B: gamma(B + 1)
    match b {
        Cell::Int(v) if *v >= 0 => {
            let mut result: APLInteger = 1;
            for i in 2..=*v {
                result = result.wrapping_mul(i);
            }
            Ok(Cell::Int(result))
        }
        _ => {
            let v = b.get_real_value()?;
            // Lanczos approximation for tgamma
            Ok(Cell::Float(tgamma(v + 1.0)))
        }
    }
}

/// Lanczos approximation of the gamma function (mirrors the C++ code in
/// `ComplexCell.cc`).
pub fn tgamma(x: APLFloat) -> APLFloat {
    // g=7, n=9 Lanczos coefficients
    const G: APLFloat = 7.0;
    const C: [APLFloat; 9] = [
        0.999_999_999_999_809_9,
        676.5203681218851,
        -1259.1392167224028,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507343278686905,
        -0.13857109526572012,
        9.984_369_578_019_572e-6,
        1.5056327351493116e-7,
    ];
    if x < 0.5 {
        // reflection formula
        std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * tgamma(1.0 - x))
    } else {
        let x1 = x - 1.0;
        let mut a = C[0];
        let t = x1 + G + 0.5;
        for (i, c) in C.iter().enumerate().skip(1) {
            a += c / (x1 + i as APLFloat);
        }
        (2.0 * std::f64::consts::PI).sqrt() * t.powf(x1 + 0.5) * (-t).exp() * a
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ValueP;

    #[test]
    fn test_cell_types() {
        assert!(Cell::int(42).is_integer_cell());
        assert!(Cell::float(1.5).is_float_cell());
        assert!(Cell::char('a' as u32).is_character_cell());
        assert!(
            Cell::pointer(std::sync::Arc::new(crate::value::ValueInner::new(
                crate::shape::Shape::scalar(),
                vec![]
            )))
            .is_pointer_cell()
        );
    }

    #[test]
    fn test_nested_roundtrip() {
        // nest a vector inside a scalar, then disclose it back
        let v = ValueP::int_vector(&[1, 2, 3]);
        let nested = ValueP::nested(v);
        assert!(nested.is_scalar());

        let disclosed = nested.disclose();
        match disclosed.cells() {
            [Cell::Int(1), Cell::Int(2), Cell::Int(3)] => {}
            other => panic!("expected the original vector, got {:?}", other),
        }
    }

    #[test]
    fn test_disclose_non_pointer_is_identity() {
        let v = ValueP::int_vector(&[1, 2]);
        // disclose on a non-pointer returns self
        let d = v.disclose();
        match d.cells() {
            [Cell::Int(1), Cell::Int(2)] => {}
            o => panic!("unexpected {:?}", o),
        }
    }

    #[test]
    fn test_pointer_equality() {
        let inner = std::sync::Arc::new(crate::value::ValueInner::new(
            crate::shape::Shape::vector(2),
            vec![Cell::Int(1), Cell::Int(2)],
        ));
        let p1 = Cell::Pointer(crate::cell::PointerCellData {
            value: inner.clone(),
        });
        let p2 = Cell::Pointer(crate::cell::PointerCellData {
            value: inner.clone(),
        });
        // same object ⇒ equal
        assert!(p1.equal(&p2, 1e-13));

        // equal CONTENT but different objects with same ravel also compare
        // equal via our deep-compare fallback
        let p3 = Cell::Pointer(crate::cell::PointerCellData {
            value: std::sync::Arc::new(crate::value::ValueInner::new(
                crate::shape::Shape::vector(2),
                vec![Cell::Int(1), Cell::Int(2)],
            )),
        });
        assert!(p1.equal(&p3, 1e-13));
    }

    #[test]
    fn test_add() {
        let a = Cell::int(2);
        let b = Cell::int(3);
        assert_eq!(bif_add(&a, &b).unwrap(), Cell::int(5));

        let f = Cell::float(1.5);
        let r = bif_add(&a, &f).unwrap();
        match r {
            Cell::Float(v) => assert!((v - 3.5).abs() < 1e-13),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn test_divide() {
        let a = Cell::int(7);
        let b = Cell::int(2);
        match bif_divide(&a, &b).unwrap() {
            Cell::Float(v) => assert!((v - 3.5).abs() < 1e-13),
            _ => panic!("expected float"),
        }

        // exact division stays integer
        match bif_divide(&Cell::int(6), &Cell::int(2)).unwrap() {
            Cell::Int(v) => assert_eq!(v, 3),
            _ => panic!("expected int"),
        }
    }

    #[test]
    fn test_complex() {
        let a = Cell::complex(1.0, 2.0);
        let b = Cell::complex(3.0, 4.0);
        match bif_add(&a, &b).unwrap() {
            Cell::Complex(c) => {
                assert!((c.re - 4.0).abs() < 1e-13);
                assert!((c.im - 6.0).abs() < 1e-13);
            }
            _ => panic!("expected complex"),
        }
    }

    #[test]
    fn test_greater() {
        // Numeric > Char
        assert!(Cell::int(1).greater(&Cell::char('z' as u32)));
        assert!(!Cell::char('a' as u32).greater(&Cell::int(-5)));

        // int comparison
        assert!(Cell::int(5).greater(&Cell::int(3)));
        assert!(!Cell::int(3).greater(&Cell::int(5)));

        // float/int promotion
        assert!(Cell::float(2.5).greater(&Cell::int(2)));
    }

    #[test]
    fn test_tolerant_equal() {
        assert!(Cell::equal(&Cell::float(1.0), &Cell::float(1.0), 1e-13));
        assert!(Cell::equal(&Cell::int(1), &Cell::float(1.0 + 1e-15), 1e-13));
        assert!(!Cell::equal(&Cell::float(1.0), &Cell::float(1.1), 1e-13));
    }

    #[test]
    fn test_factorial() {
        match bif_factorial(&Cell::int(5)).unwrap() {
            Cell::Int(v) => assert_eq!(v, 120),
            _ => panic!("expected int"),
        }
        match bif_factorial(&Cell::float(0.5)).unwrap() {
            Cell::Float(v) => assert!((v - 0.886226925).abs() < 1e-6),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn test_residue() {
        // 3 | 10 = 1
        match bif_residue(&Cell::int(3), &Cell::int(10)).unwrap() {
            Cell::Float(v) => assert!((v - 1.0).abs() < 1e-13),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn test_direction() {
        match bif_direction(&Cell::int(-7)).unwrap() {
            Cell::Int(v) => assert_eq!(v, -1),
            _ => panic!("expected int"),
        }
    }

    #[test]
    fn test_logarithm() {
        // 2⍟8 = 3 (log base 2 of 8)
        match bif_logarithm(&Cell::int(2), &Cell::int(8)).unwrap() {
            Cell::Float(v) => assert!((v - 3.0).abs() < 1e-13),
            _ => panic!("expected float"),
        }
        // 10⍟1000 = 3
        match bif_logarithm(&Cell::int(10), &Cell::int(1000)).unwrap() {
            Cell::Float(v) => assert!((v - 3.0).abs() < 1e-13),
            _ => panic!("expected float"),
        }
        // ⍟ is the same symbol for both monadic and dyadic
        // monadic: ⍟e = 1
        match bif_nat_log(&Cell::Float(std::f64::consts::E)).unwrap() {
            Cell::Float(v) => assert!((v - 1.0).abs() < 1e-13),
            _ => panic!("expected float"),
        }
    }
}
