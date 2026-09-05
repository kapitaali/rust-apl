# Rust APL — Implementation Roadmap

> **Goal:** Complete the Rust rewrite of GNU APL 2.0, covering all major subsystems.
> Priority: Quad functions first, then operators, selective assignment, system variables,
> native functions, infrastructure, and finally optional/UI features.

---

## Phase 1 — Quad Functions (the ⎕ system)

These are the most-used system functions. Many are simple; a few need external crates.

### 1.1 Simple / Pure-Rust Quad Functions
| # | Function | Description | Depends on |
|---|---|---|---|
| 1 | `⎕CR` | Character representation (format nested values) | format.rs |
| 2 | `⎕RVAL` | Random value (scalar int/float/complex) | rand crate |
| 3 | `⎕RL` | Random link (seed state) | rand crate |
| 4 | `⎕DLX` | Dancing Links (exact cover solver) | — |
| 5 | `⎕CC` | Case conversion (upper/lower/title) | unicode crate |
| 6 | `⎕RE` | Regular expression | regex crate |
| 7 | `⎕TF` | Transfer form (canonical source) | parser.rs |
| 8 | `⎕FX` | Fix function from character matrix | parser.rs |
| 9 | `⎕MAP` | Map symbol table | SymbolTable |
| 10 | `⎕MX` | Matrix operations (determinant, inverse, etc.) | — |

### 1.2 I/O Quad Functions
| # | Function | Description | Depends on |
|---|---|---|---|
| 11 | `⎕FIO` | File I/O (open, read, write, close) | std::fs | 🔲 Partial |
| 12 | `⎕JSON` | JSON parse/serialize | serde_json |
| 13 | `⎕XML` | XML parse/serialize | quick-xml |

### 1.3 System / External Quad Functions
| # | Function | Description | Depends on |
|---|---|---|---|
| 14 | `⎕FFT` | Fast Fourier Transform | rustfft crate | ✅ Done |
| 15 | `⎕SVx` | Shared variables (IPC) | — | 🔲 Stubs |
| 16 | `⎕SQL` | SQL database access | sqlx or rusqlite | ✅ Done |
| 17 | `⎕GTK` | GTK GUI | gtk4 crate | 🔲 Stub |
| 18 | `⎕PLOT` | Plotting | plotters crate | ✅ Done |
| 19 | `⎕PNG` | PNG image read/write | image crate | ✅ Done |
| 20 | `⎕PYTHON` | Python interop | pyo3 | 🔲 Stub |
| 21 | `⎕CDR` | CDR binary interchange | — | ✅ Done |

### 1.4 Quad System Variables
| # | Variable | Description | Status |
|---|---|---|---|
| 22 | `⎕IO` | Index origin (0 or 1) | ✅ Done |
| 23 | `⎕CT` | Comparison tolerance | ✅ Done |
| 24 | `⎕PP` | Print precision | ✅ Done |
| 25 | `⎕AV` | APL character vector | ✅ Done |

---

## Phase 2 — Operators

### 2.1 Each `¨` (monadic operator)
- Maps a function over each element of an array independently
- Works with all scalar primitives
- Example: `⍳¨ 3 4 5` → `(⍳3)(⍳4)(⍳5)`
- ✅ Done: monadic `f¨B` works for primitive f; dyadic `A f¨B` works for primitive f

### 2.2 Rank `⍤` (dyadic operator)
- Applies function at specified rank
- Example: `(⍤2) → apply at rank 2 (matrix level in 3D array)`
- ✅ Done: `A⍤B f C` works for primitive f and integer rank

### 2.3 Power `⍣` (dyadic operator)
- Function composition and iteration
- Example: `(f⍣3) X → f(f(f X))`
- ✅ Done: `f⍣n` for integer n (iterate), `f⍣g` for function g (until fixed point)

### 2.4 Commute `⍨` (monadic operator)
- Argument swap or constant
- Example: `A+⍨B → B+A` (swap), `+⍨B → B+B` (constant)
- ✅ Done

### 2.5 Inner Product `.` (dyadic operator)
- Standard A+ inner product
- Example: `A +.× B` (dot product for vectors)
- ✅ Done: inner product for primitives

### 2.6 Outer Product `∘.` (dyadic operator)
- Standard APL outer product
- Example: `A ∘.× B` (multiplication table)
- ✅ Done: outer product for primitives

### 2.7 Compose `∘` (dyadic operator)
- Matrix product in GNU APL (not mathematical compose)
- Example: `A ∘.× B` → `A +.× B`
- ✅ Done

### 2.8 Reverse/Scan operators (⌿, ⍀)
- Reverse first/last axis, scan with first/last axis
- ✅ Done: monadic ⌿ (reduce first axis), dyadic A⌿B (windowed reduce first axis)
- ✅ Done: monadic ⍀ (reduce last axis), dyadic A⍀B (windowed reduce last axis)

---

## Phase 3 — Selective Assignment

