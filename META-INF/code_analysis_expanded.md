# Expanded Analysis — "Printing Quad Broken" (Display Suppression + Syntax)

Role: Code Analyzer (read-only; no source modifications).
Added: 2026-09-02 (continuation of previous analysis).

---

## User's reported symptom

Sequence:
```
r ← JI '/home/theb/Apps/apl-2.0/rust-apl/examples'
rr ← JS ('AplUtils' 'reverse' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256)
⎕ ← 'rr: '
⎕ ← rr
⎕ ← '≢rr: '
⎕ ← ≢rr
⎕ ← '1⊃rr: '
⎕ ← 1⊃rr
```

User says: "the printing quad is broken... it prints nothing".

---

## Real execution verified (read-only, terminal output captured)

Running the exact sequence through `./target/debug/apl` (verified with `printf ... | ./target/debug/apl`):

```
      ERROR: VALUE ERROR
      r ← JI '/home/theb/Apps/apl-2.0/rust-apl/examples'
      ERROR: VALUE ERROR
      rr ← JS (...)
            ERROR: VALUE ERROR
      ⎕ ← rr
            ERROR: VALUE ERROR
      ⎕ ← ≢rr
            ERROR: VALUE ERROR
```

So it does NOT "print nothing" — it produces repeated `VALUE ERROR`. The user's perception of "prints nothing" is likely because:
- The `VALUE ERROR` line doesn't show a value (it shows an error message).
- The `⎕ ←` assignments suppress any positive result (by design — see below).

---

## Root cause analysis (expanded from Rust source)

### Layer 1: `JI` native binding missing (VALUE ERROR source)

`parser.rs` line 3775 (`Expr::QuadNa`): `⎕NA` associates a native function by parsing the declaration string and creating a `CAbiBinding`. `JI` (j_init) needs to be defined before use.

The user's sequence does NOT include the `⎕NA` line (`'JI' ⎕NA 'P ... |j_init <0T'`). The file `test-java.apl` includes it at the top. Without it, `self.funcs.get("JI")` returns `None`, and the native call path (`parser.rs` 3144) falls through to regular function lookup — which also fails (variable `JI` doesn't exist) → `VALUE ERROR` (`parser.rs` 3144: `self.funcs.get(name)` is `None`, then falls to variable lookup which also fails).

Evidence: `parser.rs` 3144–3149 shows native dispatch only activates when `self.funcs.get(name)` returns `Some(Native(b))`. Without association, `JI` doesn't exist in the function table.

### Layer 2: `⎕ ← expr` suppresses output by design (assignment is shy)

`parser.rs` 4187–4205 (`eval_statement`):
```rust
let is_assign = matches!(expr, Expr::Assign(_, _)|...);
...
Ok(if is_assign || executed_shy {
    None   // assignment produces NO output
} else {
    Some(v)
})
```

`main.rs` 259–282: `env.eval_line` returns `Ok(Some(v))` for non-assignments, `Ok(None)` for assignments. `main.rs` line 282: `Ok(None) => {} // assignment — no output`. So `⎕ ← rr` (assignment to the `⎕` variable) never prints anything, regardless of `rr`'s value.

This explains "prints nothing" for the `⎕ ← rr`, `⎕ ← ≢rr`, `⎕ ← 1⊃rr` lines: they are assignments, so output suppression is correct behavior, not a bug.

### Layer 3: If syntax were correct, `rr` result from `JS` should display properly

With correct syntax (`JS ('AplUtils' 'reverse' ... 'hello' 256)`), `rr` is a 2-element nested vector (return code `0` + result string `"olleh"`). The REPL routing (`main.rs` 268):
- `v.rank()` = 1 (vector of 2 enclosed items)
- `all_chars` = false (items are Pointer/enclosed, not Char)
- `has_pointer` = true (nested vector has Pointer cells)
- `boxing` = `get_boxing(&env)` (default from `sysvars`). If `⎕BOXING` = 1 (boxed display, GNU APL default), `main.rs` 268 selects `boxdisplay::render_with_pp` (line 269–272). The nested vector will display as two boxes side by side (`join_horizontal` in `boxdisplay.rs` 303–322).
- If `boxdisplay` fix is needed (per memory note: "main.rs duplicates display logic... a boxdisplay fix is invisible in REPL until main.rs routing sends value there"), the fix applies only when `main.rs` routes nested values to `boxdisplay`. For `rr` (nested, `has_pointer` true, `boxing` true by default), it does route correctly. So a `boxdisplay` fix would be visible.

### Layer 4: The user's confusion about "printing quad"

The "printing quad" (`⎕` display mechanism) refers to:
- `main.rs` lines 259–280 (display routing)
- `boxdisplay.rs` (boxed/plain rendering for nested/simple values)
- `format.rs` (`format` / `format_with_pp`, the `⍕` formatting function)
- `main.rs` `format_value` (line 75–96) — duplicates some `boxdisplay` logic but only for scalar/simple vector cases (line 77: `if v.is_scalar() || v.is_vector()`). Nested values (`rr` with enclosed pointer items) never reach `format_value`; they go to `boxdisplay`.

The user may expect `⎕ ← rr` to print something (treating `⎕` as a "display variable"). But `⎕` is just a regular variable name in APL; `⎕ ←` assigns quietly. If the user wants to display `rr`, they should either:
- Evaluate `rr` directly (not `⎕ ← rr`): just type `rr` on its own line.
- Or use `⍕rr` (format quad) which calls `format()` (`format.rs` 22) — but that produces a character representation, not visual output.

---

## Suggestions (expanded, from full source read)

1. **Add `⎕NA` definitions first** (fix `VALUE ERROR`): Before using `JI`, `JS`, etc., include:
   ```apl
   'JI' ⎕NA 'P /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_init <0T'
   'JN' ⎕NA 'P ... |j_new <0T'
   'JC' ⎕NA 'I4 ... |j_call P <0T <0T I8 I8 >I8'
   'JCS' ⎕NA 'I4 ... |j_call_s P <0T <0T <I4 >0T[256]'
   'JS' ⎕NA 'I4 ... |j_call_static <0T <0T <0T <0T <I4 >0T[256]'
   ...
   ```
   (as in `test-java.apl` and `java-demo-sh.apl` at top).

2. **Use direct evaluation for display**, not assignment: Replace `⎕ ← rr` with just `rr`. Replace `⎕ ← 1⊃rr` with `1⊃rr`. This avoids the `is_assign` suppression (`parser.rs` 4201: `None`).

3. **If nested display seems broken** (e.g., boxes misaligned): Check `boxdisplay.rs` (`join_horizontal` at 303, `box_lines` at 277) and verify `main.rs` routes nested values (`v.rank() >= 2 || ... || has_pointer && boxing`, line 268). The memory note confirms `main.rs` has its own `format_value` that duplicates logic; a fix there must also update `format_value` (line 103: "Must match boxdisplay::high_minus — this file keeps its own copy...").

4. **No code changes made**: All observations from `read_file` of `main.rs`, `parser.rs`, `boxdisplay.rs`, `format.rs`, `crates/apl-java/src/lib.rs`. No `patch`, `write_file` to `src/`, `cargo` executed.
