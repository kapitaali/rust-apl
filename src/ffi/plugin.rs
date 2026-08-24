//! Rust-plugin loading — ⎕LOADSO machinery (Phase F5).
//!
//! Load a cdylib built against the `apl-ext` contract crate:
//! 1. dlopen
//! 2. probe `apl_plugin_abi` — must equal PLUGIN_ABI_VERSION
//! 3. call `apl_plugin_create` → rebuild Box<dyn AplExtension> from raw
//! 4. run register(); convert each BindingEntry into an interpreter-side
//!    native callable wrapped in catch_unwind

use crate::cell::Cell;
use crate::ffi::loader::{LibraryCache, LoadError};
use crate::shape::Shape;
use crate::types::ErrorCode;
use crate::value::{ValueInner, ValueP};
use apl_ext::{AplError, CallContext, XCell, XValue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// One registered plugin function on the interpreter side.
pub struct PluginBinding {
    pub apl_name: String,
    pub min_args: usize,
    pub max_args: usize,
    pub parallel_safe: bool,
    /// Arc'd so Callable can be Clone
    inner: Arc<PluginFnInner>,
}

struct PluginFnInner {
    f: Mutex<Option<Box<apl_ext::BindFn>>>,
}

impl Clone for PluginBinding {
    fn clone(&self) -> Self {
        PluginBinding {
            apl_name: self.apl_name.clone(),
            min_args: self.min_args,
            max_args: self.max_args,
            parallel_safe: self.parallel_safe,
            inner: self.inner.clone(),
        }
    }
}

impl std::fmt::Debug for PluginBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginBinding")
            .field("apl_name", &self.apl_name)
            .field("min_args", &self.min_args)
            .field("max_args", &self.max_args)
            .finish()
    }
}

impl PluginBinding {
    /// Invoke the boxed function under catch_unwind; panics become
    /// DOMAIN ERROR ("EXTERNAL EXCEPTION") instead of aborting the REPL.
    pub fn call(&self, ctx: &CallContext, args: &[XValue]) -> Result<XValue, ErrorCode> {
        let n = args.len();
        if n < self.min_args || n > self.max_args {
            return Err(ErrorCode::DomainError);
        }
        let guard = self.inner.f.lock().unwrap();
        let f = match guard.as_ref() {
            Some(f) => f,
            None => return Err(ErrorCode::ValueError),
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (f)(ctx, args)));
        drop(guard);
        match result {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => {
                // map known error words; unknown text → DOMAIN with message
                eprintln!("plugin {}: {}", self.apl_name, e.message);
                Err(ErrorCode::DomainError)
            }
            Err(_) => {
                eprintln!(
                    "plugin {}: EXTERNAL EXCEPTION (panic caught)",
                    self.apl_name
                );
                Err(ErrorCode::DomainError)
            }
        }
    }
}

/// Result of loading one plugin .so.
pub struct LoadedPlugin {
    pub extension_name: String,
    pub bindings: Vec<PluginBinding>,
    /// keep the library alive for process lifetime
    _lib: Arc<libloading::Library>,
}

