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
#[derive(Clone, Debug)]
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
        // by tests / programmatic callers). Output-only (>) arguments are
        // NOT supplied at the APL level — they're results.
        let input_args = self
            .spec
            .args
            .iter()
            .filter(|ts| ts.dir != Direction::Out)
            .count();
        let total_args = self.spec.args.len();
        let exploded: Vec<ValueP> = if args.len() == 1 {
            let v = &args[0];
            // Unwrap scalar Pointer to look at inner value for explosion
            let inner_v = if v.is_scalar() {
                if let Some(Cell::Pointer(p)) = v.first_cell() {
                    ValueP { inner: p.value.clone() }
                } else {
                    v.clone()
                }
            } else {
                v.clone()
            };
            let n = inner_v.element_count();
            if n == 0 && input_args == 0 {
                Vec::new()
            } else if n > 1 && n as usize == input_args && inner_v.shape().get_rank() >= 1 {
                inner_v.cells()
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

        if exploded.len() != input_args {
            return Err(ErrorCode::DomainError);
        }

        // Prepare arguments: scalars marshal to words; pointer args (< > =)
        // allocate C-side buffers and contribute their ADDRESS as the word.
        // Output buffers (>, =) are collected for the nested result.
        // Out-only (>) args consume NO exploded item.
        struct OutBuf {
            /// declaration-order position (for result assembly)
            _pos: usize,
        }
        let mut words: Vec<u64> = Vec::with_capacity(self.spec.args.len());
        let mut out_bufs: Vec<OutBuf> = Vec::new();
        // keep raw buffers alive until after the call
        let mut owned: Vec<Vec<u8>> = Vec::new();

        let mut exp_iter = exploded.iter();
        for (pos, ts) in self.spec.args.iter().enumerate() {
            match ts.dir {
                Direction::Value => {
                    let v = match exp_iter.next() {
                        Some(v) => (*v).clone(),
                        None => return Err(ErrorCode::InternalError),
                    };
                    check_arg(ts, &v)?;
                    words.push(marshal_scalar(ts, &v)?);
                }
                Direction::In | Direction::Out | Direction::InOut => {
                    // an enclosed arg (scalar Pointer cell) unwraps to its
                    // inner array for buffer purposes
                    let dv = if matches!(ts.dir, Direction::Out)
                        && ts.array_len.is_none()
                        && !ts.array_open
                        && !ts.is_struct
                    {
                        // >scalar: pure output, no APL-side input
                        ValueP::int_vector(&[])
                    } else if matches!(ts.dir, Direction::Out)
                        && (ts.is_struct || ts.array_len.is_some() || ts.array_open)
                    {
                        // >array / >struct with declared size: pure output too
                        ValueP::int_vector(&[])
                    } else {
                        let v = exp_iter
                            .next()
                            .cloned()
                            .unwrap_or_else(|| ValueP::int_vector(&[]));
                        match (v.shape().get_rank(), v.cells().first()) {
                            (0, Some(Cell::Pointer(p))) => ValueP {
                                inner: p.value.clone(),
                            },
                            _ => v,
                        }
                    };
                    let buf = build_arg_buffer(
                        ts,
                        &dv,
                        matches!(ts.dir, Direction::In | Direction::InOut),
                    )?;
                    let addr = buf.as_ptr() as usize as u64;
                    owned.push(buf);
                    if matches!(ts.dir, Direction::Out | Direction::InOut) {
                        out_bufs.push(OutBuf { _pos: pos });
                    }
                    words.push(addr);
                }
            }
        }

        // Pointer args pass addresses — always gp-register words. Only
        // by-value F8 scalars use xmm.
        let sig: Vec<bool> = self
            .spec
            .args
            .iter()
            .map(|ts| {
                ts.dir == Direction::Value && ts.leaf == LeafType::Float && ts.width == Width::W8
            })
            .collect();

        // The call itself, capturing the scalar return (if any)
        let scalar_result: Option<ValueP> = match &self.spec.result {
            None => {
                unsafe { call_shim_void(self.addr, &sig, &words) };
                None
            }
            Some(ts) if ts.dir != Direction::Value => {
                // pointer-typed results deferred to F3d (rare in practice;
                // struct results return through >{} out-args instead)
                return Err(ErrorCode::DomainError);
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
                Some(unmarshal_scalar(ts, raw)?)
            }
        };

        // Convert output buffers (>, =) into values by re-reading the bytes.
        // `owned` holds buffers in arg order; walk the spec (not exploded —
        // Out args consume no input item) and mirror that order.
        let mut owned_iter = owned.into_iter();
        let mut out_values: Vec<ValueP> = Vec::new();
        for ts in self.spec.args.iter() {
            if matches!(ts.dir, Direction::Out | Direction::InOut) {
                let buf = owned_iter.next().ok_or(ErrorCode::InternalError)?;
                out_values.push(read_out_buffer(ts, &buf)?);
            } else if ts.dir != Direction::Value {
                let _ = owned_iter.next();
            }
        }

        // Result assembly (Dyalog rule): the declared result first (unless
        // absent), then each > / = output enclosed, in declaration order.
        match (scalar_result, out_values.len()) {
            (Some(s), 0) => Ok(s),
            (s, n) => {
                let has_scalar = s.is_some();
                let mut items: Vec<Cell> = Vec::new();
                if let Some(sv) = s {
                    items.push(Cell::Pointer(crate::cell::PointerCellData {
                        value: sv.inner.clone(),
                    }));
                }
                for ov in out_values {
                    items.push(Cell::Pointer(crate::cell::PointerCellData {
                        value: ov.inner.clone(),
                    }));
                }
                debug_assert_eq!(items.len(), if has_scalar { 1 } else { 0 } + n);
                Ok(ValueP {
                    inner: std::sync::Arc::new(ValueInner::new(
                        Shape::vector(items.len() as i64),
                        items,
                    )),
                })
            }
        }
    }
}

