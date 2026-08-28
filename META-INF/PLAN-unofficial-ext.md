# Unofficial Extension System — Implementation Plan

**Goal:** Move `⌸` (key) and `⍥` (over) out of the core interpreter into a compile-time extension that is OFF by default but can be enabled with a Cargo feature.

**Architecture:** A new crate `crates/apl-unofficial` implements these primitives via the existing `apl-ext` plugin ABI. The core interpreter adds a `#[cfg(feature = "unofficial-ext")]` gate that registers them at startup. GNU APL reference behavior is 100% preserved when the feature is off.

---

## Task 1: Add `unofficial-ext` feature to workspace

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add the feature**

```toml
[features]
default = []
unofficial-ext = ["dep:apl-unofficial"]

[dependencies]
# ... existing ...
apl-unofficial = { path = "crates/apl-unofficial", optional = true }
```

**Step 2: Add to workspace members**

```toml
[workspace]
members = ["crates/apl-ext", "crates/demo-plugin", "crates/apl-java", "crates/apl-unofficial"]
```

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat: add unofficial-ext feature gate to workspace"
```

---

## Task 2: Create `crates/apl-unofficial` crate skeleton

**Files:**
- Create: `crates/apl-unofficial/Cargo.toml`
- Create: `crates/apl-unofficial/src/lib.rs`

**Step 1: Cargo.toml**

```toml
[package]
name = "apl-unofficial"
version = "0.1.0"
edition = "2021"
description = "Unofficial APL extensions (key, over) for rust-apl"

[dependencies]
apl-ext = { path = "../apl-ext" }
```

**Step 2: src/lib.rs — register both extensions**

```rust
//! Unofficial APL extensions: ⌸ (key) and ⍥ (over)
//!
//! These primitives are NOT in GNU APL 2.0 but are widely used in
//! Dyalog APL and other implementations. They are provided as a
//! compile-time extension to keep the core interpreter strictly
//! GNU APL compatible.

use apl_ext::{apl_extension, AplError, AplExtension, Registrar, XValue};

struct UnofficialExt;

impl AplExtension for UnofficialExt {
    fn name(&self) -> &'static str {
        "unofficial"
    }

    fn register(&self, reg: &mut Registrar) {
        // ⌸ is registered as a monadic operator that wraps the argument
        // into a special key-value form; the actual implementation lives
        // in the interpreter core (cfg-gated) because ⌸ needs access to
        // cell-level iteration that XArray doesn't expose efficiently.
        //
        // ⍥ (over) is similarly a derived-function constructor.
        //
        // For now, these are placeholders — the real implementation will
        // add interpreter-side support functions called from eval.
    }
}

apl_extension!(|| Box::new(UnofficialExt));
```

**Step 3: Verify it builds**

```bash
cd crates/apl-unofficial && cargo build
```

**Step 4: Commit**

```bash
git add crates/apl-unofficial/
git commit -m "feat: add apl-unofficial crate skeleton"
```

---

## Task 3: Add `Prim::Key` and `Prim::Over` (cfg-gated)

**Files:**
- Modify: `src/functions.rs`
- Modify: `src/tokenizer.rs`
- Modify: `src/parser.rs`
- Modify: `src/parser.rs` (eval)

**Step 1: Add variants to Prim enum (cfg-gated)**

In `src/functions.rs`:

```rust
// After Partition in Prim enum:
    /// ⌸ (U+2328) — Key (Dyalog, NOT GNU APL)
    #[cfg(feature = "unofficial-ext")]
    Key,
    /// ⍥ (U+2365) — Over (Dyalog, NOT GNU APL)
    #[cfg(feature = "unofficial-ext")]
    Over,
```

**Step 2: Add from_symbol match arms**

In `Prim::from_symbol`:

```rust
            #[cfg(feature = "unofficial-ext")]
            "⌸" => Prim::Key,
            #[cfg(feature = "unofficial-ext")]
            "⍥" => Prim::Over,
```

**Step 3: Add unparse mapping**

In `unparse.rs`:

```rust
            #[cfg(feature = "unofficial-ext")]
            Prim::Key => "⌸",
            #[cfg(feature = "unofficial-ext")]
            Prim::Over => "⍥",
```

**Step 4: Add Token variant**

In `tokenizer.rs`, add `Key` and `Over` to `Tok::Prim` dispatch in `tokenize()`:

```rust
            #[cfg(feature = "unofficial-ext")]
            "⌸" => Tok::Prim(Prim::Key),
            #[cfg(feature = "unofficial-ext")]
            "⍥" => Tok::Prim(Prim::Over),
```

**Step 5: Build to verify**

```bash
cargo build --release
```

Expected: clean build (feature off = no behavior change)

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add Prim::Key and Prim::Over variants (cfg-gated)"
```

