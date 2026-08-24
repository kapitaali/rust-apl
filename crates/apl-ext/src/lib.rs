//! apl-ext — the plugin contract crate.
//!
//! Plugins depend ONLY on this crate, never on the interpreter. The
//! interpreter and the plugin share the same XArray wire layout (kept in
//! sync by EXCHANGE_ABI); this crate re-exports a stable mirror of it plus
//! the registration traits.
//!
//! A plugin is a cdylib exporting two symbols (use [apl_extension!]):
//! - `apl_plugin_abi() -> u32`        — ABI version probe
//! - `apl_plugin_create() -> *mut ()` — leaked Box<dyn AplExtension>
//!
//! # Example
//!
//! ```no_run
//! use apl_ext::{apl_extension, AplExtension, Registrar, CallContext, XValue, AplError};
//!
//! struct HexExt;
//!
//! impl AplExtension for HexExt {
//!     fn name(&self) -> &'static str { "hex" }
//!     fn register(&self, reg: &mut Registrar) {
//!         reg.bind("HEXLEN", 1, 1, |_ctx, args| {
//!             let s = args[0].as_chars();
//!             Ok(XValue::from_int(s.len() as i64))
//!         });
//!     }
//! }
//!
//! apl_extension!(|| Box::new(HexExt));
//! ```

pub mod xarray;

pub use xarray::{CellTag, XCell, XValue, EXCHANGE_ABI, MAX_RANK};

/// Bump when the plugin ABI (trait shape, registrar protocol) changes in a
/// breaking way. Independent from EXCHANGE_ABI but currently bumped with it.
pub const PLUGIN_ABI_VERSION: u32 = 1;

/// Error a binding may return; mapped onto APL error codes by the host.
#[derive(Debug, Clone)]
pub struct AplError {
    /// "DOMAIN ERROR", "LENGTH ERROR", ... — host matches known prefixes,
    /// unknown text surfaces as DOMAIN ERROR carrying this message
    pub message: String,
}

impl AplError {
    pub fn msg(m: impl Into<String>) -> Self {
        AplError { message: m.into() }
    }
}

/// Read-only view of interpreter state during a call (F5: sysvars only).
pub struct CallContext {
    pub io_origin: i64,   // ⎕IO
    pub compare_tol: f64, // ⎕CT
}

impl CallContext {
    /// Allocate scratch memory tied to this call's lifetime (v2 arena;
    /// v1 plugins should own their buffers).
    pub fn alloc(&self, len: usize) -> Vec<u8> {
        vec![0u8; len]
    }
}

/// The value type crossing the boundary — a safe wrapper over XArray.
/// See [xarray::XValue].
pub use xarray::XValue as Value;

/// What a binding returns: a value or an error.
pub type BindResult = Result<XValue, AplError>;

/// Function signature plugins register.
pub type BindFn = dyn Fn(&CallContext, &[XValue]) -> BindResult;

/// Registration handle handed to `AplExtension::register`.
///
/// The host consumes [Registrar::into_entries] after `register()` returns;
/// entries are opaque to plugins.
pub struct Registrar {
    entries: Vec<BindingEntry>,
}

/// One registered binding, consumed by the host interpreter.
pub struct BindingEntry {
    pub apl_name: String,
    pub min_args: usize,
    pub max_args: usize,
    /// parallel-safe under Rayon (default false)
    pub parallel_safe: bool,
    /// boxed function; host takes via take_fn(). None after being taken.
    pub(crate) f: Option<Box<BindFn>>,
}

impl std::fmt::Debug for BindingEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindingEntry")
            .field("apl_name", &self.apl_name)
            .field("min_args", &self.min_args)
            .field("max_args", &self.max_args)
            .field("parallel_safe", &self.parallel_safe)
            .field("fn_present", &self.f.is_some())
            .finish()
    }
}

impl BindingEntry {
    /// Take ownership of the boxed function (host-side; leaves None).
    pub fn take_fn(&mut self) -> Option<Box<BindFn>> {
        self.f.take()
    }
}

impl Default for Registrar {
    fn default() -> Self {
        Self::new()
    }
}

