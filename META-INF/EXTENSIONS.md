# EXTENSIONS.md — Foreign-Language Extension Architecture

> Design document. Status: APPROVED-FOR-IMPLEMENTATION (nothing implemented yet).
> Companion docs: REWRITE_STRATEGY.md (project conventions), API.md (internal API).

---

## 1. Goals

1. **C compatibility** — load any shared object written in C/C++ using the
   *same* declaration syntax and semantics as Dyalog's `⎕NA`. Existing Dyalog
   `⎕NA` strings (modulo Windows-only DLLs) should work unchanged.
2. **Java objects** — instantiate and call Java classes/methods from APL,
   through a JVM bridge, with object handles usable as first-class APL values.
3. **Rust-first extensibility** — Rust is our flagship extension language.
   Writing a plugin must be *better* than writing C: typed access to `ValueP`,
   no manual marshalling, compile-time safety, registered as a normal cargo
   dependency-style workflow.
4. **Uniformity** — all three languages surface through ONE runtime concept
   (a native callable in the function table) so callers, persistence, and
   tooling cannot tell them apart.

## 2. Non-goals (this phase)

- Auxiliary Processors over sockets (the `Z` TCP/IP header format exists in
  the type system but socket transport is out of scope).
- In-process embedding of the interpreter INTO other languages (libapl-style);
  we do outbound calls only.
- COM/.NET, Python bridges (the adapter pattern below leaves room).

---

## 3. The one runtime concept: `NativeCallable`

Everything external becomes a single enum stored in the ordinary function
table, so `Expr::FuncCallMono` / `FuncCallDyad` need exactly one extra arm:

```rust
// src/functions_def.rs
#[derive(Clone)]
pub enum Callable {
    /// what we have today — interpreted ∇ function or dfn body
    Interpreted(DefinedFunction),
    /// any foreign-language binding (C, Rust plugin, Java)
    Native(NativeBinding),
}

#[derive(Clone)]
pub struct NativeBinding {
    pub name: String,              // APL-side name (name-class 3)
    pub min_args: usize,
    pub max_args: usize,           // ⎕NA fns are monadic: max == 1
    pub kind: NativeKind,
}

#[derive(Clone)]
pub enum NativeKind {
    /// ⎕NA association into a .so via the C ABI
    CAbi(CAbiSpec),
    /// Rust plugin exported by a cdylib we loaded
    RustPlugin(RustPluginSpec),
    /// Java method reachable through the JVM bridge .so
    Java(JavaMethodSpec),
}
```

Migration is mechanical: `FunctionTable: HashMap<String, DefinedFunction>`
becomes `HashMap<String, Callable>`; ~10 call sites change from
`self.funcs.get(name)` destructuring to a match. All dop/dfn machinery stays
untouched inside `Interpreted`.

### Dispatch contract

```
call_callable(name, args: Vec<ValueP>) -> AplResult<ValueP>
  ├─ Interpreted(f)   -> existing call_function / run_lines loop
  └─ Native(b)        -> b.kind.call(&mut env_ctx, args)
                          ├─ CAbi       -> marshal → libloading::Symbol → unmarshal
                          ├─ RustPlugin -> direct fn pointer on PluginVTable
                          └─ Java       -> JNI bridge generic invoke
```

Native calls are always **monadic**: the right argument is a possibly-nested
vector (exactly like Dyalog `⎕NA`). Dyadic sugar (`A fn B`) desugars at parse
time to `fn ⊂(A B)` — no special parser work beyond what dfns already do.

---

## 4. Data exchange: one marshalling core, two tiers

All cross-language values flow through a **stable, versioned, POD exchange
format** (`src/ffi/exchange.rs`). This mirrors Dyalog's `A` type (the
Auxiliary-Processor array) — deliberately, so `A`-typed arguments are already
speaking our wire format.

