# rust-apl

A high-performance, memory-safe rewrite of the [GNU APL](https://www.gnu.org/software/apl/) interpreter in Rust. Built from the ground up to match GNU APL 1.7+ (ISO/IEC 13751) while adding modern features like a GTK4 GUI, parallel computation, and a plugin system.

**746 tests passing. 375/375 differential agreement with GNU APL.**

## Features

### Core Language
- **Full APL primitive set**: `+ - × ÷ ⋆ ○ ! ⌈ ⌊ ∣ ⍳ ⍴ ↑ ↓ ⌽ ⍉ ⍋ ⍒ ∈ ⊂ ⊃ ≡ ≤ < = ≥ > ≠ → ⌹ ∧ ∨` (monadic and dyadic)
- **Operators**: reduce `/`, scan `\`, each `¨`, outer product `∘.`, inner product `f.g`, commute `⍨`, rank `⍤`, power `⍣`
- **Defined functions**: `∇` with full control structures — `:If/:Else/:While/:Repeat/:Until/:Leave` and `→` branching
- **Inline dfns**: `{...}` with guards (`{c:e ⋄ f}`), self-reference (`∇`), and nested control blocks
- **Quad system**: `⎕IO ⎕CT ⎕PP ⎕CR ⎕UCS ⎕AV ⎕TS ⎕WA ⎕TC ⎕DM ⎕EN ⎕RVAL ⎕RL ⎕CC ⎕DLX ⎕TF ⎕FX ⎕MAP ⎕MX ⎕FIO ⎕JSON ⎕XML ⎕RE ⎕NS ⎕CS ⎕CDR ⎕APLOT`
- **System commands**: `)VARS )FNS )CLEAR )SAVE )LOAD )OFF`
- **Workspace persistence**: `)SAVE` and `)LOAD` for full workspace state

### Language demo: APL Calculator in GTK4
A graphical calculator with buttons, entry field, and scrollable results display. Write APL expressions with full control-flow logic for interactive computation.

```sh
cargo build --features plugin-gtk
./target/release/apl < examples/calc-demo.apl
```

### Quad Extensions
- **⎕PLOT** — plotting via the `plotters` crate
- **⎕PNG** — PNG image I/O
- **⎕SQL** — SQLite database queries
- **⎕FFT** — Fast Fourier Transform
- **⎕PYTHON** — Python shell-out
- **⎕GTK** — GTK4 GUI window

### Unofficial Extensions (Dyalog-compatible)
Enable with `--features unofficial-ext`:
- `⌸` Key — group unique elements
- `⍥` Over — function composition

### Performance
- **Parallel operations**: Rayon-powered parallelism for large arrays (4000+ elements)
- **Zero-copy**: `Arc<ValueInner>` with copy-on-write semantics
- **Release-optimized**: `cargo build --release` for production use

## Quick Start

To build the system, use cargo:

```sh
cargo build --release
./target/release/apl
```
Because rust-apl reads input from standard input stream, to give an .apl source file as argument, you have to add `<`:

```sh
./target/release/apl <my-apl.file.apl
```
You can also pipe stuff (-q skips the REPL greeting):
```sh
echo "{ →(⍵≤0)/0 ⋄ ⍵×2 } 5" | ./target/release/apl -q
```
Demo session:
```
GNU APL 2.0 (Rust) — experimental REPL
Enter APL expressions, or )OFF to exit.
      2+3
5
      ⍳5
0  1  2  3  4
      1 2 3+.×10 20 30
140
      ∇R←FACT N
        :If N≤0
          R←1
          →0
        :EndIf
        R←N×FACT N-1
      ∇
      FACT 5
120
```

## Documentation

For installation, configuration, and usage details, see the [docs/](docs/) directory:

- [Setup](docs/setup.md) — building, dependencies, configuration
- [Usage](docs/usage.md) — REPL, scripts, workspace, system commands
- [Embedding](docs/embedding.md) — using the `apl` crate in other Rust projects
- [FFI Reference](docs/ffi.md) — extending APL with C, Java, and Rust

## Project Status

See [META-INF/PROGRESS-20260901.md](META-INF/PROGRESS-20260901.md) for the detailed session log.

## License

GPLv3 (same as GNU APL). See `COPYING` in the upstream `apl-2.0/` tree.
