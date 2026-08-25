⍝ JAVA.APLWS — ergonomic Java bridge layer over libapljava.so
⍝ Raw ⎕NA symbols are associated once here; )SAVE persists both the
⍝ NA records and these dfns, so a plain )LOAD restores everything.
⍝
⍝ Surface:
⍝   env  ← classpath JInit 0          ⍝ boot JVM (or JInit '' )
⍝   h    ← env JNew 'pkg/Class'       ⍝ instantiate via no-arg ctor
⍝   n    ← (env h 'meth' '(II)I') JCall (3)(4)        ⍝ numeric instance call
⍝   s    ← (env h 'meth')       JStr  '()Ljava/lang/String;'  ⍝ string instance call
⍝   s    ← ('Cls' 'meth' '(L…;)…') JStat 'arg'              ⍝ static call
⍝   ok   ← env h JFree                   ⍝ release a handle
⍝ Numeric/string results are unwrapped; a negative bridge code comes
⍝ back instead of the value when the call fails.

'JI' ⎕NA 'P apl_java|j_init <0T'
'JN' ⎕NA 'P apl_java|j_new P <0T'
'JC' ⎕NA 'I4 apl_java|j_call P I8 <0T <0T I8 I8 >I8'
'JCS' ⎕NA 'I4 apl_java|j_call_s P I8 <0T <0T <I4 >0T[256]'
'JS' ⎕NA 'I4 apl_java|j_call_static P <0T <0T <0T <0T <I4 >0T[256]'
'JF' ⎕NA 'I4 apl_java|j_free P P'

JInit←{JI ⍵}
JNew←{⍺ JN ⍵}
JFree←{⍺ JF ⍵}
JCall←{r←JC ⍺,⍵ ⋄ 0=⊃r:1⊃⊃r ⋄ ⊃r}
JStr←{r←JCS (⍺,⊂⍵),⊂256 ⋄ 0=⊃r:1⊃⊃r ⋄ ⊃r}
JStat←{r←JS (⍺,⊂⍵),⊂256 ⋄ 0=⊃r:1⊃⊃r ⋄ ⊃r}