fn library_key(libspec: &str) -> String {
    crate::ffi::loader::candidate_paths_for(libspec)
}

/// Byte width of one element of a typespec (scalars only; structs/arrays
/// handled by their own paths).
fn leaf_width(ts: &TypeSpec) -> Result<usize, ErrorCode> {
    let w = match ts.width {
        Width::W1 => 1,
        Width::W2 => 2,
        Width::W4 => 4,
        Width::W8 => 8,
        Width::W16 => 16,
        Width::None => usize::from(matches!(ts.leaf, LeafType::UintPtr)) * 8,
    };
    if w == 0 {
        return Err(ErrorCode::DomainError);
    }
    Ok(w)
}

/// Allocate (and optionally pre-fill) a C-side buffer for a pointer arg.
///
/// - `<X` / `=X` input: buffer is filled from the APL value
/// - `>X` output: zero-filled, sized per [n] or the APL value's length
///   ([] with no input takes size from the accompanying value; a bare
///   scalar gives 1 element)
fn build_arg_buffer(
    ts: &TypeSpec,
    v: &ValueP,
    fill_from_input: bool,
) -> Result<Vec<u8>, ErrorCode> {
    // strings first: <0T <#T etc.
    if ts.special != Special::None {
        return build_string_buffer(ts, v, fill_from_input);
    }
    // structs (F3c): total buffer = C sizeof (layout-computed)
    if ts.is_struct {
        let layout = struct_layout(ts);
        let total = layout.last().map(|(s, sz)| s + sz).unwrap_or(0);
        // round up to max member alignment for the trailing pad
        let max_align = ts.members.iter().map(member_elem_width).max().unwrap_or(1);
        let total = total.div_ceil(max_align) * max_align;
        let mut buf = vec![0u8; total];
        if fill_from_input {
            fill_struct_buffer(&mut buf, ts, v)?;
        }
        return Ok(buf);
    }
    let ew = leaf_width(ts)?;
    let n: usize = if let Some(k) = ts.array_len {
        k as usize
    } else if ts.array_open || ts.array_len.is_none() && ts.dir != Direction::Value {
        // [] or unspecified: use the value's element count (min 1)
        (v.element_count().max(1)) as usize
    } else {
        1
    };
    let mut buf = vec![0u8; ew * n];
    if fill_from_input {
        fill_buffer(&mut buf, ts, v)?;
    }
    Ok(buf)
}

