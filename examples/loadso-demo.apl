⍝ Plugin Loading Demo: ⎕LOADSO
⍝ Demonstrates loading a cdylib plugin and calling its functions.
⍝
⍝ Build the demo plugin first (from repo root):
⍝   cargo build -p demo-plugin
⍝
⍝ Then run:
⍝   cargo run --bin apl < examples/loadso-demo.apl
⍝
⍝ Or from the examples directory:
⍝   ../target/debug/apl < loadso-demo.apl

⎕IO←1

⍝ === Load the demo plugin ===
⍝ Registers: STRREV, SUMI, PANICME
⎕LOADSO 'target/debug/libdemo_plugin.so'

⍝ === STRREV: reverse a character vector ===
STRREV 'hello'
⍝ → 'olleh'

STRREV 'APL is fun'
⍝ → 'nuf si LPA'

⍝ === SUMI: sum an int vector ===
SUMI 1 2 3 4 5
⍝ → 15

SUMI 10 20 30
⍝ → 60

SUMI 100 200 300 400 500
⍝ → 1500

⍝ === Error handling: plugin panic is caught ===
⍝ PANICME deliberately panics — the panic is caught by catch_unwind
⍝ and converted to DOMAIN ERROR, not aborting the REPL.
PANICME 42
⍝ → DOMAIN ERROR (caught, REPL continues)

⍝ === After error, REPL still works ===
1 + 2
⍝ → 3

STRREV 'still works'
⍝ → 'skrow llits'
