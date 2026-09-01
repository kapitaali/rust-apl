a/PROMPT.md → b/PROMPT.md
@@ -0,0 +1,157 @@
+# PROMPT.md — Continuation Guide for the GNU APL → Rust Rewrite
+
+> **You are an agent picking up this project cold.** This file tells you
+> everything you need to continue the work. Read it fully, then follow the
+> Workflow section below before writing any code.
+
+---
+
+## 1. What this project is
+
+`/home/theb/Apps/apl-2.0/` contains **GNU APL 2.0**, the original C++ interpreter
+for ISO/IEC 13751 ("Programming Language APL, Extended", © Jürgen Sauermann,
+GPLv3). It is a **reference implementation — never modify the C++ sources**.
+
+Inside it, `apl-rust/` is an in-progress **Rust rewrite** of that interpreter.
+The goal: a Rust implementation with identical APL semantics, following the
+phased migration plan in `REWRITE_STRATEGY.md`.
+
+### Key documents (read in this order)
+
+| File | Contents |
+|---|---|
+| `PROGRESS-YYYYMMDD.md` | **Start here.** Session-by-session log. Find the newest one (by date in filename) and read it — especially its "Next steps" list, which is your task queue. |
+| `REWRITE_STRATEGY.md` | Why Rust was chosen over C, architecture mapping (C++ class → Rust construct), and the 10-phase migration plan (§7) with verification gates. |
+| `API.md` | Comprehensive reference of the C++ source: 24 sections covering all 403 classes, enums, typedefs, functions. Use it to look up semantics of anything you're porting. |
+| `.api-docs/parts/*.md` | 18 deep-dive documents on specific C++ subsystems (cells, values, symbols, quads, parser, emacs mode, etc.). |
+| `apl-rust/README.md` | Status table of the Rust port: which module mirrors which C++ file. |
+
+---
+
+## 2. Current state (as of 2026-08-22)
+
+**The Rust port is a working REPL with 86 passing tests**, supporting:
+arithmetic (`+ - × ÷ ! ⌈ ⌊ ⋆ ○ ∣`), structural functions (`⍳ ⍴ ↑ ↓ ⌽ ⍉`),
+search/sort (`⍋ ⍒ ∈`, dyadic `⍳`, bracket indexing `B[i]`),
+operators (reduce `/`, scan `\`, commute `F⍨`),
+assignment (`X←...`) and selective assignment (`B[i]←x`),
+right-to-left evaluation, scalar extension, strands (`2 3⍴...`),
+and nested-array infrastructure (`Rc`-based PointerCell, enclose/disclose helpers).
+
+Zero clippy warnings. Release build verified.
+
+### Module map (`apl-rust/src/`)
+
+| Rust file | Mirrors (C++ in `src/`) | Notes |
+|---|---|---|
+| `types.rs` | `APL_types.hh`, `APL_enums.hh`, `Error.def` | typedefs, CellType bitflags, ErrorCode |
+| `cell.rs` | `Cell*.hh/cc` | enum Cell {Char, Int, Float, Complex, Pointer, Lval}; all scalar bif_* ops; Lanczos tgamma; tolerant equality (⎕CT=1e-13); ordering Pointer > Numeric > Char |
+| `shape.rs` | `Shape.hh/cc` | rank/rho/volume; MAX_RANK=8 |
+| `value.rs` | `Value.hh`, `Value_P.hh` | ValueP = Rc<ValueInner>; COW isolate(); nested()/disclose() |
+| `functions.rs` | `ScalarFunction.cc`, `Bif_F12_*` | Prim enum dispatch; monadic + dyadic eval; elementwise broadcast; reshape |
+| `tokenizer.rs` | `Tokenizer.cc` | numbers (¯, exponents), names (∆⍙_), strings ('' escape), prim glyphs, comments ⍝ |
+| `parser.rs` | `Parser.cc` + prefix machine (simplified) | recursive descent, right-to-left eval, Environment (HashMap vars), tests lock APL semantics |
+| `main.rs` | `main.cc` (minimal REPL) | vector/matrix formatting; )OFF quits |
+| `operators.rs` | `Bif_OPER1_REDUCE/SCAN` | right-to-left fold; empty-axis identity |
+| `rotate.rs` | `Bif_ROTATE` | reverse + rotate, per-row shifts |
+| `sort.rs` | `Bif_F12_SORT` | stable grade via sort_by (Heapsort not needed) |
+| `take_drop.rs` | `Bif_F12_TAKE_DROP` | prototype padding (0 / space), per-row on matrices |
+| `transpose.rs` | `Bif_F12_TRANSPOSE` | monadic (swap axes 0/1) + dyadic (full permutation) |
+| `index_of.rs` | `Bif_F12_INDEX_OF` | first occurrence; not-found = len(A) |
+| `epsilon.rs` | `Bif_EPSILON` | membership |
+
+---
+
+## 3. Non-negotiable conventions
+
+1. **Never touch the C++ sources.** All work happens in `apl-rust/`.
+2. **Check today's date** (`date '+%Y%m%d'`). At session start, read the newest
+   `PROGRESS-YYYYMMDD.md`. When you finish work (or reach a milestone), append
+   to today's progress file (create it if missing) using the established format:
+   numbered entries under "Session history", updated "Final state" line, refreshed
+   "Next steps" list.
+3. **Before claiming done:** `cargo fmt && cargo test && cargo clippy` — all
+   tests pass, zero warnings. Verify REPL behavior by piping commands:
+   `printf '2+3\n)OFF\n' | ./target/release/apl`.
+4. **Max 2 subagents** if you delegate at all (the user's API key rate-limits);
+   prefer doing work directly in-session.
+5. **Semantics-first:** when porting, read the corresponding C++ source (grep
… omitted 79 diff line(s) across 1 additional file(s)/section(s)
