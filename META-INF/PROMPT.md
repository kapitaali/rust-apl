# PROMPT.md — Continuation Guide for the GNU APL → Rust Rewrite

> **You are an agent picking up this project cold.** This file tells you
> everything you need to continue the work. Read it fully, then follow the
> Workflow section below before writing any code.

---

## 1. What this project is

`/home/theb/Apps/apl-2.0/` contains **GNU APL 2.0**, the original C++ interpreter
for ISO/IEC 13751 ("Programming Language APL, Extended", © Jürgen Sauermann,
GPLv3). It is a **reference implementation — never modify the C++ sources**.

Inside it, `rust-apl/` is an in-progress **Rust rewrite** of that interpreter.
The goal: a Rust implementation with identical APL semantics, following the
phased migration plan in `REWRITE_STRATEGY.md`.

### Key documents (read in this order)

| File | Contents |
|---|---|
| `rust-apl/META-INF/PROGRESS-YYYYMMDD.md` | **Start here.** Session-by-session log. Find the newest one (by date in filename) and read it — especially its "Next steps" list, which is your task queue. |
| `REWRITE_STRATEGY.md` | Why Rust was chosen over C, architecture mapping (C++ class → Rust construct), and the 10-phase migration plan (§7) with verification gates. |
| `API.md` | Comprehensive reference of the C++ source: 24 sections covering all 403 classes, enums, typedefs, functions. Use it to look up semantics of anything you're porting. |
| `.api-docs/parts/*.md` | 18 deep-dive documents on specific C++ subsystems (cells, values, symbols, quads, parser, emacs mode, etc.). |
| `rust-apl/README.md` | Status table of the Rust port: which module mirrors which C++ file. |

---

## 2. Current state (as of 2026-08-22)

