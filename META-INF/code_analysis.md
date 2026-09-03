# Code Analysis — java-demo-sh.apl Error Investigation

Role: Code Analyzer (read-only; no source modifications made).
Date: 2026-09-02 (baseline from META-INF files read 2026-09-02).
Target: ~/Apps/apl-2.0/rust-apl/examples/java-demo-sh.apl

---

## 1. What was read (evidence sources)

| Source | Path | Key content used |
|---|---|---|
| File under analysis | `examples/java-demo-sh.apl` | Shell-based Java FFI demo; uses `JS` (j_call_static) with `2⊃r` extraction |
| Progress log | `META-INF/PROGRESS-20260902.md` | Session 107 (746 tests, 375 diff); Java FFI status block; "Next steps" for Java FFI |
| Plan | `META-INF/PLAN-libapl.md` | FFI architecture; `libapl.h` sections; `src/ffi/libapl.rs` target |
| FFI driver | `src/ffi/cabi.rs` | `CAbiBinding`, `Direction::Out`, `owned` buffer logic, result-vector assembly |
| Deprecated alt | `examples/java-demo.apl` | GTK-based version; passes `buf` explicitly; does not use `2⊃r` |
| Shared lib check | `target/debug/libapl_java.so` | Exists (14.6MB); JNI bridge loads |

---

## 2. Error symptom

`java-demo-sh.apl` fails at every `2⊃r` call following a `JS` invocation.

Example line (from file):
```
r ← JS 'AplUtils' 'reverse' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256
⎕ ← 'Reverse "hello": ',2⊃r
```

Expected `r` to contain `(return_code, output_buffer)` (2 elements).
Actual `r` contains 1 element (return code only). `2⊃r` → INDEX ERROR.

---

## 3. Root-cause analysis (read from evidence)

### 3.1 PROGRESS-20260902.md states directly
- `j_init` succeeds (returns 1).
- `j_call_static` returns return code 1 (success) but **output buffer not captured in result vector**.
- Issue line: `2⊃r` gives INDEX ERROR because result vector has only 1 element (return code), not 2 (return code + output buffer).
- Next-steps note: "May need to fix cabi `owned` buffer handling for `>0T[n]` args. Or simplify: use `j_call_s` (String instance method) instead."

### 3.2 cabi.rs inspection (read only; lines 1–~160)
- `CAbiBinding::call` filters args by `Direction::Out` (`spec.args.iter().filter(...)`).
- `Direction::Out` args are recognized but the visible logic in the read segment does not show them being appended to the result vector after the native call.
- Pointer (`P`) args and `>0T[256]` buffer args are part of the spec; the `exploded` args logic handles nested/flat vectors but the return-path assembly for output buffers is not visible in the read portion.
- Conclusion from available text: the missing step is likely in result-vector assembly, not in argument desugaring.

### 3.3 java-demo.apl (deprecated GTK version) comparison
- Uses `JS ... 256 buf` (passes `buf` as explicit output argument).
- Does not try `2⊃r`; reads from `buf` directly.
- This confirms the design intent: output should go into a separate buffer variable, not into the result vector's second element.

### 3.4 libapl_java.so / JNI bridge
- File present (14.6MB, 11:16 build).
- `j_init` works; `j_call_static` returns code 1.
- No evidence that JNI fails to write to the output pointer; failure is in how the Rust cabi consumes/assembles the result.

---

## 4. Why java-demo-sh.apl specifically errors

- It relies on `j_call_static` (`JS`) with `>0T[256]` output spec (`>0T[256]`).
- It expects the result vector to contain both return code and filled buffer (`2⊃r`).
- The cabi either (a) does not include the `>...` buffer in the result, or (b) the buffer is not allocated/passed correctly through the `Direction::Out` path.
- `java-demo.apl` avoids the problem by using an explicit `buf` variable (no `2⊃r` dependency). `java-demo-sh.apl` does not use `buf`, so it hits the missing-output-vector path every call.

---

## 5. Suggestions (analysis, not implemented)

Based only on read evidence (no code changed):

1. **Cabi result-vector assembly (`src/ffi/cabi.rs`)**: Inspect how `Direction::Out` pointer args (`P`, `>0T[...]`) are added to the result after the native call. If missing, add them.
2. **Simplify to `j_call_s`** (PROGRESS-20260902.md "Next steps"): Replace `JS` calls with `JCS` (`j_call_s`) which uses a different spec (`P <0T <0T <I4 >0T[256]`). This may have a different result-vector behavior.
3. **Align with `java-demo.apl`**: Change `java-demo-sh.apl` lines from `r ← JS ... 256` + `2⊃r` to `buf ← ''` then `JS ... 256 buf`, reading `buf` directly. This avoids dependence on multi-element result vectors entirely.
4. **Buffer ownership** (`cabi.rs` `owned` logic): Verify that `>0T[n]` arguments create an owned mutable buffer that the JNI bridge can write into, and that the cabi reads it back into `ValueP` after the call.
5. **Verification before any fix**: Confirm with `cargo test -- --nocapture` and `cargo clippy --all-targets` (per user workflow in memory / PROMPT.md); run differential test against C++ reference only if C source changed (not relevant here, since fix is in Rust FFI layer).

---

## 6. Scope of this report

- **No source files modified.**
- **No `cargo` commands executed.**
- **No tests run.**
- **No writes to `src/` or `Cargo.toml`.**
- Only `META-INF/code_analysis.md` created (this file).

---

## 7. Related references (unmodified)

- `META-INF/PROGRESS-20260902.md` (Session 107, Java FFI status)
- `META-INF/PLAN-libapl.md` (FFI architecture, cabi mapping)
- `META-INF/ROADMAP.md` (phase 1 quad functions, phase 6 plugins / CDR / RE / plot / PNG / SQL / FFT)
- `META-INF/REWRITE_STRATEGY.md` (10-phase migration plan)
- `examples/java-demo-sh.apl` (target file)
- `examples/java-demo.apl` (deprecated GTK-based reference)
- `examples/AplUtils.java` (Java class called by both)
- `src/ffi/cabi.rs` (FFI driver, read only)
- `src/plugins/gtk.rs` (calculator keyboard, mentioned in PROGRESS-20260902.md but unrelated to Java FFI error)

---

*Report written by Code Analyzer. Read-only analysis; code suggestions documented but not implemented. User must approve any source modifications separately.*