impl Registrar {
    pub fn new() -> Self {
        Registrar {
            entries: Vec::new(),
        }
    }

    /// Bind an APL name to a function taking between min_args and max_args
    /// scalar/array items.
    ///
    /// The function is automatically wrapped in catch_unwind HERE (this
    /// code instantiates in the plugin crate, so panics are caught with
    /// the plugin's own panic runtime BEFORE crossing the .so boundary).
    /// A caught panic surfaces as DOMAIN ERROR.
    pub fn bind<F>(&mut self, apl_name: &str, min_args: usize, max_args: usize, f: F)
    where
        F: Fn(&CallContext, &[XValue]) -> BindResult + 'static,
    {
        let apl_name_owned: String = apl_name.to_string();
        let wrapped = move |ctx: &CallContext, args: &[XValue]| -> BindResult {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(ctx, args))) {
                Ok(r) => r,
                Err(_) => Err(AplError::msg(format!(
                    "EXTERNAL EXCEPTION in {}",
                    apl_name_owned
                ))),
            }
        };
        self.entries.push(BindingEntry {
            apl_name: apl_name.to_string(),
            min_args,
            max_args,
            parallel_safe: false,
            f: Some(Box::new(wrapped)),
        });
    }

    /// Like [bind] but declares the function safe to run inside Rayon
    /// parallel jobs. Only enable for pure functions without side effects.
    pub fn bind_parallel<F>(&mut self, apl_name: &str, min_args: usize, max_args: usize, f: F)
    where
        F: Fn(&CallContext, &[XValue]) -> BindResult + Send + Sync + 'static,
    {
        self.entries.push(BindingEntry {
            apl_name: apl_name.to_string(),
            min_args,
            max_args,
            parallel_safe: true,
            f: Some(Box::new(f)),
        });
    }

    /// Public (host-side) form of take_entries.
    pub fn into_entries(self) -> Vec<BindingEntry> {
        self.entries
    }
}

/// Trait every plugin implements.
pub trait AplExtension: Send + Sync {
    /// plugin identifier (diagnostics only)
    fn name(&self) -> &'static str;
    /// called once after load; register bindings here
    fn register(&self, reg: &mut Registrar);
}

/// Opaque handle transmitted across the .so boundary. A *concrete* type so
/// the pointer stays thin; the host casts back via [rebuild_extension].
pub struct ExtensionHandle {
    inner: Box<dyn AplExtension>,
}

impl ExtensionHandle {
    /// Plugin-side constructor (used by the apl_extension! macro).
    pub fn new(inner: Box<dyn AplExtension>) -> ExtensionHandle {
        ExtensionHandle { inner }
    }
}

/// Host-side: rebuild the boxed extension from the raw handle.
///
/// # Safety
/// `raw` must be a pointer returned by the apl_extension! create symbol in
/// a library whose PLUGIN_ABI_VERSION matches ours, and must not have been
/// rebuilt already.
pub unsafe fn rebuild_extension(raw: *mut ExtensionHandle) -> Box<dyn AplExtension> {
    Box::from_raw(raw).inner
}

/// Export the two loader symbols. Expands to:
/// - `#[no_mangle] extern "C" fn apl_plugin_abi() -> u32`
/// - `#[no_mangle] extern "C" fn apl_plugin_create() -> *mut ExtensionHandle`
///
/// Takes a closure producing the extension: `apl_extension!(|| Box::new(MyExt))`.
#[macro_export]
macro_rules! apl_extension {
    ($ctor:expr) => {
        #[no_mangle]
        pub extern "C" fn apl_plugin_abi() -> u32 {
            $crate::PLUGIN_ABI_VERSION
        }

        /// Returns a leaked ExtensionHandle; the host rebuilds and owns it
        /// through rebuild_extension.
        #[no_mangle]
        pub extern "C" fn apl_plugin_create() -> *mut $crate::ExtensionHandle {
            let ext: Box<dyn $crate::AplExtension> = ($ctor)();
            let handle = $crate::ExtensionHandle::new(ext);
            Box::into_raw(Box::new(handle))
        }
    };
}