```rust
#[repr(C)]                      // layout-stable across the .so boundary
pub struct XArray {
    pub abi_version: u32,       // EXCHANGE_ABI = 1
    pub rank: u32,
    pub dims: [u64; MAX_RANK],  // MAX_RANK = 8 (raise if transpose ever needs more)
    pub elem_count: u64,
    pub cells: *mut XCell,      // ravel order
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union XCell {
    int: i64,
    float: f64,
    chr: u32,                   // Unicode code point
    ptr: u64,                   // nested: INDEX into owning XArray's child table
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XTaggedCell { pub tag: CellTag, pub cell: XCell } // tag: I/F/C/P/Nested
```

Rules:
- Nested values: a Nested cell's `ptr` is an INDEX into the owning XArray's
  child table — NEVER a raw pointer. Foreign code cannot forge or dangle
  nested references; ownership stays inside Rust (children freed
  transitively on Drop). The callee owns nothing across the boundary
  unless the signature says otherwise (`>A` / `=A` copy-out semantics).
- Conversions `ValueP ⇄ XArray` live ONLY in `src/ffi/exchange.rs`.
  Everything else in the tree uses `ValueP` above the line and `XArray`
  below it. One conversion point per direction = one place to audit.

**Tiering:**
- *Tier 0 (POD)* — `I U C T F D P` scalars/vectors/arrays: cheap, no heap
  nesting. Used by CAbi scalar paths.
- *Tier 1 (full)* — `A`, structures `{...}`, anything nested: full XArray
  conversion. Used by CAbi `A/Z/{}` paths, all Rust plugins, all Java calls.

---

## 5. Adapter 1 — C ABI (`⎕NA`)

### 5.1 Declaration parsing

New module `src/ffi/nadecl.rs`: a pure parser from the Dyalog grammar to
`CAbiSpec`. Grammar (subset we accept, matching Dyalog):

```
decl      := [result] [pathname '|'] symbol arg*
arg       := [dir] [special] type [width] [array]
result    := [dir] [special] type [width]      (optional — absent = shy nil)
dir       := '<' | '>' | '='                   (default: by-value scalar)
special   := '0' | '#'                         (NUL-terminated / byte-counted)
type      := 'I'|'U'|'C'|'T'|'F'|'D'|'J'|'P'|'A'|'Z'|'∇' | 'UTF'
width     := '1'|'2'|'4'|'8'|'16'
array     := '[' [int] ']'
structure := '{' arg+ '}' [array]
count     := '[int]' on the PRECEDING arg means "next N items"
```

Parse errors are `SYNTAX ERROR`; unknown type/width combos are
`DOMAIN ERROR` at association time (fail early, not at call time).

Type table (authoritative; keep in sync with `cell.rs`):

| Code | Rust | C | Notes |
|------|------|---|-------|
| I1/I2/I4/I8 | i8/i16/i32/i64 | int8..int64 | APL ints narrow-checked; range violation = DOMAIN ERROR |
| U1/U2/U4/U8 | u8..u64 | uint8..uint64 | negative APL int → DOMAIN ERROR |
| C1/T4 | char/u32 | char/wchar_t | Unicode edition semantics; C≡T here |
| F4/F8 | f32/f64 | float/double | Int→Float promotion allowed |
| UTF8/UTF16 | String | char* / wchar* | implies string semantics |
| P | usize | uintptr_t | ALSO our Java-handle currency (§7) |
| A | XArray | XArray* | full tier-1 conversion |
| J16 | (f64,f64) | double complex | gated on complex arithmetic milestone |
| D16 | — | — | NOT SUPPORTED v1: association-time DOMAIN ERROR |

### 5.2 Loader

`src/ffi/loader.rs` wraps the `libloading` crate (new Cargo dependency):

```rust
pub struct LibraryCache {
    libs: HashMap<PathBuf, Arc<libloading::Library>>, // dlopen handle cache
    syms: HashMap<(PathBuf, String), usize>,          // resolved fn pointers
}
```

Error mapping, mirroring Dyalog exactly:
- `dlopen` failure (incl. missing transitive deps) → `FILE ERROR 2 No such
  file or directory` (message notes the possibility of a missing dependency).
