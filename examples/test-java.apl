'JS' ⎕NA 'I4 /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_call_static <0T <0T <0T <0T <I4 >0C[256]'
'JI' ⎕NA 'P /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_init <0T'

jptr ← JI '/home/theb/Apps/apl-2.0/rust-apl/examples'
r ← JS ('AplUtils' 'reverse' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256)
⎕ ← 'r: '
⎕ ← r
⎕ ← '≢r: '
⎕ ← ≢r
⎕ ← '⍴r: '
⎕ ← ⍴r
