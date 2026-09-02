⍝ Java FFI Demo: Calling Java from APL via JNI bridge
⍝ Demonstrates both standard library and custom class calls
⍝
⍝ Prerequisites:
⍝   1. Build libapljava.so: cargo build -p apl-java --features java
⍝   2. Compile AplUtils.java: javac AplUtils.java
⍝   3. Set JAVA_HOME if not already set
⍝
⍝ Run: ./target/debug/apl < examples/java-demo.apl

⍝ === Initialize JVM ===
⎕GTK 'append Starting Java FFI demo...'

⍝ Load the JNI bridge
⎕NA 'I4 apl_java|j_init <0T'

⍝ === Standard Library: java.util.Arrays ===
⎕GTK 'append Calling java.util.Arrays.sort...'

⍝ Sort an array using Java's Arrays.sort
⎕NA 'I4 java/util/Arrays/sort <F8[] I4'
⎕NA 'I4 java/util/Arrays/binarySearch <F8[] I4'

data ← 3.1 1.4 1.5 9.2 6.5 3.5 8.9
⎕GTK 'append Before sort: ',⍕data
sort data
⎕GTK 'append After sort: ',⍕data

⍝ === Standard Library: java.lang.Math ===
⎕GTK 'append Calling java.lang.Math...'

⎕NA 'F8 java/lang/Math/sin F8'
⎕NA 'F8 java/lang/Math/cos F8'
⎕NA 'F8 java/lang/Math/sqrt F8'
⎕NA 'F8 java/lang/Math/pow F8 F8'
⎕NA 'F8 java/lang/Math/random I4'

⎕GTK 'append sin(1.0): ',⍕sin 1.0
⎕GTK 'append cos(1.0): ',⍕cos 1.0
⎕GTK 'append sqrt(2.0): ',⍕sqrt 2.0
⎕GTK 'append pow(2.0, 10.0): ',⍕pow 2.0 10.0
⎕GTK 'append random: ',⍕random 0

⍝ === Standard Library: java.lang.String ===
⎕GTK 'append Calling java.lang.String...'

⎕NA 'I4 java/lang/String/length <0T'
⎕NA '0T java/lang/String/toUpperCase <0T'
⎕NA '0T java/lang/String/toLowerCase <0T'
⎕GTK 'append Length of "hello": ',⍕length 'hello'
⎕GTK 'append Uppercase: ',⍕toUpperCase 'hello world'
⎕GTK 'append Lowercase: ',⍕toLowerCase 'HELLO WORLD'

⍝ === Custom Class: AplUtils ===
⎕GTK 'append Calling custom AplUtils class...'

⎕NA '0T AplUtils/reverse <0T'
⎕NA '0T AplUtils/isPalindrome <0T'
⎕NA '0T AplUtils/upper <0T'
⎕NA '0T AplUtils/lower <0T'
⎕NA '0T AplUtils/trim <0T'
⎕NA '0T AplUtils/replace <0T'
⎕NA '0T AplUtils/sha256 <0T'
⎕NA '0T AplUtils/base64Encode <0T'
⎕NA '0T AplUtils/base64Decode <0T'
⎕NA '0T AplUtils/rot13 <0T'
⎕NA '0T AplUtils/uuid I4'
⎕NA '0T AplUtils/getProperty <0T'
⎕NA '0T AplUtils/sortCsv <0T'
⎕NA '0T AplUtils/wordCount <0T'
⎕NA '0T AplUtils/levenshtein <0T'
⎕NA '0T AplUtils/lcs <0T'

⎕GTK 'append Reverse "hello": ',⍕reverse 'hello'
⎕GTK 'append Is "racecar" palindrome: ',⍕isPalindrome 'racecar'
⎕GTK 'append Is "hello" palindrome: ',⍕isPalindrome 'hello'
⎕GTK 'append SHA-256 of "hello": ',⍕sha256 'hello'
⎕GTK 'append Base64 encode "hello world": ',⍕base64Encode 'hello world'
⎕GTK 'append ROT13 "hello world": ',⍕rot13 'hello world'
⎕GTK 'append UUID: ',⍕uuid 0
⎕GTK 'append Java version: ',⍕getProperty 'java.version'
⎕GTK 'append Sort CSV "c,a,b": ',⍕sortCsv 'c,a,b'
⎕GTK 'append Word count: ',⍕wordCount 'hello world from APL'
⎕GTK 'append Levenshtein "kitten","sitting": ',⍕levenshtein 'kitten,sitting'
⎕GTK 'append LCS "ABCBDAB","BDCABA": ',⍕lcs 'ABCBDAB,BDCABA'

⎕GTK 'append Java FFI demo complete!'