### 3.1 Bracket indexing with assignment
- `(⊂2 3) ← value` → modify row 2 col 3
- `(⊂(1 2)(3 4)) ← value` → modify multiple elements
- ✅ Done: bracket assignment for all index shapes

### 3.2 Comma catenation with assignment
- `A[1,1] ← value` → modify via flat index
- ✅ Done

### 3.3 Modified assignment
- `A +← B` → `A ← A + B`
- ✅ Done: `+← -← ×← ÷← min← max←`

### 3.4 At (`@` or `⊂`)
- `(⊂pos) ← value` → modify at position
- ✅ Done

---

## Phase 4 — Native Functions

### 4.1 `⎕NA` — Native Function Interface
- `name ⎕NA 'result lib|symbol args...'`
- Example: `mydiv ⎕NA 'I4 libc.so.6|div I4 I4'`
- ✅ Done: basic ⎕NA with I4/F8 return types, string, pointer

### 4.2 `⎕CALL` — Direct Function Call
- Call native function directly
- 🔲 Stub

### 4.3 FFI Support
- Dynamic library loading via libloading
- Type marshalling (I4, I8, F8, F16, string, pointer)
- ✅ Done: basic FFI with libloading

---

## Phase 5 — Infrastructure

### 5.1 Workspace Commands
| # | Command | Description | Status |
|---|---|---|---|
| 1 | `)INP` | Input session from file | ✅ Done |
| 2 | `)OUT` | Save session to file | ✅ Done |
| 3 | `)LIB` | List saved workspaces | ✅ Done |
| 4 | `)COPY` | Copy functions/variables from workspace | ✅ Done |
| 5 | `)ERASE` | Erase functions/variables | ✅ Done |
| 6 | `)FNS` | List functions | ✅ Done |
| 7 | `)VARS` | List variables | ✅ Done |
| 8 | `)OPS` | List operators | ✅ Done |
| 9 | `)GRP` | Grouped name display | ✅ Done |
| 10 | `)NMS` | Name space display | ✅ Done |
| 11 | `)SAVE` | Done ✓ | ✅ Done |
| 12 | `)LOAD` | Done ✓ | ✅ Done |
| 13 | `)CLEAR` | Done ✓ | ✅ Done |
| 14 | `)DROP` | Done ✓ | ✅ Done |
| 15 | `)OFF` | Done ✓ | ✅ Done |

### 5.2 State Indicator
- `)SI` — display call stack | ✅ Done
- `)SINL` — SI with line numbers | ✅ Done
- `)SVS` — shared variable status | ✅ Done (stub: no shared variables)

### 5.3 Session Management
- `)CONTINUE` — save workspace and continue | 🔲 Not started

### 5.4 Macros
- Session macros (input/output recording) ✅ Done
- Function macros |

### 5.5 Security
- `⎕SEC` — security level | ✅ Done
- Restricted operations at higher security levels | 🔲 Not enforced

### 5.6 Symbol Table
- Namespaces (`⎕NS`, `⎕CS`) | ✅ Done
- Local symbol tables per function | 🔲 Not started
- Global symbol table management | ✅ Done

---

## Phase 6 — Plugin System (Optional / UI Features) ✅ DONE

### 6.1 Plugin Infrastructure
- `AplPlugin` trait with `info()`, `register()`, `init()`, `shutdown()` | ✅ Done
- `PluginRegistry` for managing loaded plugins | ✅ Done
- `PluginRegistrar` for registering functions/sysvars | ✅ Done
- Configuration file (`config.toml`) | ✅ Done
- Build script (`build.rs`) for compile-time feature selection | ✅ Done

### 6.2 GUI
- GTK interface (`⎕GTK`) | 🔲 Stub
- Plot window management | 🔲 Not started
- Line properties, window properties | 🔲 Not started

### 6.3 Plotting
- Data plot via plotters | ✅ Done
- `⎕PLOT` function | ✅ Done
- ASCII plot | 🔲 Not started

### 6.4 Python Integration
- Python pipe | 🔲 Stub (shell-out to python3)
- Bidirectional communication | 🔲 Not started

### 6.5 CDR / Archive Format
- Binary interchange format | ✅ Done
- `⎕CDR` — CDR conversion | ✅ Done

---

## Phase 7 — Performance

### 7.1 Packed Arrays
- Extend PackedBool to packed Int/Float/Complex ✅ Done (Int)
- Bit-width selection based on range ✅ Done

### 7.2 Parallel Scalar Operations
- Parallelize all scalar operations on long vectors ✅ Done
- Extend beyond outer product and reduce/scan ✅ Done

### 7.3 SmallVec Throughout
- Wire SmallVec<[Cell; 8]> into hot paths ✅ Done
- Replace Vec<Cell> in constructors, ravel operations ✅ Done

### 7.4 Fetcher Function Pointer
- Mimic C++ Value's fetcher pattern | ✅ Done (deferred — no packed data in ValueInner yet)
- Avoid per-cell branching for packed vs unpacked |

