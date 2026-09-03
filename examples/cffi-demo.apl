⍝ C FFI Demo: Array Statistics via ⎕NA
⍝ Demonstrates calling C functions from APL using ⎕NA
⍝
⍝ Compile the C library first:
⍝   cd examples && gcc -shared -fPIC -o libstats.so libstats.c -lm
⍝
⍝ Then run this script:
⍝   ./target/debug/apl < examples/cffi-demo.apl

⎕IO←1

⍝ === Associate C functions ===
⍝ Format: 'APLNAME' ⎕NA 'library|function <return> <arg1> <arg2> ...'
⍝ <I4 = input int, <F8[] = input double array, >F8 = output double

'div' ⎕NA 'F8 libtestmath.so|divide I4 I4'
10 div 3

'add64' ⎕NA 'I8 libtestmath.so|add64 I8 I8'
100 add64 200

'clamp' ⎕NA 'U1 libtestmath.so|clamp_u8 I4'
clamp 300

'sum_i4' ⎕NA 'I4 libtestmath.so|sum_i4 <I4[] I4'
data ← 1 2 3 4 5
(⊂data) sum_i4 (≢data)

'mean' ⎕NA 'F8 examples/libstats.so|mean <F8[] I4'
fdata ← 1.2 3.4 2.1 5.6
(⊂fdata) mean (≢fdata)

'median' ⎕NA 'F8 examples/libstats.so|median <F8[] I4'
(⊂fdata) median (≢fdata)

'amin' ⎕NA 'F8 examples/libstats.so|array_min <F8[] I4'
(⊂fdata) amin (≢fdata)

'amax' ⎕NA 'F8 examples/libstats.so|array_max <F8[] I4'
(⊂fdata) amax (≢fdata)

'sum' ⎕NA 'F8 examples/libstats.so|sum_array <F8[] I4'
(⊂fdata) sum (≢fdata)

'stddev' ⎕NA 'F8 examples/libstats.so|stddev <F8[] I4'
(⊂fdata) stddev (≢fdata)

'sort' ⎕NA 'examples/libstats.so|sort =F8[] I4'
sorted ← 3.1 1.4 1.5 9.2 6.5
(⊂sorted) sort (≢sorted)
sorted

⍝ === Matrix operations ===
A ← 1.0 2.0 3.0 4.0
B ← 5.0 6.0 7.0 8.0
C ← 0.0 0.0 0.0 0.0
'matmul' ⎕NA 'examples/libstats.so|matmul <F8[] <F8[] =F8[] I4 I4 I4'
matmul (⊂A) (⊂B) (⊂C) (2) (2) (2)
C

'det' ⎕NA 'F8 examples/libstats.so|determinant <F8[] I4'
(⊂A) det 2

T ← 0.0 0.0 0.0 0.0
'transpose' ⎕NA 'examples/libstats.so|transpose <F8[] =F8[] I4 I4'
T ← ⊃1⊃transpose (⊂A) (⊂T) (2) (2)
T
