⍝ JAVA.APLWS — ergonomic Java bridge layer over libapljava.so
⍝
⍝ Raw ⎕NA symbols are associated once here; )SAVE persists both the NA
⍝ records and these dfns, so a plain )LOAD restores everything.
⍝
⍝ Calling convention: each native symbol is monadic. Multiple arguments
⍝ are bundled with comma (strand) where each argument is a separate element:
⍝
⍝   e ← JInit '/tmp'                                                       ⍝ boot JVM
⍝   h ← JNew 'pkg/Class'                                                   ⍝ instantiate
⍝   r ← JC ((⊂h), 'add' '(II)I' 3 4)                                     ⍝ instance method
⍝   s ← JCS ((⊂h), 'name' '()Ljava/lang/String;' 128)                     ⍝ string method
⍝   r ← JS 'F6cTest' 'greet' '(Ljava/lang/String;)Ljava/lang/String;' 'world' 256  ⍝ static
⍝   v ← JG ((⊂h), 'field' 'I')                                            ⍝ read field
⍝   JSF ((⊂h), 'field' 'I' 42)                                            ⍝ write field
⍝   JFree h                                                                ⍝ release handle
⍝
⍝ Note: the left arg is always enclosed (⊂h) so the cabi explode
⍝   unpacks it as a scalar P value. The remaining args form a flat
⍝   strand — strings stay as strings, numbers as numbers.
⍝
⍝ Note: JInit must be called first to boot the JVM.

'JI' ⎕NA 'P apl_java|j_init <0T'
'JN' ⎕NA 'P apl_java|j_new <0T'
'JC' ⎕NA 'I4 apl_java|j_call P <0T <0T I8 I8 >I8'
'JCS' ⎕NA 'I4 apl_java|j_call_s P <0T <0T <I4 >0T[256]'
'JS' ⎕NA 'I4 apl_java|j_call_static <0T <0T <0T <0T <I4 >0T[256]'
'JF' ⎕NA 'I4 apl_java|j_free P'
'JG' ⎕NA 'I4 apl_java|j_get_field P <0T <0T >I8'
'JSF' ⎕NA 'I4 apl_java|j_set_field P <0T <0T I8'
