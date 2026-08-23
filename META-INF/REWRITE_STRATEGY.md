# GNU APL 2.0 — Rewrite Strategy

> **Analyzing a rewrite of the GNU APL interpreter from C++ to ANSI C or Rust**  
> Based on comprehensive source analysis: 403 classes · 53 structs · 84 enums · 71 typedefs · 136+ free functions · 260+ source files · ~100K LOC C++

---

## 1. Executive Summary

GNU APL is a mature, feature-complete APL interpreter implemented in C++ (ISO/IEC 13751). This document analyzes the feasibility, strategy, and trade-offs of rewriting it in **ANSI C** or **Rust**.

**Verdict at a glance:**

| Dimension | ANSI C | Rust |
|---|---|---|
| Effort | Very High | High |
| Memory safety | Manual | Compile-time guaranteed |
| ISO compliance preservation | Easy (line-by-line) | Moderate (paradigm shift) |
| Parallelism | Already pthreads | Rayon/std::thread |
| Ecosystem | Sparse (roll your own) | Rich (crates) |
| Build simplicity | Trivial (bare cc) | Excellent (Cargo) |
| Team ramp-up | Easy (C is small) | Steep (ownership/borrowing) |
| Risk of subtle bugs | Very high | Low |
| Performance | Same | Same or better |
| Maintainability (post-rewrite) | Poor | Excellent |

**Recommendation:** Rust, with a phased migration that starts with the core cell/Value model and works outward, keeping C as an intermediate verification target only if needed. ANSI C is only recommended if the project must run on platforms without any Rust toolchain.

---

## 2. Current Architecture Analysis

### 2.1 What makes GNU APL hard to rewrite

The codebase has several deep structural features that complicate a port:

**1. The Cell polymorphism is the core of everything.**

```
Cell (abstract base, ~50 virtual methods)
├── CharCell          (Unicode)
├── IntCell           (APL_Integer)
├── FloatCell         (double or rational)
├── ComplexCell       (double[2])
├── PointerCell       (Value* for nested arrays)
└── LvalCell          (Cell* for selective assignment)
```

Every APL operation (`+`, `-`, `×`, `÷`, `⍳`, etc.) is dispatched through the Cell vTable. In C++ this is clean virtual dispatch. In C it becomes manual vTable management; in Rust it becomes enum dispatch or trait objects.

**2. Placement-new allocation pattern.**

Cells are never heap-allocated directly — they're placement-newed into the owning Value's ravel (a contiguous byte buffer). `Cell::operator new` is deliberately unimplemented to enforce this. This is critical for performance and memory layout but is fundamentally a C++ idiom. Both C and Rust can do this, but it requires explicit, unsafe, or carefully-designed pool allocators.

**3. The Value_P smart pointer.**

`Value_P` is a reference-counted handle to a `Value`. Its destructor decrements the owner count and frees the Value when it reaches zero. This is `std::shared_ptr`-like semantics with custom semantics (isolated deep copy on write). The pattern is:
- `Value_P a = b;` — shares ownership
- `a.isolate(loc);` — copy-on-write: if owner_count > 1, deep clone
- `a.isolate_deep(loc);` — recursively isolate all nested values

**4. The fetcher function pointer.**

```cpp
const Cell & (*fetcher)(ShapeItem offset, const Cell * ravel);
```

Every Value holds a function pointer that selects between packed (bit-packed boolean) and unpacked cell access. This avoids per-cell branching and is a key performance optimization.

**5. Cyclic references.**

`PointerCell` points to a `Value`, which contains `Cell`s. `LvalCell` points to another `Cell`. `DynamicObject` maintains a global doubly-linked ring of all live `Value`s and `IndexExpr`s. These cycles make Rust ownership tricky.

**6. The parallel execution model.**

A static worker pool (`CPU_pool`) runs `PJob_scalar_B` / `PJob_scalar_AB` units. Workers block on a semaphore while the master waits for terminal input. Scalar primitives on long vectors are automatically parallelized. The implementation uses pthreads atomics and spinlocks.

**7. Optional features via #ifdef.**

GTK, SQL (SQLite/PostgreSQL), FFT (GSL), QR (GSL), PNG, Python, plotting — all conditionally compiled via `config.h` `#define`s.

### 2.2 File count by subsystem

| Subsystem | Files | Port difficulty |
|---|---|---|
| Cell hierarchy (7 types) | ~15 | **Critical** — core dispatch |
| Value + Shape + Iterators | ~10 | **Critical** — memory model |
| Symbols + Workspace + SI | ~12 | Medium |
| Function hierarchy | ~20 | Hard (polymorphism) |
| Built-in functions (Bif_*) | ~25 | Medium (mostly mechanical) |
| Quad system functions | ~30 | Medium |
| Parser pipeline | ~10 | Medium |
| Infrastructure (Error, Log, etc.) | ~15 | Easy-Medium |
| Parallelism | ~5 | Medium |
| I/O system | ~15 | Easy |
| SQL subsystem | ~12 | Easy (has C API) |
| Emacs mode | ~14 | Easy (has C API) |
| GUI / Plotting | ~12 | Medium (has C API) |
| Native functions | ~8 | Medium |
| Auxiliary processors | ~6 | Medium |
| Archive / CDR | ~5 | Easy |

