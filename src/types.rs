//! Core type definitions for GNU APL.
//!
//! Mirrors `src/APL_types.hh` and `src/APL_enums.hh` from the C++ original.

/// Signed rank or axis of an APL value.
pub type SRank = i16;
pub type SAxis = i16;

/// Unsigned rank or axis.
pub type URank = u32;
pub type UAxis = u32;

/// Bitmap of axes (for `fun[X]` arguments), normalized to quad-IO = 0.
pub type AxesBitmap = u16;

/// Length of one dimension (axis) of an APL shape.
pub type ShapeItem = i64;

/// One APL character value (Unicode code point).
pub type Unicode = u32;
pub type APLChar = Unicode;

/// One APL integer value.
pub type APLInteger = i64;

/// One APL floating point value.
pub type APLFloat = f64;

/// One APL complex value.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct APLComplex {
    pub re: APLFloat,
    pub im: APLFloat,
}

impl APLComplex {
    pub fn new(re: APLFloat, im: APLFloat) -> Self {
        APLComplex { re, im }
    }

    pub fn real(self) -> APLFloat {
        self.re
    }

    pub fn imag(self) -> APLFloat {
        self.im
    }
}

impl std::ops::Add for APLComplex {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        APLComplex::new(self.re + o.re, self.im + o.im)
    }
}

impl std::ops::Sub for APLComplex {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        APLComplex::new(self.re - o.re, self.im - o.im)
    }
}

impl std::ops::Mul for APLComplex {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        APLComplex::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

impl std::ops::Div for APLComplex {
    type Output = Self;
    fn div(self, o: Self) -> Self {
        let mag2 = o.re * o.re + o.im * o.im;
        if mag2 == 0.0 {
            return APLComplex::new(f64::NAN, f64::NAN);
        }
        APLComplex::new(
            (self.re * o.re + self.im * o.im) / mag2,
            (self.im * o.re - self.re * o.im) / mag2,
        )
    }
}

impl std::ops::Neg for APLComplex {
    type Output = Self;
    fn neg(self) -> Self {
        APLComplex::new(-self.re, -self.im)
    }
}

/// Maximum rank of an APL value (from `cfg_MAX_RANK_WANTED`, default 8).
pub const MAX_RANK: usize = 8;

/// The state indicator nesting level (0 = global).
pub type SILevel = i32;

// ---------------------------------------------------------------------------
// Cell types
// ---------------------------------------------------------------------------

/// The possible cell types in the ravel of an APL value.
///
/// Mirrors the C++ `CellType` enum but as a bitflag set so that
/// aggregate queries ("does this array contain any pointers?") stay cheap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellType(u16);

impl CellType {
    pub const NONE: CellType = CellType(0);
    pub const CHAR: CellType = CellType(0x02);
    pub const POINTER: CellType = CellType(0x04);
    pub const CELLREF: CellType = CellType(0x08);
    pub const INT: CellType = CellType(0x10);
    pub const FLOAT: CellType = CellType(0x20);
    pub const COMPLEX: CellType = CellType(0x40);
    pub const NUMERIC: CellType = CellType(Self::INT.0 | Self::FLOAT.0 | Self::COMPLEX.0);
    pub const SIMPLE: CellType = CellType(Self::CHAR.0 | Self::NUMERIC.0);
    pub const MASK: CellType =
        CellType(Self::CHAR.0 | Self::NUMERIC.0 | Self::POINTER.0 | Self::CELLREF.0);

    #[inline]
    pub fn bits(self) -> u16 {
        self.0
    }

    #[inline]
    pub fn contains(self, other: CellType) -> bool {
        self.0 & other.0 == other.0
    }

    #[inline]
    pub fn intersects(self, other: CellType) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for CellType {
    type Output = CellType;
    fn bitor(self, rhs: CellType) -> CellType {
        CellType(self.0 | rhs.0)
    }
}

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/// APL error codes, mirroring `Error.def` from the C++ original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    NoError = 0,
    SyntaxError = 1,
    DomainError = 2,
    LengthError = 3,
    IndexError = 4,
    RankError = 5,
    ValueError = 6,
    NonceError = 7,
    LimitError = 8,
    SystemError = 9,
    InternalError = 10,
    /// FILE ERROR (⎕NA: shared object failed to dlopen — may be a missing
    /// dependency; Dyalog reports "FILE ERROR 2 No such file or directory")
    FileError = 11,
    // keep room; more added as subsystems are ported
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ErrorCode::NoError => "no error",
            ErrorCode::SyntaxError => "SYNTAX ERROR",
            ErrorCode::DomainError => "DOMAIN ERROR",
            ErrorCode::LengthError => "LENGTH ERROR",
            ErrorCode::IndexError => "INDEX ERROR",
            ErrorCode::RankError => "RANK ERROR",
            ErrorCode::ValueError => "VALUE ERROR",
            ErrorCode::NonceError => "NONCE ERROR",
            ErrorCode::LimitError => "LIMIT ERROR",
            ErrorCode::SystemError => "SYSTEM ERROR",
            ErrorCode::InternalError => "INTERNAL ERROR",
            ErrorCode::FileError => "FILE ERROR",
        };
        write!(f, "{}", name)
    }
}

