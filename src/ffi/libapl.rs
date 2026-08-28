//! libapl C API — GNU APL-compatible embedding interface.
//!
//! All functions in this module are `unsafe extern "C"` because they
//! dereference raw pointers passed by the C caller. The C caller is
//! responsible for passing valid, non-null pointers.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused)]
#![allow(missing_const_for_thread_local)]

use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;
use std::sync::Arc;

use crate::parser::Environment;
use crate::types::ErrorCode;
use crate::value::ValueP;

/// APL value handle.
pub struct APLValue {
    inner: ValueP,
}

/// Error codes (mirrors GNU APL's `LIBAPL_error`).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibaplError {
    NoError = 0,
    DomainError = 0x010001,
    IndexError = 0x010002,
    LengthError = 0x010003,
    RankError = 0x010004,
    SyntaxError = 0x010005,
    ValueError = 0x010006,
    NotImplementedError = 0x010007,
    VariableNotAssigned = 0x010008,
    OutBufferOverflow = 0x010009,
    InBufferOverflow = 0x01000A,
}

impl From<ErrorCode> for LibaplError {
    fn from(e: ErrorCode) -> Self {
        match e {
            ErrorCode::DomainError => LibaplError::DomainError,
            ErrorCode::IndexError => LibaplError::IndexError,
            ErrorCode::LengthError => LibaplError::LengthError,
            ErrorCode::RankError => LibaplError::RankError,
            ErrorCode::SyntaxError => LibaplError::SyntaxError,
            ErrorCode::ValueError => LibaplError::ValueError,
            _ => LibaplError::DomainError,
        }
    }
}

/// Thread-local interpreter state.
thread_local! {
    static GLOBAL_ENV: RefCell<Option<Environment>> = const { RefCell::new(None) };
    static EXPAND_LF_TO_CRLF: RefCell<bool> = const { RefCell::new(false) };
    static SAFE_MODE: RefCell<bool> = const { RefCell::new(true) };
    static RES_CALLBACK: RefCell<Option<extern "C" fn(*const APLValue, c_int) -> c_int>> = const { RefCell::new(None) };
    static GET_LINE_CB: RefCell<Option<extern "C" fn(c_int, *const c_char) -> *const c_char>> = const { RefCell::new(None) };
}

