# Setting up rust-apl

This document describes how to build, configure, and run the Rust APL interpreter.

## Prerequisites

- **Rust 1.70+** (edition 2021)
- **Cargo** package manager
- **Linux** (primary target; tested on Debian 13)

### System Dependencies

Install these with `apt-get` before building:

```sh
sudo apt-get install -y \
  libfontconfig1 \
  libfreetype6 \
  libexpat1 \
  zlib1g \
  libbz2-1.0 \
  libpng16-16 \
  libbrotli1
```

For the GTK calculator (`--features plugin-gtk`):

```sh
sudo apt-get install -y \
  libgtk-4-dev \
  libglib2.0-dev \
  libpango1.0-dev
```

For SQL support (`--features plugin-sql`):

```sh
sudo apt-get install -y libsqlite3-dev
```

### Optional dependencies for plugins:

| Feature | Depends on | Install |
|---|---|---|
| `plugin-plot` (⎕PLOT) | `plotters` crate | built-in |
| `plugin-png` (⎕PNG) | `image` crate | built-in |
| `plugin-fft` (⎕FFT) | `rustfft` crate | built-in |
| `plugin-sql` (⎕SQL) | `rusqlite` crate | built-in, needs `libsqlite3-dev` |
| `plugin-python` (⎕PYTHON) | `pyo3` | not yet integrated |
| `plugin-gtk` (⎕GTK) | `gtk4-rs` | needs GTK 4 dev libraries |
| `plugin-cdr` (⎕CDR) | — | built-in |
| `unofficial-ext` | — | Dyalog extensions |

## Building

### Standard build

```sh
cd rust-apl
cargo build --release
```

The binary is `target/release/apl`.

### Enable plugins

Although rust-apl executable is around 4 megabytes (contrast that with GNU APL executable of 10x size, 47 megabytes), you don't have to include support for everything in the build. The decision to include plugin support is made in `config.toml`.

Edit `config.toml` to set plugin states (`static` includes, `disabled` excludes):

```toml
[plugins.plugin_states]
plot = "static"
png = "static"
sql = "static"
fft = "static"
python = "disabled"
gtk = "disabled"
cdr = "disabled"
```

Or build with features directly:

```sh
cargo build --release --features "plugin-plot,plugin-png,plugin-sql,plugin-fft"
```

### Development build

```sh
cargo build           # debug build
cargo clippy          # lint check
cargo test            # run tests
```

## Running

### Interactive REPL

```sh
./target/release/apl
```

```
GNU APL 2.0 (Rust) — experimental REPL
Enter APL expressions, or )OFF to exit.
      2+3
5
      )OFF
```

### Pipe a script

```sh
echo '2+3' | ./target/release/apl -q
echo '⍳5' | ./target/release/apl -q
```

### Run a script file

```sh
./target/release/apl < script.apl
```

## Configuration

### config.toml

Compile-time plugin selection (read by `build.rs`):

```toml
[plugins.plugin_states]
plot = "static"    # or "dynamic" or "disabled"

[plugins.plot]
backend = "plotters"
width = 800
height = 600

[plugins.sql]
backend = "sqlite"
connection_string = "apl.db"
```

States:
- **`static`** — compiled into binary
- **`dynamic`** — loaded at runtime via `⎕LOADSO`
- **`disabled`** — not included

### System variables

| Variable | Default | Meaning |
|---|---|---|
| `⎕IO` | 0 | Index origin (0-based) |
| `⎕CT` | 1e-13 | Comparison tolerance |
| `⎕PP` | 10 | Print precision |
| `⎕SEC` | 0 | Security level (0=normal, 1=restricted, 2=locked) |
| `⎕BOXING` | 1 | Nested array display (1=boxed, 0=plain) |

## Verification

```sh
cargo test --release    # run all 730+ tests
```

Expected output:
```
test result: ok. 730 passed; 0 failed; 0 ignored
```

## Troubleshooting

### Build fails with "Nix socket" warning

This is a harmless warning from the Nix build daemon. The build actually succeeds. Ignore it.

### SQLite plugin fails to build

Install development headers:
```sh
sudo apt install libsqlite3-dev
```

### Plotters plugin fails

Install Cairo:
```sh
sudo apt install libcairo2-dev
```

### Release build is slow

Use `cargo build` for development. Release enables LTO and takes several minutes.

## Next steps

- [Using the interpreter](usage.md)
- [FFI reference](ffi.md)
- [META-INF/ROADMAP.md](../META-INF/ROADMAP.md)
