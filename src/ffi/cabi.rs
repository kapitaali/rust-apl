//! CAbi call driver — marshal APL args per CAbiSpec, invoke, unmarshal.
//!
//! F3 scope: by-value scalar args (I/U/C/T/F/P at any legal width) and a
//! single scalar or void result. Pointer/array/struct args are recognized
//! but rejected with DOMAIN ERROR until F3b (next increment) implements
//! buffer management.

use crate::cell::Cell;
use crate::ffi::loader::{LibraryCache, LoadError, SymbolError};
use crate::ffi::nadecl::{CAbiSpec, Direction, LeafType, Special, TypeSpec, Width};
use crate::shape::Shape;
use crate::types::ErrorCode;
use crate::value::{ValueInner, ValueP};
use std::sync::Arc;

/// everything that can go wrong during association/call, mapped to
/// Dyalog-compatible APL errors
pub enum CablError {
    /// FILE ERROR 2 (dlopen failure incl. missing deps)
    Load(LoadError),
    /// VALUE ERROR (symbol not found)
    Symbol(SymbolError),
    /// DOMAIN ERROR with message
    Domain(String),
    Syntax,
}

impl From<LoadError> for CablError {
    fn from(e: LoadError) -> Self {
        CablError::Load(e)
    }
}
impl From<SymbolError> for CablError {
    fn from(e: SymbolError) -> Self {
        CablError::Symbol(e)
    }
}

/// A resolved, ready-to-call native binding.
pub struct CAbiBinding {
    pub spec: CAbiSpec,
    pub addr: usize,
}

impl CAbiBinding {
    /// Associate: load library, resolve symbol, validate arg compatibility.
    pub fn associate(cache: &mut LibraryCache, spec: CAbiSpec) -> Result<Self, CablError> {
        validate_signature(&spec)?;
        let lib = cache.get_or_load(&spec.library)?;
        // keep the library alive for the life of the binding via leak-free
        // Arc in the cache; resolve gives us an address that stays valid as
        // long as SOME Arc to the Library lives (the cache holds one).
        let lib_path = std::path::PathBuf::from(library_key(&spec.library));
        let addr = cache.resolve(&lib, &lib_path, &spec.symbol)?;
        Ok(CAbiBinding { spec, addr })
    }

    /// Call with APL values already converted from the right-argument vector.
    pub fn call(&self, args: &[ValueP]) -> Result<ValueP, ErrorCode> {
        // Dyalog rule: all arguments arrive as ITEMS of the right-argument
        // vector. Explode when counts match; otherwise pass through (used
        // by tests / programmatic callers).
        let exploded: Vec<ValueP> = if args.len() == 1 {
            let v = &args[0];
            let n = v.element_count();
            if n > 1 && n as usize == self.spec.args.len() && v.shape().get_rank() >= 1 {
                v.cells()
                    .iter()
                    .map(|c| match c {
                        Cell::Pointer(p) => ValueP {
                            inner: p.value.clone(),
                        },
                        other => ValueP {
                            inner: std::sync::Arc::new(ValueInner::new(
                                Shape::scalar(),
                                vec![other.clone()],
                            )),
                        },
                    })
                    .collect()
            } else {
                args.to_vec()
            }
        } else {
            args.to_vec()
        };

        if exploded.len() != self.spec.args.len() {
            return Err(ErrorCode::DomainError);
        }
        for (ts, v) in self.spec.args.iter().zip(exploded.iter()) {
            check_arg(ts, v)?;
        }

        // Build typed argument slots: F8 args must land in xmm registers, so
        // the shim's SIGNATURE depends on which positions are float.
        let sig: Vec<bool> = self
            .spec
            .args
            .iter()
            .map(|ts| ts.leaf == LeafType::Float && ts.width == Width::W8)
            .collect();
        let mut words: Vec<u64> = Vec::with_capacity(self.spec.args.len());
        for (ts, v) in self.spec.args.iter().zip(exploded.iter()) {
            words.push(marshal_scalar(ts, v)?);
        }

        match &self.spec.result {
            None => {
                unsafe { call_shim_void(self.addr, &sig, &words) };
                Ok(ValueP::scalar_from(Cell::int(0))) // shy nil
            }
            Some(ts) => {
                let raw = if ts.leaf == LeafType::Float {
                    match ts.width {
                        Width::W4 => {
                            let f = unsafe { call_shim_f32t(self.addr, &sig, &words) };
                            (f as f64).to_bits()
                        }
                        _ => {
                            let f = unsafe { call_shim_f64t(self.addr, &sig, &words) };
                            f.to_bits()
                        }
                    }
                } else {
                    unsafe { call_shim_u64t(self.addr, &sig, &words) }
                };
                unmarshal_scalar(ts, raw)
            }
        }
    }
}