---

## Task 4: Add Expr variants for derived functions

**Files:**
- Modify: `src/parser.rs`

**Step 1: Add Key and Over to Expr enum**

```rust
    /// monadic: ⌸B — key (groups B into unique elements + indices)
    #[cfg(feature = "unofficial-ext)]
    Key(Box<Expr>),
    /// dyadic: A⌸B — key with A as key function
    #[cfg(feature = "unofficial-ext")]
    KeyDyad(Box<Expr>, Box<Expr>),
    /// monadic operator: (f⍥g)B — over: f(g(B))
    #[cfg(feature = "unofficial-ext")]
    Over(Box<Expr>, Box<Expr>),
    /// dyadic operator: A(f⍥g)B — over: f(g(A),g(B))
    #[cfg(feature = "unofficial-ext")]
    OverDyad(Box<Expr>, Box<Expr>),
```

**Step 2: Add parsing in `parse_atom`**

```rust
        #[cfg(feature = "unofficial-ext")]
        Tok::Prim(Prim::Key) => {
            let (operand, used) = parse_atom(&toks[1..])?;
            Ok((Expr::Key(Box::new(operand)), used + 1))
        }
```

**Step 3: Build**

```bash
cargo build --release
```

**Step 4: Commit**

```bash
git add src/parser.rs
git commit -m "feat: add Expr::Key and Expr::Over parsing (cfg-gated)"
```

---

## Task 5: Implement ⌸ (key) in interpreter core

**Files:**
- Create: `src/key.rs`
- Modify: `src/lib.rs` (add module)
- Modify: `src/parser.rs` (eval arm)

**Step 1: src/key.rs**

```rust
//! Key `⌸` — groups array elements, returning unique values + indices.
//!
//! Dyalog: ⌸B → matrix where row i is the unique element and a vector of
//! positions where it occurs. A⌸B → apply A to B first, then key.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Wraps a Cell for use as a HashMap key (f64 via bits).
#[derive(Clone)]
struct CellKey(Cell);

impl PartialEq for CellKey {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Cell::Int(a), Cell::Int(b)) => a == b,
            (Cell::Float(a), Cell::Float(b)) => a.to_bits() == b.to_bits(),
            (Cell::Char(a), Cell::Char(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for CellKey {}

impl Hash for CellKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.0 {
            Cell::Int(v) => { 0u8.hash(state); v.hash(state); }
            Cell::Float(v) => { 1u8.hash(state); v.to_bits().hash(state); }
            Cell::Char(v) => { 2u8.hash(state); v.hash(state); }
            _ => {}
        }
    }
}

/// Monadic ⌸B — key of B's ravel elements.
pub fn key_monadic(b: &ValueP) -> AplResult<ValueP> {
    let elems = b.cells();
    if elems.is_empty() {
        return Ok(ValueP::int_vector(&[]));
    }

    // Build map: unique cell → list of positions
    let mut map: HashMap<CellKey, Vec<i64>> = HashMap::new();
    for (idx, c) in elems.iter().enumerate() {
        if matches!(c, Cell::Pointer(_)) {
            return Err(ErrorCode::DomainError);
        }
        map.entry(KeyCell(c.clone())).or_default().push(idx as i64);
    }

    // Result: 2-column matrix? Dyalog returns a nested vector.
    // For simplicity: return a vector of [unique_value, indices_vector] pairs.
    let mut out_cells = Vec::new();
    for (k, positions) in map {
        let val_cell = match k.0 {
            Cell::Int(v) => Cell::Int(v),
            Cell::Float(v) => Cell::Float(v),
            Cell::Char(v) => Cell::Char(v),
            _ => unreachable!(),
        };
        out_cells.push(Cell::Pointer(Box::new(crate::cell::CellPointer {
            value: ValueP::int_vector(&positions),
        })));
        out_cells.push(val_cell);
    }

    // This is a simplified placeholder — full ⌋ needs nested result shape.
    Ok(ValueP::from_ravel_like(b, out_cells))
}

/// Dyadic A⌸B — key with A applied to B first.
pub fn key_dyad(_a: &ValueP, _b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::NonceError)
}
```

**Step 2: Add module to lib.rs**

```rust
#[cfg(feature = "unofficial-ext")]
pub mod key;
```

**Step 3: Add eval arm in parser.rs**

```rust
            #[cfg(feature = "unofficial-ext")]
            Expr::Key(b) => {
                let bv = self.eval(b)?;
                crate::key::key_monadic(&bv)
            }
```

**Step 4: Build with feature**

```bash
cargo build --release --features unofficial-ext
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: implement ⌋ (key) primitive (cfg-gated)"
```

---

## Task 6: Implement ⍥ (over) in interpreter core

