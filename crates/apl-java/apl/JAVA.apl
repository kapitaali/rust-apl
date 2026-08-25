⍝ JAVA.APLWS — ergonomic Java bridge layer over libapljava.so
⍝
⍝ Raw ⎕NA symbols are associated once here; )SAVE persists both the NA
⍝ records and these dfns, so a plain )LOAD restores everything.
⍝
⍝ Surface:
⍝   env ← JInit '/tmp'                        ⍝ boot JVM with classpath
⍝   h   ← env JNew 'pkg/Class'                ⍝ instantiate (no-arg ctor)
⍝   r   ← env h JCall 'add' '(II)I' 3 4       ⍝ numeric instance call
⍝   s   ← env h JCallS 'name' sig 0           ⍝ string instance call
⍝   env JFree h                                ⍝ release handle
⍝
⍝ Call convention: JInit and JNew are thin dfn wrappers. JCall / JCallS
⍝ take 5+ positional args so they are called monadically with an explicit
⍝ vector of enclosed items (see examples below). The dyadic JNew/JFree
⍝ work via the interpreter's native dyadic desugar (enclosed pair).

'JI' ⎕NA 'P apl_java|j_init <0T'
'JN' ⎕NA 'P apl_java|j_new P <0T'
'JC' ⎕NA 'I4 apl_java|j_call P I8 <0T <0T I8 I8 >I8'
'JCS' ⎕NA 'I4 apl_java|j_call_s P I8 <0T <0T <I4 >0T[256]'
'JS' ⎕NA 'I4 apl_java|j_call_static P <0T <0T <0T <0T <I4 >0T[256]'
'JF' ⎕NA 'I4 apl_java|j_free P P'

JInit←{JI (⊂⍵)}
JNew←{⍺ JN ⍵}
JFree←{⍺ JF ⍵}

⍝ JCall — monadic: env h JCall 'm' '(II)I' 3 4
⍝ Pass as: JC (⊂env) (⊂h) 'm' '(II)I' 3 4
⍝ Result: 2-item vector [rc, enclosed_result]
JCall←{JC (⊂⍺) (⊂⍵)}

⍝ JCallS — string instance method
⍝ Pass as: JCS (⊂env) (⊂h) 'name' '()Ljava/lang/String;' 0
⍝ Result: 2-item vector [rc, enclosed_string]
JCallS←{JCS (⊂⍺) (⊂⍵)}
