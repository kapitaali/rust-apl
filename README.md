# rust-apl

An experimental rewrite of the [GNU APL](https://www.gnu.org/software/apl/) interpreter in Rust. A from-scratch reimplementation of the C++ interpreter that powers GNU APL 1.7+ (ISO/IEC 13751), following a phased migration of the original class hierarchy into idiomatic Rust.

Phase 1–9 of the migration are substantially complete: a working REPL with **564 tests passing**, clippy clean, release build verified. Unofficial extensions (⌸ Key, ⍥ Over) available via `--features unofficial-ext`.

## What works

```
$ ./target/release/apl
      2+3
5
      ⍳5
0  1  2  3  4
      1 2 3+.×10 20 30     ⍝ inner product
140
      M←2 3⍴⍳6
      M+.×5 6 7             ⍝ matrix × vector
20  74
      ∇R←FINDSTOP N         ⍝ defined function w/ :While + :Leave
      R←0
      I←1
      :While 1
      R←R+I
      :If I≥N
      :Leave
      :EndIf
      I←I+1
      :EndWhile
      ∇
      FINDSTOP 4
10
      )SAVE myws             ⍝ workspace persistence
      )LOAD myws
```

Supported primitives: `+ - × ÷ ⋆ ○ ! ⌈ ⌊ ∣ ⍳ ⍴ ↑ ↓ ⌽ ⍉ ⍋ ⍒ ∈ ⊂ ⊃ ≡ ≤ < = ≥ > ≠ → ⌹ ∧ ∨` (monadic and/or dyadic where meaningful), operators reduce `/`, scan `\`, each `¨`, outer product `∘.`, inner product `f.g`, commute `⍨`, axis-specified `F[n]`, defined functions `∇` with `:If/:Else/:While/:Repeat/:Until/:Leave` control structures and `→` branching, system commands `)VARS )FNS )CLEAR )SAVE )LOAD )OFF`, bracket indexing `B[i]` honoring `⎕IO`, and workspace persistence.

### Unofficial extensions (Dyalog-compatible)

Enable with `--features unofficial-ext`:

```sh
cargo build --release --features unofficial-ext
./target/release/apl
      ⌸1 2 1 3 2        ⍝ Key: group unique elements + indices
      1 ┏→━━┓
      2 ┏→┓
      3 ┏→┓
      (×⍥⌈) 3.7          ⍝ Over: f(g(B))
      1
      2 (+⍥÷) 4 6        ⍝ Dyadic Over: f(g(A), g(B))
      0.75  0.6666666667
```

| Glyph | Name | Not in GNU APL |
|---|---|---|
| `⌸` (U+2328) | Key | Dyalog APL |
| `⍥` (U+2365) | Over (compose) | Dyalog APL |

These primitives live in `src/key.rs` and `src/over.rs`, gated by `#[cfg(feature = "unofficial-ext")]` throughout `src/functions.rs`, `src/tokenizer.rs`, `src/parser.rs`, and `src/lib.rs`. The core crate is 100% GNU APL compatible when the feature is off.

## Building

```sh
cargo build --release    # optimized build
cargo test               # 200 unit + integration tests
cargo clippy             # lint (clean)
./target/release/apl     # start the REPL
```

Rust 1.70+ (edition 2021). Dependencies: `smallvec`, `rayon`.

## Architecture

The C++ class hierarchy becomes a set of focused Rust modules, one concern per file:

| Rust file | Mirrors (C++ in `src/`) | Notes |
|---|---|---|
| `types.rs` | `APL_types.hh`, `APL_enums.hh`, `Error.def` | typedefs, CellType bitflags, ErrorCode |
| `cell.rs` | `Cell*.hh/cc` | enum Cell {Char, Int, Float, Complex, Pointer, Lval}; all scalar bif_* ops; boolean ∧/∨; LCM/GCD generalization |
| `shape.rs` | `Shape.hh/cc` | rank/rho/volume; MAX_RANK=8 |
| `value.rs` | `Value.hh`, `Value_P.hh` | ValueP = Arc<ValueInner>; COW isolate(); nested/disclose() |
| `functions.rs` | `ScalarFunction.cc`, `Bif_F12_*` | Prim enum dispatch; monadic + dyadic eval; elementwise broadcast (Rayon-parallel above 4096 elements) |
| `tokenizer.rs` | `Tokenizer.cc` | numbers, names (∆⍙_), strings, prim glyphs, comments ⍝, `⋄` diamond, inner/outer product `f.g` / `∘.f` |
| `parser.rs` | `Parser.cc` + prefix machine | recursive descent, right-to-left eval, Environment (HashMap vars), structured control blocks |
| `main.rs` | `main.cc` | vector/matrix/boxed formatting; )OFF quits |
| `operators.rs` | `Bif_OPER1_REDUCE/SCAN` | right-to-left fold; empty-axis identity |
| `rotate.rs` | `Bif_ROTATE` | reverse + rotate, per-row and per-axis |
| `sort.rs` | `Bif_F12_SORT` | stable grade via sort_by; ⎕IO-aware |
| `take_drop.rs` | `Bif_F12_TAKE_DROP` | prototype padding, per-row/axis |
| `transpose.rs` | `Bif_F12_TRANSPOSE` | monadic + dyadic permutation |
| `index_of.rs` | `Bif_F12_INDEX_OF` | ⎕IO-shifted results |
| `epsilon.rs` | `Bif_EPSILON` | membership |
| `outer.rs` | `Bif_OPER2_OUTER` | A ∘.f B |
| `inner.rs` | `Bif_OPER2_INNER` | A f.g B |
| `domino.rs` | `Bif_F12_DOMINO` | ⌹ matrix inverse/divide (Gauss-Jordan) |
| `functions_def.rs` | — | ∇-defined functions: parse, control-block scan, source retention |
| `sysvars.rs` | — | ⎕IO, ⎕CT, ⎕PP; system commands |
| `workspace.rs` | — | )SAVE/)LOAD persistence |
| `boxdisplay.rs` | — | 4⎕CR-style boxed display of nested arrays |

### Design notes

- **Cell as enum**: the C++ class hierarchy (`CharCell`, `IntCell`, ...) becomes one Rust enum with exhaustive matching — no vtables.
- **Value_P as Arc with COW**: mirrors the C++ reference counting + `isolate()` semantics. Swapped `Rc`→`Arc` to enable Rayon parallelism.
- **Errors as Result**: every primitive returns `Result<_, ErrorCode>`, mirroring the C++ `ErrorCode` convention.
- **Indices are 0-based** in this port; `⎕IO` is honored at the primitive level where it matters (`⍳`, indexing, `⍳`, `⍋⍒`).

## Status

See `META-INF/PROGRESS-20260828.md` for the detailed session log. The rewrite is a working interpreter with 564 tests, 375/375 differential agreement with GNU APL, and a libapl C embedding API. Remaining future work: dfns `{}` (partial), full ⎕-system, and tokenizer spans for caret display.

## License

The GNU APL original is GPLv3. This rewrite is provided under the same license (see `COPYING` in the upstream `apl-2.0/` tree).
