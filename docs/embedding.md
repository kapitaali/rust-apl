# Embedding rust-apl in Other Rust Projects

This document explains how to use the `apl` crate as a library in your own Rust projects. You can evaluate APL expressions programmatically, manipulate APL values from Rust, and extend the interpreter with custom functionality.

## Adding as a Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
apl = { git = "https://github.com/kapitaali/rust-apl.git" }
```

Or if you have a local clone:

```toml
[dependencies]
apl = { path = "/home/theb/Apps/apl-2.0/rust-apl" }
```

## Basic Usage

```rust
use apl::parser::Environment;
use apl::sysvars;

fn main() {
    // Create a new APL environment
    let mut env = Environment::new();
    
    // Initialize system variables (⎕IO, ⎕CT, ⎕PP, etc.)
    sysvars::init_sysvars(&mut env);
    
    // Evaluate an APL expression
    let result = env.eval_line("2 + 3").unwrap();
    println!("{:?}", result);  // Some(ValueP { ... })
    
    // Evaluate and unwrap the value
    let value = env.eval_line("⍳5").unwrap().unwrap();
    println!("{:?}", value);   // 0 1 2 3 4 (with ⎕IO←0)
}
```

## Working with Variables

```rust
// Set a variable from Rust
let val = env.eval_line("5 6 7").unwrap().unwrap();
env.set("MYVEC", val);

// Read a variable from Rust
let myvec = env.get("MYVEC").unwrap();
println!("{:?}", myvec);

// Use it in subsequent expressions
let result = env.eval_line("MYVEC + 10").unwrap().unwrap();
println!("{:?}", result);  // 15 16 17
```

## Working with APL Values

The `ValueP` type represents an APL value. It's an `Arc<ValueInner>` with copy-on-write semantics.

```rust
use apl::value::ValueP;
use apl::cell::Cell;

// Create a scalar integer
let scalar = ValueP::scalar_from(Cell::Int(42));

// Create a vector
let vec = ValueP::int_vector(&[1, 2, 3, 4, 5]);

// Create a matrix (2×3)
let matrix = ValueP::from_parts(
    apl::shape::Shape::matrix(2, 3),
    vec![Cell::Int(1), Cell::Int(2), Cell::Int(3),
         Cell::Int(4), Cell::Int(5), Cell::Int(6)],
).unwrap();

// Access cells
for cell in vec.cells() {
    match cell {
        Cell::Int(i) => println!("int: {}", i),
        Cell::Float(f) => println!("float: {}", f),
        Cell::Char(c) => println!("char: {}", char::from_u32(*c).unwrap()),
        _ => {}
    }
}
```

## Error Handling

```rust
match env.eval_line("1 ÷ 0") {
    Ok(Some(value)) => println!("Result: {:?}", value),
    Ok(None) => println!("No result (assignment or shy value)"),
    Err(e) => println!("Error: {:?}", e),
}
```

## Feature Flags

The crate supports feature flags for optional functionality:

```toml
[dependencies]
apl = { path = "...", features = ["unofficial-ext"] }  # ⌸ Key, ⍥ Over
```

Note: The `plugin-gtk` feature requires GTK 4 development libraries and is typically only needed for the standalone REPL.

## Advanced: Direct Expression Evaluation

If you've already parsed an expression, you can evaluate it directly:

```rust
use apl::parser::{Environment, Expr, parse};
use apl::tokenizer::tokenize;

let mut env = Environment::new();
sysvars::init_sysvars(&mut env);

// Tokenize and parse manually
let toks = tokenize("2 + 3").unwrap();
let (expr, _used) = parse(&toks).unwrap();

// Evaluate the parsed expression
let result = env.eval(&expr).unwrap();
```

## Thread Safety

`Environment` is not `Send` (it contains `Rc`-like references and thread-local state). For parallel work:

- Create one `Environment` per thread, or
- Use `eval_line` sequentially from a single thread

The underlying `ValueP` is `Send + Sync` (it uses `Arc`), so you can share values between threads after extracting them.

## Limitations

- The public API is not yet stabilized; expect breaking changes
- System variables must be initialized via `sysvars::init_sysvars()` for full functionality
- Some internal modules are `pub` but not intended for direct use
- The `parser::Environment` is the primary entry point; other modules are implementation details

## See Also

- [Setup](setup.md) — building and dependencies
- [Usage](usage.md) — APL language reference
- [FFI Reference](ffi.md) — extending APL with native code
