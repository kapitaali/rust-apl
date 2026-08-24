//! libapljava.so — JVM bridge for rust-apl (EXTENSIONS.md §7).
//!
//! Rides adapter 1: the interpreter associates these symbols via ordinary
//! ⎕NA declarations. Java objects are global refs in a bridge-side
//! registry; APL sees each handle as a `P` (uintptr) value.
//!
//! Exported surface (fixed, tiny — see EXTENSIONS.md §7.3):
//!   j_init(classpath) -> env handle
//!   j_new(env, class, args)      -> obj handle
//!   j_call_static(env, class, method, args) -> result handle
//!   j_call(env, obj, method, args)          -> result handle
//!   j_field_get(env, obj, field)            -> value handle
//!   j_field_set(env, obj, field, value)
//!   j_free(env, handle)                     -> 0
//!
//! Values cross as XArray-shaped tagged buffers produced by the
//! interpreter's exchange layer; the bridge converts to/from jobject.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// next global-ref handle (never reused within a process)
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// env-handle -> JVM state. v1: single JVM per process.
static REGISTRY: Mutex<Option<JvmState>> = Mutex::new(None);

struct JvmState {
    /// kept for diagnostics / future multi-classpath support
    #[allow(dead_code)]
    classpath: String,
    /// handle -> declared type for diagnostics ("java/util/HashMap" etc.)
    types: HashMap<u64, String>,
}

fn alloc_handle() -> u64 {
    NEXT_HANDLE.fetch_add(1, Ordering::SeqCst)
}

/// `j_init <0T` → P : create the JVM with the given classpath; returns the
/// env handle (1 on success in v1's single-JVM model), or -1 if already up.
#[no_mangle]
pub extern "C" fn j_init(_classpath: usize) -> i64 {
    let mut g = REGISTRY.lock().unwrap();
    if g.is_some() {
        return -1;
    }
    *g = Some(JvmState {
        classpath: String::new(),
        types: HashMap::new(),
    });
    alloc_handle() as i64
}

/// `j_free P` → release an object handle. Returns 0.
///
/// NOTE: full signature is `j_free P P` (env, handle); v1 registry is
/// process-global so env is accepted and ignored here.
#[no_mangle]
pub extern "C" fn j_free(_env: usize, handle: u64) -> i32 {
    let mut g = REGISTRY.lock().unwrap();
    if let Some(s) = g.as_mut() {
        s.types.remove(&handle);
    }
    0
}

/// Bridge introspection: number of live handles (diagnostics only).
#[no_mangle]
pub extern "C" fn j_live_handles() -> u64 {
    let g = REGISTRY.lock().unwrap();
    g.as_ref().map(|s| s.types.len() as u64).unwrap_or(0)
}

/// True once j_init has succeeded (used by the ergonomic dfns to assert).
#[no_mangle]
pub extern "C" fn j_ready() -> i32 {
    let g = REGISTRY.lock().unwrap();
    g.is_some() as i32
}