fn library_key(libspec: &str) -> String {
    crate::ffi::loader::candidate_paths_for(libspec)
}

// ---------------------------------------------------------------------------
// marshalling
// ---------------------------------------------------------------------------

/// Extract a single scalar cell from a value (must be rank ≤ 1, len ≤ 1
/// for scalar args).
fn scalar_cell(v: &ValueP) -> Result<Cell, ErrorCode> {
    if v.shape().get_rank() == 0 {
        return Ok(v.cells()[0].clone());
    }
    if v.element_count() == 1 {
        return Ok(v.cells()[0].clone());
    }
    Err(ErrorCode::DomainError)
}

fn check_arg(ts: &TypeSpec, _v: &ValueP) -> Result<(), ErrorCode> {
    // F3: pointer/array/struct args unsupported
    if ts.is_struct || ts.array_len.is_some() || ts.array_open {
        return Err(ErrorCode::DomainError);
    }
    if ts.dir != Direction::Value && ts.special != Special::None {
        return Err(ErrorCode::DomainError);
    }
    if ts.special != Special::None {
        return Err(ErrorCode::DomainError);
    }
    Ok(())
}

fn marshal_scalar(ts: &TypeSpec, v: &ValueP) -> Result<u64, ErrorCode> {
    let c = scalar_cell(v)?;
    let out_of_range = || Err(ErrorCode::DomainError);
    let val: u64 = match (&ts.leaf, ts.width, c) {
        (LeafType::Int | LeafType::UintPtr, _, Cell::Int(i)) => {
            let lim: i64 = match ts.width {
                Width::W1 => i8::MAX as i64,
                Width::W2 => i16::MAX as i64,
                Width::W4 => i32::MAX as i64,
                _ => i64::MAX,
            };
            if i > lim || i < -lim - 1 {
                return out_of_range();
            }
            i as u64
        }
        (LeafType::UInt, _, Cell::Int(i)) => {
            if i < 0 {
                return out_of_range();
            }
            let lim: u64 = match ts.width {
                Width::W1 => u8::MAX as u64,
                Width::W2 => u16::MAX as u64,
                Width::W4 => u32::MAX as u64,
                _ => u64::MAX,
            };
            if (i as u64) > lim {
                return out_of_range();
            }
            i as u64
        }
        (LeafType::Char | LeafType::TransChar, _, Cell::Char(ch)) => {
            let lim: u64 = match ts.width {
                Width::W1 => u8::MAX as u64,
                Width::W2 => u16::MAX as u64,
                _ => u32::MAX as u64,
            };
            if (ch as u64) > lim {
                return out_of_range();
            }
            ch as u64
        }
        (LeafType::Float, Width::W8, Cell::Int(i)) => (i as f64).to_bits(),
        (LeafType::Float, Width::W8, Cell::Float(f)) => f.to_bits(),
        (LeafType::Float, Width::W4, Cell::Int(i)) => ((i as f64) as f32).to_bits() as u64,
        (LeafType::Float, Width::W4, Cell::Float(f)) => (f as f32).to_bits() as u64,
        (_, _, got) => {
            let _ = got;
            return Err(ErrorCode::DomainError);
        }
    };
    Ok(val)
}

