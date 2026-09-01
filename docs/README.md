# rust-apl Documentation

This directory documents how to build, use, and extend the Rust implementation of GNU APL 2.0.

---

## Table of Contents

1. [Setting up the system](setup.md)
2. [Using the interpreter](usage.md)
3. [Embedding in other Rust projects](embedding.md) — using the `apl` crate as a library
4. [FFI reference](ffi.md) — extending APL with native C, Java, and Rust code

---

## Project layout

```
rust-apl/
├── src/                  # Main interpreter
│   ├── main.rs           # REPL entry point
│   ├── parser.rs         # Parser + evaluator (Environment)
│   ├── quad.rs           # ⎕ system functions
│   ├── functions_def.rs  # ∇ defined functions
│   ├── sysvars.rs        # System variables + system commands
│   ├── ffi/              # Native function interface
│   │   ├── cabi.rs       # C ABI call driver
│   │   ├── nadecl.rs     # ⎕NA declaration parser
│   │   ├── loader.rs     # LibraryCache + dlopen
│   │   └── plugin.rs     # ⎕LOADSO cdylib loader
│   ├── plugins/          # Phase 6 ext-plugins
│   ├── workspace.rs      # )SAVE/)LOAD workspace format
│   └── ...
├── crates/
│   ├── apl-ext/          # Extension crate template
│   ├── demo-plugin/      # Example cdylib plugin
│   └── apl-java/         # JNI bridge (Java FFI)
├── docs/                 # This documentation
├── META-INF/
│   ├── ROADMAP.md        # Implementation plan
│   └── PROGRESS-*.md     # Session logs
├── config.toml           # Plugin configuration
├── build.rs              # Compile-time feature selection
└── Cargo.toml
```

## Quick start

```sh
cargo build --release        # build the interpreter
cargo test                   # run all 730+ tests
./target/release/apl         # launch the REPL
```

## Status

- **Phases 1–8 complete** (Quad functions, operators, selective assignment, ⎕NA, workspace commands, plugin system, performance, GNU APL XML)
- **730+ tests passing**, release build verified
- [Unofficial Dyalog extensions](usage.md#unofficial-extensions-dyalog-compatible) (`⌸`, `⍥`) via `--features unofficial-ext`

For the complete implementation plan and status, see `META-INF/ROADMAP.md`.
