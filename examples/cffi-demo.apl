⍝ C FFI Demo: Array Statistics via ⎕NA
⍝ Demonstrates calling C functions from APL using ⎕NA
⍝
⍝ Compile the C libraries first (from repo root):
⍝   cd examples && gcc -shared -fPIC -o libstats.so libstats.c -lm
⍝   cd examples && gcc -shared -fPIC -o libstats_struct.so libstats_struct.c -lm
⍝   cp ../libtestmath.so .    ⍝ copy math lib to examples
⍝
⍝ Then run from examples directory:
⍝   cd examples && ../target/debug/apl < cffi-demo.apl

⎕IO←1

⍝ === Associate C functions ===
'div' ⎕NA 'F8 libtestmath.so|divide I4 I4'
10 div 3

'add64' ⎕NA 'I8 libtestmath.so|add64 I8 I8'
100 add64 200

'clamp' ⎕NA 'U1 libtestmath.so|clamp_u8 I4'
clamp 300

'sum_i4' ⎕NA 'I4 libtestmath.so|sum_i4 <I4[] I4'
data ← 1 2 3 4 5
(⊂data) sum_i4 (≢data)

'mean' ⎕NA 'F8 libstats.so|mean <F8[] I4'
fdata ← 1.2 3.4 2.1 5.6
(⊂fdata) mean (≢fdata)

'median' ⎕NA 'F8 libstats.so|median <F8[] I4'
(⊂fdata) median (≢fdata)

'amin' ⎕NA 'F8 libstats.so|array_min <F8[] I4'
(⊂fdata) amin (≢fdata)

'amax' ⎕NA 'F8 libstats.so|array_max <F8[] I4'
(⊂fdata) amax (≢fdata)

'sum' ⎕NA 'F8 libstats.so|sum_array <F8[] I4'
(⊂fdata) sum (≢fdata)

'stddev' ⎕NA 'F8 libstats.so|stddev <F8[] I4'
(⊂fdata) stddev (≢fdata)

'sort' ⎕NA 'libstats.so|sort =F8[] I4'
sorted ← 3.1 1.4 1.5 9.2 6.5
(⊂sorted) sort (≢sorted)
sorted

'matmul' ⎕NA 'libstats.so|matmul <F8[] <F8[] =F8[] I4 I4 I4'
A ← 1.0 2.0 3.0 4.0
B ← 5.0 6.0 7.0 8.0
C ← 0.0 0.0 0.0 0.0
matmul (⊂A) (⊂B) (⊂C) 2 2 2
C

'det' ⎕NA 'F8 libstats.so|determinant <F8[] I4'
(⊂A) det 2

T ← 0.0 0.0 0.0 0.0
'transpose' ⎕NA 'libstats.so|transpose <F8[] =F8[] I4 I4'
T ← ⊃1⊃transpose (⊂A) (⊂T) 2 2
T

⍝ === Struct example ===
'STATS' ⎕NA 'libstats_struct.so|compute_stats <F8[] I4 >F8[3]'
r ← (⊂data) STATS (≢data)
⊃r
⍝ Note: structs as {F8 F8 F8} don't work directly — use array buffer >F8[N]