fn unmarshal_scalar(ts: &TypeSpec, raw: u64) -> Result<ValueP, ErrorCode> {
    let cell = match (&ts.leaf, ts.width) {
        (LeafType::Int, Width::W1) => Cell::int((raw as u8) as i8 as i64),
        (LeafType::Int, Width::W2) => Cell::int((raw as u16) as i16 as i64),
        (LeafType::Int, Width::W4) => Cell::int((raw as u32) as i32 as i64),
        (LeafType::Int, _) => Cell::int(raw as i64),
        (LeafType::UInt, Width::W1) => Cell::int(raw as u8 as i64),
        (LeafType::UInt, Width::W2) => Cell::int(raw as u16 as i64),
        (LeafType::UInt, Width::W4) => Cell::int(raw as u32 as i64),
        (LeafType::UInt, _) => Cell::int(raw as i64),
        (LeafType::Char | LeafType::TransChar, Width::W1) => Cell::char(raw as u8 as u32),
        (LeafType::Char | LeafType::TransChar, Width::W2) => Cell::char(raw as u16 as u32),
        (LeafType::Char | LeafType::TransChar, _) => Cell::char(raw as u32),
        (LeafType::Float, Width::W4) => Cell::float(f32::from_bits(raw as u32) as f64),
        (LeafType::Float, _) => Cell::float(f64::from_bits(raw)),
        (LeafType::UintPtr, _) => Cell::int(raw as i64),
        _ => return Err(ErrorCode::DomainError),
    };
    Ok(ValueP {
        inner: Arc::new(ValueInner::new(Shape::scalar(), vec![cell])),
    })
}

// ---------------------------------------------------------------------------
// the unsafe call shim
// ---------------------------------------------------------------------------