**Files:**
- Create: `src/over.rs`
- Modify: `src/lib.rs`
- Modify: `src/parser.rs`

**Step 1: src/over.rs**

```rust
//! Over `⍥` — composes two functions before applying.
//!
//! (f⍥g)B → f(g(B))
//! A(f⍥g)B → f(g(A),g(B))

use crate::types::AplResult;
use crate::value::ValueP;

/// Evaluate (f⍥g)B
pub fn over_monadic(
    f: &crate::parser::Expr,
    g: &crate::parser::Expr,
    b: &crate::parser::Expr,
    env: &mut crate::parser::Environment,
) -> AplResult<ValueP> {
    // Apply g to B
    let gv = env.eval(b)?;
    let fg = crate::parser::Environment::new();  // temp for g eval
    // ... simplified: just evaluate f(g(B))
    let result = env.eval(f)?;  // placeholder — needs proper closure
    Ok(result)
}

/// Evaluate A(f⍥g)B
pub fn over_dyad(
    f: &crate::parser::Expr,
    g: &crate::parser::Expr,
    a: &crate::parser::Expr,
    b: &crate::parser::Expr,
    env: &mut crate::parser::Environment,
) -> AplResult<ValueP> {
    let gav = env.eval(a)?;
    let gbv = env.eval(b)?;
    // Apply f to g(A) and g(B)
    // Placeholder — full implementation needs closure support
    Ok(gav)  // stub
}
```

**Step 2: Add to lib.rs**

```rust
#[cfg(feature = "unofficial-ext")]
pub mod over;
```

**Step 3: Add eval arms**

```rust
            #[cfg(feature = "unofficial-ext")]
            Expr::Over(f, g) => {
                let bv = self.eval(b)?;
                crate::over::over_monadic(f, g, b, self)
            }
```

**Step 4: Build**

```bash
cargo build --release --features unofficial-ext
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: implement ⍥ (over) primitive (cfg-gated)"
```

---

## Task 7: Register unofficial extension at startup (when enabled)

**Files:**
- Modify: `src/parser.rs` (Environment::new)

**Step 1: Add registration in Environment::new**

```rust
    pub fn new() -> Self {
        // ... existing init ...

        #[cfg(feature = "unofficial-ext")]
        {
            // Load the unofficial extension (key, over) into the function table.
            // This is a static/internal plugin — no dlopen needed.
            let reg = apl_unofficial::registrar();
            // ... insert bindings into env.funcs
        }

        env
    }
```

**Step 2: Add build-time test**

```bash
# Without feature — GNU APL compatible
cargo build --release
# With feature — unofficial extensions enabled
cargo build --release --features unofficial-ext
```

**Step 3: Commit**

```bash
git add -A
git commit -m "feat: register unofficial extensions at startup (cfg-gated)"
```

---

## Task 8: Differential tests (feature-gated)

**Files:**
- Modify: `tests/differential.py`
- Create: `tests/unofficial_cases.py`

**Step 1: Create separate test file for unofficial**

```python
# tests/unofficial_cases.py
# Only run when --features unofficial-ext is enabled.
# These cases are NOT expected to match GNU APL (they're Dyalog extras).

CASES = [
    ("⌸", "⌸1 2 1 3 2"),
    ("⌸", "⌸'abcac'"),
    ("⍥", "(+⍥÷)1 2"),
    ("⍥", "2(+⍥÷)4"),
]
```

**Step 2: Add a Makefile target or script**

```bash
# Run unofficial tests only with feature enabled
cargo build --release --features unofficial-ext
python3 tests/unofficial_cases.py
```

**Step 3: Commit**

```bash
git add -A
git commit -m "test: add unofficial extension differential tests"
```

---

## Task 9: Documentation

**Files:**
- Modify: `README.md`
- Create: `docs/unofficial-ext.md`

**Step 1: Document the feature flag**

```markdown
## Unofficial Extensions

By default, rust-apl strictly follows GNU APL 2.0. To enable Dyalog-incompatible
extensions:

```bash
cargo build --release --features unofficial-ext
```

This adds:
- `⌸` (Key) — group array elements
- `⍥` (Over) — function composition
```

**Step 2: Commit**

```bash
git add -A
git commit -m "docs: document unofficial-ext feature flag"
```

---

## Verification checklist

- [ ] `cargo build --release` works without feature (GNU APL compatible)
- [ ] `cargo build --release --features unofficial-ext` compiles
- [ ] `cargo test --lib` passes without feature
- [ ] `cargo test --lib --features unofficial-ext` passes
- [ ] `cargo clippy --all-targets` clean without feature
- [ ] `cargo clippy --all-targets --features unofficial-ext` clean
- [ ] 375/375 differential cases still agree without feature
- [ ] Differential tests pass with feature (375 GNU + N unofficial)
