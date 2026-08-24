//! Native probe: replicate j_init's JVM creation and try every class-load
//! route against /tmp/Hello.class, printing full diagnostics.

use std::os::raw::c_void;

fn main() {
    unsafe { real_main() }
}

unsafe fn real_main() {
    let java_home = std::env::var("JAVA_HOME").expect("JAVA_HOME");
    let libjvm = format!("{}/lib/server/libjvm.so", java_home);
    let lib = Box::leak(Box::new(
        libloading::Library::new(&libjvm).expect("dlopen libjvm"),
    ));
    let create: libloading::Symbol<
        unsafe extern "system" fn(*mut *mut jni::sys::JavaVM, *mut *mut c_void, *mut c_void) -> i32,
    > = lib.get(b"JNI_CreateJavaVM").unwrap();

    let opts = ["-Djava.class.path=/tmp".to_string()];
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
    let mut envp: *mut c_void = std::ptr::null_mut();
    let rc = create(&mut jvm, &mut envp, &mut args as *mut _ as *mut c_void);
    println!("JNI_CreateJavaVM rc={rc}");
    assert_eq!(rc, jni::sys::JNI_OK);

    let invoke = &(*(*jvm)).v1_4;
    let attach = invoke.AttachCurrentThreadAsDaemon;
    let mut env_ptr: *mut jni::sys::JNIEnv = std::ptr::null_mut();
    let arc = attach(
        jvm,
        &mut env_ptr as *mut _ as *mut *mut c_void,
        std::ptr::null_mut(),
    );
    println!("attach rc={arc}");
    assert_eq!(arc, jni::sys::JNI_OK);

    let mut unowned = jni::EnvUnowned::from_raw(env_ptr);
    let outcome =
        unowned.with_env_no_catch(|env: &mut jni::Env| -> Result<(), jni::errors::Error> {
            // sanity: a known-good static field lookup
            let parsed_fsig =
                jni::signature::RuntimeFieldSignature::from_str("Ljava/lang/Object;").unwrap();
            let fsig = parsed_fsig.field_signature();
            let _ = env.get_static_field(
                jni::strings::JNIString::from("java/lang/System"),
                jni::strings::JNIString::from("in"),
                fsig,
            );
            println!("System.in lookup ok = {}", !env.exception_check());
            if env.exception_check() {
                env.exception_clear();
            }

            // route 1: raw FindClass
            match env.find_class(jni::strings::JNIString::from("Hello")) {
                Ok(_) => println!("FindClass(Hello): OK"),
                Err(e) => {
                    println!("FindClass(Hello): ERR {e:?}");
                    if env.exception_check() {
                        env.exception_clear();
                        println!("  (pending CNFE cleared)");
                    }
                }
            }

            // route 2: load_class (dots)
            match env.load_class(jni::strings::JNIString::from("Hello")) {
                Ok(_) => println!("load_class(Hello): OK"),
                Err(e) => {
                    println!("load_class(Hello): ERR {e:?}");
                    if env.exception_check() {
                        env.exception_clear();
                    }
                }
            }

            // sanity: control class
            match env.find_class(jni::strings::JNIString::from("java/lang/String")) {
                Ok(_) => println!("FindClass(java/lang/String): OK"),
                Err(e) => println!("FindClass(java/lang/String): ERR {e:?}"),
            }
            Ok(())
        });
    let ok = matches!(outcome.into_outcome(), jni::Outcome::Ok(_));
    println!("env block outcome ok = {ok}");
}