/// Build a Rust function-pointer type matching the C signature implied by
/// `sig` (true = f64 in that position) and call it. Float args must land in
/// xmm registers; enumerating the mask keeps the transmuted signature exact.
///
/// # Safety
/// `addr` must point at a real C function whose ABI matches the marshalled
/// words exactly. A wrong declaration is undefined behavior (same trust
/// model as Dyalog's ⎕NA).
macro_rules! typed_call {
    ($addr:expr, $sig:expr, $words:expr, $ret:ty) => {{
        let n = $sig.len();
        let mask: u32 = $sig
            .iter()
            .enumerate()
            .fold(0u32, |m, (i, &f)| if f { m | (1 << i) } else { m });
        let w = $words;
        match (n, mask) {
            (0, 0) => {
                let f: extern "C" fn() -> $ret = std::mem::transmute($addr);
                f()
            }
            (1, 0b0) => {
                let f: extern "C" fn(u64) -> $ret = std::mem::transmute($addr);
                f(w[0])
            }
            (1, 0b1) => {
                let f: extern "C" fn(f64) -> $ret = std::mem::transmute($addr);
                f(f64::from_bits(w[0]))
            }
            (2, 0b00) => {
                let f: extern "C" fn(u64, u64) -> $ret = std::mem::transmute($addr);
                f(w[0], w[1])
            }
            (2, 0b01) => {
                let f: extern "C" fn(f64, u64) -> $ret = std::mem::transmute($addr);
                f(f64::from_bits(w[0]), w[1])
            }
            (2, 0b10) => {
                let f: extern "C" fn(u64, f64) -> $ret = std::mem::transmute($addr);
                f(w[0], f64::from_bits(w[1]))
            }
            (2, 0b11) => {
                let f: extern "C" fn(f64, f64) -> $ret = std::mem::transmute($addr);
                f(f64::from_bits(w[0]), f64::from_bits(w[1]))
            }
            (3, 0b000) => {
                let f: extern "C" fn(u64, u64, u64) -> $ret = std::mem::transmute($addr);
                f(w[0], w[1], w[2])
            }
            (3, 0b001) => {
                let f: extern "C" fn(f64, u64, u64) -> $ret = std::mem::transmute($addr);
                f(f64::from_bits(w[0]), w[1], w[2])
            }
            (3, 0b010) => {
                let f: extern "C" fn(u64, f64, u64) -> $ret = std::mem::transmute($addr);
                f(w[0], f64::from_bits(w[1]), w[2])
            }
            (3, 0b100) => {
                let f: extern "C" fn(u64, u64, f64) -> $ret = std::mem::transmute($addr);
                f(w[0], w[1], f64::from_bits(w[2]))
            }
            (3, 0b011) => {
                let f: extern "C" fn(f64, f64, u64) -> $ret = std::mem::transmute($addr);
                f(f64::from_bits(w[0]), f64::from_bits(w[1]), w[2])
            }
            (3, 0b101) => {
                let f: extern "C" fn(f64, u64, f64) -> $ret = std::mem::transmute($addr);
                f(f64::from_bits(w[0]), w[1], f64::from_bits(w[2]))
            }
            (3, 0b110) => {
                let f: extern "C" fn(u64, f64, f64) -> $ret = std::mem::transmute($addr);
                f(w[0], f64::from_bits(w[1]), f64::from_bits(w[2]))
            }
            (3, 0b111) => {
                let f: extern "C" fn(f64, f64, f64) -> $ret = std::mem::transmute($addr);
                f(
                    f64::from_bits(w[0]),
                    f64::from_bits(w[1]),
                    f64::from_bits(w[2]),
                )
            }
            // arity 4..=8 all-int fallback (float masks rejected at associate)
            (_, m) if n >= 4 && m == 0 && n <= 8 => match n {
                4 => {
                    let f: extern "C" fn(u64, u64, u64, u64) -> $ret = std::mem::transmute($addr);
                    f(w[0], w[1], w[2], w[3])
                }
                5 => {
                    let f: extern "C" fn(u64, u64, u64, u64, u64) -> $ret =
                        std::mem::transmute($addr);
                    f(w[0], w[1], w[2], w[3], w[4])
                }
                6 => {
                    let f: extern "C" fn(u64, u64, u64, u64, u64, u64) -> $ret =
                        std::mem::transmute($addr);
                    f(w[0], w[1], w[2], w[3], w[4], w[5])
                }
                7 => {
                    let f: extern "C" fn(u64, u64, u64, u64, u64, u64, u64) -> $ret =
                        std::mem::transmute($addr);
                    f(w[0], w[1], w[2], w[3], w[4], w[5], w[6])
                }
                _ => {
                    let f: extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> $ret =
                        std::mem::transmute($addr);
                    f(w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7])
                }
            },
            _ => unreachable!("signature validated at associate time"),
        }
    }};
}

/// Associate-time signature restrictions of the current shim:
/// - no F4 arguments (declare F8 instead)
/// - float arguments only when total arity is <= 3
fn validate_signature(spec: &CAbiSpec) -> Result<(), CablError> {
    let mut f8_count = 0usize;
    for ts in &spec.args {
        if ts.is_struct || ts.array_len.is_some() || ts.array_open {
            continue;
        }
        if ts.leaf == LeafType::Float && ts.width == Width::W4 {
            return Err(CablError::Domain(
                "F4 arguments not yet supported (declare F8)".into(),
            ));
        }
        if ts.leaf == LeafType::Float {
            f8_count += 1;
        }
    }
    if spec.args.len() > 3 && f8_count > 0 {
        return Err(CablError::Domain(
            "float arguments supported only when arity is 3 or fewer".into(),
        ));
    }
    Ok(())
}

unsafe fn call_shim_void(addr: usize, sig: &[bool], words: &[u64]) {
    typed_call!(addr, sig, words, ())
}

unsafe fn call_shim_u64t(addr: usize, sig: &[bool], words: &[u64]) -> u64 {
    typed_call!(addr, sig, words, u64)
}

unsafe fn call_shim_f64t(addr: usize, sig: &[bool], words: &[u64]) -> f64 {
    typed_call!(addr, sig, words, f64)
}

unsafe fn call_shim_f32t(addr: usize, sig: &[bool], words: &[u64]) -> f32 {
    typed_call!(addr, sig, words, f32)
}
