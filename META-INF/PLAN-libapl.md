# libapl C API — Implementation Plan

**Goal:** Embed the Rust APL interpreter in C/C++ programs via the standard GNU APL `libapl.h` interface.

**Scope:** ~425-line C header covering initialization, execution, value construction/destruction, read/write access, printing, UTF conversion, callbacks, and function evaluation.

## Architecture

```
rust-apl/
├── src/
│   ├── ffi/
│   │   ├── cabi.rs        ← existing: plugin FFI (extend for libapl)
│   │   └── libapl.rs      ← NEW: libapl C API implementation
│   └── ...
├── include/
│   └── libapl.h           ← NEW: C header (copy from GNU APL)
├── META-INF/
│   └── PLAN-libapl.md     ← this file
└── Cargo.toml             ← cdylib crate type
```

## Sections to implement

### 1. Initialization & lifecycle
- `init_libapl(progname, log_startup)` — initialize the interpreter
- `expand_LF_to_CRLF(on)` — toggle LF→CRLF expansion
- `disable_safe_mode()` — enable dangerous operations

### 2. Execution
- `apl_exec(line_utf8)` — execute an APL statement
- `apl_exec_ucs(line_ucs)` — execute from UCS integers
- `apl_command(command_utf8)` — run a system command like `)VARS`
- `apl_command_ucs(command_ucs)` — UCS variant
- `repl(input_buf, input_size, output_buf, output_size, error)` — REPL step
- `fix_function(function_lines_utf8)` — define a function from lines
- `fix_function_NL(function_lines_utf8)` — define from LF-separated string

### 3. Value constructors
- `int_scalar(val)` → `APL_value`
- `double_scalar(val)` → `APL_value`
- `complex_scalar(re, im)` → `APL_value`
- `char_scalar(unicode)` → `APL_value`
- `apl_value(rank, shape)` → `APL_value` (zero-initialized)
- `char_vector(str)` → `APL_value`
- `get_var_value(name)` → `APL_value` (get variable)

### 4. Value destructor
- `release_value(val)` — decrement ref count, free if 0

### 5. Read access (all ravel indices 0-based, ⎕IO←0)
- `get_rank(val)` → `int`
- `get_axis(val, axis)` → `int64_t`
- `get_element_count(val)` → `uint64_t`
- `get_type(val, idx)` → `int` (CCT_CHAR, CCT_INT, etc.)
- `get_char(val, idx)` → `int`
- `get_int(val, idx)` → `int64_t`
- `get_real(val, idx)` → `APL_Float`
- `get_imag(val, idx)` → `APL_Float`
- `get_value(val, idx)` → `APL_value` (nested)
- `is_string(val)` → `int`

### 6. Write access
- `assign_var(name, rank, shape)` → `APL_value`
- `set_char(unicode, val, idx)`
- `set_int(new_int, val, idx)`
- `set_double(new_real, val, idx)`
- `set_complex(new_real, new_imag, val, idx)`
- `set_value(new_value, val, idx)`
- `set_var_name(name, new_value)` → `int` (error code)

### 7. Printing
- `print_value(val, FILE*)`
- `print_value_to_string(val)` → `char*` (caller frees)
- `print_ucs(FILE*, string_ucs)` — debug helper

### 8. UTF conversion
- `UTF8_to_Unicode(utf, *length)` → `int`
- `Unicode_to_UTF8(unicode, dest, *length)`

### 9. Callbacks
- `res_callback` — global result callback
- `install_get_line_from_user_cb(new_cb)` → old callback

