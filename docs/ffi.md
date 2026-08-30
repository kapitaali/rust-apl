# FFI Reference: Extending APL with Native Code

This document is the complete reference for calling native (non-APL) code from inside the Rust APL interpreter. It covers three foreign-function interfaces — C via `⎕NA`, Java via JNI, and Rust via `⎕LOADSO` — as well as the `⎕CALL` shorthand for direct dispatch. Every section includes explanatory prose, not just tables and examples, so you understand *what* each mechanism does, *why* it exists, and *how* to use it in practice.

**Prerequisites:** you should be comfortable with APL expressions and know how to compile and run the interpreter (see `setup.md`). For the C and Rust sections, familiarity with those languages is assumed.

---

## Table of Contents

- [Overview](#overview)
- [How native calls work internally](#how-native-calls-work-internally)
- [C FFI (⎕NA)](#c-ffi-38na)
  - [What ⎕NA does](#what-38na-does)
  - [How to write a ⎕NA declaration](#how-to-write-a-38na-declaration)
  - [Declaration grammar](#declaration-grammar)
  - [Direction markers (`<`, `>`, `=`)](#direction-markers)
  - [Type specifications](#type-specifications)
  - [Width suffixes](#width-suffixes)
  - [Arrays](#arrays)
  - [Strings and special markers (`0`, `#`)](#strings-and-special-markers)
  - [Structures](#structures)
  - [Full examples](#full-c-ffi-examples)
  - [Library loading order](#library-loading-order)
  - [Troubleshooting C FFI](#troubleshooting-c-ffi)
- [Java FFI (JNI)](#java-ffi-jni)
- [Rust FFI (⎕LOADSO)](#rust-ffi-loadso)
  - [What ⎕LOADSO does](#what-38loadso-does)
  - [Why use a cdylib plugin instead of ⎕NA](#why-use-a-cdylib-plugin)
  - [Creating a cdylib plugin step-by-step](#creating-a-cdylib-plugin-step-by-step)
  - [The `AplPlugin` trait](#the-aplp-lugin-trait)
  - [Registering functions, sysvars, and operators](#registering-functions-sysvars-and-operators)
  - [Full example: a statistics plugin](#full-example-a-statistics-plugin)
  - [Loading and using the plugin in APL](#loading-and-using-the-plugin-in-apl)
  - [The `apl-ext` crate reference](#the-apl-ext-crate-reference)
- [⎕CALL — direct native call](#38call-direct-native-call)
- [Error handling](#error-handling)
- [Threading model and library lifetime](#threading-model-and-library-lifetime)
- [Security levels (⎕SEC)](#security-levels-38sec)

---

## Overview

The interpreter provides three separate mechanisms for calling code written in other languages. Each solves a different problem:

| Mechanism | Language | Best for |
|---|---|---|
| `⎕NA` | C | Calling an existing C library function directly — no wrapper needed |
| JNI (`crates/apl-java/`) | Java | Calling Java methods; requires the JVM |
| `⎕LOADSO` | Rust | Shipping a self-contained extension that registers multiple functions at runtime |

All three go through the same internal `CAbiBinding` interface, which handles the platform-specific details of loading libraries, resolving symbols, marshalling APL values to C types, and converting results back. This means once you understand how one mechanism works, the others feel familiar.

**Which should you use?**
- You have a `.so` / `.dll` you want to call from APL? → `⎕NA`
- You want to bundle several functions, manage state, or use Rust crates? → `⎕LOADSO`
- You need to call into a Java codebase? → JNI bridge

---

## How native calls work internally

Before diving into syntax, it helps to understand the pipeline:

```
⎕NA 'I4 lib.so|foo I4 I4'
        │
        ▼
┌──────────────────┐
│  Parser (nadecl) │  validates grammar, builds CAbiSpec
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│  LibraryCache    │  dlopen(lib) → handle cached for process lifetime
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│  Symbol resolve  │  dlsym(handle, "foo") → function pointer
└──────┬───────────┘
       │
       ▼
┌──────────────────┐
│  CAbiBinding     │  marshals APL ValueP args → C ABI layout
│  (cabi.rs)       │  calls the function pointer
│                  │  marshals return value → APL ValueP
└──────────────────┘
```

The `LibraryCache` (`src/ffi/loader.rs`) holds loaded libraries open for the lifetime of the process, so repeated calls to `⎕NA` for the same library are cheap — the `dlopen` only happens once.

---

## C FFI (⎕NA)

### What ⎕NA does

`⎕NA` ("Native Association") is the bridge between APL and compiled C libraries. It lets you call any exported function in a shared library (`*.so` on Linux, `*.dylib` on macOS, `*.dll` on Windows) directly from an APL expression — no wrapper code, no compilation step. You write a *declaration* that tells the interpreter the function's name, the library it lives in, the types of its arguments, and the type of its return value. The interpreter then loads the library, resolves the symbol, and handles every subsequent call automatically.

This is the fastest way to reuse existing C code from APL.

### How to write a ⎕NA declaration

A declaration has the form:

```apl
name ⎕NA 'declaration_string'
```

- **`name`** — the APL name that will invoke the native function
- **`declaration_string`** — a single-quoted APL string describing the C function's signature

Inside the string, a pipe character `|` separates the *library path* from the *symbol name*. Everything after the pipe describes the return type and argument types.

### Declaration grammar

The grammar below defines every valid declaration. Read it top-to-bottom: a `decl` is an optional result type, then an optional library path with symbol name, then zero or more arguments. Each argument is a `typespec` — a type optionally preceded by a direction marker, a special string marker, a width suffix, and an array suffix.

```
decl      := [result] [pathname '|'] symbol arg*
result    := typespec
pathname  := path chars up to the LAST '|'
arg       := typespec
typespec  := [dir] [special] type [width] [array]
dir       := '<' | '>' | '='
special   := '0' | '#'
type      := I | U | C | T | F | D | J | P | A | Z | ∇ | UTF
width     := 1 | 2 | 4 | 8 | 16
array     := '[' [int] ']'
```

**How to read this:** `[brackets]` mean "optional". `|` separates alternatives. `*` means "zero or more". So a valid declaration might be as simple as `'lib.so|foo'` (no return, no args, just call a void function) or as complex as `'>I4 libc.so.6|div_t I4 <I4'` (returns a struct, takes two ints, first passed by reference).

The pipe `|` always splits library path from symbol name. The declaration parser finds the *last* `|` in the string, so your library path itself may contain pipes if needed (unlikely in practice).

### Direction markers

A `typespec` can begin with a direction marker that controls how APL values are marshalled:

| Marker | Name | Meaning |
|---|---|---|
| `<` | In | Pass a pointer *to* the APL data (the callee reads it). The APL value is not modified. |
| `>` | Out | Pass a pointer to a buffer *for the callee to fill*. The APL value is ignored on call; the buffer's contents become the result on return. |
| `=` | InOut | Pass a pointer that the callee may read *and* write. The APL value seeds the buffer; updated contents are written back. |
| *(none)* | Value | Pass the scalar by value (copy). For arrays, a pointer to the ravel is passed. |

Most arguments use **Value** (no marker) — you pass the data in and it isn't modified. Use **Out** when a C function has an output parameter (e.g., `int compute(int input, int* output)`). Use **InOut** when a function updates a buffer in place.

### Type specifications

Each argument and the return value must have a type. The type maps directly to a C type:

| Type | C type | Notes |
|---|---|---|
| `I` | `intN_t` | Signed integer |
| `U` | `uintN_t` | Unsigned integer |
| `C` | `char` | Single byte |
| `T` | `wchar_t` | Platform character width |
| `F` | `float` / `double` | IEEE-754 |
| `D` | `_Decimal128` or `decimal128` | 128-bit decimal (platform-dependent) |
| `J` | `double _Complex` | Two `double`s: real, imaginary |
| `P` | `uintptr_t` | Pointer-sized integer |
| `A` | `AplArray*` | Full APL array (ravel + shape) |
| `Z` | `AplArrayHeader*` | Array header only (no ravel) |
| `∇` | function pointer | Platform function pointer |
| `UTF8` | `char*` | NUL-terminated UTF-8 string |
| `UTF16` | `wchar_t*` | NUL-terminated UTF-16 string |

### Width suffixes

Integer and float types accept a width suffix indicating the number of bytes. Without a suffix, `I` defaults to 4 bytes and `F` defaults to 8 bytes (double).

| Type | Allowed widths |
|---|---|
| `I` (signed int) | `I1` (8-bit), `I2` (16-bit), `I4` (32-bit), `I8` (64-bit) |
| `U` (unsigned int) | `U1`, `U2`, `U4`, `U8` |
| `F` (float) | `F4` (float), `F8` (double) |

`C`, `T`, `P`, `A`, `Z`, and `∇` have fixed widths and must *not* take a suffix.

**Important:** the width must match the actual C function's signature. Declaring `I4` for a C `short` parameter will corrupt the call stack.

### Arrays

Appending `[n]` to a typespec declares a fixed-length array argument. The interpreter passes a pointer to the APL array's ravel (the flat list of elements in row-major order).

```apl
'F8[3]'    ⍝ pointer to 3 doubles (e.g., a 3D vector)
'I4[100]'  ⍝ pointer to 100 int32s
```

`[]` (empty brackets) means the length is determined at call time — the interpreter passes the APL array's actual length. This is the most common form:

```apl
'<F8[]'    ⍝ pointer to N doubles, N taken from the APL argument
```

The APL argument must be a vector (or be ravelled) when using array types.

### Strings and special markers

Two special markers modify how string types are handled:

| Marker | Meaning | Use with |
|---|---|---|
| `0` | NUL-terminated string | `C`, `UTF8` |
| `#` | Byte-counted string (length prefix) | `C`, `UTF8` |

`'0C'` means "pass a NUL-terminated `char*`." APL automatically appends `\0` to char vectors passed this way.

`'#C'` means "pass a `char*` preceded by a length integer." The interpreter prepends the byte count.

If neither marker is given, `C` is treated as a single byte, not a string.

### Structures

A structure is declared by wrapping multiple `typespec`s in curly braces:

```apl
'{F8 F8}'    ⍝ a struct containing two doubles
'{I4 I4 C}'  ⍝ a struct containing two ints and a char
```

The APL argument must be a vector whose ravel matches the struct's layout. The interpreter passes a pointer to the ravel's bytes.

### Full C FFI examples

#### Example 1: simple arithmetic

Suppose you have a small math library:

```c
// mymath.c → compile with: gcc -shared -o libmymath.so mymath.c
#include <math.h>

double hypot(double a, double b) {
    return sqrt(a*a + b*b);
}
```

Load and call it from APL:

```apl
      HYPOT ⎕NA 'F8 ./libmymath.so|hypot F8 F8'
      HYPOT 3.0 4.0
5
```

**What happened:** the declaration says "return a double (`F8`), the library is `./libmymath.so`, the symbol is `hypot`, and it takes two doubles." The interpreter loads the library once, caches the handle, and calls `hypot(3.0, 4.0)` directly.

#### Example 2: output parameter

```c
int process(const char* input, int input_len, char* output, int output_size, int* output_used);
```

```apl
      PROCESS ⎕NA 'I4 ./lib.so|process <C[] I4 >C[] I4 >I4'
```

Here `<C[]` is the input string (read-only), `I4` is the input length, `>C[]` is the output buffer (callee fills it), the second `I4` is the output buffer size, and `>I4` is where the callee writes how many bytes it actually used.

#### Example 3: struct by value

```c
typedef struct { double re, im; } Complex;
Complex mul(Complex a, Complex b) {
    Complex r = {a.re*b.re - a.im*b.im, a.re*b.im + a.im*b.re};
    return r;
}
```

```apl
      MUL ⎕NA 'F8 F8 {F8 F8} {F8 F8}'
```

**Wait — why does a struct return appear as two `F8`s?** Because the interpreter decomposes small structs into their scalar members for APL consumption. A `{F8 F8}` return becomes a 2-element float vector.

### Library loading order

The declaration's library path (the part before the `|`) is resolved in this order:

1. **Absolute path**: `'/usr/lib/x86_64-linux-gnu/libm.so.6|sin'` — loaded directly.
2. **Relative path**: `'./plugins/mylib.so|foo'` — resolved relative to the current directory.
3. **Library name only**: `'mylib.so|foo'` — searched via `LD_LIBRARY_PATH` and the system cache (`ldconfig`).
4. **Bare symbol**: `'|sin'` — the interpreter searches every library already loaded by the process (including the interpreter itself).

If no library is found, APL raises `FILE ERROR 2`.

### Troubleshooting C FFI

| Symptom | Likely cause | Fix |
|---|---|---|
| `FILE ERROR 2` | Library not found, or library has unmet dependencies | Run `ldd lib.so` to check dependencies; use absolute path |
| `VALUE ERROR` | Symbol not in the library | Check with `nm -D lib.so \| grep symbol`; remember C++ name mangling — use `extern "C"` |
| `DOMAIN ERROR` | Signature mismatch | Double-check widths and directions match the C header |
| Crash / segfault | Wrong struct layout, wrong width, or missing `extern "C"` | Re-read the C header; add `extern "C"` to C++ libraries; verify `I4` vs `I8` etc. |

---

## Java FFI (JNI)

Java interop lets APL call methods on Java objects running in a JVM. This is provided by the `crates/apl-java/` bridge crate, which creates a JVM via `JNI_CreateJavaVM` and caches resolved classes and methods.

**When to use it:** you have a Java library you want to call (e.g., enterprise APIs, Hadoop/Spark, or your own Java code). The JNI bridge is heavier than `⎕NA` because it spawns a JVM, but it lets you work with Java objects as APL values.

### Setup

```sh
cd crates/apl-java
cargo build --release
cd ../..
⎕LOADSO './target/release/libapl_java.so'
```

### Calling Java

Once loaded, you can call static Java methods by name:

```apl
      ⎕JNI 'java/lang/Math.sqrt' 2.0
1.4142135623730951
```

The bridge marshals APL numeric scalars to JNI `jdouble`, calls the method, and wraps the result back into an APL value.

### JNI internals

The bridge (in `crates/apl-java/src/lib.rs`) does the following at load time:

1. Creates a JVM with the classpath pointing to `./java/` (or the `CLASSPATH` env var).
2. Wraps the `JNIEnv*` in a thread-local so every call can use it.
3. Resolves class + method via `FindClass` / `GetStaticMethodID` and caches them.

Method resolution errors raise `DOMAIN ERROR` in APL.

---

## Rust FFI (⎕LOADSO)

### What ⎕LOADSO does

`⎕LOADSO` ("Load Shared Object") loads a Rust-compiled cdylib (dynamic library) at runtime and calls its entry point to register functions, system variables, and operators into the running interpreter. Unlike `⎕NA`, which associates *one* function per call, a single `⎕LOADSO` can register *any number* of symbols in one shot.

**Syntax:**

```apl
⎕LOADSO 'path/to/libplugin.so'
```

### Why use a cdylib plugin instead of ⎕NA

- **State:** plugins can hold Rust state (`struct` fields) between calls. `⎕NA` is stateless.
- **Convenience:** one `⎕LOADSO` registers dozens of functions. No need to `⎕NA` each one.
- **Composability:** plugins can depend on any Cargo crate (regex, reqwest, etc.) and expose a clean APL API.
- **Distribution:** ship one `.so` file; the end user does not need a Rust toolchain.

### Creating a cdylib plugin step-by-step

#### 1. Create a new Cargo project

```sh
cargo init --lib my-plugin
cd my-plugin
```

#### 2. Set the crate type to cdylib

The crate type `cdylib` produces a C-compatible dynamic library (not a Rust-specific rlib). This is what `dlopen` expects.

```toml
# my-plugin/Cargo.toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
apl-ext = { path = "../crates/apl-ext" }
```

`apl-ext` is the official extension crate. It provides the `AplPlugin` trait, the `PluginRegistrar` helper, and the `export_plugin!` macro.

#### 3. Implement the `AplPlugin` trait

Every plugin must define a struct and implement `AplPlugin`. The interpreter calls `register()` immediately after loading, passing a `PluginRegistrar` you use to add functions, system variables, and operators.

```rust
use apl_ext::{AplPlugin, PluginInfo, PluginRegistrar, AplResult, ValueP};

pub struct MyPlugin;

impl AplPlugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "my-plugin".into(),
            version: "0.1.0".into(),
            description: "My custom functions".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        // functions, sysvars, operators go here
        Ok(())
    }
}

// Required: exports the plugin entry point
apl_ext::export_plugin!(MyPlugin);
```

#### 4. Export the plugin

The `export_plugin!` macro generates an `extern "C" fn apl_init()` that the interpreter calls after `dlopen`. It creates your struct, calls `register()`, and if everything succeeds, the registered symbols are immediately available in APL.

### The `AplPlugin` trait

```rust
pub trait AplPlugin {
    /// Metadata — shown in )FNS after loading.
    fn info(&self) -> PluginInfo;

    /// Register functions, sysvars, operators here.
    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()>;

    /// Called before register(); return Err to abort load.
    fn init(&self) -> AplResult<()> { Ok(()) }

    /// Called at interpreter exit (cleanup).
    fn shutdown(&self) -> AplResult<()> { Ok(()) }
}
```

`PluginInfo` is purely informational — it shows up in `)FNS` listings so users know which plugins are loaded.

### Registering functions, sysvars, and operators

`PluginRegistrar` provides three registration methods:

| Method | What it adds |
|---|---|
| `add_function(name, closure)` | A callable APL function. The closure receives `&[ValueP]` and returns `AplResult<ValueP>`. |
| `add_sysvar(name, value)` | A read-only system variable (returns this value every time it's read). |
| `add_operator(name, closure)` | A derived operator (e.g., `myop/` for reduce). |

#### Registering a function

```rust
reg.add_function("double", |args| {
    // args[0] is the right argument; args[1] (if present) is the left argument
    let x = args[0].as_int()
        .map_err(|_| AplError::DomainError)?;
    Ok(ValueP::int(x * 2))
})?;
```

Functions can be monadic (one arg) or dyadic (two args). The closure receives both; check `args.len()` to dispatch.

#### Registering a system variable

```rust
reg.add_sysvar("MYPLUGIN.VERSION", ValueP::char_vector(b"0.1.0"))?;
```

Once registered, `⎕MYPLUGIN.VERSION` returns that value in APL expressions.

#### Registering an operator

```rust
reg.add_operator("myreduce", |f, args| {
    // Reduce-style operator: f/ vector
    let vec = args[0].as_int_vector()?;
    let mut acc = 0i64;
    for &x in vec {
        acc = f(&[ValueP::int(acc), ValueP::int(x)])?.as_int()?;
    }
    Ok(ValueP::int(acc))
})?;
```

### Full example: a statistics plugin

This plugin adds three functions (`STDDEV`, `MEAN`, `MEDIAN`) and one system variable (`STATS.VERSION`).

**`Cargo.toml`:**
```toml
[package]
name = "stats"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
apl-ext = { path = "../crates/apl-ext" }
```

**`src/lib.rs`:**
```rust
use apl_ext::{AplPlugin, PluginInfo, PluginRegistrar, AplResult, ValueP, AplError};

pub struct StatsPlugin;

impl AplPlugin for StatsPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "stats".into(),
            version: "0.1.0".into(),
            description: "Mean, median, stddev".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        // STDDEV — population standard deviation
        reg.add_function("stddev", |args| {
            let v = args[0].as_float_vector()?;
            let n = v.len() as f64;
            if n < 1.0 { return Err(AplError::DomainError); }
            let mean = v.iter().sum::<f64>() / n;
            let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
            Ok(ValueP::float(var.sqrt()))
        })?;

        // MEAN — arithmetic mean
        reg.add_function("mean", |args| {
            let v = args[0].as_float_vector()?;
            let n = v.len() as f64;
            if n < 1.0 { return Err(AplError::DomainError); }
            Ok(ValueP::float(v.iter().sum::<f64>() / n))
        })?;

        // MEDIAN — middle value of sorted data
        reg.add_function("median", |args| {
            let mut v = args[0].as_float_vector()?;
            let n = v.len();
            if n == 0 { return Err(AplError::DomainError); }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mid = n / 2;
            Ok(ValueP::float(if n % 2 == 0 {
                (v[mid - 1] + v[mid]) / 2.0
            } else {
                v[mid]
            }))
        })?;

        // Version sysvar
        reg.add_sysvar("STATS.VERSION", ValueP::char_vector(b"0.1.0"))?;

        Ok(())
    }
}

apl_ext::export_plugin!(StatsPlugin);
```

Build it:

```sh
cd stats
cargo build --release
```

### Loading and using the plugin in APL

```apl
      ⎕LOADSO './stats/target/release/libstats.so'
      DATA←1.0 2.0 3.0 4.0 5.0
      MEAN DATA
3
      STDDEV DATA
1.4142135623730951
      MEDIAN DATA
3
      STATS.VERSION
0.1.0
      )FNS               ⍝ plugin functions appear alongside ∇-defined fns
 STDDEV  MEAN  MEDIAN
```

### The `apl-ext` crate reference

The extension crate (`crates/apl-ext/`) defines:

```rust
/// Implemented by every plugin.
pub trait AplPlugin {
    fn info(&self) -> PluginInfo;
    fn register(&self, reg: &PluginRegistrar) -> AplResult<()>;
    fn init(&self) -> AplResult<()> { Ok(()) }
    fn shutdown(&self) -> AplResult<()> { Ok(()) }
}

/// Metadata shown in )FNS and )PLUGINS listings.
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

/// Passed to register(); call add_function / add_sysvar / add_operator.
pub struct PluginRegistrar<'a> {
    func_table: &'a mut FunctionTable,
    sysvars: &'a mut HashMap<String, ValueP>,
    operators: &'a mut HashMap<String, OperatorEntry>,
}
```

See `crates/demo-plugin/` for a minimal working plugin you can copy and modify.

---

## ⎕CALL — direct native call

`⎕CALL` invokes a `⎕NA`-bound function by name, reusing the binding already established:

```apl
      HYPOT ⎕NA 'F8 ./libmymath.so|hypot F8 F8'
      ⎕CALL 'HYPOT' 3.0 4.0
```

This is useful when you want to dispatch dynamically — for example, calling different C functions based on a string computed at runtime. Without `⎕CALL`, you'd have to `⎕NA` the function each time (which re-parses and re-dlopens). With `⎕CALL`, the binding is cached the first time you `⎕NA` it; subsequent `⎕CALL`s reuse it.

Internally, `⎕CALL` looks up the name in the function table, finds the `CAbiBinding`, and calls it with the supplied arguments. If the name has no binding, APL raises `VALUE ERROR`.

---

## Error handling

The interpreter converts native errors and invalid usage into standard APL error signals:

| APL error | When it happens |
|---|---|
| `FILE ERROR 2` | `dlopen` failed (library not found, architecture mismatch, missing dependency). |
| `VALUE ERROR` | `dlsym` failed (symbol not found — check `nm -D` and `extern "C"`), or the name isn't in the function table. |
| `DOMAIN ERROR` | Argument count or type doesn't match what the C function expects. |
| `SYNTAX ERROR` | The ⎕NA declaration string doesn't match the grammar. |
| `SECURITY ERROR` | Current `⎕SEC` level prohibits this operation (see next section). |
| `NOMEM` | The native code (or the interpreter's marshaler) failed to allocate memory. |

When a C function itself crashes (segfault, null deref), there is no APL-level safety net — the whole process dies. Write and test your native code thoroughly.

---

## Threading model and library lifetime

- **LibraryCache** (`src/ffi/loader.rs`) holds `dlopen` handles in a `Mutex<HashMap>`. Libraries are loaded once per process and never unloaded.
- **Native calls run synchronously on the REPL thread.** A long-running native call blocks the interpreter.
- **Plugins should be `Send + Sync`** if they share state between calls. The interpreter does not currently dispatch native calls on worker threads, but future releases may.
- **System variables** registered by plugins are stored in a `HashMap<String, ValueP>` and are read-only from APL.

---

## Security levels (⎕SEC)

`⎕SEC` controls which dangerous operations are permitted. The interpreter checks `⎕SEC` *before* dispatching to native code:

| Level | What's blocked |
|---|---|
| 0 (normal) | Everything is allowed. |
| 1 (restricted) | `⍎` (execute), `⎕NA`, `⎕LOADSO`, `)COPY`, `)INP`. |
| 2 (locked) | Everything in level 1, plus `)SAVE`, `)LOAD`, `)OUT`, `⎕FIO`. |

```apl
      ⎕SEC←1
      MYDIV ⎕NA 'I4 lib.so|div I4 I4'
SECURITY ERROR: ⎕NA is blocked at ⎕SEC=1 (requires ⎕SEC<1)
```

This lets untrusted workspaces run safely — a user can set `⎕SEC←2` to guarantee no file I/O or native code runs.

---

## See also

- [Setting up the interpreter](setup.md) — prerequisites, build instructions
- [Using the interpreter](usage.md) — APL syntax, system commands, quad functions
- `src/ffi/cabi.rs` — internal call driver (marshalling + libffi)
- `src/ffi/nadecl.rs` — ⎕NA grammar implementation
- `crates/apl-ext/` — extension trait and macro definitions
- `crates/demo-plugin/` — a minimal working plugin you can copy