---

## 3. ANSI C Rewrite Strategy

### 3.1 Approach

Translate the C++ code to C89/C99 mechanically, preserving all behavior. The goal is a line-by-line, test-compatible port.

### 3.2 Class → Struct + vTables

Every class becomes a `struct` with its data members, plus an associated `vTable` struct holding function pointers.

**Example: Cell hierarchy**

```c
// Cell_vTable.hh becomes:
typedef struct Cell_vTable {
    void (*init_other)(void * self, void * other, Value * owner, const char * loc);
    bool (*greater)(const void * self, const void * other);
    bool (*equal)(const void * self, const void * other, double qct);
    Unicode (*get_char_value)(const void * self);
    APL_Integer (*get_int_value)(const void * self);
    APL_Float (*get_real_value)(const void * self);
    // ... ~50 function pointers per vTable
} Cell_vTable;

// Cell becomes:
typedef struct Cell {
    const Cell_vTable * vtable;  // replaces vptr
} Cell;

// CharCell:
typedef struct CharCell {
    const Cell_vTable * vtable;
    union { Unicode aval; } value;
} CharCell;
```

**Virtual call:** `cell->vtable->greater(cell, other)` instead of `cell->greater(other)`.

**Constructor pattern:** Placement new is replaced by explicit init functions:

```c
void CharCell_init(Cell * Z, Unicode av) {
    static const CharCell_vTable = { ... };  // static const, initialized once
    Z->vtable = &CharCell_vTable;
    ((CharCell*)Z)->value.aval = av;
}
```

### 3.3 Memory Model

**Value's ravel:** Since C has no placement new, the Value struct must include a flexible array member (C99) for the inline ravel:

```c
typedef struct Value {
    Shape shape;
    Cell * (*fetcher)(ShapeItem offset, Cell * ravel);
    int owner_count;
    ShapeItem pointer_cell_count;
    uint16_t flags;
    ShapeItem valid_ravel_items;
    ShapeItem nz_subcell_count;
    Cell * ravel;          // points to inline_ravel[] or heap
    Cell inline_ravel[];   // C99 flexible array member (or [1] for C89)
} Value;
```

**Cell init into ravel:** Use an explicit cell pool:

```c
// Allocate a Value with room for N cells
Value * Value_alloc(Shape * sh, const char * loc) {
    ShapeItem count = shape_volume(sh);
    Value * v = malloc(sizeof(Value) + count * sizeof(Cell));
    v->ravel = v->inline_ravel;
    v->owner_count = 1;
    return v;
}
```

**Placement of sub-cells:** Calculate offset and cast:

```c
Cell * cell_slot = &value->ravel[offset];
CharCell_init(cell_slot, unicode_val);  // writes into the ravel slot
```

### 3.4 Value_P in C

```c
typedef struct Value_P {
    Value * value_p;
} Value_P;

void Value_P_init_scalar(Value_P * self, const char * loc) {
    self->value_p = Value_alloc_scalar(loc);
}

void Value_P_destroy(Value_P * self) {
    if (self->value_p && --self->value_p->owner_count == 0) {
        Value_destroy(self->value_p);
        free(self->value_p);
    }
    self->value_p = NULL;
}

void Value_P_copy(Value_P * self, const Value_P * other, const char * loc) {
    Value_P_destroy(self);
    self->value_p = other->value_p;
    self->value_p->owner_count++;
}

void Value_P_isolate(Value_P * self, const char * loc) {
    if (self->value_p->owner_count > 1) {
        Value * clone = Value_deep_clone(self->value_p);
        Value_P_destroy(self);
        self->value_p = clone;
        self->value_p->owner_count = 1;
    }
}
```

### 3.5 Templates → Macros

C++ templates like `Heapsort<T>` become C macros:

```c
#define HEAPSORT_DECLARE(Type, suffix) \
    void Heapsort_##suffix(Type ** data, int len, bool ascending);

#define HEAPSORT_IMPLEMENT(Type, suffix, greater, swap) \
    void Heapsort_##suffix(Type ** data, int len, bool ascending) { \
        /* standard heapsort using greater(a,b) and swap(&a,&b) macros */ \
    }

// Usage:
HEAPSORT_DECLARE(Cell, Cell)       // → Heapsort_Cell
HEAPSORT_DECLARE(int, Int)         // → Heapsort_Int
```

`Parallel_job_list<T>` becomes a macro-based generic container:

```c
#define JOB_LIST_DECLARE(T, suffix) \
    typedef struct { \
        T * jobs; \
        int count; \
        int capacity; \
        int started_loc; \
    } JobList_##suffix; \
    void JobList_##suffix##_start(JobList_##suffix * self); \
    T * JobList_##suffix##_next_job(JobList_##suffix * self); \
    // ...
```

### 3.6 STL Containers → C Equivalents

| C++ | C approach |
|---|---|
| `std::vector<T>` | Manual realloc'd array (like klib's `kvec.h`) |
| `std::string` | Manual `char*` + length + capacity |
| `std::map<K,V>` | Hash table (uthash, htable) or balanced tree |
| `std::list<T>` | Doubly-linked list with embedded links |
| `std::ostream` | `FILE*` or callback-based output |

### 3.7 Error Handling

Most error handling already uses `ErrorCode` returns, so this is mechanical. C++ exceptions are not used (the codebase returns error codes). The `Error` class becomes a simple struct with a `print_error(FILE *)` function.

### 3.8 Parallel System

The parallel system already uses pthreads directly, so it ports almost unchanged. The atomics become `pthread_mutex_t` or C11 `<stdatomic.h>` if available.

### 3.9 Build System

With no dependencies:

```makefile
CC = gcc
CFLAGS = -std=c99 -O2 -Wall -Wextra -pedantic
LDFLAGS = -lm -lpthread

# Optional features (no make needed — just define flags)
ifdef HAVE_SQLITE3
CFLAGS += -DHAVE_SQLITE3
LDFLAGS += -lsqlite3
endif

OBJS = main.o Cell.o Value.o Symbol.o ... apl: $(OBJS)
	$(CC) -o $@ $(OBJS) $(LDFLAGS)
```

**Without `make` at all:** A single build script:

```sh
#!/bin/sh
gcc -std=c99 -O2 -o apl *.c -lm -lpthread
```

That's it. ~260 `.c` files compiled together. Slow to compile, but trivially simple.

### 3.10 Advantages of ANSI C

- **Ubiquitous.** Runs on anything with a C compiler — embedded systems, old Unix, mainframes, WASM.
- **No dependency hell.** No crate registry, no cargo, no toolchain version conflicts.
- **Simple mental model.** What you see is what you get — no borrow checker, no traits, no monomorphization.
- **Easy debugging.** GDB maps source directly — no name mangling, no trait indirection.
- **Incremental port possible.** Can compile C and C++ together during transition.
- **Preserves ISO compliance** mechanically — same algorithms, same dispatch, same results.

### 3.11 Disadvantages of ANSI C

- **Extremely verbose.** Every virtual call, every allocation, every error check is manual.
- **No type safety across the Cell hierarchy.** `Cell*` is just a pointer — you can pass a `CharCell*` where an `IntCell*` is expected.
- **Manual memory management everywhere.** No RAII. Every `malloc` needs a corresponding `free`, often in 5 different code paths.
- **VTable boilerplate.** 7 cell types × 50 methods = 350 function pointers to declare, initialize, and maintain.
- **No generics.** Every container must be hand-rolled or macro-generated for each type.
- **Bug surface area.** Use-after-free, double-free, buffer overflows — all silent.
- **Testing burden.** Every manual memory operation needs a test. The C++ smart pointers and destructors already provide this.

### 3.12 Estimated Effort

- **Core Cell + Value model:** 3-4 months (placement new, vTables, refcounting)
- **Built-in functions:** 2-3 months (mostly mechanical but error-prone)
- **Quad functions:** 1-2 months (many optional)
- **Parser + Workspace:** 2-3 months
- **Infrastructure:** 1-2 months
- **Testing + ISO compliance verification:** 3-4 months (critical — every APL edge case must pass)
- **Total:** ~12-18 months for one experienced developer
- **Risk of subtle bugs:** Very high. Memory corruption in the cell/Value model would take months to find.

---

## 4. Rust Rewrite Strategy

### 4.1 Approach

Don't mechanically translate — **re-architect** using Rust idioms. Preserve APL semantics and ISO compliance, but redesign the internals.

### 4.2 Cell Hierarchy → Rust Enum

This is where Rust shines. The Cell hierarchy is a textbook algebraic data type:

```rust
#[derive(Clone)]
pub enum Cell {
    Char(CharCell),
    Int(IntCell),
    Float(FloatCell),
    Complex(ComplexCell),
    Pointer(PointerCell),
    Lval(LvalCell),
}

#[derive(Clone)]
pub struct CharCell {
    value: Unicode,  // u32
}

#[derive(Clone)]
pub struct IntCell {
    value: i64,
}

#[derive(Clone)]
pub struct FloatCell {
    value: f64,  // or rational: (i64, i64) when cfg_RATIONAL_NUMBERS_WANTED
}

#[derive(Clone)]
pub struct ComplexCell {
    real: f64,
    imag: f64,
}

// PointerCell and LvalCell need special handling (see §4.4)
```

**All 50 virtual methods become match expressions:**