- Library loaded, `dlsym` miss → `VALUE ERROR`.
- Search path when no pathname given: `$APL_LIB_PATH` (our addition,
  colon-separated), then OS defaults. `.so` suffix assumed on Linux.

### 5.3 Call protocol

Generated per-binding closure:

1. Evaluate the right argument vector; arity-check against decl.
2. Marshal each arg per its spec:
   - `<X` input pointer: build C-side buffer, pass address, discard after.
   - `>X` output: allocate zeroed buffer sized by `[n]`/`[]` rules; the
     returned buffer becomes element k of the (nested) result.
   - `=X` in/out: copy-in, pass address, copy-out into result.
   - `0T`/`#T` strings: `<` copies APL chars+NUL/length byte;
     `>` allocates and returns enclosed char vector.
   - `∇` function pointers: **deferred** — v2 item (needs callback trampolines).
3. Call via `libloading::Symbol<unsafe extern "C" fn(...) -> ...>` built from
   the parsed signature (a small `unsafe` shim generated by a macro keyed on
   arity ≤ 12; beyond that → LIMIT ERROR at association time).
4. Unmarshal result (+ any `>`/`=` outputs, appended in declaration order,
   each enclosed — Dyalog rule).
5. **Panic containment**: the call runs under `std::panic::catch_unwind`;
   a caught panic or OS-level fault we can trap becomes
   `EXTERNAL EXCEPTION` (DOMAIN ERROR family) rather than aborting the REPL.

Workspace persistence: `NA <apl-name> <declaration-string>` record — we save
the ORIGINAL text, re-parse and re-dlopen on )LOAD (never persist raw
pointers). Same trick as DFN records today.

### 5.4 ⎕NA itself

Implemented as a system-function arm (sysvars.rs gains statement-form
handling for `⎕NA`):

```
'div' ⎕NA 'F8 math|divide I4 I4'    → establishes div, shy result 'div'
⎕NA 'F8 math|divide I4 I4'          → establishes divide
```

Association replaces any existing binding of that name (name-class stays 3).

---

## 6. Adapter 2 — Rust plugins (flagship)

### 6.1 The contract crate

New workspace member `crates/apl-ext` (published later as its own crate).
Plugins depend ONLY on it, never on the interpreter:

```rust
// crates/apl-ext/src/lib.rs  (the entire public API surface)
pub const PLUGIN_ABI_VERSION: u32 = 1;

pub trait AplExtension {
    fn name(&self) -> &'static str;
    /// called once after load; may register functions and set state
    fn register(&self, reg: &mut Registrar);
}

pub struct Registrar { /* opaque */ }
impl Registrar {
    pub fn bind<F>(&mut self, apl_name: &str, min: usize, max: usize, f: F)
        where F: Fn(&CallContext, &[XArray]) -> Result<XArrayOrShy, AplError>;
}

pub struct CallContext<'a> { /* sysvars read (⎕IO,⎕CT,⎕PP), arena allocator */ }
pub struct AplError { pub kind: ErrKind, pub message: String }
```

### 6.2 Export macro

```rust
apl_extension! {
    fn ext() -> Box<dyn AplExtension> { Box::new(MyExt) }
}
```

expands to the two symbols the loader probes (both `extern "C"`, `#[no_mangle]`):

```c
uint32_t apl_plugin_abi(void);            // -> PLUGIN_ABI_VERSION
void*    apl_plugin_create(void);         // -> Box<AplExtension> leaked raw
```

### 6.3 Loading

