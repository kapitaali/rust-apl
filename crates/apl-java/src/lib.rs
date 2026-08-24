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
use std::sync::{Mutex, OnceLock};

/// next object handle (never reused within a process)
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// live object handles -> declared type (diagnostics)
static HANDLES: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);

/// JVM state. v1: single JVM per process; false until j_init succeeds.
static JVM_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// leaked libjvm handle + raw JavaVM pointer, set by j_init. The raw
/// pointer is not thread-safe to *share* per Rust's rules, but the JNI
/// spec guarantees a JavaVM is valid across threads — hence unsafe Sync.
struct JvmGlobal {
    _lib: &'static libloading::Library,
    jvm: *mut jni::sys::JavaVM,
}
unsafe impl Send for JvmGlobal {}
unsafe impl Sync for JvmGlobal {}
static GLOBAL_JVM: OnceLock<JvmGlobal> = OnceLock::new();

fn alloc_handle() -> u64 {
    NEXT_HANDLE.fetch_add(1, Ordering::SeqCst)
}

unsafe fn cstr_to_string(p: *const std::os::raw::c_char) -> String {
    std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
}

#[cfg(feature = "java")]
mod vm {
    use super::*;
    use jni::Outcome;

    /// Attach the current thread (daemon) and run `f` with an owned Env.
    /// ⎕NA calls arrive on the interpreter thread, which may differ from the
    /// creating thread, so every entry attaches first. Uses jni 0.22's
    /// AttachGuard via EnvUnowned::from_raw + with_env_no_catch.
    pub fn with_env<T>(f: impl FnOnce(&mut jni::Env) -> Result<T, i64>) -> Result<T, i64> {
        let g = GLOBAL_JVM.get().ok_or(-30)?;
        unsafe {
            let invoke = &(*(*g.jvm)).v1_4;
            let attach = invoke.AttachCurrentThreadAsDaemon;
            let mut env_ptr: *mut jni::sys::JNIEnv = std::ptr::null_mut();
            let rc = attach(
                g.jvm,
                &mut env_ptr as *mut _ as *mut *mut std::ffi::c_void,
                std::ptr::null_mut(),
            );
            if rc != jni::sys::JNI_OK {
                return Err(-31);
            }
            // EnvUnowned borrows the attached env; with_env_no_catch hands
            // us a proper &mut Env and detaches when done. The closure's
            // error type must be jni::errors::Error, so our i64 codes are
            // smuggled out through `slot` before mapping.
            let slot: std::cell::RefCell<Option<i64>> = std::cell::RefCell::new(None);
            let mut unowned = jni::EnvUnowned::from_raw(env_ptr);
            let outcome =
                unowned.with_env_no_catch(|env: &mut jni::Env| -> Result<T, jni::errors::Error> {
                    f(env).map_err(|code| {
                        *slot.borrow_mut() = Some(code);
                        jni::errors::Error::JniCall(jni::errors::JniError::Unknown)
                    })
                });
            match outcome.into_outcome() {
                Outcome::Ok(t) => Ok(t),
                _ => Err(slot.into_inner().unwrap_or(-32)),
            }
        }
    }

    /// Call toString() on a returned jobject so any object result surfaces
    /// as text (Dyalog's bridge does the same for non-numeric results).
    pub fn to_rust_string(env: &mut jni::Env, obj: &JObject) -> Result<String, i64> {
        let parsed_sig = jni::signature::RuntimeMethodSignature::from_str("()Ljava/lang/String;")
            .map_err(|_| -42)?;
        let sig = parsed_sig.method_signature();
        let js = env
            .call_method(obj, jni::strings::JNIString::from("toString"), sig, &[])
            .and_then(|v| v.l())
            .map_err(|_| -40)?;
        // JString::to_string decodes modified-UTF-8 directly (get_string is
        // deprecated in 0.22).
        let obj_str = unsafe { jni::objects::JString::from_raw(env, js.as_raw()) };
        // Infallible lossy modified-UTF-8 decode (get_string deprecated in 0.22).
        Ok(obj_str.to_string())
    }
}

use jni::objects::JObject;

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

    let _ = GLOBAL_JVM.set(JvmGlobal { _lib: lib, jvm });
    JVM_READY.store(true, Ordering::SeqCst);
    alloc_handle() as i64
}

/// Stub build: no JVM support compiled in.
#[cfg(not(feature = "java"))]
#[no_mangle]
pub extern "C" fn j_init(_classpath: usize) -> i64 {
    -10
}