```rust
impl Cell {
    pub fn greater(&self, other: &Cell) -> bool {
        match (self, other) {
            // PointerCell > NumericCell > CharCell
            (Cell::Pointer(_), Cell::Pointer(_)) => { /* compare rank→shape→ravel */ }
            (Cell::Pointer(_), _) => true,
            (_, Cell::Pointer(_)) => false,
            
            // Numerics by value
            (Cell::Int(a), Cell::Int(b)) => a.value > b.value,
            (Cell::Int(a), Cell::Float(b)) => /* promote and compare */,
            // ... all combinations
            
            // Chars by code point
            (Cell::Char(a), Cell::Char(b)) => a.value > b.value,
            _ => unreachable!(),  // type mismatch = DOMAIN ERROR at higher level
        }
    }
    
    pub fn get_int_value(&self) -> Result<i64, ErrorCode> {
        match self {
            Cell::Int(c) => Ok(c.value),
            Cell::Float(c) if (c.value - c.value.round()).abs() < CT => Ok(c.value as i64),
            _ => Err(E_DOMAIN_ERROR),
        }
    }
}
```

**Advantages:**
- No vTables, no function pointer casting, no `void*`
- Compiler checks all match arms are exhaustive
- No undefined behavior from wrong-type access
- Often faster than vTables (branch prediction on enum tags)

### 4.3 Value Memory Model

The inline ravel pattern can be preserved with `#[repr(C)]`:

```rust
#[repr(C)]
pub struct Value {
    shape: Shape,
    ravel: [Cell; 0],  // C99 flexible array equivalent (needs unsafe)
}

// Or use a Vec<Cell> for the ravel and accept heap allocation
pub struct Value {
    shape: Shape,
    ravel: Vec<Cell>,  // heap-allocated, simpler
}
```

**Trade-off:** The C++ version uses inline ravel for short values to avoid heap allocation. A Rust port could:
1. Use `Vec<Cell>` — simpler, one extra heap allocation per short Value
2. Use `SmallVec<[Cell; 8]>` from the `smallvec` crate — stack-allocates up to 8 cells, then heap
3. Use `Box<[Cell]>` — single heap alloc, sized exactly
4. Keep the inline pattern with `union { Cell inline_ravel[N]; Cell * heap_ravel; }` — complex but zero-cost

**Recommendation:** `SmallVec<[Cell; 8]>` from the `smallvec` crate. This matches the C++ optimization almost exactly with safe Rust.

### 4.4 PointerCell / LvalCell — Handling Cycles

These are the hardest part of the Rust rewrite because they create reference cycles:

```
Value A → ravel: [PointerCell → Value B]
Value B → ravel: [LvalCell → Cell in Value A's ravel]
```

**Options:**

**Option A: `Rc<RefCell<Value>>`** (closest to C++ semantics)

```rust
pub struct PointerCell {
    value: Rc<RefCell<Value>>,
}

pub struct LvalCell {
    target: *const Cell,        // raw pointer into another Value's ravel
    owner: *const Value,        // for ownership tracking
}
```

`Rc` = reference count (like `Value_P`). `RefCell` = runtime borrow checking. Raw pointer for `LvalCell` (needed because it points into another Value's ravel, creating a cycle that `Rc` can't express).

**Pros:** Closest to existing semantics. `RefCell` runtime checks catch borrow violations.
**Cons:** `Rc<RefCell<T>>` has runtime overhead (refcount + borrow flag). Can panic on borrow violation. Not thread-safe (need `Arc<Mutex<T>>` for parallel).

**Option B: Arena allocation + handles**

```rust
pub struct ValueHandle(u32);  // index into a global ValueArena

pub struct PointerCell {
    target: ValueHandle,
}

pub struct Arena {
    values: Vec<Value>,
}
```

All Values live in a global arena. `PointerCell` holds an index, not a reference. Garbage collection via mark-and-sweep or generational references.

**Pros:** No refcount overhead. No borrow checker fights. Easy to snapshot for `isolate_deep()`.
**Cons:** Major redesign. Need GC or manual lifetime management.

**Option C: `Arc<RwLock<Value>>` for parallelism + unsafe for LvalCell**

```rust
pub struct PointerCell {
    value: Arc<RwLock<Value>>,
}

pub struct LvalCell {
    target: NonNull<Cell>,
    owner: NonNull<Value>,
}
```

Thread-safe reference counting + raw pointers for the cycle. This is what the parallel system needs anyway.

**Recommendation:** Option C. `Arc` for shared ownership (thread-safe), `RwLock` for interior mutability (needed because `Value::set_shape_item()` etc. mutate), raw pointers for `LvalCell` (unsafe but contained). The `NonNull` types are `!Send + !Sync` by default, which is correct because `LvalCell` is always thread-local.

### 4.5 The Fetcher Function Pointer