`⎕LOADSO 'path/to/plugin.so'` (new system function; distinct from ⎕NA so
declarations can't be confused):
1. dlopen → probe `apl_plugin_abi` → mismatch vs ours ⇒
   `DOMAIN ERROR: plugin ABI 2 > supported 1` (forward-compat: equal-major
   required, minor ignored).
2. `apl_plugin_create` → rebuild `Box<dyn AplExtension>` from raw.
3. `register()` runs; each `bind()` inserts a `NativeBinding{RustPlugin}`
   whose vtable slot holds the raw `Arc<dyn Fn>` rebuilt from the boxed trait.

Because both sides speak `XArray` and share the exchange module's layout,
calls are a pointer swap + conversion — measurably cheaper than CAbi for
nested data, and completely safe (catch_unwind still wraps plugin panics).

### 6.4 Why Rust plugins get preferential treatment

| Concern | C via ⎕NA | Rust plugin |
|---------|-----------|-------------|
| Type safety | none (trusts decl) | checked at compile time |
| Nested arrays | manual | `&[XArray]` slices + helpers |
| Errors | magic return codes | `Result<XArray, AplError>` |
| State | global vars in .so | `Box<dyn Any>` slot in context |
| Testing | out-of-band | `cargo test` against apl-ext mocks |

Documented recommendation: **new extensions in Rust; ⎕NA for legacy/system
libraries** (libc, OpenSSL, vendor SDKs).

---

## 7. Adapter 3 — Java

### 7.1 Shape of the solution

One bridge shared library, `libapljava.so`, written in **Rust** using the
`jni` crate (JNI Invocation API). It is itself a consumer of the SAME CAbi
machinery — no third adapter mechanism is needed at the interpreter level;
`Java(JavaMethodSpec)` is bookkeeping sugar over a small fixed set of CAbi
entry points. This is what keeps the three languages uniform: **Java rides on
adapter 1's plumbing.**

### 7.2 Object handles

Every Java object referenced from APL is a global ref held in a bridge-side
registry; APL sees its **handle number as a `P` value** (plain integer).
Handles are printable, comparable, storable in variables/arrays/workspaces
(persisted as numbers; on )LOAD stale handles resolve to NULL OBJECT ERROR on
use — documented, matching how Dyalog's own external handles behave).

### 7.3 Bridge surface (fixed, tiny)

```
'JInit'    ⎕NA 'P apljava|j_init <0T'                ⍝ classpath, returns env handle
'JNew'     ⎕NA 'P apljava|j_new P <0T <A'            ⍝ env, class, ctor args (XArray) -> obj handle
'JCallS'   ⎕NA 'A apljava|j_call_static P <0T <0T <A' ⍝ env, class, method, args -> XArray
'JCall'    ⎕NA 'A apljava|j_call P P <0T <A'          ⍝ env, obj, method, args -> XArray
'JGet'     ⎕NA 'A apljava|j_field P P <0T'             ⍝ env, obj, field -> XArray
'JSet'     ⎕NA '   apljava|j_field_set P P <0T <A'     ⍝ void
'JFree'    ⎕NA 'I4 apljava|j_free P P'                 ⍝ env, handle -> release global ref
```

Conversion policy inside the bridge (mirrors Dyalog's Java bridge spirit):
- scalars ↔ int/double/boolean/char/String
- APL vector ↔ primitive array or `Object[]` (nested XArrays)
- returned objects → new handles; Strings → char vectors; boxed numerics →
  unwrapped to APL scalars.

### 7.4 Ergonomic layer

Raw ⎕NA calls are wrapped ONCE in a shipped workspace `JAVA.APLWS`
(interpreted dfns — dogfoods our own dfn engine):

```apl
Jvm←JInit ''                                  ⍝ lazy singleton
obj←'java/util/HashMap' JNew ⊂⍬
obj JCall 'put' ('key' 'value')
h←obj JCall 'get' ⊂,'key'
```

If a future need demands faster dispatch, `Java(JavaMethodSpec)` lets us
hoist a specific `(class, method)` pair into a prebound NativeBinding —
same table, no new concepts.

---

## 8. Cross-cutting concerns

### 8.1 Threading & Rayon

Native calls are OPAQUE to the scheduler. Hard rule: `reduce`/`scan`/
parallel primitives NEVER invoke native callables inside a Rayon job unless
the binding was declared parallel-safe (`Registrar::bind_parallel`, Rust
plugins only; CAbi/Java bindings default unsafe and force sequential
fallback with a one-time warning to stderr). Prevents silent data races in
foreign code.

### 8.2 Memory ownership

Interpreter-side arena owns every XArray handed across the boundary for the
duration of one call. Output buffers (`>`/`=A`) are allocated from the arena
and converted to ValueP before the arena resets. Plugins receive an arena
allocator in CallContext if THEY want to return big data without copying
(v2 optimization; v1 copies).

### 8.3 Errors

| Source | APL surface |
|--------|-------------|
| decl parse fail | SYNTAX ERROR |
| bad type/width combo | DOMAIN ERROR (association time) |
| .so missing/deps | FILE ERROR 2 |
| symbol missing | VALUE ERROR |
| arg count/type mismatch detectable pre-call | DOMAIN ERROR |
| foreign panic/trap | EXTERNAL EXCEPTION (DOMAIN ERROR class, unique message) |
| plugin `Err(AplError)` | the mapped ErrorCode verbatim |
| Java exception | DOMAIN ERROR carrying `⎕DM`-style info (exception class + message) |

### 8.4 Security posture

Loading a shared object executes arbitrary code at load time — same trust
level as running a shell script. Documented loudly. No network/auto-download
of plugins, ever. `$APL_LIB_PATH` is honored but never implicitly extended.

---

## 9. Module map (where things land)

```
src/ffi/mod.rs          — public re-exports, EXCHANGE_ABI
src/ffi/exchange.rs     — ValueP ⇄ XArray (single conversion audit point)
src/ffi/nadecl.rs       — ⎕NA declaration parser → CAbiSpec
src/ffi/loader.rs       — LibraryCache, dlopen/dlsym, error mapping
src/ffi/cabi.rs         — marshal/call/unmarshal driver for CAbiSpec
src/ffi/plugin.rs       — RustPluginSpec load/probe/vtable rebuild
src/ffi/java.rs         — JavaMethodSpec + JAVA.APLWS bootstrap glue
crates/apl-ext/         — the contract crate (separate Cargo package)
src/functions_def.rs    — Callable enum, FunctionTable migration
src/sysvars.rs          — ⎕NA / ⎕LOADSO statement arms
src/workspace.rs        — NA / PLG records (text, re-resolved on LOAD)
Cargo.toml              — + libloading, + jni (feature-gated: feature = "java")
```

Feature gates: `ffi` (default on), `java` (off by default until first use —
keeps the base binary dependency-light).

## 10. Implementation phases

| Phase | Deliverable | Tests prove |
|-------|-------------|-------------|
| F1 | exchange.rs + round-trip property tests | nested ValueP survives XArray conversion bit-exact |
| F2 | nadecl.rs | parse table: every example line in §5 of Dyalog doc accepted/rejected correctly |
| F3 | loader.rs + cabi.rs + ⎕NA arm | `div 10 4 → 2.5` against a hand-built test.so; `>F8[]` output nesting |
| F4 | FunctionTable→Callable migration | full suite green, zero behavior change |
| F5 | apl-ext crate + ⎕LOADSO | demo plugin registering `HEX ← {...}`-style helper; panic containment test |
| F6 | java feature + libapljava + JAVA.APLWS | HashMap put/get round-trip from REPL |
| F7 | workspace NA records + docs | )SAVE/)LOAD re-binds div; README section |

Each phase lands as its own commit, suite green before moving on (house rule).

## 11. Open questions (decide during F2/F3)

1. `∇` function-pointer callbacks (APL fn passed INTO C) — needs a trampoline
   with a stable C signature calling back into `call_callable`. Deferred v2;
   decl parser will reject `∇` with a clear message until then.
2. Should `=A` mutate-in-place ever alias the caller's ValueP (copy-on-write
   hazard)? Default: always copy (safe); revisit if benchmarks complain.
3. Windows/macOS targets — loader is cfg'd but only Linux is CI-tested now.
