//! libapljava.so — JVM bridge for rust-apl (EXTENSIONS.md §7).
//!
//! Rides adapter 1: the interpreter associates these symbols via ordinary
//! ⎕NA declarations. Java objects are global refs in a bridge-side
//! registry; APL sees each handle as a `P` (uintptr) value.
//!
//! Exported surface (fixed, tiny — see EXTENSIONS.md §7.3):
//!   j_init(classpath) -> env handle
//!   j_ready() -> 1 once initialized
//!   j_new(env, class) -> object handle
//!   j_live_handles() -> diagnostics count
//!   j_free(env, handle) -> release a handle
//!
//! Feature `java`: real JNI Invocation API via JNI_CreateJavaVM loaded
//! dynamically from $JAVA_HOME/lib/server/libjvm.so. Without it the same
//! symbols export with stubbed internals so the bridge round-trips through
//! ⎕NA on machines without a JDK.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// next object handle (never reused within a process)
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// live object handles -> declared type (diagnostics)
static HANDLES: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);

/// JVM state. v1: single JVM per process; false until j_init succeeds.
static JVM_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn alloc_handle() -> u64 {
    NEXT_HANDLE.fetch_add(1, Ordering::SeqCst)
}

unsafe fn cstr_to_string(p: *const std::os::raw::c_char) -> String {
    std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
}

/// `j_init <0T` → P : create the JVM with the given classpath (NUL-
/// terminated C string; NULL/0 = default). Returns an env handle ≥1, or
/// a negative code:
///   -1 already initialized
///   -2 JAVA_HOME not set
///   -3 libjvm not found / failed to load
///   -4 JNI_CreateJavaVM symbol missing
///   -5 JNI_CreateJavaVM returned non-OK
/// Stub build (no `java` feature): always -10.
#[cfg(feature = "java")]
#[no_mangle]
pub extern "C" fn j_init(classpath: usize) -> i64 {
    if JVM_READY.load(Ordering::SeqCst) {
        return -1;
    }
    let cp = if classpath != 0 {
        unsafe { cstr_to_string(classpath as *const std::os::raw::c_char) }
    } else {
        String::new()
    };

    let java_home = match std::env::var("JAVA_HOME") {
        Ok(v) if !v.is_empty() => v,
        _ => return -2,
    };
    let libjvm = format!("{}/lib/server/libjvm.so", java_home);
    // Safety: dlopen of the JDK's own libjvm — trusted runtime component.
    // Leaked deliberately: the JVM lives for the process lifetime.
    let lib: &'static libloading::Library =
        match Box::leak(Box::new(unsafe { libloading::Library::new(&libjvm) })) {
            Ok(l) => l,
            Err(_) => return -3,
        };

    let create: libloading::Symbol<
        unsafe extern "system" fn(
            *mut *mut jni::sys::JavaVM,
            *mut *mut std::ffi::c_void,
            *mut std::ffi::c_void,
        ) -> i32,
    > = match unsafe { lib.get(b"JNI_CreateJavaVM") } {
        Ok(f) => f,
        Err(_) => return -4,
    };

    let opts = if cp.is_empty() {
        vec![]
    } else {
        vec![format!("-Djava.class.path={}", cp)]
    };
    let mut jvm_opts: Vec<jni::sys::JavaVMOption> = opts
        .iter()
        .map(|o| jni::sys::JavaVMOption {
            optionString: o.as_ptr() as *mut std::os::raw::c_char,
            extraInfo: std::ptr::null_mut(),
        })
        .collect();
    let mut args = jni::sys::JavaVMInitArgs {
        version: jni::sys::JNI_VERSION_1_8,
        nOptions: jvm_opts.len() as i32,
        options: jvm_opts.as_mut_ptr(),
        ignoreUnrecognized: jni::sys::JNI_FALSE,
    };
    let mut jvm: *mut jni::sys::JavaVM = std::ptr::null_mut();
    let mut env: *mut std::ffi::c_void = std::ptr::null_mut();
    let rc = unsafe {
        create(
            &mut jvm,
            &mut env,
            &mut args as *mut _ as *mut std::ffi::c_void,
        )
    };
    if rc != jni::sys::JNI_OK || jvm.is_null() {
        return -5;
    }

    JVM_READY.store(true, Ordering::SeqCst);
    alloc_handle() as i64
}

/// Stub build: no JVM support compiled in.
#[cfg(not(feature = "java"))]
#[no_mangle]
pub extern "C" fn j_init(_classpath: usize) -> i64 {
    -10
}

/// `j_new P <0T` → P : register an instance of `class`; returns a handle.
/// v1 records the type string; full FindClass/NewObject wiring under the
/// `java` feature lands with method dispatch.
#[no_mangle]
pub extern "C" fn j_new(_env: usize, class: usize) -> i64 {
    if !JVM_READY.load(Ordering::SeqCst) {
        return -20;
    }
    let name = if class != 0 {
        unsafe { cstr_to_string(class as *const std::os::raw::c_char) }
    } else {
        String::new()
    };
    let handle = alloc_handle();
    let mut h = HANDLES.lock().unwrap();
    h.get_or_insert_with(HashMap::new).insert(handle, name);
    handle as i64
}

/// `j_ready` → I4 : 1 once j_init has succeeded, else 0.
#[no_mangle]
pub extern "C" fn j_ready() -> i32 {
    JVM_READY.load(Ordering::SeqCst) as i32
}

/// `j_live_handles` → P : number of registered object handles (diagnostics).
#[no_mangle]
pub extern "C" fn j_live_handles() -> u64 {
    let h = HANDLES.lock().unwrap();
    h.as_ref().map(|m| m.len() as u64).unwrap_or(0)
}

/// `j_free P P` → I4 : release an object handle; returns 0.
#[no_mangle]
pub extern "C" fn j_free(_env: usize, handle: u64) -> i32 {
    let mut h = HANDLES.lock().unwrap();
    if let Some(m) = h.as_mut() {
        m.remove(&handle);
    }
    0
}