```rust
// C++:
// const Cell & (*fetcher)(ShapeItem offset, const Cell * ravel);

// Rust: use an enum instead of a function pointer
enum Fetcher {
    Packed,    // bit-packed boolean ravel
    Unpacked,  // Cell-sized ravel
}

impl Value {
    pub fn get_cell(&self, offset: usize) -> &Cell {
        match self.fetcher {
            Fetcher::Unpacked => &self.ravel[offset],
            Fetcher::Packed => {
                // compute bit from packed ravel
                // (returns a static Cell or a stack-allocated one)
                unreachable!("packed needs special handling")
            }
        }
    }
}
```

### 4.6 Error Handling

The C++ codebase already uses `ErrorCode` returns. In Rust, this becomes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    NoError = 0,
    SyntaxError = E_SYNTAX_ERROR,
    DomainError = E_DOMAIN_ERROR,
    // ... keep the same numeric values as C++ for compatibility
}

pub type ApiResult<T> = Result<T, ErrorCode>;
```

**Macros** (`DOMAIN_ERROR`, `Assert`, etc.) become inline functions or `debug_assert!`:

```rust
macro_rules! domain_error {
    () => { return Err(ErrorCode::DomainError); };
}

macro_rules! never_reach {
    ($msg:expr) => { unreachable!($msg); };
}
```

### 4.7 Parallelism

```rust
use rayon::prelude::*;

// Instead of manual PJob lists:
pub fn eval_scalar_AB(
    fun: &ScalarFunction,
    a: &Value,
    b: &Value,
    z: &mut Value,
) -> Result<(), ErrorCode> {
    let len = z.element_count();
    let chunks: Vec<&mut [Cell]> = z.ravel.chunks_mut(CHUNK_SIZE).collect();
    
    chunks.par_iter_mut().for_each(|chunk| {
        for cell in chunk {
            // apply scalar operation
        }
    });
    Ok(())
}
```

Rayon handles the thread pool, work-stealing, and chunking automatically. Much simpler than the C++ `Parallel` + `PJob` + `CPU_pool` system.

### 4.8 Optional Features → Cargo Features

```toml
[features]
default = []
gtk = ["dep:gtk4"]
sqlite = ["dep:rusqlite"]
postgres = ["dep:postgres"]
fft = ["dep:rustfft"]
plot = ["dep:plotters"]
python = ["dep:pyo3"]
libapl = []
parallel = ["dep:rayon"]
```

Each optional feature is a separate crate dependency, enabled at compile time. Much cleaner than `#ifdef` chains.

### 4.9 Build System

```toml
[package]
name = "gnu-apl"
version = "2.0.0"
edition = "2021"

[dependencies]
libc = "0.2"
smallvec = "1.13"
rayon = { version = "1.10", optional = true }
rusqlite = { version = "0.32", optional = true }
postgres = { version = "0.19", optional = true }
gtk4 = { version = "0.9", optional = true }
rustfft = { version = "6.2", optional = true }
plotters = { version = "0.3", optional = true }
pyo3 = { version = "0.23", optional = true }

[[bin]]
name = "apl"
path = "src/main.rs"
```

**Build commands:**
```sh
cargo build                          # minimal
cargo build --all-features           # everything
cargo build --features sqlite,fft    # selective
cargo test                           # run all tests
cargo build --release --features parallel  # optimized parallel build
```

### 4.10 Advantages of Rust

- **Memory safety.** No use-after-free, no double-free, no buffer overflows. The borrow checker enforces this at compile time.
- **Type-safe Cell dispatch.** Exhaustive `match` means you can't forget a cell type.
- **Enum Cell hierarchy.** Cleaner than C++ inheritance or C vTables.
- **Cargo ecosystem.** No more autotools, no `config.h` soup.
- **Testing.** Built-in `cargo test`, `#[test]`, property-based testing with `proptest`.
- **Parallelism.** Rayon is vastly simpler than manual `PJob` pools.
- **Error handling.** `Result<T, E>` is more ergonomic than error code macros.
- **Documentation.** `cargo doc` generates API docs from comments.
- **FFI.** Can call existing C libraries (GSL, SQLite) via `bindgen` or manual `extern "C"`.

### 4.11 Disadvantages of Rust

- **Steep learning curve.** Ownership, borrowing, lifetimes — the entire team must learn these.
- **Cycle handling.** `PointerCell`/`LvalCell` require unsafe code or `Arc<RwLock<T>>` overhead.
- **Compile times.** Rust compilation is slower than C.
- **Toolchain dependency.** Need `rustc` + `cargo`. Not available on all target platforms (embedded, some mainframes, some game consoles).
- **DynamicObject global list.** The global doubly-linked ring of all live Values requires `static mut` or `LazyLock<Mutex<Vec<NonNull<Value>>>>` — either unsafe or has locking overhead.
- **Placement new pattern.** Either accept heap allocation per cell, or use `Pin<Box<[Cell]>>` or custom allocators.
- **Risk of over-engineering.** The temptation to make everything "idiomatic Rust" could delay the rewrite significantly.