/// Fill a buffer from an APL value per the leaf type.
fn fill_buffer(buf: &mut [u8], ts: &TypeSpec, v: &ValueP) -> Result<(), ErrorCode> {
    if ts.is_struct {
        return fill_struct_buffer(buf, ts, v);
    }
    let ew = leaf_width(ts)?;
    let cells = v.cells();
    let count = buf.len() / ew;
    for i in 0..count.min(cells.len()) {
        let raw: u64 = match (&ts.leaf, &cells[i]) {
            (LeafType::Int | LeafType::UintPtr, Cell::Int(x)) => *x as u64,
            (LeafType::UInt, Cell::Int(x)) => *x as u64,
            (LeafType::Char | LeafType::TransChar, Cell::Char(c)) => *c as u64,
            (LeafType::Float, Cell::Int(x)) => (*x as f64).to_bits(),
            (LeafType::Float, Cell::Float(f)) => f.to_bits(),
            _ => return Err(ErrorCode::DomainError),
        };
        let bytes = raw.to_ne_bytes();
        buf[i * ew..(i + 1) * ew].copy_from_slice(&bytes[..ew]);
    }
    Ok(())
}

/// NUL-terminated (`0T`) or byte-counted (`#T`) string buffers.
fn build_string_buffer(
    ts: &TypeSpec,
    v: &ValueP,
    fill_from_input: bool,
) -> Result<Vec<u8>, ErrorCode> {
    // gather char codes from the value (vector of chars)
    let mut units: Vec<u8> = Vec::new();
    for c in v.cells() {
        match c {
            Cell::Char(ch) => {
                match ts.width {
                    Width::W1 | Width::W2 => {
                        // narrow encoding: low byte(s); W2 = UTF-16LE unit
                        if ts.width == Width::W2 {
                            units.extend((*ch as u16).to_le_bytes());
                        } else {
                            units.push(*ch as u8);
                        }
                    }
                    _ => {
                        // T4/wide: encode as UTF-8 bytes for portability of
                        // C-string semantics
                        let s = char::from_u32(*ch).unwrap_or('\u{FFFD}').to_string();
                        units.extend_from_slice(s.as_bytes());
                    }
                }
            }
            Cell::Int(i) => units.push(*i as u8),
            _ => return Err(ErrorCode::DomainError),
        }
    }
    let mut buf = units;
    match ts.special {
        Special::NulTerm => {
            if fill_from_input {
                buf.push(0);
            } else if let Some(k) = ts.array_len {
                // declared [n]: exactly n bytes, NUL-terminated by contract
                buf.resize(k as usize, 0);
            } else {
                // output: allocate generously (input len + 64) — C writes at
                // most this much; caller must respect the cap
                let base = buf.len();
                buf.resize(base + 64, 0);
            }
        }
        Special::ByteCounted => {
            if !fill_from_input {
                if let Some(k) = ts.array_len {
                    buf.resize(k as usize, 0);
                } else {
                    let base = buf.len();
                    buf.resize(base + 64, 0);
                }
            }
        }
        Special::None => unreachable!(),
    }
    Ok(buf)
}

/// Read back an output buffer into an APL value after the call.
fn read_out_buffer(ts: &TypeSpec, buf: &[u8]) -> Result<ValueP, ErrorCode> {
    if ts.special != Special::None {
        // strings: take up to NUL (0T) or the whole filled region (#T)
        let end = match ts.special {
            Special::NulTerm => buf.iter().position(|&b| b == 0).unwrap_or(buf.len()),
            _ => buf.len(),
        };
        let cps: Vec<u32> = decode_chars(ts, &buf[..end]);
        return Ok(ValueP::char_vector(&cps));
    }
    if ts.is_struct {
        return read_struct_buffer(buf, ts);
    }
    let ew = leaf_width(ts)?;
    let n = buf.len() / ew;
    let mut cells = Vec::with_capacity(n);
    for i in 0..n {
        let mut b = [0u8; 8];
        b[..ew].copy_from_slice(&buf[i * ew..(i + 1) * ew]);
        let raw = u64::from_ne_bytes(b);
        cells.push(raw_to_cell(&ts.leaf, ts.width, raw)?);
    }
    // rank-1 vector of n elements
    Ok(ValueP {
        inner: std::sync::Arc::new(ValueInner::new(Shape::vector(n as i64), cells)),
    })
}

fn decode_chars(ts: &TypeSpec, bytes: &[u8]) -> Vec<u32> {
    match ts.width {
        Width::W2 => bytes
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]) as u32)
            .collect(),
        _ => bytes.iter().map(|&b| b as u32).collect(),
    }
}

