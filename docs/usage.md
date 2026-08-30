# Using the Rust APL Interpreter

This document covers the interactive REPL, APL expressions, system commands, and extensions.

## Table of Contents

- [Starting the REPL](#starting-the-repl)
- [Basic expressions](#basic-expressions)
- [Primitive functions](#primitive-functions)
- [Operators](#operators)
- [Defined functions](#defined-functions)
- [System variables](#system-variables)
- [System commands](#system-commands)
- [Workspace persistence](#workspace-persistence)
- [Quad functions](#quad-functions)
- [Quad system variables](#quad-system-variables)
- [Unofficial extensions (Dyalog-compatible)](#unofficial-extensions-dyalog-compatible)
- [Pipe testing](#pipe-testing)
- [Troubleshooting](#troubleshooting)

---

## Starting the REPL

```sh
$ ./target/release/apl
GNU APL 2.0 (Rust) — experimental REPL
Enter APL expressions, or )OFF to exit.
      2+3
5
```

The prompt shows 6 spaces of indentation. Type an APL expression and press Enter.

Exit with `)OFF` or Ctrl+D.

---

## Basic expressions

APL evaluates right-to-left. There is no operator precedence — all functions have equal precedence.

```apl
      2+3
5
      2 × 3+4          ⍝ right-to-left: 2 × (3+4) = 14
14
      (2 × 3)+4        ⍝ parentheses override: (2 × 3)+4 = 10
10
```

### Vectors and matrices

```apl
      ⍳5               ⍝ index generator: 0 1 2 3 4
0 1 2 3 4
      2 3 4 + 10 20 30 ⍝ elementwise addition
12 23 34
      2 3⍴⍳6           ⍝ reshape into 2×3 matrix
0 1 2
3 4 5
```

### Nested arrays

```apl
      (1 2)(3 4 5)     ⍝ nested vector
┏→━━━━┓
┃1 2 ┃3 4 5┃
┗∼━━━━┛
```

---

## Primitive functions

### Arithmetic

| Glyph | Monadic | Dyadic |
|---|---|---|
| `+` | conjugate | add |
| `-` | negate | subtract |
| `×` | signum | multiply |
| `÷` | reciprocal | divide |
| `⋆` | exponential | power |
| `○` | pi times | circular/logarithms |
| `!` | factorial | binomial |
| `⌈` | ceiling | maximum |
| `⌊` | floor | minimum |
| `∣` | absolute value | modulus |
| `?` | roll | deal |

### Comparison

| Glyph | Meaning |
|---|---|
| `<` | less than |
| `≤` | less than or equal |
| `=` | equal |
| `≥` | greater than or equal |
| `>` | greater than |
| `≠` | not equal |
| `≡` | depth (structure match) |
| `≢` | tally (shape mismatch) |

### Structural

| Glyph | Monadic | Dyadic |
|---|---|---|
| `⍴` | shape | reshape |
| `⍳` | index generator | index of |
| `↑` | mix | take |
| `↓` | split | drop |
| `⌽` | reverse | rotate |
| `⍉` | transpose | dyadic transpose |
| `⍋` | grade up | grade up by key |
| `⍒` | grade down | grade down by key |
| `∪` | unique | union |
| `∩` | — | intersection |
| `⊂` | enclose | partition |
| `⊃` | disclose | pick |
| `∊` | — | membership |
| `∣` | absolute value | modulus |
| `⍷` | — | find |

### Logical

| Glyph | Monadic | Dyadic |
|---|---|---|
| `∧` | — | logical AND |
| `∨` | — | logical OR |
| `⍱` | — | NOR |
| `⍲` | — | NAND |
| `∼` | not | without |
| `→` | — | branch |

---

## Operators

### Reduce `/`

```apl
      +/ 1 2 3 4 5    ⍝ sum
15
      ×/ 1 2 3 4 5    ⍝ product
120
```

### Scan `\`

```apl
      +\ 1 2 3 4 5    ⍝ running sum
1 3 6 10 15
```

### Each `¨`

```apl
      ⍳¨ 3 4 5
┏→┓ ┏→━┓ ┏→━━┓
┃0 1 2┃0 1 2 3┃0 1 2 3 4┃
┗∼┛ ┗∼━┛ ┗∼━━┛
```

### Outer product `∘.`

```apl
      1 2 3 ∘.× 10 20 30
10 20 30
20 40 60
30 60 90
```

### Inner product `f.g`

```apl
      1 2 3 +.× 10 20 30
140
      M←2 3⍴⍳6
      M +.× 5 6 7
20 74
```

### Commute `⍨`

```apl
      5 +⍨ 3           ⍝ 3+5 = 8
8
      5 ×⍨ 3           ⍝ 3×5 = 15
15
```

### Axis `F[axis]`

```apl
      M←3 4⍴⍳12
      +/[1] M          ⍝ sum along axis 1
12 15 18 21
```

---

## Defined functions

### Monadic

```apl
      ∇ R←DOUBLE N
[1]    R←N×2
[2]    ∇
      DOUBLE 21
42
```

### Dyadic

```apl
      ∇ R←A SUM B
[1]    R←A+B
[2]    ∇
      5 SUM 7
12
```

### Recursive

```apl
      ∇ R←FAC N
[1]    →(N=0)/BASE
[2]    R←N×∇ N-1
[3]    →0
[4]   BASE: R←1
[5]    ∇
      FAC 5
120
```

### Control structures

```apl
      :If I≥N
          :Leave
      :EndIf
      :While cond
          body
      :EndWhile
```

---

## System variables

| Variable | Default | Purpose |
|---|---|---|
| `⎕IO` | 0 | index origin |
| `⎕CT` | 1e-13 | comparison tolerance |
| `⎕PP` | 10 | print precision |
| `⎕BOXING` | 1 | boxed display (1=on, 0=off) |
| `⎕SEC` | 0 | security level |
| `⎕AV` | 256 chars | APL character vector |
| `⎕WA` | bytes | workspace available |
| `⎕TS` | 8 ints | timestamp |

---

## System commands

| Command | Purpose |
|---|---|
| `)OFF` | exit |
| `)VARS` | list variables |
| `)FNS` | list functions |
| `)OPS` | list operators |
| `)GRP` | group names by type |
| `)NMS` | names grouped by first letter |
| `)SAVE name` | save workspace |
| `)LOAD name` | load workspace |
| `)CLEAR` | wipe workspace |
| `)DROP name` | delete workspace |
| `)CONTINUE` | save and exit |
| `)ERASE name` | erase function/variable |
| `)INP file` | input session from file |
| `)OUT file` | save session as APL source |
| `)COPY name` | copy from workspace |
| `)LIB` | list saved workspaces |
| `)SI` | state indicator |
| `)SINL` | state indicator with line numbers |
| `)SVS` | shared variable status |

---

## Workspace persistence

```apl
      )SAVE mywork      ⍝ creates mywork.xml
      )LOAD mywork      ⍝ loads workspace
      )CLEAR            ⍝ wipe current workspace
      )DROP mywork      ⍝ delete mywork.xml
      )CONTINUE         ⍝ save to CONTINUE.xml and exit
```

---

## Quad functions

| Function | Description | Example |
|---|---|---|
| `⎕CR B` | character representation | `4 ⎕CR matrix` |
| `⎕RVAL B` | random value | `⎕RVAL 2 3` |
| `⎕TF B` | transfer form | `⎕CR 'FAC'` |
| `⎕FX B` | fix function from matrix | |
| `⎕MX B` | matrix operations | `3 ⎕MX matrix` (trace) |
| `⎕DLX B` | dancing links | |
| `⎕FFT B` | FFT | |
| `⎕RE B` | regex | `0 'o' ⎕RE 'hello'` |
| `⎕SQL B` | SQL query | |
| `⎕PNG B` | PNG image I/O | |
| `⎕CDR B` | CDR binary | |
| `⎕PLOT B` | plot data | |
| `⎕GPLOT B` | GTK plot (future) | |
| `⎕PYTHON B` | Python pipe | |
| `⎕SVC` | shared variable control | |
| `⎕SVO B` | shared variable off | |
| `⎕SVQ B` | shared variable query | |
| `⎕SVR B` | shared variable read | |
| `⎕SVS B` | shared variable set | |

### Quad system variables

| Variable | Description |
|---|---|
| `⎕AI` | argument info |
| `⎕ARG` | command line args |
| `⎕LC` | line counter |
| `⎕EM` | error message |
| `⎕ET` | error token |
| `⎕TS` | timestamp |
| `⎕TZ` | timezone |
| `⎕UL` | user lock |
| `⎕WA` | workspace available |
| `⎕CT` | comparison tolerance |
| `⎕FC` | format codes |
| `⎕IO` | index origin |
| `⎕L` | latent expression |
| `⎕LX` | latent expression |
| `⎕PP` | print precision |
| `⎕PR` | prompt |
| `⎕PS` | print separator |
| `⎕PW` | print width |
| `⎕R` | random seed |
| `⎕RL` | random link |
| `⎕SYL` | symbol table length |
| `⎕X` | execution stack |

---

## Unofficial extensions (Dyalog-compatible)

Enable with `--features unofficial-ext`:

```sh
cargo build --release --features unofficial-ext
```

| Glyph | Name | Description |
|---|---|---|
| `⌸` (U+2328) | Key | group unique elements |
| `⍥` (U+2365) | Over | function composition |

```apl
      ⌸1 2 1 3 2
1 ┏→━━┓
2 ┏→┓
3 ┏→┓
(×⍥⌈) 3.7
1
```

---

## Pipe testing

```sh
echo '2+3' | ./target/release/apl
echo '⍳5' | ./target/release/apl
echo 'FAC←{⍵=0:1 ⋄ ⍵×∇ ⍵-1} ⋄ FAC 5' | ./target/release/apl
```

---

## Troubleshooting

### "Nix socket" warning

Harmless. The build actually succeeds. Ignore it.

### DOMAIN ERROR

Check argument types. Most primitives require numeric input.

### VALUE ERROR

Check that the function/variable name exists.

### SYNTAX ERROR

Check parentheses and brackets are balanced.

### Workspace not loading

Ensure the `.xml` file exists in the current directory.