### 4.12 Estimated Effort

- **Core Cell model (enum + match dispatch):** 2-3 months
- **Value + Shape + memory model:** 2-3 months (arena vs Rc vs SmallVec decisions)
- **Built-in functions:** 1-2 months (mechanical with match instead of virtual)
- **Quad functions:** 1-2 months
- **Parser + Workspace:** 2-3 months
- **Infrastructure:** 1-2 months (error, logging, etc.)
- **Parallelism:** 1 month (Rayon is simpler)
- **Testing + ISO compliance:** 2-3 months (still critical — Rust doesn't make APL edge cases disappear)
- **Total:** ~12-16 months for one experienced Rust developer
- **Risk of subtle bugs:** Low. Rust eliminates memory bugs; logic bugs remain but are caught by `Result` types and exhaustive matching.

---

## 5. Comparative Analysis

### 5.1 Effort Comparison

| Phase | ANSI C | Rust |
|---|---|---|
| Core Cell + Value | 3-4 mo | 2-3 mo |
| Built-ins | 2-3 mo | 1-2 mo |
| Quads | 1-2 mo | 1-2 mo |
| Parser + Workspace | 2-3 mo | 2-3 mo |
| Infrastructure | 1-2 mo | 1-2 mo |
| Parallelism | 1 mo | 1 mo |
| Testing + ISO verify | 3-4 mo | 2-3 mo |
| **Total** | **12-18 mo** | **12-16 mo** |

Similar total effort. C is more verbose; Rust has more design decisions.

### 5.2 Risk Profile

| Risk | ANSI C | Rust |
|---|---|---|
| Memory corruption | Very high | Eliminated |
| Type confusion | High | Eliminated |
| ISO compliance drift | Low (mechanical) | Medium (paradigm shift) |
| Build breakage | Low | Low |
| Performance regression | Low | Low |
| Security vulnerabilities | High | Low |
| Maintenance after port | Hard | Easy |
| Team availability | Easy (many C devs) | Harder (fewer Rust devs) |

### 5.3 Ecosystem Fit

| Need | ANSI C | Rust |
|---|---|---|
| GTK GUI | `gtk.h` (native) | `gtk4` crate (excellent) |
| SQLite | `sqlite3.h` (native) | `rusqlite` (excellent) |
| PostgreSQL | `libpq-fe.h` (native) | `postgres` crate (excellent) |
| FFT | GSL (native) | `rustfft` crate (pure Rust) |
| PNG | `libpng` (native) | `png` crate (pure Rust) |
| Regex | PCRE2 (native) | `regex` crate (pure Rust, faster) |
| Plotting | Manual Cairo | `plotters` crate (pure Rust) |
| Python FFI | `Python.h` (native) | `pyo3` crate (excellent) |
| Emacs protocol | Custom TCP | Custom TCP (same) |
| TLS/SSL | OpenSSL (native) | `rustls` crate (pure Rust, safer) |

Rust's ecosystem is actually stronger for this project — many optional features can use pure-Rust crates with better safety and ergonomics than their C counterparts.

### 5.4 Performance

Both languages can match or exceed C++ performance for this workload:

- **C:** Same as C++ with `-O2` — manual vTables are slightly slower than compiler-generated vTables, but the difference is negligible for APL workloads (the bottleneck is memory allocation, not dispatch).
- **Rust:** Enum dispatch is often faster than C++ vTables because the compiler can optimize across match arms. `SmallVec` matches the inline-ravel optimization. Rayon's work-stealing is comparable to the manual `Parallel` pool.

### 5.5 Portability

| Platform | ANSI C | Rust |
|---|---|---|
| Linux x86_64 | ✓ | ✓ |
| macOS | ✓ | ✓ |
| Windows | ✓ | ✓ |
| *BSD | ✓ | ✓ |
| WebAssembly | ✓ (emscripten) | ✓ (wasm32) |
| Embedded (ARM Cortex-M) | ✓ | Partial (needs `no_std`) |
| Mainframe (z/OS) | ✓ | ✗ |
| HP-UX / Solaris | ✓ | Partial |
| Haiku / SerenityOS | ✓ | Partial |

C wins on exotic/old platforms. Rust wins on modern ones.

---

## 6. Recommendation

### 6.1 Recommendation: Rust

**Rust is the better choice** for rewriting GNU APL, with the following caveats:

1. **Only if the target platform has a Rust toolchain.** If you need to support mainframes, embedded systems, or esoteric Unix variants, use C.

2. **Only if the development team (or key developer) is willing to invest 2-3 months learning Rust** before the rewrite begins. The learning curve is real and front-loaded.

3. **The rewrite should be a redesign, not a translation.** Trying to mechanically map C++ classes to Rust structs+traits will produce ugly, slow code. The Cell enum, `SmallVec` for ravels, and Rayon for parallelism should be designed from scratch.

### 6.2 When to choose ANSI C instead

- The project must run on platforms without Rust support
- The development team has no bandwidth to learn Rust
- The goal is a line-by-line mechanical port for verification purposes
- You need to integrate with a larger C codebase

### 6.3 Hybrid approach: C core, Rust wrappers

Another option not yet discussed: rewrite only the core interpreter in C (the Cell/Value model, built-in functions, parser), and write a Rust layer on top for the optional features (GTK, SQL, plotting, emacs mode, Python bridge). This gives you:
- C portability for the core
- Rust safety for the features most likely to have bugs (GUI, networking, Python FFI)
- Incremental adoption

---

## 7. Phased Migration Plan

### Phase 0: Preparation (2-4 weeks)

- [ ] Set up the new project structure (Cargo.toml or Makefile)
- [ ] Port the test suite first — every APL test case from `src/testcases/` must pass
- [ ] Establish a CI pipeline that runs the full test suite on every commit
- [ ] Document the APL compliance requirements (ISO 13751 + GNU extensions)
- [ ] Set up benchmarking for key operations (scalar reduce, matrix multiply, sorting)

### Phase 1: Core Cell Model (4-6 weeks)

**Goal:** All cell types, placement into ravel, dispatch, comparison, and arithmetic.

**Rust:**
- [ ] `Cell` enum with all 6 variants
- [ ] `Value` struct with `SmallVec` ravel
- [ ] `Value_P` → `Rc<RefCell<Value>>` / `Arc<RwLock<Value>>`
- [ ] All 50+ Cell methods as match expressions
- [ ] `greater()`, `equal()`, tolerantly-equal helpers
- [ ] All `bif_*` operations for each cell type
- [ ] Packed boolean ravel support (fetcher enum)

**ANSI C:**
- [ ] `Cell` struct + vTable with 50 function pointers
- [ ] Per-type vTables for CharCell, IntCell, etc.
- [ ] `Value` struct with flexible array ravel
- [ ] `Value_P` with manual refcount functions
- [ ] All 50+ methods as vTable entries
- [ ] Cell pool allocator for placement into ravel

**Verification:** Run all scalar arithmetic test cases from the existing test suite.

### Phase 2: Value + Shape + Iterators (3-4 weeks)

**Goal:** Multi-dimensional arrays, indexing, iteration.

- [ ] `Shape` struct with all dimension operations
- [ ] `Value` constructors (scalar, vector, matrix, general)
- [ ] `Value_P` copy-on-write (isolate, isolate_deep)
- [ ] `ArrayIterator`, `AxisIterator`, `IndexIterator`
- [ ] `IndexExpr` evaluation (simple, choose, reach, indexed assignment)
- [ ] Value history ring buffer

**Verification:** All array creation, reshaping, indexing test cases.

### Phase 3: Symbol Table + Workspace (3-4 weeks)

**Goal:** Named variables, functions, workspace state.

- [ ] `Symbol` struct (assign, localize, trace, stop)
- [ ] `SymbolTable` (hash table)
- [ ] `Workspace` (global state, system variables)
- [ ] `StateIndicator` (call stack)
- [ ] `NamedObject`, `ValueHistory`

**Verification:** All workspace save/load, symbol table, localization test cases.

### Phase 4: Parser + Prefix Machine (4-6 weeks)

**Goal:** Parse APL text, evaluate expressions.

- [ ] `Tokenizer` (Unicode → Token stream)
- [ ] `Parser` (recursive descent, parse APL statements)
- [ ] `Prefix` machine (reduce token trees)
- [ ] All `eval_*` entry points for the Function hierarchy
- [ ] Derived function binding (operator+function, function+axis)

**Verification:** All parser test cases, all expression evaluation test cases.

### Phase 5: Built-in Functions (4-6 weeks)

**Goal:** All primitive functions and operators.

- [ ] `ScalarFunction` (all math primitives: + - × ÷ ⋆ ○ ⌈ ⌊ ∣ = < ≤ > ≥ ≠ ∧ ∨ ⍲ ⍱)
- [ ] `NonscalarFunction` (⍴ ⌷ ⍳ ≡ ≢ ⊤ ⊥ ⌽ ⊖ ⍉ ⍋ ⍒ ↑ ↓)
- [ ] `Bif_F12_*` (all 8 F12 functions)
- [ ] `Bif_OPER1_*` (⍨ ¨ / ⌿ \\ ⍀)
- [ ] `Bif_OPER2_*` (inner, outer, power, rank)
- [ ] Heapsort integration for ⍋ ⍒

**Verification:** All function test cases from the existing suite.

### Phase 6: Quad System Functions (3-4 weeks)

**Goal:** All ⎕XX system functions.

- [ ] `QuadFunction` base class/trait
- [ ] All standalone quads (CC, DLX, FFT, FX, GTK, JSON, MAP, PNG, RE, RL, RVAL, TF, WA, XML)
- [ ] All function groups (CR, FIO, MX, PLOT, SQL)
- [ ] System variables (IO, CT, FC, PP, PW, RL, etc.)
- [ ] ProcessorID, SystemLimits

**Verification:** All quad function test cases.

### Phase 7: I/O + Infrastructure (2-3 weeks)

**Goal:** User interaction, output, configuration.

- [ ] `LineInput` (with libedit/readline)
- [ ] `Output` (formatted printing)
- [ ] `PrintBuffer` (multi-pass APL formatting)
- [ ] `PrintContext`, `PrintOperator`
- [ ] `Error` class, error macros
- [ ] `Logging`, `Security`, `Performance`
- [ ] `UserPreferences`, `TabExpansion`

**Verification:** Output formatting test cases, all error messages.

### Phase 8: Parallelism (2-3 weeks)

**Goal:** Multi-core scalar execution.

**C:** Port the existing `Parallel`/`CPU_pool`/`PJob` system (already pthreads).
**Rust:** Replace with Rayon — much simpler.

**Verification:** Performance benchmarks on multi-core systems.

### Phase 9: Optional Features (4-6 weeks, parallelizable)

**Goal:** All the optional subsystems.

- [ ] GTK GUI (Quad_GTK)
- [ ] SQL (SQLite + PostgreSQL providers)
- [ ] Plotting (ASCII, GTK, XCB backends)
- [ ] Native function interface
- [ ] Python bridge
- [ ] Emacs mode
- [ ] Auxiliary processors (AP100, AP210)
- [ ] Archive/XML save/load
- [ ] libapl embedding API

**Verification:** All optional-feature test cases.

### Phase 10: Full Compliance Verification (4-6 weeks)

**Goal:** Prove the rewrite matches the original.

- [ ] Run the complete APL test suite
- [ ] Compare output byte-for-byte with the C++ version
- [ ] Benchmark all operations
- [ ] Fuzz testing (random APL programs, compare outputs)
- [ ] Test on all supported platforms
- [ ] Security audit

---

## 8. Appendix: Key Rust Type Definitions

### 8.1 Core Cell Types

```rust
use std::sync::{Arc, RwLock};

pub type Unicode = u32;
pub type APL_Integer = i64;
pub type APL_Float = f64;
pub type ShapeItem = i64;

#[derive(Clone)]
pub struct CharCell { pub value: Unicode }

#[derive(Clone)]
pub struct IntCell { pub value: i64 }

#[derive(Clone)]
pub struct FloatCell { pub value: f64 }

#[derive(Clone)]
pub struct ComplexCell { pub real: f64, pub imag: f64 }

#[derive(Clone)]
pub struct PointerCell { 
    pub value: Arc<RwLock<Value>>  // shared ownership
}

#[derive(Clone)]
pub struct LvalCell {
    pub target: std::ptr::NonNull<Cell>,  // raw pointer into ravel
    pub owner: std::ptr::NonNull<Value>,
}

#[derive(Clone)]
pub enum Cell {
    Char(CharCell),
    Int(IntCell),
    Float(FloatCell),
    Complex(ComplexCell),
    Pointer(PointerCell),
    Lval(LvalCell),
}
```

### 8.2 Value

```rust
use smallvec::SmallVec;

pub struct Value {
    shape: Shape,
    fetcher: Fetcher,
    ravel: SmallVec<[Cell; 8]>,  // inline up to 8 cells
}

pub enum Fetcher {
    Packed,
    Unpacked,
}

// Value_P equivalent:
pub type Value_P = Arc<RwLock<Value>>;  // for shared ownership
// For thread-local ownership:
pub type LocalValue_P = Rc<RefCell<Value>>;
```

### 8.3 Shape

```rust
#[derive(Clone)]
pub struct Shape {
    rank: u32,
    dims: [ShapeItem; MAX_RANK],  // MAX_RANK = 8
    volume: ShapeItem,
}
```

### 8.4 Error Type

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode { /* same numeric values as C++ */ }

pub type AplResult<T> = std::result::Result<T, ErrorCode>;

impl std::error::Error for ErrorCode {}
impl std::fmt::Display for ErrorCode { /* ... */ }
```

### 8.5 Key Traits

```rust
// For all cell operations (replaces Cell vTable)
pub trait CellOps {
    fn greater(&self, other: &Cell) -> bool;
    fn equal(&self, other: &Cell, qct: f64) -> bool;
    fn get_int_value(&self) -> AplResult<i64>;
    fn get_real_value(&self) -> AplResult<f64>;
    // ...
}

// For callable functions (replaces Function hierarchy)
pub trait Callable {
    fn eval_b(&self, b: Value_P) -> AplResult<Value_P>;
    fn eval_ab(&self, a: Value_P, b: Value_P) -> AplResult<Value_P>;
    // ...
}
```

---

*End of REWRITE_STRATEGY.md*