/// Struct fill (F3c): the APL value is an enclosed vector — item i fills
/// member i. Each member is itself a mini-fill into the member's byte span.
/// Padding bytes stay zero; C ABI alignment is honored by computing offsets
/// with the same rules the compiler uses for the declared member sequence.
fn fill_struct_buffer(buf: &mut [u8], ts: &TypeSpec, v: &ValueP) -> Result<(), ErrorCode> {
    // unwrap one enclosure level if present
    let inner: ValueP = match v.cells().first() {
        Some(Cell::Pointer(p)) => ValueP {
            inner: p.value.clone(),
        },
        _ => v.clone(),
    };
    let items: Vec<ValueP> = if inner.element_count() > 1 && inner.shape().get_rank() >= 1 {
        inner
            .cells()
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
        vec![inner.clone()]
    };
    if items.len() != ts.members.len() {
        return Err(ErrorCode::LengthError);
    }
    let layout = struct_layout(ts);
    for ((m, item), (off, sz)) in ts.members.iter().zip(items.iter()).zip(layout) {
        fill_buffer(&mut buf[off..off + sz], m, item)?;
    }
    Ok(())
}

/// Byte width of one struct member element (scalar or fixed array).
fn member_elem_width(m: &TypeSpec) -> usize {
    leaf_width(m).unwrap_or(8) // validated at associate time
}

/// Compute byte offsets+sizes of each member following C layout rules:
/// each member starts at the next multiple of its natural alignment; the
/// struct's total size is rounded up to the maximum member alignment.
fn struct_layout(ts: &TypeSpec) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(ts.members.len());
    let mut off = 0usize;
    let mut max_align = 1usize;
    for m in &ts.members {
        let ew = member_elem_width(m);
        let n = m.array_len.unwrap_or(1).max(1) as usize;
        let raw = ew * n;
        let align = ew.max(1);
        max_align = max_align.max(align);
        // align up this member's start
        let start = off.div_ceil(align) * align;
        out.push((start, raw));
        off = start + raw;
    }
    // round total up to max_align (not needed for spans themselves, kept
    // for documentation parity with C sizeof)
    let _ = off.div_ceil(max_align) * max_align;
    out
}

/// Struct read-back (>{} / ={}): rebuild each member from its byte span and
/// enclose the members into a nested result vector.
#[allow(dead_code)]
fn read_struct_buffer(buf: &[u8], ts: &TypeSpec) -> Result<ValueP, ErrorCode> {
    let mut members = Vec::with_capacity(ts.members.len());
    for (m, (off, sz)) in ts.members.iter().zip(struct_layout(ts)) {
        members.push(read_out_buffer(m, &buf[off..off + sz])?);
    }
    let cells: Vec<Cell> = members
        .into_iter()
        .map(|mv| {
            Cell::Pointer(crate::cell::PointerCellData {
                value: mv.clone_inner_arc(),
            })
        })
        .collect();
    Ok(ValueP {
        inner: std::sync::Arc::new(ValueInner::new(Shape::vector(cells.len() as i64), cells)),
    })
}

fn raw_to_cell(leaf: &LeafType, width: Width, raw: u64) -> Result<Cell, ErrorCode> {
    Ok(match (leaf, width) {
        (LeafType::Int, Width::W1) => Cell::int((raw as u8) as i8 as i64),
        (LeafType::Int, Width::W2) => Cell::int((raw as u16) as i16 as i64),
        (LeafType::Int, Width::W4) => Cell::int((raw as u32) as i32 as i64),
        (LeafType::Int | LeafType::UintPtr, _) => Cell::int(raw as i64),
        (LeafType::UInt, Width::W1) => Cell::int(raw as u8 as i64),
        (LeafType::UInt, Width::W2) => Cell::int(raw as u16 as i64),
        (LeafType::UInt, _) => Cell::int(raw as i64),
        (LeafType::Char | LeafType::TransChar, Width::W1) => Cell::char(raw as u8 as u32),
        (LeafType::Char | LeafType::TransChar, Width::W2) => Cell::char(raw as u16 as u32),
        (LeafType::Char | LeafType::TransChar, _) => Cell::char(raw as u32),
        (LeafType::Float, Width::W4) => Cell::float(f32::from_bits(raw as u32) as f64),
        (LeafType::Float, _) => Cell::float(f64::from_bits(raw)),
        _ => return Err(ErrorCode::DomainError),
    })
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
    // by-value structs unsupported in the shim (pass >{} pointers instead)
    // BUT allow structs as output buffers (Direction::Out/InOut)
    if ts.is_struct && !matches!(ts.dir, Direction::Out | Direction::InOut) {
        return Err(ErrorCode::DomainError);
    }
    if ts.array_len.is_some() || ts.array_open {
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
