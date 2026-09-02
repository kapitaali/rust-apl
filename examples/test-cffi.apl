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