### 10. Evaluation
- `eval__fun(f)` — niladic
- `eval__fun_B(f, B)` — monadic
- `eval__A_fun_B(A, f, B)` — dyadic
- `eval__fun_X_B(f, X, B)` — monadic with axis
- `eval__A_fun_X_B(A, f, X, B)` — dyadic with axis
- `eval__L_oper_B(L, op, B)` — monadic operator
- `eval__A_L_oper_B(A, L, op, B)` — dyadic operator
- `eval__L_oper_R_B(L, op, R, B)` — dyadic operator (both operands)
- `eval__A_L_oper_R_B(A, L, op, R, B)` — dyadic operator (both operands + args)
- `eval__L_oper_X_B(L, op, X, B)` — monadic operator with axis
- `eval__A_L_oper_X_B(A, L, op, X, B)` — dyadic operator with axis
- `eval__L_oper_R_X_B(L, op, R, X, B)` — dyadic operator (both operands + axis)
- `eval__A_L_oper_R_X_B(A, L, op, R, X, B)` — dyadic operator (both operands + axis + args)

### 11. Utilities
- `get_owner_count(val)` → `int`
- `get_function_ucs(name, *L, *R)` → `APL_function`

## Implementation approach

1. **Value representation**: `APL_value` is `*mut c_void` pointing to a `ValueP` (or a wrapper struct for scalars). Reference counting via `Arc<ValueInner>`.

2. **Error handling**: All functions return `LIBAPL_error` (0 = success). Map `ErrorCode` variants to `LAE_*` codes.

3. **String handling**: Accept UTF-8, convert to Rust `String`, return allocated strings (caller frees with `free()` or `release_value()`).

4. **Global state**: Use `lazy_static!` or `once_cell` for the global `Environment`. libapl is single-threaded by default.

5. **Evaluation**: Use the existing `Environment::eval()` after parsing the expression. For operator evaluation, construct the appropriate `Expr` and evaluate.

## Tasks

1. Add `cdylib` to `Cargo.toml` and `crate-type = ["cdylib", "lib"]`
2. Create `src/ffi/libapl.rs` with all `#[no_mangle]` extern "C" functions
3. Create `include/libapl.h` (copy from GNU APL)
4. Implement initialization (`init_libapl`, `expand_LF_to_CRLF`, `disable_safe_mode`)
5. Implement execution (`apl_exec`, `apl_command`, `repl`, `fix_function`)
6. Implement value constructors (`int_scalar`, `double_scalar`, `apl_value`, etc.)
7. Implement value destructor (`release_value`)
8. Implement read access (`get_rank`, `get_axis`, `get_type`, `get_char`, etc.)
9. Implement write access (`set_char`, `set_int`, `set_double`, `set_value`, etc.)
10. Implement printing (`print_value`, `print_value_to_string`)
11. Implement UTF conversion (`UTF8_to_Unicode`, `Unicode_to_UTF8`)
12. Implement callbacks (`res_callback`, `install_get_line_from_user_cb`)
13. Implement evaluation functions (`eval__fun_B`, `eval__A_fun_B`, etc.)
14. Implement utilities (`get_owner_count`, `get_function_ucs`)
15. Test with a small C program
16. Update README

## Error codes (from GNU APL Error.def)

Need to map all `ErrorCode` variants to `LAE_*` codes. Key ones:
- `LAE_NO_ERROR = 0`
- `LAE_DOMAIN_ERROR`
- `LAE_INDEX_ERROR`
- `LAE_LENGTH_ERROR`
- `LAE_RANK_ERROR`
- `LAE_SYNTAX_ERROR`
- `LAE_VALUE_ERROR`
- `LAE_NOT_SUPPORTED`
- `LAE_VARIABLE_NOT_ASSIGNED`
- `LAE_VARIABLE_NOT_CONTEXT`
- `LAE_NOT_AXIS`
- `LAE_NOT_SINGLE_NUMERIC`
- `LAE_NOT_CHAR_VECTOR`
- `LAE_NOT_HEAT_NUMBER`
- `LAE_NOT_COMPLEX`
- `LAE_NOT_APL_VALUE`
- `LAE_NOT_APL_FUNCTION`
- `LAE_OUT_BUFFER_OVERFLOW`
- `LAE_IN_BUFFER_OVERFLOW`