**The Rust port is a working REPL with 86 passing tests**, supporting:
arithmetic (`+ - × ÷ ! ⌈ ⌊ ⋆ ○ ∣`), structural functions (`⍳ ⍴ ↑ ↓ ⌽ ⍉`),
search/sort (`⍋ ⍒ ∈`, dyadic `⍳`, bracket indexing `B[i]`),
operators (reduce `/`, scan `\`, commute `F⍨`),
assignment (`X←...`) and selective assignment (`B[i]←x`),
right-to-left evaluation, scalar extension, strands (`2 3⍴...`),
and nested-array infrastructure (`Rc`-based PointerCell, enclose/disclose helpers).

Zero clippy warnings. Release build verified.

### Module map (`rust-apl/src/`)

| Rust file | Mirrors (C++ in `src/`) | Notes |
|---|---|---|
| `types.rs` | `APL_types.hh`, `APL_enums.hh`, `Error.def` | typedefs, CellType bitflags, ErrorCode |
| `cell.rs` | `Cell*.hh/cc` | enum Cell {Char, Int, Float, Complex, Pointer, Lval}; all scalar bif_* ops; Lanczos tgamma; tolerant equality (⎕CT=1e-13); ordering Pointer > Numeric > Char |
| `shape.rs` | `Shape.hh/cc` | rank/rho/volume; MAX_RANK=8 |
| `value.rs` | `Value.hh`, `Value_P.hh` | ValueP = Rc<ValueInner>; COW isolate(); nested()/disclose() |
| `functions.rs` | `ScalarFunction.cc`, `Bif_F12_*` | Prim enum dispatch; monadic + dyadic eval; elementwise broadcast; reshape |
| `tokenizer.rs` | `Tokenizer.cc` | numbers (¯, exponents), names (∆⍙_), strings ('' escape), prim glyphs, comments ⍝ |
| `parser.rs` | `Parser.cc` + prefix machine (simplified) | recursive descent, right-to-left eval, Environment (HashMap vars), tests lock APL semantics |
| `main.rs` | `main.cc` (minimal REPL) | vector/matrix formatting; )OFF quits |
| `operators.rs` | `Bif_OPER1_REDUCE/SCAN` | right-to-left fold; empty-axis identity |
| `rotate.rs` | `Bif_ROTATE` | reverse + rotate, per-row shifts |
| `sort.rs` | `Bif_F12_SORT` | stable grade via sort_by (Heapsort not needed) |
| `take_drop.rs` | `Bif_F12_TAKE_DROP` | prototype padding (0 / space), per-row on matrices |
| `transpose.rs` | `Bif_F12_TRANSPOSE` | monadic (swap axes 0/1) + dyadic (full permutation) |
| `index_of.rs` | `Bif_F12_INDEX_OF` | first occurrence; not-found = len(A) |
| `epsilon.rs` | `Bif_EPSILON` | membership |

---

## 3. Non-negotiable conventions

1. **Never touch the C++ sources.** All work happens in `rust-apl/`.
2. **Check today's date** (`date '+%Y%m%d'`). At session start, read the newest
   `rust-apl/META-INF/PROGRESS-YYYYMMDD.md`. When you finish work (or reach a milestone), append
   to today's progress file in `rust-apl/META-INF/` (create it if missing) using the
   established format: numbered entries under "Session history", updated "Final
   state" line, refreshed "Next steps" list.
3. **Before claiming done:** `cargo fmt && cargo test && cargo clippy` — all
   tests pass, zero warnings. Verify REPL behavior by piping commands:
   `printf '2+3\n)OFF\n' | ./target/release/apl`.
4. **Max 2 subagents** if you delegate at all (the user's API key rate-limits);
   prefer doing work directly in-session.
5. **Semantics-first:** when porting, read the corresponding C++ source (grep
   `src/*.cc` for the function) to confirm exact behavior — fold direction,
   error codes, edge cases (empty axes, prototypes). The C++ code is the spec.
6. **Errors as values:** every primitive returns `Result<_, ErrorCode>`;
   mirror C++ error codes (DOMAIN_ERROR, RANK_ERROR, LENGTH_ERROR, INDEX_ERROR).
7. **Indices are 0-based** in this port (monadic `⍳` generates `0..n`);
   document any deviation from ⎕IO←1 conventions explicitly.
8. **Tests lock semantics.** Every feature gets tests that pin exact expected
   outputs, including non-commutative directions (`-/1 2 3 = 2`,
   `-\\1 2 3 = 1 ¯1 2`) so future refactors can't silently change behavior.

### Code style

- One concern per module (`take_drop.rs`, not `misc.rs`).
- Module doc-comment cites the C++ files mirrored.
- `#[cfg(test)] mod tests` inside each module with helper fns (`ints(&v)`).
- Clippy auto-fix sometimes strips imports used only in tests — if `cargo test`
  fails after a clippy fix, re-add `use crate::cell::Cell;` etc. to the test mod.

---

## 4. How to port a new feature (recipe)

1. **Find the C++ implementation**: `grep -rn "SYMBOL" ~/Apps/apl-2.0/src/*.hh`
   to locate the class, then read its `.cc`. Note: valence (monadic/dyadic),
   axis handling, error cases, empty-input behavior.
2. **Create or extend a Rust module** mirroring that file's name.
3. **Add a `Prim` variant** (in `functions.rs`) if it's a function;
   wire into `from_symbol()` for parsing, `PRIM_SYMBOLS` (tokenizer.rs),
   and both `eval_monadic` / `eval_dyadic` match arms as appropriate.
4. **Write tests first-ish**: pin down expected outputs (compute them by hand
   from APL semantics — e.g. Dyalog/GNU APL documentation — before running).
5. **fmt, test, clippy, REPL-verify**, then update PROGRESS.

---

## 5. Task queue (from newest rust-apl/META-INF/PROGRESS file — verify against it!)

As of this writing, next steps are:

1. **Enclose `⊂B` / disclose `⊃B`** as first-class primitives — infrastructure
   already exists (`ValueP::nested()` / `ValueP::disclose()` in value.rs);
   just needs Prim variants + wiring + tests.
2. **Each `¨` operator** — apply F element-wise, wrapping results as pointers.
   Tokenizer: `¨`; parse like Reduce/Scan; results nest scalars.
3. **Nested literals** — `(1 2)(3 4)` strand-of-parens syntax in the tokenizer/parser.
4. **Mixed arrays** — char+numeric ravels (mostly works already via Cell enum).
5. Later phases: defined functions (∇ editor), system commands (`)SAVE` etc.),
   boxed display (like ⎕CR), Rayon parallelism, libapl embedding.

Always cross-check the newest `rust-apl/META-INF/PROGRESS` file — it may be ahead of this list.

---

## 6. Useful commands

```sh
cd ~/Apps/apl-2.0/rust-apl

cargo build --release          # optimized build (~15 s)
cargo test                     # run all unit tests (< 1 s)
cargo clippy                   # lint (must be clean)
cargo fmt                      # format

# REPL smoke test:
printf '⍳5\n2×3+4\nM←2 3⍴⍳6\n+/M\n)OFF\n' | ./target/release/apl

# Look up C++ semantics:
grep -rn "Bif_F12_TAKE_DROP::eval_AB" ../src/Bif_F12_TAKE_DROP.cc
```

## 7. Known limitations / deferred items

- Reduce/scan operate on the **last axis only** (no axis specification `[X]`).
- Take/drop likewise last-axis only for rank ≥ 2.
- Dyadic transpose does not support merged/repeated axes (DOMAIN ERROR).
- Index-of uses linear search (C++ has a binary-search path for large A).
- Monadic commute (`F⍨B = B F B`) not implemented — only dyadic swap.
- No defined functions, no system commands beyond )OFF, single-threaded.
- Parser is recursive descent, not the full prefix machine; no lambdas/dfns.
