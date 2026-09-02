⍝ Java FFI Demo: Calling Java from APL via JNI bridge
⍝ Demonstrates both standard library and custom class calls
⍝
⍝ Prerequisites:
⍝   1. Build libapljava.so: cargo build -p apl-java --features java
⍝   2. Set JAVA_HOME if not already set
⍝   3. Compile AplUtils.java: javac AplUtils.java
⍝
⍝ Run: ./target/debug/apl < examples/java-demo-sh.apl

⍝ === Load the JNI bridge ===
'JI' ⎕NA 'P /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_init <0T'
'JN' ⎕NA 'P /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_new <0T'
'JC' ⎕NA 'I4 /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_call P <0T <0T I8 I8 >I8'
'JCS' ⎕NA 'I4 /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_call_s P <0T <0T <I4 >0T[256]'
'JS' ⎕NA 'I4 /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_call_static <0T <0T <0T <0T <I4 >0T[256]'
'JF' ⎕NA 'I4 /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_free P'
'JG' ⎕NA 'I4 /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_get_field P <0T <0T >I8'
'JSF' ⎕NA 'I4 /home/theb/Apps/apl-2.0/rust-apl/target/debug/libapl_java.so|j_set_field P <0T <0T I8'

⍝ === Initialize JVM (classpath = directory containing .class files) ===
JI '/home/theb/Apps/apl-2.0/rust-apl/examples'

⍝ === Static method calls (String -> String) ===
⎕ ← 'Calling java.lang.System.getProperty...'
r ← JS 'java/lang/System' 'getProperty' '(Ljava/lang/String;)Ljava/lang/String;' 'java.version' 256
⎕ ← 'Java version: ',2⊃r

⎕ ← 'Calling custom AplUtils class...'
r ← JS 'AplUtils' 'reverse' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256
⎕ ← 'Reverse "hello": ',2⊃r

r ← JS 'AplUtils' 'isPalindrome' '(Ljava/lang/String;)Ljava/lang/String;' 'racecar' 256
⎕ ← 'Is "racecar" palindrome: ',2⊃r

r ← JS 'AplUtils' 'isPalindrome' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256
⎕ ← 'Is "hello" palindrome: ',2⊃r

r ← JS 'AplUtils' 'sha256' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256
⎕ ← 'SHA-256 of "hello": ',2⊃r

r ← JS 'AplUtils' 'base64Encode' '(Ljava/lang/String;)Ljava/lang/String;' 'hello world' 256
⎕ ← 'Base64 encode "hello world": ',2⊃r

r ← JS 'AplUtils' 'rot13' '(Ljava/lang/String;)Ljava/lang/String;' 'hello world' 256
⎕ ← 'ROT13 "hello world": ',2⊃r

r ← JS 'AplUtils' 'uuid' '(I)Ljava/lang/String;' '0' 256
⎕ ← 'UUID: ',2⊃r

r ← JS 'AplUtils' 'sortCsv' '(Ljava/lang/String;)Ljava/lang/String;' 'c,a,b' 256
⎕ ← 'Sort CSV "c,a,b": ',2⊃r

r ← JS 'AplUtils' 'wordCount' '(Ljava/lang/String;)Ljava/lang/String;' 'hello world from APL' 256
⎕ ← 'Word count: ',2⊃r

r ← JS 'AplUtils' 'levenshtein' '(Ljava/lang/String;)Ljava/lang/String;' 'kitten,sitting' 256
⎕ ← 'Levenshtein "kitten","sitting": ',2⊃r

r ← JS 'AplUtils' 'lcs' '(Ljava/lang/String;)Ljava/lang/String;' 'ABCBDAB,BDCABA' 256
⎕ ← 'LCS "ABCBDAB","BDCABA": ',2⊃r

⎕ ← 'Java FFI demo complete!'