impl std::error::Error for ErrorCode {}

/// Rich APL error — carries the error code and optional source context
/// (source line + caret range) for user-friendly display.
///
/// Use `AplError::from(ErrorCode)` to convert, or `AplError::with_source()`
/// when you have source context for caret display.
#[derive(Debug, Clone)]
pub struct AplError {
    pub code: ErrorCode,
    /// Human-readable explanation (empty = none)
    pub message: String,
    /// Source line that triggered the error (None = no source context)
    pub source_line: Option<String>,
    /// Caret start position in the source line (0-based, inclusive)
    pub caret_start: Option<usize>,
    /// Caret end position in the source line (0-based, exclusive)
    pub caret_end: Option<usize>,
}

impl AplError {
    /// Construct a bare error with no source context.
    pub fn bare(code: ErrorCode) -> Self {
        Self {
            code,
            message: String::new(),
            source_line: None,
            caret_start: None,
            caret_end: None,
        }
    }

    /// Construct with an explanatory message (no source context).
    pub fn with_message(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source_line: None,
            caret_start: None,
            caret_end: None,
        }
    }

    /// Construct with full source context for caret display.
    pub fn with_source(
        code: ErrorCode,
        source_line: impl Into<String>,
        caret_start: usize,
        caret_end: usize,
    ) -> Self {
        Self {
            code,
            message: String::new(),
            source_line: Some(source_line.into()),
            caret_start: Some(caret_start),
            caret_end: Some(caret_end),
        }
    }

    /// Construct with both message and source context.
    pub fn with_source_and_message(
        code: ErrorCode,
        message: impl Into<String>,
        source_line: impl Into<String>,
        caret_start: usize,
        caret_end: usize,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            source_line: Some(source_line.into()),
            caret_start: Some(caret_start),
            caret_end: Some(caret_end),
        }
    }

    /// True if no source context was attached.
    pub fn is_bare(&self) -> bool {
        self.source_line.is_none()
    }
}

impl From<ErrorCode> for AplError {
    fn from(code: ErrorCode) -> Self {
        Self::bare(code)
    }
}

impl std::fmt::Display for AplError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code_name = match self.code {
            ErrorCode::NoError => "no error",
            ErrorCode::SyntaxError => "SYNTAX ERROR",
            ErrorCode::DomainError => "DOMAIN ERROR",
            ErrorCode::LengthError => "LENGTH ERROR",
            ErrorCode::IndexError => "INDEX ERROR",
            ErrorCode::RankError => "RANK ERROR",
            ErrorCode::ValueError => "VALUE ERROR",
            ErrorCode::NonceError => "NONCE ERROR",
            ErrorCode::LimitError => "LIMIT ERROR",
            ErrorCode::SystemError => "SYSTEM ERROR",
            ErrorCode::InternalError => "INTERNAL ERROR",
            ErrorCode::FileError => "FILE ERROR",
        };
        write!(f, "{}", code_name)?;

        if !self.message.is_empty() {
            write!(f, " [{}]", self.message)?;
        }

        if let Some(line) = &self.source_line {
            let indent = "      ";
            writeln!(f)?;
            write!(f, "{}{}", indent, line)?;
            if let (Some(start), Some(end)) = (self.caret_start, self.caret_end) {
                writeln!(f)?;
                let mut caret_line = String::new();
                for _ in 0..indent.len() { caret_line.push(' '); }
                for _ in 0..start { caret_line.push(' '); }
                for _ in start..end { caret_line.push('^'); }
                write!(f, "{}", caret_line)?;
            }
        }

        Ok(())
    }
}

impl std::error::Error for AplError {}

/// Convenient Result alias used throughout the crate.
pub type AplResult<T> = Result<T, ErrorCode>;
