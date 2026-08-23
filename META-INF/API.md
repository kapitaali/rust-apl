# GNU APL 2.0 — API Reference

> **Comprehensive reference for the GNU APL 2.0 interpreter**  
> Repository: `~/Apps/apl-2.0` (GNU APL 2.0, ISO/IEC 13751, © Dr. Jürgen Sauermann)  
> Auto-generated from `src/` headers and sources.  
> **Scope:** 403 classes · 53 structs · 84 enums · 71 typedefs · 136+ functions  
> **Source files documented:** 260+ `.hh` / `.cc` / `.icc` / `.def` files under `src/`

---

## Table of Contents

1. [Overview & Architecture](#1-overview--architecture)
2. [Build System](#2-build-system)
3. [Core Types & Constants](#3-core-types--constants)
4. [The Cell Hierarchy](#4-the-cell-hierarchy)
5. [Value, Shape & Iterators](#5-value-shape--iterators)
6. [Symbols, SymbolTable & Workspace](#6-symbols-symboltable--workspace)
7. [Function Class Hierarchy](#7-function-class-hierarchy)
8. [Built-in Functions (F12 & OPER)](#8-built-in-functions-f12--oper)
9. [Quad System Functions](#9-quad-system-functions)
10. [Parser Pipeline](#10-parser-pipeline)
11. [Error, Logging & Infrastructure](#11-error-logging--infrastructure)
12. [Parallelism & Threading](#12-parallelism--threading)
13. [I/O System](#13-io-system)
14. [GUI & Plotting](#14-gui--plotting)
15. [SQL Subsystem](#15-sql-subsystem)
16. [Native Functions & Python Bridge](#16-native-functions--python-bridge)
17. [Emacs Mode](#17-emacs-mode)
18. [WebSocket / try-GNU-APL](#18-websocket--try-gnu-apl)
19. [Auxiliary Processors](#19-auxiliary-processors)
20. [Archive / Workspace Format](#20-archive--workspace-format)
21. [libapl Embedding API](#21-libapl-embedding-api)
22. [Data Structures Reference](#22-data-structures-reference)
23. [Algorithms Reference](#23-algorithms-reference)
24. [Glossary of Files](#24-glossary-of-files)


---

## 1. Overview & Architecture

GNU APL is an interpreter for ISO/IEC 13751 ("APL Extended"), implemented in C++ using a classic autotools build.

**Interpreter pipeline:**
```
APL text → Tokenizer → Parser → Prefix machine → eval_* → Result Value
```

**Core object model:**
- **`Value`** — an APL array (shape + ravel of `Cell`s)
- **`Cell`** — one element of a ravel (`CharCell`, `IntCell`, `FloatCell`, `ComplexCell`, `PointerCell`, `LvalCell`)
- **`Symbol`** — a named entity (variable, function, operator) in a symbol table
- **`Workspace`** — the global execution environment (variables, functions, state)
- **`Function`** — everything callable (primitive, defined, native, derived, user)
- **`Token`** — a lexical unit from the parser
- **`StateIndicator`** — the call stack / execution context
- **`Value_P`** — smart pointer to a `Value` (reference-counted)

**Memory model:** Values are allocated via `Value_P` constructors; the ravel lives inline for short values (up to `cfg_SHORT_VALUE_LENGTH_WANTED`) and on the heap otherwise. Cells are placement-newed into the ravel; `Cell::operator new` is intentionally unimplemented to prevent heap allocation.

**Parallelism:** Optional multi-core scalar execution via a static worker thread pool (`Parallel`, `CPU_pool`, `PJob_*`).

**Configuration:** `./configure` generates `config.h`, `Makefile` from `configure.ac`. Key `cfg_*` switches control rank, packed arrays, parallelism, SQL backends, etc.


---

## 2. Build System

| File | Role |
|---|---|
| `Makefile` | Top-level wrapper — runs `./configure` for most targets |
| `Makefile.incl` | Pre-configure targets: `help`, `develop`, `develop_lib`, `parallel`, `parallel1` |
| `configure.ac` → `configure` | Autoconf source → script; probes for optional deps (GSL, SQLite3, GTK3, PostgreSQL, Python, libapl) |
| `config.h` | Generated config (already built, dated Mar 15) |
| `aclocal.m4` | Autoconf macros |
| `aclocal.patch` | Patch for aclocal.m4 |
| `dynamic_lookup-11.patch` | macOS libtool patch |
| `debian/` | Debian packaging |
| `rpm/` | RPM packaging |

**Build targets:** `make`, `make install`, `make check`, `make distcheck`, `make parallel` (multi-core).

**Configure options** (from `README-2-configure`): `--enable-libapl`, `--with-sqlite3`, `--with-postgres`, `--with-gsl`, `--with-gtk3`, `--with-python`, `--with-qt5`, `--with-plot`, `--with-x`, `--with-cc=N`, `--with-core-count=N`, etc.


---

## 3. Core Types & Constants

### 3.1 Type Definitions (`APL_types.hh`)

| Typedef | Underlying | Meaning |
|---|---|---|
| `sRank`, `sAxis` | `int16_t` | Signed rank / axis |
| `uRank`, `uAxis` | `uint32_t` | Unsigned rank / axis |
| `AxesBitmap` | `uint16_t` | Bitmap of axes (for `fun[X]`) |
| `ShapeItem` | `int64_t` | Length of one dimension |
| `ulong` | `unsigned long` | Shorthand |
| `long_long` | `long long` | Shorthand |
| `ulong_long` | `unsigned long long` | Shorthand |
| `SI_level` | `int` | State Indicator nesting level |
| `APL_Char` | `Unicode` | One Unicode character |
| `APL_Integer` | `int64_t` | 64-bit integer |
| `APL_Float` | `double` | Floating point (or `APL_Float` class if `APL_Float_is_class=1`) |
| `APL_Complex` | `complex<APL_Float>` | Complex number |
| `APL_time_us` | `int64_t` | Microseconds since 1970 |
| `cFunction_P` | `const Function *` | Const function pointer |
| `cMonOP`, `cDyaOP` | `const Function *` | Operator pointers |
| `Depth` | `int32_t` | Nesting depth |

### 3.2 Global Enums (`APL_enums.hh`)

- **`AP_num`**: Auxiliary processor numbers. `NO_AP=-1`, `AP_NULL=0`, `AP_INTERPRETER=1000`, `AP_FIRST_USER=1001`
- **`Assign_state`**: Parser assignment state. `ASS_none`, `ASS_arrow_seen`, `ASS_var_seen`, `ASS_unknown`
- **`Bitmask`**: `BIT_0`..`BIT_31` — bit constants for `uint32_t`
- **`Cause`**: `NO_CAUSE`, `CAUSE_SHUTDOWN`, `CAUSE_ERASED`
- **`CDR_type`**: CDR record types. `CDR_BOOL1`, `CDR_INT32`, `CDR_FLT64`, `CDR_CPLX128`, `CDR_CHAR8`, `CDR_CHAR32`, `CDR_PROG64`, `CDR_NEST32`
- **`CellType`**: Cell categories. `CT_CHAR`, `CT_POINTER`, `CT_CELLREF`, `CT_INT`, `CT_FLOAT`, `CT_COMPLEX`, `CT_NUMERIC`, `CT_SIMPLE`, `CT_MASK`. Subtypes: `CTS_BIT`, `CTS_X8/U8/S8`, `CTS_X16/U16/S16`, `CTS_X32/U32/S32`, `CTS_X64/U64/S64`
- **`Col_flags`**: Print column flags. `has_j`, `real_has_E`, `imag_has_E`
- **`Comp_result`**: Comparison result. `COMP_LT=-1`, `COMP_EQ=0`, `COMP_GT=1`
- **`CoreCount`**: `CCNT_UNKNOWN=-1`, `CCNT_0`, `CCNT_1`
- **`CoreNumber`**: `CNUM_INVALID=-1`, `CNUM_MASTER=0`, `CNUM_WORKER1=1`
- **`CPU_Number`**: `CPU_0=0`
- **`CPU_count`**: `CPU_CNT_1=1`
- **`Function_Line`**: Line number in a function (0 = return line)
- **`Fun_signature`**: `SIG_Z_A_F2_B`, `SIG_Z_A_F_B`, etc.
- **`Lambda_number`**: `LAMBDA_F`, `LAMBDA_G`, etc.
- **`ListCategory`**: `LIST_NONE`, `LIST_NUM`, `LIST_CPX`
- **`LX_mode`**: Latent expression mode
- **`Multiline_status`**: `ML_NONE`, `ML_BODY`, `ML_ARGUMENT`, etc.
- **`ParseMode`**: `PM_FUNCTION`, `PM_STATEMENT`, etc.
- **`PrintStyle`**: `PS_NONE`, `PS_MATRIX`, `PS_FUNCTION`, etc.
- **`SI_mode`**: State indicator mode
- **`Silence`**: `SILENCE_OFF`, `SILENCE_ON`
- **`Sort_order`**: `SORT_ASCENDING`, `SORT_DESCENDING`
- **`Symbol_Event`**: `SE_ASSIGN`, `SE_ERASE`, `SE_PUSH`, `SE_POP`, `SE_LOCAL`, etc.
- **`TimeScale`**: `TS_SECOND`, `TS_MILLI`, `TS_MICRO`
- **`ValueFlags`**: `VF_member`, `VF_packed`, `VF_marked`, `VF_complete`
- **`VH_event`**: Value history events. `VHE_CREATED`, `VHE_ASSIGNED`, `VHE_ERASED`
- **`VoidCount`**: `VC_0`, `VC_1`

### 3.3 Unicode Constants (`Unicode.hh`)

The `Unicode` enum defines all APL characters via `Avec.def` (`char_def` / `char_uni` macros). Key values:

- **APL glyphs:** `UNI_Quad_Quad` (⎕), `UNI_Quad_Quad1` (▯), `UNI_DIAMOND` (◊), `UNI_DEL` (∇), `UNI_ALPHA` (⍺), `UNI_OMEGA` (⍵), etc.
- **Cursor control:** `UNI_EOF=-1`, `UNI_CursorUp=-2`, `UNI_CursorDown=-3`, `UNI_CursorRight=-4`, `UNI_CursorLeft=-5`, `UNI_CursorEnd=-6`, `UNI_CursorHome=-7`, `UNI_InsertMode=-8`
- **Padding:** `UNI_iPAD_U0`..`UNI_iPAD_L9` — internal spacing characters
- **Diffout:** `UNI_DIFF_DIGITS`, `UNI_DIFF_SPACES`, `UNI_DIFF_REAL`, etc. (used in testcase comparison)

**Inline helpers:** `nibble(Unicode)`, `sixbit(Unicode)`, `is_iPAD_char(Unicode)`

### 3.4 APL_Float as Class (`APL_Float_as_class.hh`)

When `APL_Float_is_class=1`, `APL_Float` wraps `APL_Float_Base` (a `double`). Provides:
- `APL_Float_Base::_get()`, `_set(double)`, `operator APL_Float()`, `operator double()`
- Wrap macros: `wrap1(type, fun)` and `wrap2(type, fun)` for math functions

**Required API for a custom APL_Float class:**
```
abs, acos, asin, asinh, atan, ceil, cos, cosh, acosh, exp, floor, isfinite, log, round, sqrt, sin, sinh, tan, tgamma, tanh
atan2, fmod, pow
operator-, operator+, operator<, >, <=, >=, ==, !=, is_normal
operator+=, -=, *=, /=
```

### 3.5 Structs

- **`AP_num3`** — three processor numbers (proc, parent, grand)
- **`Function_PC2`** — two Function_PCs marking a range in a function body
- **`labVal`** — a label: symbol + line number
- **`_twc`** — iterator state: to, weight, current
- **`ravel_comp_len`** — CDR comparison length


---

## 4. The Cell Hierarchy

Every element of an APL array's ravel is a `Cell`. Cells are never `new`'d directly — they're placement-newed into the owning `Value`'s ravel.

```
Cell (abstract base)
├── CharCell          (Unicode)
├── NumericCell
│   ├── RealCell
│   │   ├── IntCell  (APL_Integer)
│   │   └── FloatCell (APL_Float or rational)
│   └── ComplexCell   (APL_Float[2])
├── PointerCell       (Value* for nested arrays)
└── LvalCell          (Cell* for selective assignment)
```

### 4.1 `Cell` (base class)

**Key virtual methods:**
- `init(const Cell & other, Value & owner, const char * loc)` — deep copy via `init_other()`
- `init_other(void * other, Value & owner, const char * loc)` — placement new
- `init_from_value(Value * value, Value & owner, const char * loc)` — from a Value
- `greater(const Cell & other)` — ordering: PointerCell > NumericCell > CharCell
- `A_greater_B(A, B, unused)` — static comparator for Heapsort
- `equal(const Cell & other, double qct)` — tolerant equality
- `to_value(const char * loc)` — extract Value_P
- `init_type(const Cell & other, Value & owner, const char * loc)` — set type only

**Static helpers:** `same_half_plane(A, B)`, `tolerantly_equal(A, B, qct)`, `integral_within(A, qct)`

**Value accessors** (throw DOMAIN_ERROR on wrong type): `get_char_value()`, `get_byte_value()`, `get_int_value()`, `get_real_value()`, `get_imag_value()`, `get_complex_value()`, `get_pointer_value()`, `get_lval_value()`

**Near-X tests:** `get_near_bool()`, `get_near_int()`, `get_checked_near_int()`, `is_near_zero()`, `is_near_one()`, `is_near_bool()`, `is_near_int()`, `is_near_int64_t()`, `is_near_real()`

**Other:** `CDR_size()`, `release()`, `is_integer_cell()`, `is_simple_cell()`, `is_float_cell()`, `is_character_cell()`, `is_pointer_cell()`, `is_lval_cell()`, `is_numeric()`, `is_member_anchor()`, `get_cell_type()`, `get_cell_subtype()`, `get_classname()`, `character_representation()`, `need_scaling()`, `deep_cell_types()`, `deep_cell_subtypes()`, `bif_*()` (all scalar primitive operations)

**Placement new:** `operator new` declared but not implemented. `operator =` likewise.

### 4.2 `CharCell : public Cell`

Stores a single Unicode character.
- **Constructors:** `CharCell(Unicode av)`
- **Key methods:** `get_char_value()`, `get_byte_value()`, `is_character_cell()`, `zU(Cell * Z, Unicode uni)` — placement init (friend-only in libapl)
- **Bitwise:** `bif_not_bitwise()`, `bif_and_bitwise()`, `bif_or_bitwise()`, `bif_equal_bitwise()`, `bif_not_equal_bitwise()`

### 4.3 `NumericCell : public Cell`

Base for `RealCell` and `ComplexCell`. Implements all operations shared by numeric types.
- **Static:** `zV(Cell * Z, APL_Float)`, `zV(Cell * Z, APL_Complex)`, `zV(Cell * Z, APL_Float real, APL_Float imag)`
- **Operations:** `bif_not()`, `bif_not_bitwise()`, `bif_and()`, `bif_and_bitwise()`, `bif_equal_bitwise()`, `bif_not_equal_bitwise()`, `bif_binomial()`, `bif_nand()`, `bif_nand_bitwise()`, `bif_nor()`, `bif_nor_bitwise()`, `bif_or()`, `bif_or_bitwise()`
- **Binomial:** `K33_binomial(Z, N, K, negate)`, `complex_binomial()`, `real_binomial()`, `integer_binomial(Z, N, K, negate)`
- **GCD:** `int_gcd()`, `flt_gcd()`, `cpx_gcd()`
- **Helpers:** `cpx_max_real(a)`

### 4.4 `RealCell : public NumericCell`

Base for `IntCell` and `FloatCell`. Real-only operations:
- `bif_circle_fun()`, `bif_circle_fun_inverse()`, `do_bif_circle_fun()`, `bif_logarithm()`

### 4.5 `IntCell : public RealCell`

Stores an `APL_Integer` (`value.ival`).
- **Constructors:** `IntCell()`, `IntCell(APL_Integer i)`
- **Statics:** `boolean_FALSE`, `boolean_TRUE`
- **All bif_* operations:** `bif_add`, `bif_subtract`, `bif_multiply`, `bif_divide`, `bif_ceiling`, `bif_floor`, `bif_magnitude`, `bif_exponential`, `bif_factorial`, `bif_power`, `bif_nat_log`, `bif_negative`, `bif_pi_times`, `bif_pi_times_inverse`, `bif_reciprocal`, `bif_roll`, `bif_maximum`, `bif_minimum`, `bif_residue`, `bif_near_int64_t`, `bif_within_quad_CT`, `bif_add_inverse`, `bif_multiply_inverse`
- **Helpers:** `swap_ivals(IntCell & other)` for heapsort, `get_int_value()`, `set_int_value()`
- **Rational support:** `get_numerator()`, `get_denominator()` (when `cfg_RATIONAL_NUMBERS_WANTED`)

### 4.6 `FloatCell : public RealCell`

Stores `APL_Float` or a rational (`value.fval.u1.num / value.fval.denominator`).
- **Constructors:** `FloatCell(APL_Float r)`, `FloatCell(APL_Integer numer, APL_Integer denom)` (rational)
- **`dfval()`** — get as double (handles rational)
- **`is_finite()`**, `get_numerator()`, `get_denominator()`
- **All bif_* operations** (same set as IntCell)
- **`need_scaling()`** — true for floats that need exponential format
- **Release:** `release()` when `APL_Float_is_class` — calls `release_APL_Float()`

### 4.7 `ComplexCell : public NumericCell`

Stores `APL_Float[2]` (real, imag).
- **Constructors:** `ComplexCell(APL_Complex c)`, `ComplexCell(APL_Float r, APL_Float i)`
- **`mag2()`** — |z|²
- **`gamma()`** — Lanczos approximation for Γ(x+iy)
- **`do_bif_circle_fun(Z, fun, b)`** — all 30+ circle functions
- **All bif_* operations:** `bif_add`, `bif_subtract`, `bif_multiply`, `bif_divide`, `bif_ceiling`, `bif_conjugate`, `bif_direction`, `bif_exponential`, `bif_factorial`, `bif_floor`, `bif_magnitude`, `bif_nat_log`, `bif_negative`, `bif_pi_times`, `bif_pi_times_inverse`, `bif_reciprocal`, `bif_roll`, `bif_power`, `bif_equal`, `bif_logarithm`, `bif_maximum`, `bif_minimum`, `bif_residue`, `bif_circle_fun`, `bif_circle_fun_inverse`, `bif_add_inverse`, `bif_multiply_inverse`
- **Statics:** `ONE()`, `PLUS_i()`, `MINUS_i()`, `zC(Cell * Z, APL_Float r, APL_Float j)`

### 4.8 `PointerCell : public Cell`

Points to a nested `Value`.
- **Constructors:** `PointerCell(Value * val, Value & owner)`, `(Value * val, Value & owner, uint32_t magic)` (allows simple scalar)
- **`get_pointer_value()`** — returns `Value_P`
- **`get_cell_owner()`** — returns `Value *`
- **`isolate()`** — make sole owner
- **`isolate_deep()`** — isolate value + all sub-values
- **`release()`** — decrement owner count
- **`is_member_anchor()`** — true if this value is a workspace member
- **`deep_cell_types()`**, **`deep_cell_subtypes()`** — aggregate cell types in nested value

### 4.9 `LvalCell : public Cell`

Points to another `Cell` — used for selective assignment (e.g. `(⌷A)←B`).
- **Constructors:** `LvalCell(Cell * cell, Value * cell_owner)`, `LvalCell(const LvalCell & other)`
- **`get_lval_value()`** — returns `Cell *`
- **`get_cell_owner()`** — returns `Value *`
- **`check_consistency()`** — assert owner owns this cell
- **`cLvalCell()`**, **`vLvalCell()`** — downcasts


---

## 5. Value, Shape & Iterators

### 5.1 `Shape` (`Shape.hh/cc`)

Represents the shape (rank + dimensions) of an APL value. Stored inline, never heap-allocated.

**Members:** `uRank rho_rho` (rank), `ShapeItem rho[MAX_RANK]`, `ShapeItem volume`

**Constructors:**
- `Shape()` — scalar (rank 0, volume 1)
- `Shape(ShapeItem len)` — vector
- `Shape(rows, cols)` — matrix
- `Shape(height, rows, cols)` — cube
- `Shape(uRank rk, const ShapeItem * sh)` — arbitrary
- `Shape(const Shape & other)` — copy
- `Shape(const Value & A, int qio_A)` — from APL value (used by `↑`)

**Key methods:**
- `abs()` — negate negative dimensions
- `frame_shape(uRank cnt)` — upper `cnt` dimensions
- `chunk_shape(uRank cnt)` — lower `cnt` dimensions
- `operator+(const Shape & lower)` — catenate shapes
- `get_rank()`, `get_shape_item(r)`, `get_transposed_shape_item(r)`
- `get_first_shape_item()`, `get_last_shape_item()`, `get_cols()`, `get_rows()`
- `set_shape_item(r, sh)`, `recompute_volume()`, `increment_shape_item(r)`
- `add_shape_item(len)`, `expand_rank(new_rank)`
- `expand(const Shape & B)` — expand rank and axes
- `insert_axis(axis, len)`, `without_axis(axis)`, `without_first_axis()`, `without_last_axis()`
- `is_empty()`, `get_volume()`
- `operator==`, `operator!=`

### 5.2 `Value` (`Value.hh/cc/.icc`)

The fundamental APL array type. Inherits `DynamicObject`.

**Storage layout:**
```cpp
Shape shape;
const Cell & (*fetcher)(ShapeItem offset, const Cell * ravel);
int owner_count;
ShapeItem pointer_cell_count;
uint16_t flags;     // VF_member, VF_packed, VF_marked, VF_complete
ShapeItem valid_ravel_items;
ShapeItem nz_subcell_count;
Cell * ravel;       // points to short_value[] or heap
```

**Constructors** (all protected — use `Value_P`):
- `Value(loc)` — scalar
- `Value(const Cell & cell, loc)` — scalar from cell
- `Value(ShapeItem len, loc)` — vector
- `Value(rows, cols, loc)` — matrix
- `Value(const Shape & sh, loc)` — general
- `Value(const Shape & sh, uint64_t * bits, loc)` — packed
- `Value(const UCS_string & ucs, loc)` — char vector
- `Value(const UTF8_string & utf, loc)` — char vector
- `Value(const CDR_string & cdr, loc)` — char vector
- `Value(const PrintBuffer & pb, loc)` — char matrix
- `Value(loc, const Shape * sh)` — shape as int vector

**Type tests:** `is_scalar()`, `is_simple_scalar()`, `is_numeric_scalar()`, `is_character_scalar()`, `is_empty()`, `is_zilde()`, `is_str0()`, `is_vector()`, `is_scalar_or_vector()`, `is_scalar_extensible()`, `is_char_string()`, `is_char_vector()`, `is_apl_char_vector()`, `is_char_array()`, `is_int_scalar()`, `is_deep()`

**Accessors:** `element_count()`, `nz_element_count()`, `get_shape()`, `get_shape_item(r)`, `get_rank()`, `get_cols()`, `get_rows()`, `get_first()`, `get_cfirst()`, `get_expand_Cell()`, `get_lval_cellowner()`

**Mutation:** `set_shape_item()`, `get()`, `Z()`, `operator[](ShapeItem)`, `set_owner_count()`, `add_owner()`, `set_owner()`, `get_owner_count()`, `get_pointer_cell_count()`

**Static:** `init()`, `short_value_length()`, `get_eri()`, `get_ravel_is_Pointer()`, `set_ravel_is_Pointer()`, `get_ravel_is_T()`, `set_ravel_is_T()`, `get_ravel_is_Char()`, `set_ravel_is_Char()`, `get_packed()`, `set_packed()`

### 5.3 `Value_P` and `Value_P_Base` (`Value_P.hh/.icc`)

Smart pointer for `Value`. Reference-counted ownership.

**`Value_P_Base`** (non-constructing operations):
- `reset()` — decrement owner, clear
- `clear(loc)` — reset + value event
- `operator+()`, `operator!()` — validity tests
- `operator->()`, `operator*()` — access
- `get()` — raw pointer
- `init_pointer()`, `clear_pointer(loc)`, `isolate(loc)`, `isolate_deep(loc)`, `move(other, loc)`

**`Value_P`** (constructors + destructor):
- `Value_P()` — null
- `Value_P(const char * loc)` — scalar
- `Value_P(const Cell & cell, loc)` — scalar from cell
- `Value_P(ShapeItem len, loc)` — vector
- `Value_P(rows, cols, loc)` — matrix
- `Value_P(const Shape & sh, loc)` — general
- `Value_P(const Shape & sh, uint64_t * bits, loc)` — packed
- `Value_P(const UCS_string & ucs, loc)` — char vector
- `Value_P(const UTF8_string & utf, loc)` — char vector
- `Value_P(const CDR_string & cdr, loc)` — char vector
- `Value_P(const PrintBuffer & pb, loc)` — char matrix
- `Value_P(loc, const Shape * sh)` — shape as vector
- `Value_P(Value * val, loc)` — from raw pointer
- `Value_P(const Value_P & other, loc)` — copy
- `Value_P(const Value_P & other)` — copy
- `operator=(const Value_P & other)` — assignment
- `~Value_P()` — destructor (decrements owner)

### 5.4 Iterators

- **`ArrayIterator`** (`ArrayIterator.hh`) — iterates a multi-dimensional shape along axes, tracking `_twc` state
- **`AxisIterator`** (`ArrayIterator.hh`) — iterates one axis
- **`IndexIterator`** (`IndexIterator.hh/cc`) — iterates index expressions
- **`IndexExpr`** (`IndexExpr.hh/cc`) — represents an APL index expression (e.g. `A[1;2 3]`)


---

## 6. Symbols, SymbolTable & Workspace

### 6.1 `Symbol` (`Symbol.hh/cc`)

A named entity in the workspace. Inherits `NamedObject`.

**Members:** value stack, assignment state, localization level, trace/stop points.

**Key methods:** `assign(Value_P val)`, `assign_indexed()`, `pop()`, `localize()`, `unlocalize()`, `is_erased()`, `is_local()`, `is_global()`, `is_shared()`, `is_assigned()`, `get_value()`, `get_symbol_pos()`, `get_name()`, `get_function()`, `is_macro()`, `is_lambda()`, `set_trace()`, `set_stop()`, `get_trace()`, `get_stop()`

**Subclasses:**
- **`LAMBDA`** — lambda result
- **`ALPHA`** / **`ALPHA_U`** — ⍺ / ⍶
- **`CHI`** — χ
- **`OMEGA`** / **`OMEGA_U`** — ⍵ / ⍹
- **`SystemVariable`** — all ⎕xx names

### 6.2 `SystemVariable` (`SystemVariable.hh/cc/.def`)

System variables and functions. Three flavors:
- **`NL_SystemVariable`** — cannot be localized (push/pop no-op)
  - **`RO_SystemVariable`** — additionally read-only
    - `Quad_L`, `Quad_R`, `Quad_X`, `Quad_LX`, `Quad_SYL`
  - Localizable: `Quad_CT`, `Quad_FC`, `Quad_IO`, `Quad_PP`, `Quad_PR`, `Quad_PS`, `Quad_PW`, `Quad_TZ`
  - **`Quad_Quad`** (⎕) and **`Quad_QUOTE`** (⍞) — evaluated I/O

### 6.3 `SymbolTable` and `SystemSymTab` (`SymbolTable.hh/cc`)

- **`SymbolTable`** : `SymbolTableBase<Symbol, SYMBOL_HASH_TABLE_SIZE>` — user names
- **`SystemSymTab`** : `SymbolTableBase<SystemName, 255>` — ⎕xx names
- **`SystemName`** — entry of `SystemSymTab`

### 6.4 `Workspace` (`Workspace.hh/cc/.icc`)

The singleton execution environment. Contains a `SymbolTable` and `SystemSymTab`.

**Key methods:** `get_IO()`, `get_CT()`, `get_FC(n)`, `get_PP()`, `get_PW()`, `get_PR()`, `get_PS()`, `get_TZ()`, `get_WA()`, `get_RL()`, `get_AV()`, `get_v_Quad_X()`, `get_symbol()`, `clear_symbols()`, `list()`, `save()`, `load()`, `copy()`, `erase()`

### 6.5 `NamedObject` (`NamedObject.hh/cc`)

Base class for anything with a name (Id).

### 6.6 `ValueHistory` (`ValueHistory.hh/cc`)

Ring buffer of recent value assignments (configurable size via `VALUE_HISTORY`).

**Events:** `VHE_CREATED`, `VHE_ASSIGNED`, `VHE_ERASED`


---

## 7. Function Class Hierarchy

The complete function hierarchy in `src/`:

```
NamedObject ──┐
              ├── Function (abstract base of ALL functions/operators)
FunctionGroup─┘   ├── PrimitiveFunction (built-ins)
                  │   ├── NonscalarFunction (⍎ ⌷ ∈ ≡ ≢ ⊤ ⊥ ⌽ ⊖ ⍉ ⍴ ∪ ∩ ⊣ ⊢ ...)
                  │   │   └── NonscalarFunction_default_identity (⌽ ⊖ ⍉ ⍴)
                  │   ├── ScalarFunction (+ - × ÷ ! ⋆ ○ ⌈ ⌊ ∣ ⍟ = < ≤ > ≥ ≠ ∧ ∨ ...)
                  │   └── PrimitiveOperator (all primitive operators)
                  ├── DerivedFunction (bound operators/axes)
                  │   ├── Derived_LO_M (LO M, e.g. +/)
                  │   ├── Derived_LO_M_X (LO M[X])
                  │   ├── Derived_LO_D_RO (LO D RO, e.g. +.×)
                  │   ├── Derived_LO_D_X_RO (LO D[X] RO)
                  │   └── Derived_F_X (F[X], plain function + axis)
                  ├── NativeFunction (C++ function from .so)
                  ├── UserFunction (defined function)
                  ├── Macro, Lambda, Nabla
                  └── QuadFunction (system functions)
```

### 7.1 `Function` (`Function.hh/cc`)

**Virtual eval entry points:** `eval()`, `eval_B()`, `eval_AB()`, `eval_XB()`, `eval_AXB()`, `eval_LB()`, `eval_LXB()`, `eval_ALB()`, `eval_ALXB()`, `eval_LRB()`, `eval_LRXB()`, `eval_ALRB()`, `eval_ALRXB()`

**Key methods:** `get_arity()`, `get_valence()`, `has_result()`, `has_alpha()`, `get_name()`, `get_token_tag()`, `is_macro()`, `is_lambda()`, `is_native()`, `is_user_function()`, `is_quad()`, `is_primitive()`, `is_derived()`, `is_scalar()`, `is_nonscalar()`, `is_operator()`, `print()`, `get_signature()`, `get_PC()`, `set_PC()`, `get_loc()`, `get_body()`

### 7.2 `PrimitiveFunction` (`PrimitiveFunction.hh/cc`)

Built-in functions. Static singletons `fun` for each token tag.

### 7.3 `ScalarFunction` (`ScalarFunction.hh/cc`)

All scalar primitives. Entry points: `scalar_eval_AB()`, `scalar_eval_AXB()`, `scalar_eval_LB()`, `scalar_eval_LXB()`, `scalar_eval_ALB()`, `scalar_eval_ALXB()`, `scalar_eval_LRB()`, `scalar_eval_LRXB()`, `scalar_eval_ALRB()`, `scalar_eval_ALRXB()`

**Parallel execution:** Each entry dispatches to `ScalarFunction::eval()` which may run in parallel via the `Parallel` pool.

### 7.4 `PrimitiveOperator` (`PrimitiveOperator.hh`)

All primitive operators (/ ⌿ \ ⍀ ¨ ⍣ ⍤ .).

### 7.5 `DerivedFunction` (`DerivedFunction.hh/cc`)

Cache of bound operator/axis combinations. `DerivedFunctionCache` arena of `MAX_FUN_OPER`.

### 7.6 `NativeFunction` (`NativeFunction.hh/cc`)

C++ function loaded from a `.so` shared library via `dlopen()`.

### 7.7 `UserFunction` (`UserFunction.hh/cc`)

Defined function (∇ editor). Body is a token string.

### 7.8 `Macro` (`Macro.hh/cc/.def`)

System operators implemented as hidden APL functions.

### 7.9 `Nabla` (`Nabla.hh/cc`)

The ∇ full-screen function editor.

### 7.10 `QuadFunction` (`QuadFunction.hh/cc`)

Base class for all `⎕XX` system functions. Always returns a result (`has_result() = true`). Wrong valence raises `VALENCE_ERROR`.

### 7.11 `FunctionGroup`

Standalone mixin for function groups (⌹ ⎕CR ⎕FFT FIO ⎕MX ⎕RVAL ⎕SQL).

### 7.12 `NamedObject`, `Executable`

`Executable` — base for anything that can be executed (functions, operators, etc.)


---

## 8. Built-in Functions (F12 & OPER)

### 8.1 F12 Functions (`Bif_F12_*.hh/cc`)

All derive from `PrimitiveFunction`. Override `eval_B()`, `eval_AB()`, `eval_XB()`, `eval_AXB()`. Each has a static `fun` instance bound to a `TokenTag`.

| Symbol | Class | File | Semantics |
|---|---|---|---|
| `,` `⍪` | `Bif_F12_COMMA` / `Bif_F12_COMMA1` | `Bif_F12_COMMA.cc/.hh` | Ravel, catenate, laminate |
| `⌹` | `Bif_F12_DOMINO` | `Bif_F12_DOMINO.cc/.hh` | Matrix divide, matrix inverse, QR factorization |
| `⍕` | `Bif_F12_FORMAT` | `Bif_F12_FORMAT.cc/.hh` | Format (monadic: ⍕B; dyadic: A ⍕B) |
| `⍳` | `Bif_F12_INDEX_OF` | `Bif_F12_INDEX_OF.cc/.hh` | Index of (dyadic ⍳) |
| `⍸` | `Bif_F12_INTERVAL_INDEX` | `Bif_F12_INTERVAL_INDEX.cc/.hh` | Interval index |
| `⊂` `⊃` | `Bif_F12_PARTITION` / `Bif_F12_PICK` | `Bif_F12_PARTITION_PICK.cc/.hh` | Partition, pick |
| `⍋` `⍒` | `Bif_F12_SORT` / `Bif_F12_SORT2` | `Bif_F12_SORT.cc/.hh` | Grade up/down |
| `↑` `↓` | `Bif_F12_TAKE_DROP` | `Bif_F12_TAKE_DROP.cc/.hh` | Take, drop |

**`Bif_F12_COMMA` details:**
- Monadic `,B` — ravel
- `A,B` — catenate along last axis
- `A,[X]B` — catenate or laminate with axis

**`Bif_F12_DOMINO` details:**
- Monadic `⌹B` — matrix inverse
- `A⌹B` — matrix divide (solves A = X⌹B using QR)
- Uses `QR_factorization_GSL.cc` or `LApack.cc`

**`Bif_F12_SORT` details:**
- `⍋B` — grade up (returns permutation)
- `⍒B` — grade down
- Uses `Heapsort<Cell>::sort()` with `Cell::greater()`

### 8.2 Primitive Operators (`Bif_OPER1_*.hh/cc`, `Bif_OPER2_*.hh/cc`)

| Symbol | Class | File | Semantics |
|---|---|---|---|
| `⍨` | `Bif_OPER1_COMMUTE` | `Bif_OPER1_COMMUTE.cc/.hh` | Commute/duplicate |
| `¨` | `Bif_OPER1_EACH` | `Bif_OPER1_EACH.cc/.hh` | Each (map over elements) |
| `/` `⌿` `\` `⍀` | `Bif_OPER1_REDUCE` | `Bif_OPER1_REDUCE.cc/.hh` | Reduce (first axis) |
| `⍀` | `Bif_OPER1_SCAN` | `Bif_OPER1_SCAN.cc/.hh` | Scan (prefix reduction) |
| `.` | `Bif_OPER2_INNER` | `Bif_OPER2_INNER.cc/.hh` | Inner product |
| `∘.` | `Bif_OPER2_OUTER` | `Bif_OPER2_OUTER.cc/.hh` | Outer product |
| `⍣` | `Bif_OPER2_POWER` | `Bif_OPER2_POWER.cc/.hh` | Power (function iteration) |
| `⍤` | `Bif_OPER2_RANK` | `Bif_OPER2_RANK.cc/.hh` | Rank |

**Common args:** `LO` (left function operand), `RO` (right function operand), `A` (left value), `B` (right value), `X` (axis). `Shape3(shape, axis)` splits shape into `(h, m, l)` — above axis, axis itself, below.


---

## 9. Quad System Functions

All live in `Quad_*.hh/cc`. Each is a singleton `static Quad_XX fun;` of a class deriving from `QuadFunction`. Function groups use `subfunction_infos[]` from a `.def` file.

### 9.1 Function Groups

| System Function | File | Sub-functions |
|---|---|---|
| `⎕CR` | `Quad_CR.cc/.hh/.def` | `display`, `plain`, `box`, `box8`, `html`, `latex`, `json`, `xml`, `tm`, `boxU`, `htmlU`, `jsonU`, `xmlU` |
| `⎕FIO` | `Quad_FIO.cc/.hh/.def` | `errno`, `strerror`, `fopen`, `fclose`, `fread`, `fwrite`, `fgets`, `fgetc`, `feof`, `ferror`, `ftell`, `fseek`, `fflush`, `fsync`, `fstat`, `unlink`, `mkdir`, `rmdir`, `printf`, `sprintf`, `write`, `read`, `open`, `close`, `socket`, `connect`, `bind`, `listen`, `accept`, `send`, `recv`, `sendto`, `recvfrom`, `poll`, `getsockopt`, `setsockopt`, `gethostbyname`, `getservbyname`, `gettimeofday`, `gmtime`, `localtime`, `mktime`, `strftime`, `statvfs`, `readlink`, `symlink`, `realpath`, `glob`, `opendir`, `readdir`, `closedir` |
| `⎕MX` | `Quad_MX.cc/.hh/.def` | `matmul`, `inv`, `det`, `rank`, `cond`, `norm`, `svd`, `eig`, `qr`, `lu`, `chol`, `schur`, `expm`, `logm`, `sqrtm`, `sqrtn` |
| `⎕PLOT` | `Quad_PLOT.cc/.hh/.def` | `setup`, `data`, `render`, `save`, `clear`, `close`, `list`, `help` |
| `⎕RE` | `Quad_RE.cc/.hh` | `compile`, `exec`, `match`, `replace`, `split`, `capture`, `named_capture`, `count`, `positions` |

### 9.2 Standalone Functions

| System Function | File | Purpose |
|---|---|---|
| `⎕CC` | `Quad_CC.cc/.hh` | Character classification |
| `⎕DLX` | `Quad_DLX.cc/.hh` | Knuth's Dancing Links (Algorithm X) |
| `⎕FFT` | `Quad_FFT.cc/.hh` | Fast Fourier Transform |
| `⎕FX` | `Quad_FX.cc/.hh` | Fix (compile) a function from text |
| `⎕GTK` | `Quad_GTK.cc/.hh` | GTK3 GUI interface (open window, widget calls, events) |
| `⎕JSON` | `Quad_JSON.cc/.hh` | JSON parsing and generation |
| `⎕MAP` | `Quad_MAP.cc/.hh` | Map (transform) over arrays |
| `⎕PNG` | `Quad_PNG.cc/.hh` | PNG image loading/saving |
| `⎕RL` | `Quad_RL.cc/.hh` | Random Link (random number generator) |
| `⎕RVAL` | `Quad_RVAL.cc/.hh` | Random value generation |
| `⎕SQL` | `Quad_SQL.cc/.hh` | SQL database interface |
| `⎕SVx` | `Quad_SVx.cc/.hh` | Shared variable operations |
| `⎕TF` | `Quad_TF.cc/.hh` | Transfer format (APL ↔ text conversion) |
| `⎕WA` | `Quad_WA.cc/.hh` | Workspace available (memory info) |
| `⎕XML` | `Quad_XML.cc/.hh` | XML parsing and generation |


---

## 10. Parser Pipeline

### 10.1 `Token` (`Token.hh/cc/.def`)

A lexical unit. Contains `TokenTag tag`, `Function_PC pc`, source location.

**Key methods:** `get_tag()`, `get_int_value()`, `get_float_value()`, `get_complex_value()`, `get_string_value()`, `get_symbol()`, `get_function()`, `get_apl_val()`, `set_tag()`, `set_int_value()`, `get_token_name()`, `operator<<`

**`TokenTag`** — 32-bit tag encoding token type (high 16 bits = Id, low 16 bits = flags). Built from `Token.def` via `tok_def()` macro.

### 10.2 `Token_string` (`Token_string.hh/cc`)

String of tokens. Used for function bodies, value history, etc.

### 10.3 `Tokenizer` (`Tokenizer.hh/cc`)

Lexical analyzer. Converts Unicode input to tokens.

**Key methods:** `tokenize(const UCS_string & input, Token_string & result)`, `push()` / `pop()` for state

### 10.4 `Parser` (`Parser.hh/cc`)

Recursive-descent parser for APL statements. Produces a `Prefix` tree.

### 10.5 `Prefix` (`Prefix.hh/cc/.cc/.def`)

The prefix-machine interpreter. Walks the token tree and evaluates.

**Key structures:**
- `Prefix(Parser & parser, Token_string & body)` — constructor from parsed body
- `reduce()` — reduce the prefix expression
- States: `P_, A_, B_, L_, R_, M_, D_, E_, F_, G_, etc.` (prefix parser states)

### 10.6 Enums

- **`ParseMode`**: `PM_FUNCTION`, `PM_STATEMENT`, `PM_PARENT`, etc.
- **`Function_Line`**: line number (0 = return line)
- **`Fun_signature`**: `SIG_Z_A_F2_B`, `SIG_Z_A_F_B`, `SIG_Z_A_B`, etc.


---

## 11. Error, Logging & Infrastructure

### 11.1 Error Handling (`Error.hh/cc/.def`)

- **`ErrorCode`** enum — generated from `Error.def` via X-macro. Each code has form `(maj << 16) | min`. Categories: `E_` prefix (e.g., `E_SYNTAX_ERROR`, `E_DOMAIN_ERROR`, `E_LENGTH_ERROR`, `E_RANK_ERROR`, `E_INDEX_ERROR`, `E_VALUE_ERROR`, `E_SYSTEM_ERROR`, `E_NOT_FOUND`, `E_NOT_YET_IMPLEMENTED`, etc.)

- **`Error`** class:
  - `Error(ErrorCode error, const char * loc)` — construct
  - `Error(ErrorCode error, const char * loc, const char * fmt, ...)` — formatted
  - `get_error_code()`, `get_loc()`, `print()`, `what()`, `operator<<`

- **Macros:** `DOMAIN_ERROR`, `SYNTAX_ERROR`, `RANK_ERROR`, `LENGTH_ERROR`, `INDEX_ERROR`, `VALUE_ERROR`, `SYSTEM_ERROR`, `NOT_IMPLEMENTED_ERROR`, `VALENCE_ERROR`, `LEFT_SYNTAX_ERROR`, `RIGHT_SYNTAX_ERROR`, `NEVERReach`, `Assert`, `Assert1`, `LIMIT_ERROR`, `LIMIT_ERROR_RANK`, `LIMIT_ERROR_LENGTH`, `LIMIT_ERROR_DEPTH`, `LIMIT_ERROR_VOLUME`, `LIMIT_ERROR_SI_DEPTH`

### 11.2 Assertions (`Assert.hh/cc`)

- `Assert(expr)` — abort on false (debug builds)
- `Assert1(expr, val)` — abort with value
- `AssertP(expr, msg)` — abort with message

### 11.3 Backtrace (`Backtrace.hh/cc`)

- **`Backtrace`** class — captures and prints C++ call stack
- **`PC_src`** struct — program counter + source location

### 11.4 Logging (`Logging.hh/cc/.def`)

- **`LOG_base`** macro — generate log functions from `Logging.def`
- **`Log(category, expr)`** — log an expression under a category
- **`Log_enable(category)`** / **`Log_disable(category)`** — toggle categories
- Categories from `Logging.def`: `LOG_delete`, `LOG_error`, `LOG_parser`, `LOG_pre_fix`, `LOG_Bif`, `LOG_Quad`, `LOG_Svar`, `LOG_Thread`, `LOG_command`, `LOG_memory`, `LOG_memory_total`, `LOG_perf`, `LOG_secure`, `LOG_show`, `LOG_verbose`, `LOG_write`, `LOG_assert`

### 11.5 Security (`Security.hh/cc/.def`)

- **`Security`** class — performs security checks (file access, environment variables)
- **Macros:** `SECURE(expr)` — check security before expression
- Categories: `SEC_file`, `SEC_env`, `SEC_system`, `SEC_network`

### 11.6 Performance (`Performance.hh/cc/.def`)

- **`Performance`** class — counter of performance events
- **`PERF_Increment(category, amount)`** — increment counter
- Categories from `Performance.def`: various operation counters

### 11.7 Common Definitions (`Common.hh/cc`)

**Universal include.** Key contents:

- **Externs:** `got_WINCH`, `COUT`, `CERR`, `UERR`, `MORE_ERROR()`, `common_new/common_delete`, `gtk_init_done`, `apl_CAPABILITIES`
- **Loop macros:** `loop(v, e)`, `rev_loop(v, e)`
- **Timing:** `now()` → µs since epoch, `cycle_counter()` via RDTSC. Class `Probe` — software performance probes.
- **Semaphores:** `__sem_init/__sem_destroy` wrappers
- **`InterruptContext` class:** ^C handling — first ^C sets `attention_raised`, second within 1s sets `interrupt_raised`. Methods: `attention_is_raised()`, `interrupt_is_raised()`, `control_C(int)`
- **Misc:** `YMDhmsu` struct, `skip_spaces()`, `yes_no()`, `charP()`, `voidP()`, `Function_PC` arithmetic, `LOC`/`Loc(f,l)`, `Q(x)`, `Q1(x)`, `ALLOCA`, `VALUE_HISTORY`, `HEX/HEX2/4/8/16/UNI`, `NULL_TERMINATE`, `SPRINTF`, `nibble()`, `sixbit()`

**`init_modules(argv0, log_startup)`** — argv-dependent startup (Quad_WA::init, Avec::init, LibPaths::init, Value::init, VH_entry::init)

**`init_modules2(log_startup)`** — argv-independent startup (Output::init, Svar_DB::init, LineInput::init, Parallel::init)

**`cleanup(soft)`** — proper shutdown (disconnect Svar_DB, clean native functions, close line history, reset colors, clean thread contexts, close windows)

### 11.8 `DynamicObject` (`DynamicObject.hh/cc`)

Base class of `Value` and `IndexExpr`. Remembers allocation site, threads all live objects into doubly-linked rings.

**Key methods:** `unlink()`, `print()`, `print_new()`, `print_chain()`, `rValue()`, `pIndexExpr()`

### 11.9 Identifiers (`Id.hh/cc/.def`, `IdEnums.hh`)

- **`enum Id`** — from `Id.def` (pp/qf/qv/sf/st macros). Covers all internal objects.
- **`ID` class** — pairs `enum Id` with UTF-8 name
- **`all_IDs`** vector — lazily built, sorted, binary-searched
- **Methods:** `get_name(Id)`, `get_name_UCS(Id)`, `get_system_function(Id)`, `get_system_variable(Id)`, `get_token_tag(Id)`

### 11.10 System Limits (`SystemLimits.hh/.def`)

- **`SystemLimits.def`** — table behind `⎕SYL` (one row per limit: name, enum, value)
- **`syl1`** — constant (configure-dependent), **`syl2`** — variable (runtime-changeable), **`syl3`** — constant
- **Methods:** `get_system_limit(Quad_SYL_item index)`, `set_system_limit(Quad_SYL_item index, int64_t value)`

### 11.11 `ProcessorID` (`ProcessorID.hh/cc`)

- **`AP_num3`** — processor id struct
- **`SvoPid`** — left argument of `⎕SVO/⎕SVQ` ↔ remote IP address, user account
- **`ProcAuth`** — authentication: `AP_num3` id + allowed remote `rsvopid` list
- **`Network_Profile`** — vectors of both
- **`ProcessorID`** class — `init(log_startup)`, `get_id()`, `get_own_ID()`, `get_parent_ID()`, `read_network_profile()`, `disconnect()`

### 11.12 `static_Objects` (`static_Objects.hh/cc`)

Controls initialization order of static objects. `INFO(m, l)` macro forces construction order. Constructed: `DynamicObject::all_values`, `DynamicObject::all_index_exprs`, `Workspace::the_workspace`, `StateIndicator::top_level_error`, `Quad_CR/EC/ES` singletons, `CPU_pool::the_CPUs`, all `Macro` objects.


---

## 12. Parallelism & Threading

### 12.1 `Parallel` (`Parallel.hh/cc`)

Static multi-core execution support. Enabled only when `cfg_CORE_COUNT_WANTED != 0` and `HAVE_AFFINITY_NP`.

**Classes:**
- **`CPU_pool`** — static vector of usable `CPU_Number`s. Methods: `init()`, `add_CPU()`, `get_CPU()`, `get_count()`, `resize()`, `change_core_count()`, `lock_pool()`, `unlock_pool()`
- **`Parallel`** — owns `run_parallel`, global semaphores `print_sema` and `pthread_create_sema`, `worker_main()` worker loop

**Key methods:** `Parallel::init()`, `worker_main()`, `acquire_lock()`, `release_lock()`, `PRINT_LOCKED(x)` macro

**Worker lifecycle:** `blocked ⇄ busy-waiting ⇄ working`. Workers are blocked during terminal input and switch to working when scalar primitives run on long vectors.

**`CPU_pool::init()` core detection:** Honors `cfg_CORE_COUNT_WANTED` cases (≥1 static, 0 sequential, -1 all cores via affinity mask, -2 from --cc N, -3 runtime-changeable via ⎕SYL). Orders CPUs even-numbered first for hyperthreading.

### 12.2 `PJob.hh` (header-only)

Units of parallel work:
- **`PJob_scalar_B`** — monadic scalar: `value_B`, `value_Z`, `len_Z`, `ErrorCode`, `fun`, `fun1`; accessors `B_at(b)`, `Z_at(z)`
- **`PJob_scalar_AB`** — dyadic: adds `value_A`, `inc_A`/`inc_B` strides, `fun2`
- **`Parallel_job_list_base`** — base with `started_loc`
- **`Parallel_job_list<T, has_destructor>`** — template stack. Methods: `start()`, `next_job()`, `cancel_jobs()`, `get_current_job()`, `busy()`, `get_size()`

### 12.3 `Thread_context` (`Thread_context.hh/cc`)

Per-worker execution context. Holds local symbol table, parse state, error state.


---

## 13. I/O System

### 13.1 `InputFile` (`InputFile.hh/cc`)

Multi-source input: file, string, or pipe.

### 13.2 `IO_Files` (`IO_Files.hh/cc`)

File I/O via native functions. Tracks open file handles.

### 13.3 `LineInput` (`LineInput.hh/cc`)

Line-editing input with history. Uses `libreadline` if available.

### 13.4 `Output` (`Output.hh/cc`)

Output formatting and dispatching. Routes to `COUT`, `CERR`, or `UERR`.

### 13.5 `PrintBuffer` (`PrintBuffer.hh/cc`)

Builds formatted output (APL display). Handles alignment, scaling, padding.

### 13.6 `PrintContext` (`PrintContext.hh`)

Holds print parameters: `⎕PP` (print precision), `⎕PW` (print width), format flags.

### 13.7 `PrintOperator` (`PrintOperator.hh`)

Operators for printing APL values (⍕).

### 13.8 `TabExpansion` (`TabExpansion.hh/cc`)

TAB-completion for APL symbols and system commands.

### 13.9 `UserPreferences` (`UserPreferences.hh/cc`)

User-configurable settings (colors, history file, keybindings).

### 13.10 `FileBuffers` (`FileBuffers.hh`)

Special filebufs:
- **`CinOut_filebuf`** — stdin echo
- **`ErrOut_filebuf`** — CERR, optional LF→CRLF
- **`DiffOut`** — compares output against reference (testcases)

### 13.11 `LibPaths` (`LibPaths.hh/cc`)

Library references 0–9 → directories. Binary path and APL library root discovery.


---

## 14. GUI & Plotting

### 14.1 GTK GUI (`Gtk/Gtk_server.cc`, `Gtk_enums.hh`, `Gtk_enum_map.def`, `Gtk_map.def`)

GTK3 integration via `⎕GTK`. Opens windows from Glade XML, calls widget methods, receives events.

**Key functions:** `Gtk_server::init()`, `open_window()`, `close_window()`, `call_widget_method()`, `wait_for_event()`, `poll_event()`

### 14.2 Plotting (`Quad_PLOT.cc/.hh/.def`, `Plot_*.cc/.hh`)

- **`Quad_PLOT`** — system function for plotting
- **`Plot_data`** — plot data model
- **`Plot_window_properties`** — window configuration
- **`Plot_line_properties`** — line style, color, width
- **`Plot_ascii`** — ASCII art plot rendering
- **`Plot_gtk`** — GTK-based plot rendering
- **`Plot_xcb`** — XCB (X11) rendering

**Flow:** `setup` → `data` → `render` → `save` / `clear` / `close`

### 14.3 Python Pipe (`PythonPipe.hh/cc`, `python_apl.cc`)

Bridge to Python. Allows APL to call Python functions and vice versa.


---

## 15. SQL Subsystem

### 15.1 `Quad_SQL` (`Quad_SQL.hh/cc`)

Function group (0–11) providing SQL database access. Providers: SQLite3 (`SqliteProvider`), PostgreSQL (`PostgresProvider`).

**Sub-function table:**

| N | Name | Call | Purpose |
|---|---|---|---|
| 0 | list | `⎕SQL[0]` | List sub-functions |
| 1 | open | `name ⎕SQL[1] args` | Open database |
| 2 | close | `⎕SQL[2] ref` | Close database |
| 3 | query | `query ⎕SQL[3,db] params` | SELECT |
| 4 | update | `query ⎕SQL[4,db] params` | UPDATE/INSERT |
| 5 | begin | `⎕SQL[5] ref` | Begin transaction |
| 6 | commit | `⎕SQL[6] ref` | Commit |
| 7 | rollback | `⎕SQL[7] ref` | Rollback |
| 8 | tables | `⎕SQL[8] ref` | List tables |
| 9 | columns | `ref ⎕SQL[9] table` | List columns |

### 15.2 SQL Provider Abstraction (`sql/`)

- **`Provider`** — abstract base
- **`SqliteProvider`** / **`PostgresProvider`** — concrete backends
- **`Connection`** — abstract database connection
- **`SqliteConnection`** / **`PostgresConnection`** — concrete
- **`ArgListBuilder`** — builds argument lists for parameterized queries
- **`SqliteResultValue`** — wraps SQLite result values


---

## 16. Native Functions & Python Bridge

### 16.1 `Native_interface.hh`

Contract for native functions loaded via `dlopen()`:
- `get_function_mux(const char *)` — name→function-pointer dispatch
- `get_signature()` — mandatory, returns `SIG_Z_A_F2_B` etc.
- `close_fun()` — optional, returns true if caller may `dlclose()`
- `eval_B`, `eval_AB`, `eval_XB`, `eval_AXB` — eval entry points

### 16.2 Native Templates (`native/`)

- **`template_F0.cc`** — niladic function template
- **`template_F12.cc`** — monadic/dyadic function template
- **`template_OP1.cc`** — monadic operator template
- **`template_OP2.cc`** — dyadic operator template
- **`template.hh`** — common header
- **`file_io.cc`** — legacy wrapper around `⎕FIO`

### 16.3 Python Bridge (`PythonPipe.hh/cc`, `python_apl.cc`)

Allows APL to call Python functions via a pipe/socket.


---

## 17. Emacs Mode

### 17.1 Protocol (`emacs.hh/cc`)

Native library for GNU APL ↔ Emacs integration. Listens on a TCP socket, serves a line/tag protocol.

**Constants:**
- `PROTOCOL_VERSION "1.6"`
- `END_TAG "APL_NATIVE_END_TAG"`
- `NOTIFICATION_START_TAG` / `NOTIFICATION_END_TAG`

### 17.2 Network Layer (`Listener`, `NetworkConnection`, `network.hh/cc`, `LockWrapper.hh/cc`)

- **`Listener`** — abstract listener interface
- **`TcpListener`** / **`UnixSocketListener`** — concrete listeners
- **`NetworkConnection`** — per-client connection (framing, dispatch, replies)
- **`NetworkCommand`** — abstract base for protocol commands

### 17.3 Wire Commands

- **`RunCommand`** — execute a statement (`⍎`)
- **`SendCommand`** — upload function source
- **`SiCommand`** / **`SicCommand`** — state indicator listing/clear
- **`SystemFnCommand`** — list system names
- **`SystemVariableCommand`** — list/get system variables
- **`VariablesCommand`** — list workspace variables
- **`VersionCommand`** — report version
- **`DefCommand`** — define function
- **`FnCommand`** — show function source
- **`FnTagCommand`** — show function tag
- **`FollowCommand`** — follow execution
- **`GetVarCommand`** — get variable value
- **`HelpCommand`** — show help
- **`TraceData`** — variable-update notifications

### 17.4 Helpers (`TempFileWrapper.hh/cc`, `util.hh/cc`)

- **`TempFileWrapper`** / **`FileWrapper`** — temporary file management
- **`split()`** — string splitting utility


---

## 18. WebSocket / try-GNU-APL

The `websock/` directory contains the try-GNU-APL web front-end. No C/C++ code — pure HTML/JavaScript + node.js.

| Directory | Contents | Runs where |
|---|---|---|
| `client/` | `apl_js.html`, `APL_keyboard.html`, `APL_keyboard2.html` | Web browser |
| `server/` | `wsock.js` (node.js WebSocket server), `apl_js.apache2` (Apache config) | Server host |

**Request flow:** `GET /try-GNU-APL` → HTTP server → node.js WebSocket server → spawns `apl` binary → connects browser terminal to APL interpreter via WebSocket.


---

## 19. Auxiliary Processors

### 19.1 AP Server (`src/APs/`)

- **`APmain.cc`** — auxiliary processor main loop
- **`AP100.cc`** — AP 100 (file server)
- **`AP210.cc`** — AP 210 (shared variable server)
- **`APserver.cc`** — common server infrastructure
- **`APmain.hh`** — common declarations
- **`Svar_DB_server.cc`** — shared variable database server

### 19.2 Shared Variable Database (`Svar_DB.hh/cc`, `Svar_record.hh/cc`, `Svar_signals.def`)

- **`Svar_DB`** — shared variable registry. Methods: `init()`, `register_processor()`, `unregister_processor()`, `get_record()`, `set_signal()`, `is_registered_id()`, `DB_tcp_error()`
- **`Svar_record`** — one shared variable entry
- **`Svar_signals.def`** — signal definitions for IPC

### 19.3 `Executable` (`Executable.hh/cc`)

Base class for anything that can be executed (functions, operators, etc.).


---

## 20. Archive / Workspace Format

### 20.1 `XML_Saving_Archive` / `XML_Loading_Archive` (`Archive.hh/cc`)

The `.xml` workspace format used by `)SAVE`, `)LOAD`, `)COPY`.

**Classes:**
- **`XML_Archive`** — base
- **`XML_Saving_Archive`** — writes workspace to XML
- **`XML_Loading_Archive`** — reads workspace from XML

**Key methods:** `save_workspace()`, `load_workspace()`, `save_symbol()`, `load_symbol()`, `save_value()`, `load_value()`, `save_function()`, `load_function()`

### 20.2 Archive enums

- **`ArchiveSyntax`** — XML version tags
- **`Vid`** — variable IDs
- **`Fid`** — function IDs

### 20.3 CDR (`CDR.hh/cc`, `CDR_string.hh`)

Common Data Representation format for inter-process communication.

**Types:** `CDR_BOOL1`, `CDR_INT32`, `CDR_FLT64`, `CDR_CPLX128`, `CDR_CHAR8`, `CDR_CHAR32`, `CDR_PROG64`, `CDR_NEST32`


---

## 21. libapl Embedding API

### 21.1 `libapl.h` / `libapl.cc`

C/C++ embedding API for GNU APL.

**Key functions:**
- `apl_init(int argc, char *argv[])` — initialize the interpreter
- `apl_eval(const char *expr)` — evaluate an APL expression, returns result
- `apl_eval_to_string(const char *expr)` — evaluate and return result as string
- `apl_read_file(const char *filename)` — read and evaluate a file
- `apl_cleanup(void)` — shut down
- `apl_get_var(const char *name)` — get a workspace variable
- `apl_set_var(const char *name, int rank, int64_t *shape, void *data)` — set a workspace variable
- `apl_get_error()` — get last error
- `apl_print(ostream & out, apl_value_t val)` — print a value
- `apl_eval_to_var(const char *expr, const char *varname)` — evaluate and assign result to variable
- `apl_exec(const char *cmd)` — execute a system command (e.g. `)LOAD`)
- `apl_get_workspace()` — get workspace info
- `apl_get_function(const char *name)` — get a defined function
- `apl_edit_function(const char *name, const char *body)` — edit a function
- `apl_set_interrupt(int)` — raise interrupt
- `apl_get_char(int)` — read one character from input
- `apl_write_char(int)` — write one character to output
- `apl_get_value_count()` — get number of values allocated
- `apl_get_SI_level()` — get SI depth


---

## 22. Data Structures Reference

### 22.1 Fundamental

| Structure | File | Description |
|---|---|---|
| `Shape` | `Shape.hh` | Rank + dimensions + volume |
| `Value` | `Value.hh` | APL array (shape + ravel of Cells) |
| `Value_P` | `Value_P.hh` | Reference-counted smart pointer to Value |
| `Cell` | `Cell.hh` | Abstract base for ravel elements |
| `Token` | `Token.hh` | Lexical unit |
| `Symbol` | `Symbol.hh` | Named entity in workspace |
| `Function` | `Function.hh` | Abstract callable |

### 22.2 Cell Storage

```cpp
union Cell_Value {
   APL_Integer   ival;      // IntCell
   Unicode       aval;      // CharCell
   double        fval;      // FloatCell (or rational: num + denom)
   APL_Float     cval[2];   // ComplexCell (real, imag)
   struct { Value * valp; Value * owner; } pval;  // PointerCell / LvalCell
};
```

### 22.3 Parallelism

| Structure | File | Description |
|---|---|---|
| `PJob_scalar_B` | `PJob.hh` | One monadic scalar job |
| `PJob_scalar_AB` | `PJob.hh` | One dyadic scalar job |
| `Parallel_job_list<T, D>` | `PJob.hh` | Template stack of pending jobs |
| `CPU_pool` | `Parallel.hh` | Static vector of CPU numbers |
| `Thread_context` | `Thread_context.hh` | Per-worker execution context |

### 22.4 Parser/Interpreter

| Structure | File | Description |
|---|---|---|
| `Token_string` | `Token_string.hh` | Vector of tokens |
| `Prefix` | `Prefix.hh` | Prefix parser machine |
| `StateIndicator` | `StateIndicator.hh` | Call stack / SI frame |
| `IndexExpr` | `IndexExpr.hh` | Index expression |
| `IndexIterator` | `IndexIterator.hh` | Index iteration state |

### 22.5 String Types

| Structure | File | Description |
|---|---|---|
| `UCS_string` | `UCS_string.hh` | Unicode (UCS-4) string |
| `UTF8_string` | `UTF8_string.hh` | UTF-8 encoded string |
| `CDR_string` | `CDR_string.hh` | CDR-encoded string |
| `Simple_string` | `Simple_string.hh` | Simple char vector |
| `PrintBuffer` | `PrintBuffer.hh` | Formatted output buffer |

### 22.6 Utility

| Structure | File | Description |
|---|---|---|
| `AP_num3` | `APL_types.hh` | Processor triple (proc, parent, grand) |
| `Function_PC2` | `APL_types.hh` | Function PC range (low, high) |
| `labVal` | `APL_types.hh` | Label: symbol + line |
| `_twc` | `APL_types.hh` | Iterator state (to, weight, current) |
| `ravel_comp_len` | `APL_types.hh` | CDR comparison length |
| `YMDhmsu` | `Common.hh` | UTC calendar ⇄ µs |
| `Probe` | `Common.hh` | Software performance probe |
| `InterruptContext` | `Common.hh` | ^C state machine |
| `PC_src` | `Backtrace.hh` | PC + source location |
| `TraceData` | `TraceData.hh` | Variable update trace |
| `fcall_edge` | `Doxy.hh` | Call graph edge |
| `Format_sub` | `Bif_F12_FORMAT.hh` | Format sub-record |
| `Matrix` | `Bif_F12_DOMINO.hh` | Matrix for ⌹ |
| `norm_result` | `Bif_F12_DOMINO.hh` | Norm/scale result |
| `Partition` | `Bif_F12_PARTITION_PICK.hh` | Partition descriptor |

### 22.7 Archive

| Structure | File | Description |
|---|---|---|
| `_val_par` | `Archive.hh` | Value parameter |
| `_vid_pvid` | `Archive.hh` | Variable/previous-ID pair |
| `fun_map` | `Archive.hh` | Function name mapping |
| `_derived_todo` | `Archive.hh` | Derived function work item |

### 22.8 Quad System

| Structure | File | Description |
|---|---|---|
| `Format_sub` | `Bif_F12_FORMAT.hh` | Format conversion sub-record |
| `Format_LIFER` | `Bif_F12_FORMAT.hh` | LIFER format |
| `Matrix` | `Bif_F12_DOMINO.hh` | Matrix for QR/LU |
| `norm_result` | `Bif_F12_DOMINO.hh` | Norm result for ⌹ |
| `Partition` | `Bif_F12_PARTITION_PICK.hh` | Partition record |
| `subfunction_info` | (generated) | Function group sub-function entry |


---

## 23. Algorithms Reference

### 23.1 Cell Operations

- **`greater()`** — Cell ordering: PointerCell > NumericCell > CharCell. Numerics by value; chars by code point; pointers by rank→shape→ravel.
- **`equal()`** — Tolerant equality via `⎕CT`. `tolerantly_equal(A, B, qct)` uses `|A-B| ≤ qct × max(|A|,|B|)`.
- **`integral_within()`** — checks if float is close to integer within `⎕CT`.
- **`same_half_plane()`** — ISO p.15 complex half-plane test.

### 23.2 Heapsort (`Heapsort.hh`)

Template `Heapsort<T>` with `sort(T ** data, int data_len, bool ascending, const void * comp_arg)`. Uses `T::A_greater_B()` for comparison and `T::swap()` for swapping.

**Specializations:**
- `Heapsort<Cell>::sort()` — sorts Cell pointers via `Cell::greater()`
- `Heapsort<IntCell>::sort()` — sorts IntCells via `IntCell::greater()`
- `Heapsort<ID>::sort()` — sorts IDs via `greater_id`

### 23.3 Linear Algebra

- **`LApack.cc/.hh`** — LAPACK wrapper for LU, QR, SVD, eigenvalue decomposition
- **`QR_factorization_GSL.cc/.hh`** — GSL-based QR factorization (used by `⌹` when GSL available)

**`Bif_F12_DOMINO` (⌹):**
- Monadic: inverse via LU decomposition
- Dyadic: solve Ax=b via QR factorization
- Fallback: normal equations (AᵀAx = Aᵀb) for least-squares

### 23.4 Fast Fourier Transform (`Quad_FFT.cc`)

FFT via GSL or fallback DFT. Supports forward/inverse, real/complex, arbitrary dimensions.

### 23.5 Regular Expressions (`Regexp.hh/cc`)

PCRE2 wrapper for `⎕RE`. Supports: compile, exec, match, replace, split, capture, named_capture, count, positions.

### 23.6 Polynomial (`Polynomial.hh/cc`)

Polynomial arithmetic for root-finding and interpolation.

### 23.7 Dancing Links (`Quad_DLX.cc`)

Knuth's Algorithm X via dancing links (exact cover solver).

### 23.8 Index Expression Evaluation (`IndexExpr.hh/cc`)

Evaluates `A[i;j;k]` indexing:
- Simple indexing: `A[1;2]`
- Choose indexing: `A[(1 2)(3 4)]`
- Reach indexing: `A[1 2]`
- Assignment indexed: `A[1;2]←42`

### 23.9 Prefix Parser (`Prefix.cc`)

The core evaluator. Token classes: `TC_LAMBDA`, `TC_VARIABLE`, `TC_FUN_XXX`, `TC_OPER_XXX`, `TC_LABEL`, `TC_END`, `TC_FUN1`, `TC_FUN2`, etc.

Reduction rules:
- `VALUE FUN VALUE → RESULT` (dyadic)
- `FUN VALUE → RESULT` (monadic)
- `OPER FUN → RESULT` (operator binding)
- `VALUE OPER FUN → RESULT` (dyadic operator)
- `FUN FUN → RESULT` (inner/outer product)
- `FUN POWER NUMBER FUN → RESULT` (power operator)
- `FUN RANK ARRAY → RESULT` (rank operator)
- `VAR ← VALUE → RESULT` (assignment)
- `LABEL: → RESULT` (label)
- `GOTO → RESULT` (branch)

### 23.10 Command Dispatch (`Command.cc/.def`)

Commands from `Command.def`:
| Command | Description |
|---|---|
| `)CLEAR` | Clear workspace |
| `)ERASE name` | Erase symbol |
| `)FNS` / `)VARS` / `)OPS` / `)SI` | List symbols |
| `)SAVE [lib:]ws` | Save workspace |
| `)LOAD [lib:]ws` | Load workspace |
| `)COPY [lib:]ws [items]` | Copy items |
| `)IN file` | Import .atf transfer file |
| `)OUT file [items]` | Export to file |
| `)DUMP [file] [items]` | Dump workspace to file |
| `)INLOCK` | Input lock |
| `)LIBS` / `]LIB` | List libraries |
| `)OFF` / `)CONTINUE` | Exit / continue |
| `)SIC` / `)SISR` | Clear / reset SI |
) `)STACK` | Show SI stack |
| `)HIST` | Show value history |
| `)HELP` | Show help |
| `)MEM` | Memory info |
| `)SYS` | System info |
| `)WMENU` | Workspace menu |
| `)WCHANGES` | Show changes |
| `)NEXTFILE` | Load next file in sequence |
| `]KEYB` | Keyboard layout |
| `]BOX` | Box drawing |
| `]Doxy` | Generate documentation |
| `]EC` / `]ES` / `]EM` / `]ELX` | Error quad functions |
| `]SYMBOLS` | Symbol table dump |
| `]USERCMD` | User-defined commands |
| `]COLOR` | Set colors |

### 23.11 Print Formatting (`PrintBuffer.cc`)

Multi-pass formatting:
1. Pass 1: compute cell widths, row heights, total size
2. Pass 2: render with alignment, scaling, padding
3. Handles: complex scaling (E-format), rational numbers, nested arrays, character padding


---

## 24. Glossary of Files

| File | Purpose |
|---|---|
| `APL_types.hh` | Core typedefs |
| `APL_enums.hh` | Global enums |
| `APL_Float_as_class.hh` | APL_Float class example |
| `Unicode.hh` | Unicode enum + pad chars |
| `Avec.hh/cc/.def` | Atomic Vector (⎕AV) |
| `Cell.hh/cc/.icc` | Cell base class |
| `CharCell.hh/cc` | CharCell |
| `IntCell.hh/cc` | IntCell |
| `FloatCell.hh/cc` | FloatCell |
| `ComplexCell.hh/cc` | ComplexCell |
| `NumericCell.hh` | NumericCell base |
| `RealCell.hh` | RealCell base |
| `PointerCell.hh/cc` | PointerCell |
| `LvalCell.hh/cc` | LvalCell |
| `Shape.hh/cc` | Shape |
| `Value.hh/cc/.icc` | Value |
| `Value_P.hh/.icc` | Value smart pointer |
| `ConstCell_P.hh` | ConstCell_P |
| `Symbol.hh/cc` | Symbol |
| `SymbolTable.hh/cc` | SymbolTable |
| `SystemVariable.hh/cc/.def` | SystemVariable |
| `NamedObject.hh/cc` | NamedObject |
| `Workspace.hh/cc/.icc` | Workspace |
| `StateIndicator.hh/cc` | StateIndicator |
| `ValueHistory.hh/cc` | ValueHistory |
| `Function.hh/cc` | Function base |
| `PrimitiveFunction.hh/cc` | PrimitiveFunction |
| `ScalarFunction.hh/cc` | ScalarFunction |
| `PrimitiveOperator.hh` | PrimitiveOperator |
| `DerivedFunction.hh/cc` | DerivedFunction |
| `NativeFunction.hh/cc` | NativeFunction |
| `Native_interface.hh` | Native function contract |
| `QuadFunction.hh/cc` | QuadFunction base |
| `Quad_CC.hh/cc` | ⎕CC |
| `Quad_CR.hh/cc/.def` | ⎕CR |
| `Quad_DLX.hh/cc` | ⎕DLX |
| `Quad_FFT.hh/cc` | ⎕FFT |
| `Quad_FIO.hh/cc/.def` | ⎕FIO |
| `Quad_FX.hh/cc` | ⎕FX |
| `Quad_GTK.hh/cc` | ⎕GTK |
| `Quad_JSON.hh/cc` | ⎕JSON |
| `Quad_MAP.hh/cc` | ⎕MAP |
| `Quad_MX.hh/cc/.def` | ⎕MX |
| `Quad_PLOT.hh/cc/.def` | ⎕PLOT |
| `Quad_PNG.hh/cc` | ⎕PNG |
| `Quad_RE.hh/cc` | ⎕RE |
| `Quad_RL.hh/cc` | ⎕RL |
| `Quad_RVAL.hh/cc` | ⎕RVAL |
| `Quad_SQL.hh/cc` | ⎕SQL |
| `Quad_SVx.hh/cc` | ⎕SVx |
| `Quad_TF.hh/cc` | ⎕TF |
| `Quad_WA.hh/cc` | ⎕WA |
| `Quad_XML.hh/cc` | ⎕XML |
| `Quad_Quad.hh/cc` | ⎕ |
| `Bif_F12_COMMA.hh/cc` | , ⍪ |
| `Bif_F12_DOMINO.hh/cc` | ⌹ |
| `Bif_F12_FORMAT.hh/cc` | ⍕ |
| `Bif_F12_INDEX_OF.hh/cc` | ⍳ |
| `Bif_F12_INTERVAL_INDEX.hh/cc` | ⍸ |
| `Bif_F12_PARTITION_PICK.hh/cc` | ⊂ ⊃ |
| `Bif_F12_SORT.hh/cc` | ⍋ ⍒ |
| `Bif_F12_TAKE_DROP.hh/cc` | ↑ ↓ |
| `Bif_OPER1_COMMUTE.hh/cc` | ⍨ |
| `Bif_OPER1_EACH.hh/cc` | ¨ |
| `Bif_OPER1_REDUCE.hh/cc` | / ⌿ \ ⍀ |
| `Bif_OPER1_SCAN.hh/cc` | ⍀ |
| `Bif_OPER2_INNER.hh/cc` | . |
| `Bif_OPER2_OUTER.hh/cc` | ∘. |
| `Bif_OPER2_POWER.hh/cc` | ⍣ |
| `Bif_OPER2_RANK.hh/cc` | ⍤ |
| `Token.hh/cc/.def` | Token |
| `TokenEnums.hh` | Token enums |
| `Token_string.hh/cc` | Token_string |
| `Tokenizer.hh/cc` | Tokenizer |
| `Parser.hh/cc` | Parser |
| `Prefix.hh/cc/.def` | Prefix machine |
| `UCS_string.hh/cc` | UCS_string |
| `UCS_string_vector.hh/cc` | UCS_string_vector |
| `UTF8_string.hh/cc` | UTF8_string |
| `CDR.hh/cc` | CDR |
| `CDR_string.hh` | CDR_string |
| `ArrayIterator.hh` | ArrayIterator |
| `IndexExpr.hh/cc` | IndexExpr |
| `IndexIterator.hh/cc` | IndexIterator |
| `Common.hh/cc` | Common definitions |
| `Error.hh/cc/.def` | Error handling |
| `Error_macros.hh` | Error macros |
| `ErrorCode.hh` | ErrorCode enum |
| `Assert.hh/cc` | Assertions |
| `Backtrace.hh/cc` | Backtrace |
| `Logging.hh/cc/.def` | Logging |
| `Security.hh/cc/.def` | Security |
| `Performance.hh/cc/.def` | Performance counters |
| `DynamicObject.hh/cc` | DynamicObject |
| `Id.hh/cc/.def` | Identifiers |
| `IdEnums.hh` | Id enums |
| `SystemLimits.hh/.def` | ⎕SYL system limits |
| `ProcessorID.hh/cc` | Processor ID |
| `Parallel.hh/cc` | Parallel execution |
| `PJob.hh` | Parallel job definitions |
| `Thread_context.hh/cc` | Thread context |
| `static_Objects.hh/cc` | Static init order |
| `Malloc_hooks.cc` | Malloc tracing (disabled) |
| `sbrk.cc` | top_of_memory() |
| `InputFile.hh/cc` | InputFile |
| `IO_Files.hh/cc` | IO_Files |
| `LineInput.hh/cc` | LineInput |
| `Output.hh/cc` | Output |
| `PrintBuffer.hh/cc` | PrintBuffer |
| `PrintContext.hh` | PrintContext |
| `PrintOperator.hh` | PrintOperator |
| `TabExpansion.hh/cc` | TabExpansion |
| `UserPreferences.hh/cc` | UserPreferences |
| `Doxy.hh/cc` | Doxy (documentation generator) |
| `DiffOut.cc` | DiffOut |
| `FileBuffers.hh` | FileBuffers |
| `LibPaths.hh/cc` | LibPaths |
| `Regexp.hh/cc` | Regexp |
| `Polynomial.hh/cc` | Polynomial |
| `LApack.hh/cc` | LAPACK wrapper |
| `QR_factorization_GSL.hh/cc` | QR via GSL |
| `Heapsort.hh` | Heapsort template |
| `Focus.icc` | Focus |
| `LAdebug.icc` | LAdebug |
| `NumericCell.icc` | NumericCell inlines |
| `Value.icc` | Value inlines |
| `Value_P.icc` | Value_P inlines |
| `Cell.icc` | Cell inlines |
| `UserFunction.hh/cc` | UserFunction |
| `UserFunction_header.hh/cc` | UserFunction_header |
| `Macro.hh/cc/.def` | Macro |
| `Command.hh/cc/.def` | Command dispatch |
| `Help.def` | Help text |
| `Nabla.hh/cc` | Nabla editor |
| `Archive.hh/cc` | XML archive |
| `libapl.cc/.h` | libapl API |
| `main.cc` | main() |
| `PythonPipe.hh/cc` | Python pipe |
| `python_apl.cc` | python_apl |
| `Gtk_server.cc` | GTK server |
| `Gtk_enum_map.def` | GTK enum map |
| `Gtk_enums.hh` | GTK enums |
| `Gtk_map.def` | GTK map |
| `Plot_ascii.cc` | ASCII plot |
| `Plot_data.hh/cc` | Plot data |
| `Plot_gtk.cc` | GTK plot |
| `Plot_line_properties.hh/cc` | Plot line props |
| `Plot_window_properties.hh/cc` | Plot window props |
| `Plot_xcb.cc` | XCB plot |
| `apl-sqlite.hh/cc` | SQLite backend |
| `ArgListBuilder.hh` | ArgListBuilder |
| `Connection.hh/cc` | SQL Connection |
| `PostgresArgListBuilder.cc/.hh` | PostgreSQL arg builder |
| `PostgresConnection.cc/.hh` | PostgreSQL connection |
| `PostgresProvider.cc/.hh` | PostgreSQL provider |
| `Provider.hh` | SQL Provider |
| `SqliteArgListBuilder.cc/.hh` | SQLite arg builder |
| `SqliteConnection.cc/.hh` | SQLite connection |
| `SqliteProvider.cc/.hh` | SQLite provider |
| `SqliteResultValue.cc/.hh` | SQLite result |
| `native/file_io.cc` | Legacy ⎕FIO wrapper |
| `native/template_F0.cc` | Native F0 template |
| `native/template_F12.cc` | Native F12 template |
| `native/template_OP1.cc` | Native OP1 template |
| `native/template_OP2.cc` | Native OP2 template |
| `native/template.hh` | Native template header |
| `emacs_mode/*` | Emacs mode (14 files) |
| `APs/*` | Auxiliary Processors |
| `Svar_DB.hh/cc` | Shared variable DB |
| `Svar_record.hh/cc` | Shared variable record |
| `Svar_signals.def` | Shared variable signals |
| `Missing_Libraries.cc` | Missing library diagnostics |
| `Executable.hh/cc` | Executable base |
| `websock/*` | try-GNU-APL web front-end |
| `wslib3/`, `wslib4/`, `wslib5/` | APL workspace libraries |
| `workspaces/` | Example workspaces |
| `support-files/` | Support files |
| `doc/` | Documentation |
| `tools/` | Build tools |
| `erlang/` | Erlang integration |
| `gnu-apl.d/` | GNU APL desktop file |

---

*End of GNU APL 2.0 API Reference*


