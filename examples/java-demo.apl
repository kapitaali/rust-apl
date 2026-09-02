⍝ Java FFI Demo: Calling Java from APL via JNI bridge
⍝
⍝ Prerequisites:
⍝   1. Build libapljava.so: cargo build -p apl-java --features java
⍝   2. Set JAVA_HOME if not already set
⍝   3. Compile AplUtils.java: javac AplUtils.java
⍝
⍝ Run: ./target/debug/apl < examples/java-demo.apl

⍝ === Load the JNI bridge ===
'JI' ⎕NA 'P apl_java|j_init <0T'
'JN' ⎕NA 'P apl_java|j_new <0T'
'JC' ⎕NA 'I4 apl_java|j_call P <0T <0T I8 I8 >I8'
'JCS' ⎕NA 'I4 apl_java|j_call_s P <0T <0T <I4 >0T[256]'
'JS' ⎕NA 'I4 apl_java|j_call_static <0T <0T <0T <0T <I4 >0T[256]'
'JF' ⎕NA 'I4 apl_java|j_free P'
'JG' ⎕NA 'I4 apl_java|j_get_field P <0T <0T >I8'
'JSF' ⎕NA 'I4 apl_java|j_set_field P <0T <0T I8'

⍝ === Initialize JVM ===
⎕GTK 'append Starting Java FFI demo...'
JInit '/tmp'

⍝ === Static method calls (String -> String) ===
⎕GTK 'append Calling java.lang.System.getProperty...'
buf ← ''
r ← JS 'java/lang/System' 'getProperty' '(Ljava/lang/String;)Ljava/lang/String;' 'java.version' 256 buf
⎕GTK 'append Java version: ',buf

⎕GTK 'append Calling custom AplUtils class...'
buf ← ''
r ← JS 'AplUtils' 'reverse' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256 buf
⎕GTK 'append Reverse "hello": ',buf

buf ← ''
r ← JS 'AplUtils' 'isPalindrome' '(Ljava/lang/String;)Ljava/lang/String;' 'racecar' 256 buf
⎕GTK 'append Is "racecar" palindrome: ',buf

buf ← ''
r ← JS 'AplUtils' 'isPalindrome' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256 buf
⎕GTK 'append Is "hello" palindrome: ',buf

buf ← ''
r ← JS 'AplUtils' 'sha256' '(Ljava/lang/String;)Ljava/lang/String;' 'hello' 256 buf
⎕GTK 'append SHA-256 of "hello": ',buf

buf ← ''
r ← JS 'AplUtils' 'base64Encode' '(Ljava/lang/String;)Ljava/lang/String;' 'hello world' 256 buf
⎕GTK 'append Base64 encode "hello world": ',buf

buf ← ''
r ← JS 'AplUtils' 'rot13' '(Ljava/lang/String;)Ljava/lang/String;' 'hello world' 256 buf
⎕GTK 'append ROT13 "hello world": ',buf

buf ← ''
r ← JS 'AplUtils' 'uuid' '(I)Ljava/lang/String;' '0' 256 buf
⎕GTK 'append UUID: ',buf

buf ← ''
r ← JS 'AplUtils' 'sortCsv' '(Ljava/lang/String;)Ljava/lang/String;' 'c,a,b' 256 buf
⎕GTK 'append Sort CSV "c,a,b": ',buf

buf ← ''
r ← JS 'AplUtils' 'wordCount' '(Ljava/lang/String;)Ljava/lang/String;' 'hello world from APL' 256 buf
⎕GTK 'append Word count: ',buf

buf ← ''
r ← JS 'AplUtils' 'levenshtein' '(Ljava/lang/String;)Ljava/lang/String;' 'kitten,sitting' 256 buf
⎕GTK 'append Levenshtein "kitten","sitting": ',buf

buf ← ''
r ← JS 'AplUtils' 'lcs' '(Ljava/lang/String;)Ljava/lang/String;' 'ABCBDAB,BDCABA' 256 buf
⎕GTK 'append LCS "ABCBDAB","BDCABA": ',buf

⎕GTK 'append Java FFI demo complete!'
