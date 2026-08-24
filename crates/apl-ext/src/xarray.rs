//! XValue — the plugin-facing value type.
//!
//! A safe owned mirror of the interpreter's XArray wire format (same
//! layout constants; the host converts across). Plugins never see raw
//! pointers.

/// Bump when XArray/XCell layout changes in a breaking way. MUST match the
/// interpreter's value — verified by the ABI handshake at load time.
pub const EXCHANGE_ABI: u32 = 1;

pub const MAX_RANK: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellTag {
    Int = 0,
    Float = 1,
    Char = 2,
}

/// One element of a plugin-visible array.
///
/// F5 keeps this flat (no nested pointers): nested APL values arrive as
/// separate arguments after an `enclose`, or via typed accessors added in
/// later phases. Flat cells keep the contract trivially FFI-safe.
#[derive(Debug, Clone, Copy)]
pub enum XCell {
    Int(i64),
    Float(f64),
    Char(u32),
}

impl XCell {
    pub fn tag(&self) -> CellTag {
        match self {
            XCell::Int(_) => CellTag::Int,
            XCell::Float(_) => CellTag::Float,
            XCell::Char(_) => CellTag::Char,
        }
    }
}

/// An owned APL-shaped array: dims + ravel of tagged cells.
#[derive(Debug, Clone)]
pub struct XValue {
    dims: Vec<u64>,
    cells: Vec<XCell>,
}

// keep the tag exported for completeness even though XCell is an enum
#[allow(dead_code)]
fn _tag_exhaustive(t: CellTag) -> &'static str {
    match t {
        CellTag::Int => "int",
        CellTag::Float => "float",
        CellTag::Char => "char",
    }
}

const _: () = {
    assert!(MAX_RANK >= 1, "MAX_RANK must allow at least vectors");
};

impl XValue {
    /// Build from dims + ravel; validates the product against the ravel
    /// length and MAX_RANK.
    pub fn build(dims: &[u64], cells: Vec<XCell>) -> Result<XValue, String> {
        if dims.len() > MAX_RANK {
            return Err(format!("rank {} exceeds MAX_RANK {}", dims.len(), MAX_RANK));
        }
        let n: u64 = dims.iter().product();
        if n != cells.len() as u64 {
            return Err(format!("dims product {} != {} cells", n, cells.len()));
        }
        Ok(XValue {
            dims: dims.to_vec(),
            cells,
        })
    }

    /// rank-0 scalar constructors
    pub fn from_int(v: i64) -> XValue {
        XValue {
            dims: vec![],
            cells: vec![XCell::Int(v)],
        }
    }
    pub fn from_float(v: f64) -> XValue {
        XValue {
            dims: vec![],
            cells: vec![XCell::Float(v)],
        }
    }
    pub fn from_char(c: u32) -> XValue {
        XValue {
            dims: vec![],
            cells: vec![XCell::Char(c)],
        }
    }

    /// vector constructors
    pub fn int_vector(vs: &[i64]) -> XValue {
        XValue {
            dims: vec![vs.len() as u64],
            cells: vs.iter().map(|&v| XCell::Int(v)).collect(),
        }
    }
    pub fn float_vector(vs: &[f64]) -> XValue {
        XValue {
            dims: vec![vs.len() as u64],
            cells: vs.iter().map(|&v| XCell::Float(v)).collect(),
        }
    }
    pub fn char_vector(cs: &[u32]) -> XValue {
        XValue {
            dims: vec![cs.len() as u64],
            cells: cs.iter().map(|&c| XCell::Char(c)).collect(),
        }
    }

    /// convenience: build from a &str (NOT the FromStr trait)
    pub fn from_str_val(s: &str) -> XValue {
        Self::char_vector(&s.chars().map(|c| c as u32).collect::<Vec<_>>())
    }

    pub fn dims(&self) -> &[u64] {
        &self.dims
    }

    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn cells(&self) -> &[XCell] {
        &self.cells
    }

    // ---- typed accessors ----

    /// All cells as chars (empty for non-char arrays).
    pub fn as_chars(&self) -> Vec<u32> {
        self.cells
            .iter()
            .filter_map(|c| match c {
                XCell::Char(ch) => Some(*ch),
                _ => None,
            })
            .collect()
    }

    /// All cells as ints (empty for non-int arrays).
    pub fn as_ints(&self) -> Vec<i64> {
        self.cells
            .iter()
            .filter_map(|c| match c {
                XCell::Int(i) => Some(*i),
                _ => None,
            })
            .collect()
    }

    /// All cells as floats; ints promote (empty for char arrays).
    pub fn as_floats(&self) -> Vec<f64> {
        self.cells
            .iter()
            .filter_map(|c| match c {
                XCell::Float(f) => Some(*f),
                XCell::Int(i) => Some(*i as f64),
                XCell::Char(_) => None,
            })
            .collect()
    }

    /// Single scalar int, if shape is scalar/1-element and int-typed.
    pub fn as_int_scalar(&self) -> Option<i64> {
        if self.cells.len() == 1 {
            match self.cells[0] {
                XCell::Int(i) => Some(i),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Single scalar float (ints promote).
    pub fn as_float_scalar(&self) -> Option<f64> {
        if self.cells.len() == 1 {
            match self.cells[0] {
                XCell::Float(f) => Some(f),
                XCell::Int(i) => Some(i as f64),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Decode char cells into a Rust String (lossy on invalid codepoints).
    pub fn as_string(&self) -> String {
        self.as_chars()
            .iter()
            .filter_map(|&c| char::from_u32(c))
            .collect()
    }
}
