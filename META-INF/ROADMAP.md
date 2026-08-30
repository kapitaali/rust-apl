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
| 11 | `⎕FIO` | File I/O (open, read, write, close) | std::fs |
| 12 | `⎕JSON` | JSON parse/serialize | serde_json |
| 13 | `⎕XML` | XML parse/serialize | quick-xml |

### 1.3 System / External Quad Functions
| # | Function | Description | Depends on |
|---|---|---|---|
| 14 | `⎕FFT` | Fast Fourier Transform | rustfft crate |
| 15 | `⎕SVx` | Shared variables (IPC) | — |
| 16 | `⎕SQL` | SQL database access | sqlx or rusqlite |
| 17 | `⎕GTK` | GTK GUI | gtk4 crate |
| 18 | `⎕PLOT` | Plotting | plotters crate |
| 19 | `⎕PNG` | PNG image read/write | image crate |

### 1.4 Quad System Variables
| # | Variable | Description | Status |
|---|---|---|---|
| 20 | `⎕IO` | Index origin (0 or 1) | ✅ Done |
| 21 | `⎕CT` | Comparison tolerance | ✅ Done |
| 22 | `⎕PP` | Print precision | ✅ Done |
| 23 | `⎕AV` | APL character vector | ✅ Done |

---

## Phase 2 — Operators

### 2.1 Each `¨` (monadic operator)
- Maps a function over each element of an array independently
- Works with all scalar primitives
- Example: `⍳¨ 3 4 5` → `(⍳3)(⍳4)(⍳5)`
- ✅ Done: monadic `f¨B` works for primitive f; dyadic `A f¨B` works for primitive f
- ✅ Named-function each: `f¨B` and `A f¨B` work when f is a defined function

### 2.2 Rank `⤡` (dyadic operator)
- Selects subarrays of specified rank for a function
- Example: `⍤ 2` applies a function to each 2-cell of a matrix
- ✅ Done: monadic `(f⍤k)B` and dyadic `A(f⍤kl kr)B` work

### 2.3 Axis Specification
- Extend reduce `/`, scan `\\`, rotate `⌽ ⊖`, take `↑`, drop `↓` to accept axis arguments
- Example: `+/[1] M` sums along first axis instead of last
- ✅ Done: `LO/[n]B` and `LO\[n]B` work for reduce/scan with axis
- ⚠️ Rotate/take/drop axis already supported via `A⌽[n]B` syntax

### 2.4 Power `⍣` (dyadic operator)
- Function iteration: `f⍣n` applies f n times
- Inverse: `f⍣¯1` applies the inverse of f
- Example: `2×⍣3 ⊢ 1` → 16
- ✅ Done: `f⍣N B` and `(F⍣N) B` work for primitives and named functions

---

## Phase 3 — Selective Assignment

### 3.1 LvalCell
- Pointer cells that reference another value's ravel slot
- Enables `arr[idx]←val` and `(expr)←val`
- ✅ Done: LvalCellData in cell.rs, COW isolation in value.rs

### 3.2 Modified Assignment
- `arr[idx]+=val` and similar
- ✅ Done: `NAME +← expr` works (shorthand for `NAME ← NAME + expr`)

### 3.3 Structural Selective Assignment
- `arr[idx]←val` where idx is an array of indices
- `(arr1 arr2)[idx]←val` — multiple arrays
- ✅ Done: `NAME[idx]←expr`, `NAME[i;j;...]←expr`, `(selector)←value`, `(A⊃NAME)←expr`

---

## Phase 4 — Native Functions

### 4.1 Native Function Loader — Done ✓
- Load `.so` shared libraries via `libloading`
- Resolve function signatures
- Call via FFI with `CAbiBinding::associate`

### 4.2 Native Function Interface — Done ✓
- Define the C ABI for native functions
- Pass ValueP pointers, return ValueP
- Memory management across the FFI boundary

### 4.3 ⎕NA Quad Function — Done ✓
- Syntax: `name ⎕NA 'result lib|symbol arg1 arg2 ...'`
- Format: `|` separates library from symbol (e.g., `libc.so.6|div`)
- Type codes: `I4` (int), `F8` (double), `F4` (float), etc.
- Verified: `10 mydiv 3` with `'I4 libc.so.6|div I4 I4'` → `3`

---

## Phase 5 — Infrastructure

### 5.1 Symbol Table
- Full symbol table with namespaces ✅ Done
- `⎕NS` — namespace creation ✅ Done
- `⎕CS` — current namespace switching ✅ Done

### 5.3 Workspace Commands
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

### 5.4 Macros
- Session macros (input/output recording) ✅ Done
- Function macros |

### 5.5 Security
- `⎕SEC` — security level
- Restricted operations at higher security levels

---

## Phase 6 — Optional / UI Features

### 6.1 Auxiliary Processors
- AP100 (shared variable server)
- AP210 (AP210 protocol)
- Full IPC implementation

### 6.2 GUI
- GTK interface (`⎕GTK`)
- Plot window management
- Line properties, window properties

### 6.3 Plotting
- ASCII plot (`Plot_ascii.cc`)
- Data plot (`Plot_data.cc`)
- GTK plot (`Plot_gtk.cc`)
- XCB plot (`Plot_xcb.cc`)

### 6.4 Python Integration
- Python pipe (`PythonPipe.cc`)
- Bidirectional communication

### 6.5 CDR / Archive Format
- Binary interchange format
- `⎕CDR` — CDR conversion
- `⎕INP` — input with CDR

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
```

---

## Non-Obvious Notes

- `∘` (U+2218 RING OPERATOR) is matrix product in GNU APL, not compose
- `⍥` (U+2365) and `⌸` (U+2328) are NOT in GNU APL 2.0 (Dyalog extensions)
- Cell::Char holds u32 codepoint; ValueP::char_vector takes &[u32]
- libapl uses thread-local Environment + Mutex-protected callbacks
- XML archive uses custom text format with ²...⁰ char mode and type tags
- PackedBool uses bit-packing in u64 words
- SmallVec<[Cell; 8]> avoids heap allocation for arrays with ≤8 elements
- Outer product parallelized when result has ≥4096 elements
