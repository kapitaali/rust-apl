# C and Java FFI Demos

## C FFI Demo (`examples/libstats.c`) — ✅ WORKING

Array statistics library compiled to `libstats.so`:
- `mean`, `stddev`, `stddev_sample`, `array_min`, `array_max`, `sum_array`
- `median` (sorts a copy), `sort` (in-place quicksort), `normalize` (to [0,1])
- `correlation` (Pearson), `matmul`, `determinant`, `transpose`

**Compile**: `cd examples && gcc -shared -fPIC -o libstats.so libstats.c -lm`

### Dyadic Array Passing Convention

Array functions take the array and its length as separate arguments. The correct calling form is **dyadic** (two separate args), NOT monadic with an enclosed pair:

```apl
'mean' ⎕NA 'F8 examples/libstats.so|mean <F8[] I4'

⍝ CORRECT: dyadic form — array + length as separate args
(⊂data) mean (≢data)

⍝ WRONG: monadic form — passes only the array, no length
mean data
```

**Why this works**: The cabi explode logic in `cabi.rs` unpacks a single vector into its cells when `args.len() == 1 && n > 1 && n as usize == input_args`. But for functions that take BOTH an array AND a scalar length, you need two separate arguments so the explode produces `[array_arg, length_arg]`.

**Verified results**:
- `mean` of `1.2 3.4 2.1 5.6` → `3.075`
- `median` → `2.75`
- `sum_i4` of `1 2 3 4 5` → `15`
- `stddev` → `1.654350326`

## Java FFI Demo (`examples/AplUtils.java`) — ⚠️ INFRASTRUCTURE ONLY

Custom class with static methods:
- `reverse`, `isPalindrome`, `upper`, `lower`, `trim`
- `replace`, `indexOf`, `charAt`, `concat`, `substring`
- `sortCsv`, `wordCount`, `levenshtein`, `lcs`
- `sha256`, `base64Encode`, `base64Decode`, `rot13`
- `uuid`, `getProperty`, `getEnv`

**Compile**: `javac examples/AplUtils.java`
**Requires**: JDK + `JAVA_HOME=/path/to/jdk cargo build -p apl-java --features java`

### Known Issue: Multi-Argument Native Calls

The `j_call_static` function expects 6 arguments:
```
j_call_static(class, method, sig, arg, cap, out_buf) -> I4
```

But the cabi explode logic only handles `args.len() == 1` (single vector arg). For multi-argument native calls with mixed types (strings + integers + output buffers), the current infrastructure doesn't correctly unpack them. The `jcall` apl-java symbol works for 2-argument functions (like `jcall P <0T <0T I8 I8 >I8`) but fails for 5-6 argument functions.

### Pitfall: Java FFI + GTK Event Loop

**Do NOT call Java FFI functions from within a GTK calculator event loop.** The GTK main loop consumes all events, including the `GtkEvent::ButtonClicked` events that the APL loop waits on. Java FFI calls block waiting for events that the GTK loop has already consumed, causing a deadlock.

**Solution**: Run Java FFI demos in shell mode (`examples/java-demo-sh.apl`), not from `calc-demo.apl`.

### Status

- The JNI bridge (`libapljava.so`) builds and initializes correctly
- `j_init` returns 1 (success)
- `j_call_static` returns `(return_code, output_buffer)` as a 2-element vector
- The return buffer extraction (`2⊃r`) gives INDEX ERROR, suggesting the output buffer may need pre-allocation or the return format is nested differently
- This needs debugging in `crates/apl-java/src/lib.rs` around the `j_call_static` function

## GNU APL Limitation

GNU APL 2.0 does NOT support `⎕NA` — returns "NOT YET IMPLEMENTED" for all `⎕NA` declarations. C FFI and Java FFI demos only work in rust-apl. This was verified by running test scripts against the reference binary at `~/Apps/apl-2.0/src/apl`.