---

## Phase 8 — Full GNU APL XML Compatibility

### 8.1 Value Elements
- `flg`, `vid`, `parent`, `rk` attributes
- `sh-N` shape attributes

### 8.2 Ravel Elements
- Packed ravel with `bytes` attribute (hex)
- Normal ravel with Unicode padding

### 8.3 Function Elements
- `fid`, `creation-time`, `exec-properties`, `tag`
- `Canonical` and `Source` sub-elements

---

## Phase 9 — Remaining Features (Current)

These are the unfinished items from earlier phases, now the active workstream.

### 9.1 File I/O (`⎕FIO`)
- ✅ Done: Real file handle tracking, read/write line/bytes, seek/tell
- All 10 functions (0-9) work correctly

### 9.2 Direct Native Calls (`⎕CALL`)
- ✅ Done: Calls ⎕NA-bound functions by name
- 🔲 TODO: dyadic ⎕CALL with explicit args

### 9.3 Shared Variables (`⎕SVx`)
- ✅ Done: in-process registry with offer/query/read/set/cancel
- ⎕SVC lists, ⎕SVO offers, ⎕SVQ queries, ⎕SVR reads, ⎕SVS sets

### 9.4 Session Continuity (`)CONTINUE`)
- ✅ Done: saves workspace to CONTINUE.xml and exits

### 9.5 Local Symbol Tables
- ✅ Done: shadowing in `call_function` via `collect_assigned_names`
- Locals saved/restored on function entry/exit

### 9.6 Security Enforcement (`⎕SEC`)
- ✅ Done: implemented as extension via plugin middleware hooks

### 9.7 ASCII Plotting
- ✅ Done: bar charts (vectors), line plots (matrices), legend for series
- 🔲 TODO: scatter plots, smoother rendering

### 9.8 Bidirectional Python
- ✅ Done: pyo3 integration (in-process) with shell-out fallback
- pyo3 feature: `cargo build --features plugin-python`

---

## Implementation Order

```
Phase 1.1 → Phase 1.2 → Phase 1.3 → Phase 1.4
    ↓
Phase 2.1 → Phase 2.2 → Phase 2.3 → Phase 2.4
    ↓
Phase 3.1 → Phase 3.2 → Phase 3.3
    ↓
Phase 4.1 → Phase 4.2
    ↓
Phase 5.1 → Phase 5.2 → Phase 5.3 → Phase 5.4 → Phase 5.5
    ↓
Phase 6.1 → Phase 6.2 → Phase 6.3 → Phase 6.4 → Phase 6.5
    ↓
Phase 7.1 → Phase 7.2 → Phase 7.3 → Phase 7.4
    ↓
Phase 8.1 → Phase 8.2 → Phase 8.3
    ↓
Phase 9.1 → Phase 9.2 → Phase 9.3 → Phase 9.4 → Phase 9.5 → Phase 9.6 → Phase 9.7 → Phase 9.8
```

---

## Non-Obvious Notes

- `∘` (U+2218 RING OPERATOR) is matrix product in GNU APL, not compose
- `⍥` (U+2365) and `⌸` (U+2328) are NOT in GNU APL 2.0 (Dyalog extensions)
- Cell::Char holds u32 codepoint; ValueP::char_vector takes &[u32]
- libapl uses thread-local Environment + Mutex-protected callbacks
- XML archive uses custom text format with ²...⁰ char mode and type tags
- rusqlite uses `bundled` feature (compiles SQLite from source, no system lib needed)
- `⎕PLOT` auto_open defaults to `false` (xdg-open triggers URI dialogs on headless systems)
- `⎕SEC` is an extension (not in GNU APL) implemented via plugin middleware hooks
- Plugin hooks: `before_eval` blocks expressions, `before_syscmd` blocks commands

---

## Phase 10 — Additional GNU APL Features

These are minor system variables and features from GNU APL not yet implemented.

### 10.1 System Variables
| Variable | Description | Status |
|---|---|---|
| `⎕A` | Alphabetic characters (A-Z, a-z) | 🔲 Not implemented |
| `⎕D` | Digits (0-9) | 🔲 Not implemented |
| `⎕PW` | Print width (line width for output) | 🔲 Not implemented |
| `⎕LX` | Latent expression (startup code) | 🔲 Not implemented |
| `⎕EM` | Error message (last error) | 🔲 Stub (returns empty) |
| `⎕EC` | Error code (last error) | 🔲 Stub (returns 0) |
| `⎕WI` | Workspace information | 🔲 Not implemented |

### 10.2 Cross-Process IPC
- Shared variables across processes (currently in-process only)
- Requires TCP socket server (AP210 equivalent)

### 10.3 Implementation Order
```
⎕A → ⎕D → ⎕PW → ⎕LX → ⎕EM → ⎕EC → ⎕WI → IPC
```