/// Load + probe + register a plugin. Returns bindings ready for insertion
/// into the FunctionTable via FunctionTable::insert_plugin.
pub fn load_plugin(cache: &mut LibraryCache, libspec: &str) -> Result<LoadedPlugin, String> {
    // reuse the LibraryCache search order (APL_LIB_PATH etc.)
    let arc = cache.get_or_load(libspec).map_err(load_err_text)?;

    unsafe {
        // probe ABI
        let abi_fn: libloading::Symbol<unsafe extern "C" fn() -> u32> =
            arc.get(b"apl_plugin_abi\0").map_err(|e| {
                format!(
                    "ABI probe failed for {}: {} (is it an apl-ext plugin?)",
                    libspec, e
                )
            })?;
        let got = abi_fn();
        if got != apl_ext::PLUGIN_ABI_VERSION {
            return Err(format!(
                "plugin ABI mismatch for {}: plugin reports {}, host supports {}",
                libspec,
                got,
                apl_ext::PLUGIN_ABI_VERSION
            ));
        }

        // create the extension
        let create_fn: libloading::Symbol<unsafe extern "C" fn() -> *mut apl_ext::ExtensionHandle> =
            arc.get(b"apl_plugin_create\0")
                .map_err(|e| format!("create symbol missing for {}: {}", libspec, e))?;
        let raw = create_fn();
        if raw.is_null() {
            return Err(format!("plugin {} returned null extension", libspec));
        }
        // SAFETY: raw came from the plugin's create symbol, ABI-verified
        // above; rebuild_extension is itself unsafe (documented there)
        #[allow(unused_unsafe)]
        let ext = unsafe { apl_ext::rebuild_extension(raw) };

        // register
        let mut reg = apl_ext::Registrar::new();
        ext.register(&mut reg);
        let ext_name = ext.name().to_string();
        drop(ext); // extension may be stateless; bindings outlive it

        let mut bindings = Vec::new();
        for mut entry in reg.into_entries() {
            let f = entry
                .take_fn()
                .ok_or_else(|| format!("binding {} lost its function", entry.apl_name))?;
            bindings.push(PluginBinding {
                apl_name: entry.apl_name.clone(),
                min_args: entry.min_args,
                max_args: entry.max_args,
                parallel_safe: entry.parallel_safe,
                inner: Arc::new(PluginFnInner {
                    f: Mutex::new(Some(f)),
                }),
            });
        }

        Ok(LoadedPlugin {
            extension_name: ext_name,
            bindings,
            _lib: arc,
        })
    }
}

fn load_err_text(e: LoadError) -> String {
    format!("cannot load {}: {}", e.spec, e.detail)
}

// ---------------------------------------------------------------------------
// XValue ⇄ ValueP conversion (host side only)
// ---------------------------------------------------------------------------

/// Convert an interpreter value into the plugin-facing XValue.
///
/// Nested/Complex/Lval cells are rejected — flatten or enclose at the APL
/// level. Char vectors pass through; mixed arrays keep per-cell tags.
pub fn value_to_xvalue(v: &ValueP) -> Result<XValue, ErrorCode> {
    let rank = v.shape().get_rank() as usize;
    let dims: Vec<u64> = (0..rank)
        .map(|ax| v.shape().get_shape_item(ax as i16) as u64)
        .collect();
    let cells: Vec<XCell> = v
        .cells()
        .iter()
        .map(|c| -> Result<XCell, ErrorCode> {
            Ok(match c {
                Cell::Int(i) => XCell::Int(*i),
                Cell::Float(f) => XCell::Float(*f),
                Cell::Char(ch) => XCell::Char(*ch),
                _ => return Err(ErrorCode::DomainError),
            })
        })
        .collect::<Result<_, _>>()?;
    XValue::build(&dims, cells).map_err(|_| ErrorCode::RankError)
}

/// Convert a plugin result back into an interpreter value.
pub fn xvalue_to_value(x: &XValue) -> Result<ValueP, ErrorCode> {
    let cells: Vec<Cell> = x.cells().iter().map(xcell_to_cell).collect();
    let dims: Vec<i64> = x.dims().iter().map(|&d| d as i64).collect();
    let shape = Shape::from_dims(&dims).map_err(|_| ErrorCode::RankError)?;
    Ok(ValueP {
        inner: Arc::new(ValueInner::new(shape, cells)),
    })
}

fn xcell_to_cell(c: &XCell) -> Cell {
    match c {
        XCell::Int(i) => Cell::int(*i),
        XCell::Float(f) => Cell::float(*f),
        XCell::Char(ch) => Cell::char(*ch),
    }
}

/// Build the CallContext snapshot handed to plugin functions.
pub fn make_context(io_origin: i64, compare_tol: f64) -> CallContext {
    CallContext {
        io_origin,
        compare_tol,
    }
}

/// Convenience: registry mapping APL names to bindings, used by sysfunc arm.
pub type PluginRegistry = HashMap<String, PluginBinding>;

#[allow(dead_code)]
fn _unused_path_type() -> PathBuf {
    PathBuf::new()
}

#[allow(dead_code)]
fn _unused_error_type(_e: AplError) {}
