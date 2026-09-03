# FFI Reference: Extending APL with Native Code

This document covers calling native code from APL. The interpreter currently supports two foreign-function interfaces:

| Mechanism | Language | How it works |
|---|---|---|
| `⎕NA` | C | Direct call to any exported symbol in a shared library |
| `⎕NA` + JNI bridge | Java | Call Java methods via `libapl_java.so` (JVM spawned internally) |

Both use the same `⎕NA` declaration syntax and the same internal `CAbiBinding` marshalling layer.

**Prerequisites:** see `setup.md` for build instructions. For Java FFI, a JDK must be installed and `JAVA_HOME` set.

---

## Table of Contents

- [C FFI (⎕NA)](#c-ffi-38na)
  - [Quick start](#quick-start)
  - [Declaration syntax](#declaration-syntax)
  - [Direction markers](#direction-markers)
  - [Types and widths](#types-and-widths)
  - [Arrays](#arrays)
  - [Strings](#strings)
  - [Output buffers](#output-buffers)
  - [Examples](#examples)
  - [Troubleshooting](#troubleshooting)
- [Java FFI (JNI)](#java-ffi-jni)
  - [Quick start](#java-quick-start)
  - [How it works](#how-it-works)
  - [Examples](#java-examples)
- [Internal architecture](#internal-architecture)

---

## C FFI (⎕NA)

### Quick start

Given a C library `libstats.so` with functions like `double mean(double* arr, int len)`:

```apl
      'mean' ⎕NA 'F8 libstats.so|mean <F8[] I4'
      data ← 1.2 3.4 2.1 5.6
      (⊂data) mean (≢data)
3.075
```

The declaration says: return a double (`F8`), symbol is `mean` in `libstats.so`, take a double array (`<F8[]`) and an int (`I4`).

### Declaration syntax

```apl
name ⎕NA 'declaration'
```

The declaration string has the form:

```
[return_type] library|symbol arg1 arg2 ...
```

- **`return_type`** — optional, defaults to void (no return value)
- **`library`** — path to `.so` file (relative or absolute)
- **`symbol`** — exported C function name
- **`argN`** — type specifications for each argument

The pipe `|` separates library path from symbol name. The parser finds the **last** `|`, so library paths may contain pipes if needed.

### Direction markers

| Marker | Meaning | Use when |
|---|---|---|
| `<` | Input — pass pointer to APL data (read-only) | C function reads from a buffer |
| `>` | Output — pass buffer for callee to fill | C function writes results |
| `=` | Input/Output — callee reads and writes | C function updates buffer in place |
| *(none)* | By value — pass scalar directly | C function takes a value |

### Types and widths

| Type | C type | Default width |
|---|---|---|
| `I` | signed int | 4 bytes |
| `U` | unsigned int | 4 bytes |
| `F` | float/double | 8 bytes |
| `C` | char | 1 byte |
| `T` | wchar_t | platform |
| `P` | uintptr_t | platform |

Width suffix: `I4` = 32-bit int, `I8` = 64-bit int, `F4` = float, `F8` = double.

### Arrays

`<F8[]` — pointer to N doubles, N taken from the APL argument's length. The APL argument must be a vector.

`<I4[100]` — pointer to exactly 100 ints (fixed size).

### Strings

`<0C` — NUL-terminated `char*` (APL appends `\0` automatically).

`<#C` — byte-counted string (length prefix prepended).

### Output buffers

When a C function has an output parameter (e.g., `void compute(const double* in, double* out, int n)`), declare it with `>`:

```apl
      'compute' ⎕NA 'lib.so|compute <F8[] >F8[] I4'
```

The interpreter allocates a buffer, passes its address to the C function, then reads back the contents as the result.

For functions that return results through output buffers, extract with pick (`⊃`):

```apl
      r ← compute input (≢input)
      result ⊃1⊃r    ⍝ disclose the output buffer
```

### Examples

#### Scalar arithmetic

```c
// libtestmath.so
double divide(int a, int b) { return (double)a / b; }
long long add64(long long a, long long b) { return a + b; }
int clamp_u8(int v) { return v < 0 ? 0 : (v > 255 ? 255 : v); }
```

```apl
      'div' ⎕NA 'F8 libtestmath.so|divide I4 I4'
      10 div 3
3.333333333

      'add64' ⎕NA 'I8 libtestmath.so|add64 I8 I8'
      100 add64 200
300

      'clamp' ⎕NA 'U1 libtestmath.so|clamp_u8 I4'
      clamp 300
255
```

#### Array statistics

```c
// libstats.so
double mean(double* arr, int len) { /* ... */ }
double median(double* arr, int len) { /* ... */ }
double stddev(double* arr, int len) { /* ... */ }
void sort(double* arr, int len) { /* in-place */ }
```

```apl
      'mean' ⎕NA 'F8 libstats.so|mean <F8[] I4'
      data ← 1.2 3.4 2.1 5.6
      (⊂data) mean (≢data)
3.075

      'sort' ⎕NA 'libstats.so|sort =F8[] I4'
      sorted ← 3.1 1.4 1.5 9.2 6.5
      (⊂sorted) sort (≢sorted)
      sorted
1.4 1.5 3.1 6.5 9.2
```

#### Matrix operations

```c
void matmul(double* a, double* b, double* out, int rows_a, int cols_a, int cols_b);
double determinant(double* mat, int n);
void transpose(double* mat, double* out, int rows, int cols);
```

```apl
      A ← 1.0 2.0 3.0 4.0
      B ← 5.0 6.0 7.0 8.0
      C ← 0.0 0.0 0.0 0.0

      'matmul' ⎕NA 'libstats.so|matmul <F8[] <F8[] =F8[] I4 I4 I4'
      matmul (⊂A) (⊂B) (⊂C) 2 2 2
      C
19 22 43 50

      'det' ⎕NA 'F8 libstats.so|determinant <F8[] I4'
      (⊂A) det 2
¯2

      T ← 0.0 0.0 0.0 0.0
      'transpose' ⎕NA 'libstats.so|transpose <F8[] =F8[] I4 I4'
      T ← ⊃1⊃transpose (⊂A) (⊂T) 2 2
      T
1 3 2 4
```

### Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `FILE ERROR` | Library not found | Use absolute path or set `APL_LIB_PATH` |
| `VALUE ERROR` | Symbol not found | Check with `nm -D lib.so \| grep symbol` |
| `DOMAIN ERROR` | Signature mismatch | Verify widths and directions match C header |
| `INDEX ERROR` | `⎕IO` not honored | Set `⎕IO←1` for 1-based indexing |
| Crash / segfault | Wrong struct layout or width | Re-read C header; verify `I4` vs `I8` |

---

## Java FFI (JNI)

### Quick start

```sh
# Build the JNI bridge
JAVA_HOME=/path/to/jdk cargo build -p apl-java --features java

# Compile Java class
javac AplUtils.java

# Run APL
JAVA_HOME=/path/to/jdk ./target/debug/apl < examples/java-demo-sh.apl
```

### How it works

The `libapl_java.so` bridge:

1. Creates a JVM via `JNI_CreateJavaVM` on first `j_init` call
2. Caches the `JNIEnv*` for subsequent calls
3. Resolves classes/methods via `FindClass` / `GetStaticMethodID`
4. Marshals APL values to JNI types, calls the method, wraps the result back

All Java calls go through `⎕NA` declarations that reference `libapl_java.so`.

### Bridge functions

| Symbol | Signature | Purpose |
|---|---|---|
| `j_init` | `<0T → P` | Initialize JVM with classpath (string) |
| `j_new` | `<0T → P` | Create new object (class name) |
| `j_call` | `P <0T <0T I8 I8 >I8 → I4` | Call instance method |
| `j_call_s` | `P <0T <0T <I4 >0C[256] → I4` | Call String instance method |
| `j_call_static` | `<0T <0T <0T <0T <I4 >0C[256] → I4` | Call static String method |
| `j_free` | `P → I4` | Release object handle |
| `j_get_field` | `P <0T <0T >I8 → I4` | Read instance field |
| `j_set_field` | `P <0T <0T I8 → I4` | Write instance field |

### Examples

#### Setup

```apl
      'JI' ⎕NA 'P libapl_java.so|j_init <0T'
      'JS' ⎕NA 'I4 libapl_java.so|j_call_static <0T <0T <0T <0T <I4 >0C[256]'

      jptr ← JI '/path/to/class/files'
```

#### Static method calls

```apl
      r ← JS ('java/lang/System' 'getProperty' '(Ljava/lang/String;)Ljava/lang/String;' 'java.version' 256)
      ⊃2⊃r
25.0.4.1

      r ← JS ('AplUtils' 'reverse' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256)
      ⊃2⊃r
olleh

      r ← JS ('AplUtils' 'sha256' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256)
      ⊃2⊃r
2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
```

#### Important notes

- **Parentheses required:** `JS ('arg1' 'arg2' ...)` — the args must be enclosed in parentheses for strand grouping
- **Disclose result:** `⊃2⊃r` — the cabi wraps output in Pointer cells, so disclose to get the actual string
- **Set `⎕IO←1`:** For 1-based indexing of output buffers
- **Build with JAVA_HOME:** `JAVA_HOME=/path/to/jdk cargo build -p apl-java --features java`

---

## Internal architecture

```
⎕NA 'F8 lib.so|foo I4 I4'
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

**Key source files:**
- `src/ffi/cabi.rs` — call driver (marshalling, libffi)
- `src/ffi/nadecl.rs` — `⎕NA` grammar parser
- `src/ffi/loader.rs` — library cache and symbol resolution
- `crates/apl-java/src/lib.rs` — JNI bridge

**Library search order:**
1. Absolute path (`/usr/lib/lib.so`)
2. Relative path (`./lib.so`)
3. Current directory (`./lib.so`)
4. `APL_LIB_PATH` environment variable (colon-separated)
5. System search order (`LD_LIBRARY_PATH`, `ldconfig`)

---

## See also

- [Setting up the interpreter](setup.md) — build instructions, feature flags
- [Using the interpreter](usage.md) — APL syntax, system commands
- `examples/cffi-demo.apl` — complete C FFI demo
- `examples/java-demo-sh.apl` — complete Java FFI demo
- `examples/libstats.c` — example C library
- `examples/AplUtils.java` — example Java class
