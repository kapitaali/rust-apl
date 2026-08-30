# FFI Reference: Extending APL with Native Code

This document describes how to extend the Rust APL interpreter with native code in C, Java, and Rust.

## Table of Contents

- [Overview](#overview)
- [C FFI (⎕NA)](#c-ffi-na)
  - [How ⎕NA works](#how-na-works)
  - [Declaration grammar](#declaration-grammar)
  - [Type specifications](#type-specifications)
  - [Examples](#c-ffi-examples)
- [Java FFI (JNI)](#java-ffi-jni)
- [Rust FFI (⎕LOADSO)](#rust-ffi-loadso)
  - [Creating a cdylib plugin](#creating-a-cdylib-plugin)
  - [Plugin structure](#plugin-structure)
  - [Registering functions](#registering-functions)
  - [Full example](#rust-plugin-example)
- [⏍CALL — direct native call](#call-direct-native-call)
- [Error handling](#error-handling)
- [Threading model](#threading-model)

---

## Overview

The interpreter provides three mechanisms for native code:

| Mechanism | Language | Use case |
|---|---|---|
| `⎕NA` | C | Call existing C library functions directly |
| JNI (`crates/apl-java/`) | Java | Call Java methods via JNI |
| `⎕LOADSO` | Rust | Load cdylib plugins at runtime |

All native calls go through the same `CAbiBinding` interface and respect `⎕SEC` security levels.

---

## C FFI (⎕NA)

### How ⎕NA works

`⎕NA` associates a native function with an APL name:

```apl
      MYDIV ⎕NA 'I4 libc.so.6|div I4 I4'
      MYDIV 10 2
5
```

Format: `name ⎕NA 'declaration'`

The declaration parser (`src/ffi/nadecl.rs`) validates and resolves:
1. Library path → dlopen
2. Symbol name → dlsym
3. Signature validation (arg widths, directions)

### Declaration grammar

```
decl      := [result] [pathname '|'] symbol arg*
result    := typespec
pathname  := path chars up to the LAST '|'
arg       := typespec
typespec  := [dir] [special] type [width] [array]
dir       := '<' | '>' | '='       (In | Out | InOut)
special   := '0' | '#'             (NUL-terminated / byte-counted)
type      := I | U | C | T | F | D | J | P | A | Z | ∇ | UTF
width     := 1 | 2 | 4 | 8 | 16
array     := '[' [int] ']'
```

### Type specifications

| Type | Meaning | Widths |
|---|---|---|
| `I` | signed int | 1, 2, 4, 8 |
| `U` | unsigned int | 1, 2, 4, 8 |
| `C` | char | 1 |
| `T` | trans char (wchar_t) | platform |
| `F` | float | 4, 8 |
| `D` | decimal | 16 |
| `J` | complex | 16 (two f64s) |
| `P` | pointer (uintptr_t) | platform |
| `A` | APL array | — |
| `Z` | APL array header | — |
| `∇` | function pointer | platform |
| `UTF8` | UTF-8 string | — |
| `UTF16` | UTF-16 string | — |

### C FFI Examples

#### Simple arithmetic

```c
// mymath.c
int add(int a, int b) { return a + b; }
double scale(double x, double f) { return x * f; }
```

```apl
      ADD ⎕NA 'I4 mymath.so|add I4 I4'
      SCALE ⎕NA 'F8 mymath.so|scale F8 F8'
      ADD 3 4
7
      SCALE 10.5 2.0
21
```

#### Return value + output parameter

```c
// Returns count, fills buffer
int process(const char* input, char* output, int* out_len) {
    int len = strlen(input);
    *out_len = len;
    memcpy(output, input, len);
    return 0;
}
```

```apl
      PROCESS ⎕NA 'I4 mylib.so|process <C[] >C[] >I4'
```

#### Structure passing

```c
typedef struct { double x, y; } Point;
double distance(Point a, Point b) {
    double dx = a.x - b.x, dy = a.y - b.y;
    return sqrt(dx*dx + dy*dy);
}
```

```apl
      DIST ⎕NA 'F8 mylib.so|distance {F8 F8} {F8 F8}'
```

### Library loading order

1. Absolute path: `'/usr/lib/libm.so.6|sin'`
2. Relative path: `'./mylib.so|foo'`
3. OS search: `'libm.so.6|sin'` (uses `LD_LIBRARY_PATH`)
4. System default: `'|sin'` (searches all loaded libs)

### Error conditions

| Error | Cause |
|---|---|
| `FILE ERROR 2` | Library not found or missing dependencies |
| `VALUE ERROR` | Symbol not found in library |
| `DOMAIN ERROR` | Signature mismatch (wrong arg count/type) |
| `SYNTAX ERROR` | Invalid ⎕NA declaration |

---

## Java FFI (JNI)

Java interop is provided via `crates/apl-java/`.

### Setup

1. Build the JNI bridge:
```sh
cd crates/apl-java
cargo build --release
```

2. Initialize in APL:
```apl
      ⎕LOADSO 'libapl_java.so'
```

### Calling Java

```java
// MyLib.java
public class MyLib {
    public static int add(int a, int b) { return a + b; }
}
```

```java
public class Main {
    public static void main(String[] args) {
        APL apl = new APL();
        apl.eval("ADD 3 4");
    }
}
```

### JNI bridge internals

The bridge (`src/lib.rs` in `crates/apl-java/`):
- Creates a JVM via JNI `JNI_CreateJavaVM`
- Finds and caches Java classes/methods
- Marshalls APL values to JNI types
- Handles exceptions as APL errors

---

## Rust FFI (⎕LOADSO)

### Creating a cdylib plugin

#### 1. Create a new crate

```sh
cargo init --lib my-plugin
cd my-plugin
```

#### 2. Configure Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
apl-ext = { path = "../crates/apl-ext" }
```

#### 3. Plugin structure

```rust
// src/lib.rs
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
        reg.add_function("double", |args| {
            let x = args[0].as_int()?;
            Ok(ValueP::int(x * 2))
        })?;
        Ok(())
    }
}

// Required export
apl_ext::export_plugin!(MyPlugin);
```

#### 4. Registering functions

The `PluginRegistrar` provides:

| Method | Description |
|---|---|
| `add_function(name, fn)` | Register a function |
| `add_sysvar(name, value)` | Register a system variable |
| `add_operator(name, fn)` | Register an operator |

#### 5. Loading in APL

```sh
cd my-plugin
cargo build --release
```

```apl
      ⎕LOADSO './my-plugin/target/release/libmy_plugin.so'
      DOUBLE 21
42
```

### Full Rust plugin example

```toml
# my-plugin/Cargo.toml
[package]
name = "mash"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
apl-ext = { path = "../crates/apl-ext" }
```

```rust
// my-plugin/src/lib.rs
use apl_ext::{AplPlugin, PluginInfo, PluginRegistrar, AplResult, ValueP, AplError};
use std::f64::consts::PI;

pub struct MashPlugin;

impl AplPlugin for MashPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "mash".into(),
            version: "0.1.0".into(),
            description: "Math and stats functions".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        // Hypotenuse: √(x²+y²)
        reg.add_function("hypot", |args| {
            let x = args[0].as_float()?;
            let y = args[1].as_float()?;
            Ok(ValueP::float((x*x + y*y).sqrt()))
        })?;

        // Standard deviation
        reg.add_function("stddev", |args| {
            let data = args[0].as_float_vector()?;
            let n = data.len() as f64;
            if n < 2.0 { return Err(AplError::DomainError); }
            let mean = data.iter().sum::<f64>() / n;
            let variance = data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
            Ok(ValueP::float(variance.sqrt()))
        })?;

        // Register a constant
        reg.add_sysvar("MASH.PI", ValueP::float(PI))?;

        Ok(())
    }
}

apl_ext::export_plugin!(MashPlugin);
```

Usage:
```apl
      ⎕LOADSO './mash/target/release/libmash.so'
      HYPOT 3 4
5
      X←1.0 2.0 3.0 4.0 5.0
      STDDEV X
1.5811388300841898
      MASH.PI
3.14159265358979
```

### The apl-ext crate

The extension crate (`crates/apl-ext/`) provides:

```rust
pub trait AplPlugin {
    fn info(&self) -> PluginInfo;
    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()>;
    fn init(&self) -> AplResult<()> { Ok(()) }
    fn shutdown(&self) -> AplResult<()> { Ok(()) }
}

pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

pub struct PluginRegistrar<'a> {
    func_table: &'a mut FunctionTable,
    sysvars: &'a mut HashMap<String, ValueP>,
    // ...
}
```

### Demo plugin

See `crates/demo-plugin/` for a working example.

---

## ⏍CALL — direct native call

`⎕CALL` invokes a `⎕NA`-bound function by name without re-parsing the declaration:

```apl
      MYDIV ⎕NA 'I4 libc.so.6|div I4 I4'
      ⎕CALL 'MYDIV' 10 2
5
```

This is useful for programmatic dispatch (e.g., calling different native functions in a loop).

---

## Error handling

| APL Error | Native equivalent |
|---|---|
| `FILE ERROR 2` | dlopen failure (missing lib, missing deps) |
| `VALUE ERROR` | dlsym failure (symbol not found) |
| `DOMAIN ERROR` | signature mismatch, wrong arg count |
| `SYNTAX ERROR` | invalid ⎕NA declaration text |
| `SECURITY ERROR` | ⎕SEC level blocks operation |
| `NOMEM` | allocation failure |

---

## Threading model

- **LibraryCache** is protected by `Mutex`; libraries stay loaded for process lifetime
- Native calls are **not** async-blocking — they run on the REPL thread
- `⎕SEC` enforcement happens in the evaluator, not the native layer
- cdylib plugins should be `Send + Sync` if they share state

## Security levels (⎕SEC)

| Level | Blocked operations |
|---|---|
| 0 (normal) | all allowed |
| 1 (restricted) | `⍎`, `⎕NA`, `⎕LOADSO`, `)COPY`, `)INP` |
| 2 (locked) | + `)SAVE`, `)LOAD`, `)OUT`, `⎕FIO` |

```apl
      ⎕SEC←1
      ⍎'2+3'
SECURITY ERROR: EXECUTE is blocked at ⎕SEC=1 (requires ⎕SEC<1)
```

---

## See also

- [Setting up](setup.md)
- [Using the interpreter](usage.md)
- `src/ffi/cabi.rs` — call driver internals
- `src/ffi/nadecl.rs` — ⎕NA grammar
- `crates/apl-ext/` — extension trait
- `crates/demo-plugin/` — example plugin