/// `j_new P <0T` → P : instantiate `class` via its no-arg ctor; returns a
/// global-ref handle. Errors: -20 not ready, -22 FindClass fail,
/// -23 NewObject fail, -24 global-ref fail.
#[cfg(feature = "java")]
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
    let made = vm::with_env(|env| {
        let cls = match env
            .find_class(jni::strings::JNIString::from(name.as_str()))
            .map_err(|_| -22)
        {
            Ok(c) => c,
            // Daemon-attached threads can't see app classes via FindClass,
            // and the failed lookup leaves a PENDING ClassNotFoundException
            // that must be cleared before any further JNI call.
            // load_class = Class.forName route (dots, not slashes).
            Err(_) => {
                env.exception_clear();
                env.load_class(jni::strings::JNIString::from(
                    name.replace('/', ".").as_str(),
                ))
                .map_err(|_| -22)?
            }
        };
        let parsed_ctor =
            jni::signature::RuntimeMethodSignature::from_str("()V").map_err(|_| -23)?;
        let ctor = parsed_ctor.method_signature();
        let obj: JObject = env.new_object(&cls, ctor, &[]).map_err(|_| -23)?;
        let r = env.new_global_ref(&obj).map_err(|_| -24)?;
        let mut h = HANDLES.lock().unwrap();
        h.get_or_insert_with(HashMap::new)
            .insert(handle, name.clone());
        Ok(r.as_raw() as u64 as usize)
    });
    match made {
        Ok(_) => handle as i64,
        Err(code) => code,
    }
}

#[cfg(not(feature = "java"))]
#[no_mangle]
pub extern "C" fn j_new(_env: usize, _class: usize) -> i64 {
    -10
}

/// `j_call_static P <0T <0T <0T <0T <I4 >0T[256]` → I4 : invoke a static method.
///
/// # Safety
/// `out_buf` must point to `cap` writable bytes (the ⎕NA-declared `>0T[n]`
/// buffer); class/method/sig/arg must be valid NUL-terminated C strings (or 0).
/// Args: class name, method name, JNI signature, single-String argument
/// (empty = niladic), output capacity, out buffer. NOTE: the capacity MUST
/// be passed explicitly — ⎕NA hands callees only the buffer ADDRESS, so a
/// hidden length parameter would read register garbage (found the hard way:
/// results silently truncated). Returns 0 on success, negative on failure:
///   -20 not ready, -25 FindClass fail, -26 GetStaticMethodID fail,
///   -27 call failed, -29 null/zero buffer.
#[cfg(feature = "java")]
#[no_mangle]
pub unsafe extern "C" fn j_call_static(
    _env: usize,
    class: usize,
    method: usize,
    sig: usize,
    arg: usize,
    cap: usize,
    out_buf: *mut u8,
) -> i32 {
    if !JVM_READY.load(Ordering::SeqCst) {
        return -20;
    }
    let cls_name = unsafe { cstr_at(class) };
    let m_name = unsafe { cstr_at(method) };
    let m_sig = unsafe { cstr_at(sig) };
    let a_str = unsafe { cstr_at(arg) };
    if out_buf.is_null() || cap == 0 {
        return -29;
    }
    let out_cap = cap;

    let wrote = vm::with_env(|env| -> Result<usize, i64> {
        use jni::signature::RuntimeMethodSignature;
        use jni::strings::JNIString;

        let parsed = RuntimeMethodSignature::from_str(&m_sig).map_err(|_| -26)?;
        let msig = parsed.method_signature();

        let cls = match env
            .find_class(JNIString::from(cls_name.as_str()))
            .map_err(|_| -25)
        {
            Ok(c) => c,
            // Daemon-attached threads can't see app classes via FindClass,
            // and the failed lookup leaves a PENDING ClassNotFoundException
            // that must be cleared before any further JNI call.
            // load_class = Class.forName route (dots, not slashes).
            Err(_) => {
                env.exception_clear();
                env.load_class(jni::strings::JNIString::from(
                    cls_name.replace('/', ".").as_str(),
                ))
                .map_err(|_| -25)?
            }
        };
        let jmethod = env
            .get_static_method_id(&cls, JNIString::from(m_name.as_str()), msig)
            .map_err(|_| -26)?;

        // v1: single-String-arg methods only. Empty arg string = no args.
        // call_static_method_unchecked wants the RETURN type + raw jvalues.
        let ret_jtype = jni::signature::JavaType::Object;
        let ret = if a_str.is_empty() {
            unsafe {
                env.call_static_method_unchecked(
                    &cls,
                    jmethod,
                    ret_jtype,
                    &[jni::sys::jvalue {
                        l: std::ptr::null_mut(),
                    }],
                )
            }
        } else {
            let jarg = env.new_string(a_str.as_str()).map_err(|_| -27)?;
            let jval = jni::objects::JValue::Object(&jarg).as_jni();
            unsafe { env.call_static_method_unchecked(&cls, jmethod, ret_jtype, &[jval]) }
        }
        .and_then(|v| v.l())
        .map_err(|_| -27)?;

        let text = vm::to_rust_string(env, &ret)?;
        let bytes = text.as_bytes();
        let n = bytes.len().min(out_cap - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, n);
            *out_buf.add(n) = 0;
        }
        Ok(n)
    });
    match wrote {
        Ok(_) => 0,
        Err(code) => code as i32,
    }
}

#[cfg(not(feature = "java"))]
#[no_mangle]
pub unsafe extern "C" fn j_call_static(
    _env: usize,
    _class: usize,
    _method: usize,
    _sig: usize,
    _arg: usize,
    _cap: usize,
    _out_buf: *mut u8,
) -> i32 {
    -10
}

/// Read a NUL-terminated C string at `p` (NULL → empty).
unsafe fn cstr_at(p: usize) -> String {
    if p == 0 {
        String::new()
    } else {
        cstr_to_string(p as *const std::os::raw::c_char)
    }
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