//═══════════════════════════════════════════════════════════════════════════════
// 1. Initialization & lifecycle
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn init_libapl(_progname: *const c_char, _log_startup: c_int) {
    GLOBAL_ENV.with(|env| {
        if env.borrow().is_none() {
            *env.borrow_mut() = Some(Environment::new());
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn expand_LF_to_CRLF(on: c_int) -> c_int {
    EXPAND_LF_TO_CRLF.with(|expand| {
        let prev = *expand.borrow() as c_int;
        *expand.borrow_mut() = on != 0;
        prev
    })
}

#[no_mangle]
pub unsafe extern "C" fn disable_safe_mode() {
    SAFE_MODE.with(|safe| {
        *safe.borrow_mut() = false;
    });
}

//═══════════════════════════════════════════════════════════════════════════════
// 2. Execution
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn apl_exec(line_utf8: *const c_char) -> LibaplError {
    if line_utf8.is_null() {
        return LibaplError::ValueError;
    }
    let line = unsafe { CStr::from_ptr(line_utf8) };
    let line = match line.to_str() {
        Ok(s) => s,
        Err(_) => return LibaplError::DomainError,
    };
    GLOBAL_ENV.with(|env| {
        let mut env = env.borrow_mut();
        let env = env.as_mut().unwrap();
        match env.eval_line(line) {
            Ok(_) => LibaplError::NoError,
            Err(e) => e.into(),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn apl_command(command_utf8: *const c_char) -> *const c_char {
    if command_utf8.is_null() {
        return ptr::null();
    }
    let command = unsafe { CStr::from_ptr(command_utf8) };
    let command = match command.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null(),
    };
    // Strip leading ')' if present (system commands start with ')')
    let command = command.trim_start_matches(')').trim_start();
    GLOBAL_ENV.with(|env| {
        let mut env = env.borrow_mut();
        let env = env.as_mut().unwrap();
        let output = crate::sysvars::syscmd(command, env);
        match output {
            Some(lines) => {
                let joined = lines.join("\n");
                let c_string = CString::new(joined).unwrap_or_else(|_| CString::new("").unwrap());
                c_string.into_raw()
            }
            None => ptr::null(),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn repl(
    _input_buffer: *mut c_char,
    _input_bufsize: *mut c_int,
    _output_buffer: *mut c_char,
    _output_bufsize: *mut c_int,
    error: *mut LibaplError,
) -> c_int {
    unsafe {
        if !error.is_null() {
            *error = LibaplError::NoError;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn fix_function(_function_lines_utf8: *const *const c_char) -> LibaplError {
    LibaplError::NotImplementedError
}

#[no_mangle]
pub extern "C" fn fix_function_NL(_function_lines_utf8: *const c_char) -> LibaplError {
    LibaplError::NotImplementedError
}

//═══════════════════════════════════════════════════════════════════════════════
// 3. Value constructors
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub extern "C" fn int_scalar(val: i64, _loc: *const c_char) -> *mut APLValue {
    let v = ValueP::scalar_from(crate::cell::Cell::int(val));
    Box::into_raw(Box::new(APLValue { inner: v }))
}

#[no_mangle]
pub extern "C" fn double_scalar(val: f64, _loc: *const c_char) -> *mut APLValue {
    let v = ValueP::scalar_from(crate::cell::Cell::float(val));
    Box::into_raw(Box::new(APLValue { inner: v }))
}

#[no_mangle]
pub extern "C" fn complex_scalar(real: f64, imag: f64, _loc: *const c_char) -> *mut APLValue {
    let v = ValueP::scalar_from(crate::cell::Cell::complex(real, imag));
    Box::into_raw(Box::new(APLValue { inner: v }))
}

#[no_mangle]
pub extern "C" fn char_scalar(unicode: c_int, _loc: *const c_char) -> *mut APLValue {
    let v = ValueP::scalar_from(crate::cell::Cell::char(unicode as u32));
    Box::into_raw(Box::new(APLValue { inner: v }))
}

#[no_mangle]
pub unsafe extern "C" fn apl_value(
    rank: c_int,
    shape: *const i64,
    _loc: *const c_char,
) -> *mut APLValue {
    if rank < 0 || shape.is_null() {
        return ptr::null_mut();
    }
    let rank = rank as usize;
    let shape_vec = unsafe { std::slice::from_raw_parts(shape, rank).to_vec() };
    let total: i64 = shape_vec.iter().product();
    let cells: Vec<crate::cell::Cell> = (0..total).map(|_| crate::cell::Cell::int(0)).collect();
    let shape_obj = crate::shape::Shape::from_dims(&shape_vec).unwrap();
    let v = ValueP::from_parts(shape_obj, cells)
        .unwrap_or(ValueP::scalar_from(crate::cell::Cell::int(0)));
    Box::into_raw(Box::new(APLValue { inner: v }))
}

#[no_mangle]
pub unsafe extern "C" fn char_vector(
    str_utf8: *const c_char,
    _loc: *const c_char,
) -> *mut APLValue {
    if str_utf8.is_null() {
        return ptr::null_mut();
    }
    let s = unsafe { CStr::from_ptr(str_utf8) };
    let s = s.to_str().unwrap_or("");
    let codepoints: Vec<crate::types::Unicode> = s.chars().map(|c| c as u32).collect();
    let v = ValueP::char_vector(&codepoints);
    Box::into_raw(Box::new(APLValue { inner: v }))
}

#[no_mangle]
pub unsafe extern "C" fn get_var_value(
    var_name_utf8: *const c_char,
    _loc: *const c_char,
) -> *mut APLValue {
    if var_name_utf8.is_null() {
        return ptr::null_mut();
    }
    let name = unsafe { CStr::from_ptr(var_name_utf8) };
    let name = match name.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    GLOBAL_ENV.with(|env| match env.borrow().as_ref().unwrap().get(name) {
        Some(v) => Box::into_raw(Box::new(APLValue { inner: v.clone() })),
        None => ptr::null_mut(),
    })
}

//═══════════════════════════════════════════════════════════════════════════════
// 4. Value destructor
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn release_value(val: *mut APLValue, _loc: *const c_char) {
    if !val.is_null() {
        unsafe {
            drop(Box::from_raw(val));
        }
    }
}

//═══════════════════════════════════════════════════════════════════════════════
// 5. Read access
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn get_rank(val: *const APLValue) -> c_int {
    if val.is_null() {
        return -1;
    }
    unsafe { (&*val).inner.rank() as c_int }
}

#[no_mangle]
pub unsafe extern "C" fn get_axis(val: *const APLValue, axis: u32) -> i64 {
    if val.is_null() {
        return -1;
    }
    let val = unsafe { &*val };
    let rank = val.inner.rank() as usize;
    if axis as usize >= rank {
        return -1;
    }
    val.inner.shape().get_shape_item(axis as i16) as i64
}

#[no_mangle]
pub unsafe extern "C" fn get_element_count(val: *const APLValue) -> u64 {
    if val.is_null() {
        return 0;
    }
    unsafe { (&*val).inner.element_count() as u64 }
}

#[no_mangle]
pub unsafe extern "C" fn get_type(val: *const APLValue, idx: u64) -> c_int {
    if val.is_null() {
        return -1;
    }
    let val = unsafe { &*val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return -1;
    }
    match &val.inner.cells()[i] {
        crate::cell::Cell::Char(_) => 0x02,    // CCT_CHAR
        crate::cell::Cell::Int(_) => 0x10,     // CCT_INT
        crate::cell::Cell::Float(_) => 0x20,   // CCT_FLOAT
        crate::cell::Cell::Complex(_) => 0x40, // CCT_COMPLEX
        crate::cell::Cell::Pointer(_) => 0x04, // CCT_POINTER
        crate::cell::Cell::Lval(_) => 0x04,    // CCT_CELLREF → treat as pointer
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_char(val: *const APLValue, idx: u64) -> c_int {
    if val.is_null() {
        return -1;
    }
    let val = unsafe { &*val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return -1;
    }
    match val.inner.cells()[i].get_char_value() {
        Ok(c) => c as c_int,
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_int(val: *const APLValue, idx: u64) -> i64 {
    if val.is_null() {
        return 0;
    }
    let val = unsafe { &*val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return 0;
    }
    val.inner.cells()[i].get_int_value().unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn get_real(val: *const APLValue, idx: u64) -> f64 {
    if val.is_null() {
        return 0.0;
    }
    let val = unsafe { &*val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return 0.0;
    }
    val.inner.cells()[i].get_real_value().unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn get_imag(val: *const APLValue, idx: u64) -> f64 {
    if val.is_null() {
        return 0.0;
    }
    let val = unsafe { &*val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return 0.0;
    }
    match val.inner.cells()[i].get_complex_value() {
        Ok(c) => c.im,
        Err(_) => 0.0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_value(val: *const APLValue, idx: u64) -> *mut APLValue {
    if val.is_null() {
        return ptr::null_mut();
    }
    let val = unsafe { &*val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return ptr::null_mut();
    }
    match &val.inner.cells()[i] {
        crate::cell::Cell::Pointer(p) => {
            let nested = ValueP {
                inner: p.value.clone(),
            };
            Box::into_raw(Box::new(APLValue { inner: nested }))
        }
        _ => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn is_string(val: *const APLValue) -> c_int {
    if val.is_null() {
        return 0;
    }
    let val = unsafe { &*val };
    // A char vector: rank 1, all Char cells, non-empty
    if val.inner.rank() != 1 {
        return 0;
    }
    let cells = val.inner.cells();
    if cells.is_empty() {
        return 0;
    }
    if cells.iter().all(|c| c.is_character_cell()) {
        1
    } else {
        0
    }
}

//═══════════════════════════════════════════════════════════════════════════════
// 6. Write access
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn set_var_value(
    var_name_utf8: *const c_char,
    new_value: *const APLValue,
    _loc: *const c_char,
) -> c_int {
    if var_name_utf8.is_null() || new_value.is_null() {
        return -1;
    }
    let name = unsafe { CStr::from_ptr(var_name_utf8) };
    let name = match name.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let val = unsafe { &*new_value };
    GLOBAL_ENV.with(|env| {
        env.borrow_mut()
            .as_mut()
            .unwrap()
            .set(name, val.inner.clone());
    });
    0
}

#[no_mangle]
pub unsafe extern "C" fn set_char(unicode: c_int, val: *mut APLValue, idx: u64) {
    if val.is_null() {
        return;
    }
    let val = unsafe { &mut *val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return;
    }
    let mut inner = val.inner.clone();
    let inner_ref = inner.make_mut();
    if i < inner_ref.cells().len() {
        inner_ref.ravel_mut()[i] = crate::cell::Cell::char(unicode as u32);
    }
    val.inner = inner;
}

#[no_mangle]
pub unsafe extern "C" fn set_int(new_int: i64, val: *mut APLValue, idx: u64) {
    if val.is_null() {
        return;
    }
    let val = unsafe { &mut *val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return;
    }
    let mut inner = val.inner.clone();
    let inner_ref = inner.make_mut();
    if i < inner_ref.cells().len() {
        inner_ref.ravel_mut()[i] = crate::cell::Cell::int(new_int);
    }
    val.inner = inner;
}

#[no_mangle]
pub unsafe extern "C" fn set_double(new_real: f64, val: *mut APLValue, idx: u64) {
    if val.is_null() {
        return;
    }
    let val = unsafe { &mut *val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return;
    }
    let mut inner = val.inner.clone();
    let inner_ref = inner.make_mut();
    if i < inner_ref.cells().len() {
        inner_ref.ravel_mut()[i] = crate::cell::Cell::float(new_real);
    }
    val.inner = inner;
}

#[no_mangle]
pub unsafe extern "C" fn set_complex(new_real: f64, new_imag: f64, val: *mut APLValue, idx: u64) {
    if val.is_null() {
        return;
    }
    let val = unsafe { &mut *val };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return;
    }
    let mut inner = val.inner.clone();
    let inner_ref = inner.make_mut();
    if i < inner_ref.cells().len() {
        inner_ref.ravel_mut()[i] = crate::cell::Cell::complex(new_real, new_imag);
    }
    val.inner = inner;
}

#[no_mangle]
pub unsafe extern "C" fn set_value(new_value: *const APLValue, val: *mut APLValue, idx: u64) {
    if val.is_null() || new_value.is_null() {
        return;
    }
    let val = unsafe { &mut *val };
    let new_value = unsafe { &*new_value };
    let i = idx as usize;
    if i >= val.inner.element_count() as usize {
        return;
    }
    let mut inner = val.inner.clone();
    let inner_ref = inner.make_mut();
    if i < inner_ref.cells().len() {
        // Wrap the nested value as a PointerCell
        let nested = ValueP {
            inner: new_value.inner.inner.clone(),
        };
        inner_ref.ravel_mut()[i] = nested
            .first_cell()
            .cloned()
            .unwrap_or(crate::cell::Cell::int(0));
    }
    val.inner = inner;
}

//═══════════════════════════════════════════════════════════════════════════════
// 7. Printing
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn print_value(val: *const APLValue, out: *mut libc::FILE) {
    if val.is_null() || out.is_null() {
        return;
    }
    let val = unsafe { &*val };
    let lines = crate::boxdisplay::render_with_pp(&val.inner, 10);
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            unsafe {
                libc::fputc(b'\n' as c_int, out);
            }
        }
        let c_string = CString::new(line.as_str()).unwrap_or_else(|_| CString::new("").unwrap());
        unsafe {
            libc::fputs(c_string.as_ptr(), out);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn print_value_to_string(val: *const APLValue) -> *mut c_char {
    if val.is_null() {
        return ptr::null_mut();
    }
    let val = unsafe { &*val };
    let lines = crate::boxdisplay::render_with_pp(&val.inner, 10);
    let s = lines.join("\n");
    let c_string = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
    c_string.into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn print_ucs(out: *mut libc::FILE, string_ucs: *const u32) {
    if string_ucs.is_null() || out.is_null() {
        return;
    }
    unsafe {
        let mut len = 0;
        while *string_ucs.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(string_ucs, len);
        for &c in slice {
            if let Some(ch) = std::char::from_u32(c) {
                let mut buf = [0u8; 4];
                let encoded = ch.encode_utf8(&mut buf);
                for &b in encoded.as_bytes() {
                    libc::fputc(b as c_int, out);
                }
            }
        }
    }
}

//═══════════════════════════════════════════════════════════════════════════════
// 8. UTF conversion
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn UTF8_to_Unicode(utf: *const c_char, length: *mut c_int) -> c_int {
    if utf.is_null() {
        return -1;
    }
    let bytes = unsafe { CStr::from_ptr(utf) }.to_bytes();
    if bytes.is_empty() {
        return -1;
    }
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => {
            let len = c.len_utf8() as c_int;
            if !length.is_null() {
                unsafe {
                    *length = len;
                }
            }
            c as u32 as c_int
        }
        None => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn Unicode_to_UTF8(unicode: c_int, dest: *mut c_char, length: *mut c_int) {
    if dest.is_null() {
        return;
    }
    let c = match std::char::from_u32(unicode as u32) {
        Some(c) => c,
        None => return,
    };
    let mut buf = [0u8; 7];
    let encoded = c.encode_utf8(&mut buf);
    unsafe {
        ptr::copy_nonoverlapping(encoded.as_ptr(), dest as *mut u8, encoded.len());
        if encoded.len() < 7 {
            *dest.add(encoded.len()) = 0;
        }
        if !length.is_null() {
            *length = encoded.len() as c_int;
        }
    }
}

//═══════════════════════════════════════════════════════════════════════════════
// 9. Callbacks
//═══════════════════════════════════════════════════════════════════════════════

/// Default result callback (returns 0 = print).
extern "C" fn default_result_cb(_: *const APLValue, _: c_int) -> c_int {
    0
}

/// Default input callback (returns EOF).
extern "C" fn default_input_cb(_: c_int, _: *const c_char) -> *const c_char {
    ptr::null()
}

#[no_mangle]
pub unsafe extern "C" fn install_result_callback(
    new_callback: extern "C" fn(*const APLValue, c_int) -> c_int,
) -> extern "C" fn(*const APLValue, c_int) -> c_int {
    RES_CALLBACK.with(|cb| {
        let old = if let Some(f) = cb.borrow().as_ref() {
            *f
        } else {
            default_result_cb
        };
        *cb.borrow_mut() = Some(new_callback);
        old
    })
}

#[no_mangle]
pub unsafe extern "C" fn install_get_line_from_user_cb(
    new_callback: extern "C" fn(c_int, *const c_char) -> *const c_char,
) -> extern "C" fn(c_int, *const c_char) -> *const c_char {
    GET_LINE_CB.with(|cb| {
        let old = if let Some(f) = cb.borrow().as_ref() {
            *f
        } else {
            default_input_cb
        };
        *cb.borrow_mut() = Some(new_callback);
        old
    })
}

//═══════════════════════════════════════════════════════════════════════════════
// 10. Evaluation functions
//═══════════════════════════════════════════════════════════════════════════════

/// Evaluate a function niladically.
/// The function pointer is treated as a Prim enum value cast to a pointer.
#[no_mangle]
pub unsafe extern "C" fn eval__fun(fun: *const c_void) -> *mut APLValue {
    if fun.is_null() {
        return ptr::null_mut();
    }
    // Cast the void* back to a Prim value (stored as usize)
    let prim_val = fun as usize;
    // We need to reconstruct the Prim from its discriminant.
    // For now, return null — this requires a proper function table lookup.
    let _ = prim_val;
    ptr::null_mut()
}

/// Evaluate a function monadic: fun B
#[no_mangle]
pub unsafe extern "C" fn eval__fun_B(fun: *const c_void, b: *const APLValue) -> *mut APLValue {
    if fun.is_null() || b.is_null() {
        return ptr::null_mut();
    }
    let b = unsafe { &*b };
    let prim_val = fun as usize;
    let _ = prim_val;
    // TODO: implement proper function evaluation
    ptr::null_mut()
}

/// Evaluate a function dyadic: A fun B
#[no_mangle]
pub unsafe extern "C" fn eval__A_fun_B(
    a: *const APLValue,
    fun: *const c_void,
    b: *const APLValue,
) -> *mut APLValue {
    if fun.is_null() || a.is_null() || b.is_null() {
        return ptr::null_mut();
    }
    let a = unsafe { &*a };
    let b = unsafe { &*b };
    let prim_val = fun as usize;
    let _ = (a, b, prim_val);
    // TODO: implement proper function evaluation
    ptr::null_mut()
}

//═══════════════════════════════════════════════════════════════════════════════
// 11. Utilities
//═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn get_owner_count(val: *const APLValue) -> c_int {
    if val.is_null() {
        return 0;
    }
    unsafe { Arc::strong_count(&(&*val).inner.inner) as c_int }
}

#[no_mangle]
pub unsafe extern "C" fn get_function_ucs(
    _name: *const u32,
    _L: *mut *const c_void,
    _R: *mut *const c_void,
) -> *const c_void {
    ptr::null()
}
