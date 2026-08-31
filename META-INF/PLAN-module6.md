# Module 6: Bidirectional Python Communication — Full Implementation Plan

> **Goal:** Implement ALL remaining features so that `calc-demo.apl` runs as an interactive GTK calculator.

---

## Current Blocker Analysis

Running `./target/release/apl < examples/calc-demo.apl` revealed:

| Feature | Status | Error |
|---------|--------|-------|
| `∇name` dfn header | Partial | `run_calc defined` then fails |
| `→condition/label` branch | Unimplemented | `VALUE ERROR` |
| `⍎` execute | Unimplemented | `VALUE ERROR` |
| String concat `(⍕x),' → ',⍕y` | Broken | `VALUE ERROR` |
| Recursive dfn | Unknown | Never reached due to above |

---

## Implementation Plan

### Phase 1: `⍎` Execute (1-2 hours)

**Location:** `src/parser.rs` line ~3348

**What:** `⍎B` evaluates a character vector as APL code.

**Steps:**
1. Add `eval_monadic` case for `Prim::Format` that already exists (line 3344-3347)
2. Add `Prim::Execute` branch:
   ```rust
   if *p == crate::functions::Prim::Execute {
       let code = if b.is_string() || b.is_vector() {
           // Convert ValueP to string
           let s = value_to_string(&b)?;
           // Tokenize, parse, eval
           let tokens = crate::tokenizer::tokenize(&s)?;
           let (ast, _) = crate::parser::parse(&tokens)?;
           return self.eval(&ast);
       } else {
           return Err(ErrorCode::DomainError);
       };
   }
   ```
3. Add `Prim::Execute` variant to `src/functions.rs` if not present
4. Token map: `"⍎" => Prim::Execute` in tokenizer
5. Tests:
   - `⍎'6 × 7'` → `42`
   - `⍎'2 + 3'` → `5`
   - `⍎(⍕2+3)` → `5` (number input)
   - `⍎'⍳5'` → `0 1 2 3 4`
   - `⍎'foo'` → `VALUE ERROR`

**Files:** `src/functions.rs`, `src/tokenizer.rs`, `src/parser.rs`

---

### Phase 2: Branch Arrow `→condition/label` (2-3 hours)

**Current state:** Line 1416-1418 parses `→expr` as `Monadic(Branch, expr)`. Line 3312-3321 evaluates and pushes onto `branch_stack`. But `run_lines` (line 2326) never reads `branch_stack` for jumps.

**What's missing:**
- `run_lines` must check `branch_stack` after each statement and jump to the target line
- `→label` where `label` is a number: jump to that line number
- `→(condition)/label`: conditional jump
- `0` = exit function, `N` = jump to line N

**Steps:**
1. Modify `run_lines` loop (line 2333-2415):
   ```rust
   // After each statement, check for branch
   if let Some(target) = self.consume_branch()? {
       match target {
           0 => return Ok(()),  // exit function
           n if n > 0 && (n as usize) < to => {
               pc = n as usize;
               continue;
           }
           _ => return Err(ErrorCode::RangeError),
       }
   }
   ```
2. Add `consume_branch()` method:
   ```rust
   fn consume_branch(&mut self) -> AplResult<Option<i64>> {
       if let Some(t) = self.branch_stack.last() {
           if *t != LEAVE_SENTINEL {
               return Ok(self.branch_stack.pop().flatten());
           }
       }
       Ok(None)
   }
   ```
3. Handle `→condition/label` syntax: parse `→(expr)` as branch with condition
4. Tests:
   - `∇r ← f x →(x=0)/0 ⋄ r ← x × f x-1 ⋄ 0` (factorial)
   - `∇r ← f x →(x=1)/done ⋄ r ← x ⋄ done: r+1`
   - `→5` (jump to line 5)
   - `→0` (exit function)

**Files:** `src/parser.rs`

---

### Phase 3: Recursive Dfns (1-2 hours)

**Current state:** Dfns are installed into `self.funcs` via `install_dfn()`. The body is stored as `Vec<Expr>`. When the body references its own name, it should resolve via `call_function`.

**Issue:** The dfn body may not have access to its own name during evaluation.

**Steps:**
1. Verify `install_dfn` stores the dfn with its name in `self.funcs`
2. When evaluating `Expr::FuncCallMono(name, arg)` where `name` is the dfn's own name, it should call itself
3. Add self-reference test:
   ```apl
   ∇r ← fact n
   →(n=0)/base
   r ← n × fact n-1
   →done
   base:
   r ← 1
   done:
   ```
4. Ensure `call_function` handles recursive calls without stack overflow (Rust stack is fine for reasonable depth)

**Files:** `src/parser.rs` (verify existing logic)

---

### Phase 4: String Concatenation Fix (1 hour)

**Issue:** `(⍕input),' → ',⍕result` fails because:
- `⍕input` returns a character vector
- `' → '` is a character vector
- `,` (catenate) on character vectors should produce a longer character vector
- But mixing with `⍕result` (also char vector) may create nested arrays

**Steps:**
1. Test: `(⍕2+3),' → ',⍕2+3` → should give char vector `'5 → 5'`
2. If `,` on char vectors creates nested array, fix `comma.rs` to flatten char vectors
3. Alternative: use `⊃` (pick) or explicit string building
4. Tests:
   - `(⍕5),' → ',⍕5` → `'5 → 5'`
   - `'hello',' ','world'` → `'hello world'`

**Files:** `src/comma.rs`, `src/functions.rs`

---

### Phase 5: Update calc-demo.apl (30 min)

Once all features work, update the demo to use them:

```apl
⍝ Interactive calculator with branching and execute
∇run_calc
  ⎕GTK 'text Enter expression (or "quit"):'
  input ← ⎕
  →(input≡'quit')/done
  result ← ⍎input
  ⎕GTK 'text (⍕input),'' → '',⍕result'
  run_calc
∇

done:
  ⎕GTK 'text Goodbye!'
  ⎕GTK 'close'
```

---

## Implementation Order

```
Phase 1: ⍎ Execute         (standalone, testable)
    ↓
Phase 2: → Branch          (enables control flow)
    ↓
Phase 3: Recursive Dfns    (verify with factorial)
    ↓
Phase 4: String Concat     (fix display)
    ↓
Phase 5: Update Demo       (integration test)
```

---

## Test Plan

| Test | Expected |
|------|----------|
| `⍎'6 × 7'` | `42` |
| `⍎'2+3'` | `5` |
| `⍎'⍳5'` | `0 1 2 3 4` |
| `⍎'foo'` | `VALUE ERROR` |
| `∇r ← f x →(x=0)/0 ⋄ r ← x × f x-1 ⋄ 0` applied to `f 5` | `120` |
| `→5` in a 10-line function | jumps to line 5 |
| `→0` in a function | exits function |
| `(⍕5),' → ',⍕5` | `'5 → 5'` |
| `calc-demo.apl` runs interactively | GTK window + REPL loop |

---

## Files to Modify

- `src/functions.rs` — add `Prim::Execute`
- `src/tokenizer.rs` — add `"⍎" => Prim::Execute`
- `src/parser.rs` — implement execute, branch jump, recursive dfn support
- `src/comma.rs` — fix char vector concatenation
- `examples/calc-demo.apl` — update to use new features

---

## Estimated Effort

| Phase | Time |
|-------|------|
| 1. Execute | 1-2 hrs |
| 2. Branch | 2-3 hrs |
| 3. Recursion | 1-2 hrs |
| 4. String concat | 1 hr |
| 5. Demo update | 30 min |
| **Total** | **5-9 hrs** |
