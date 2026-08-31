⍝ APL Calculator Demo — uses ⎕GTK for GUI display
⍝
⍝ Run with: cargo run --release --features plugin-gtk < examples/calc-demo.apl
⍝ Or from REPL: )INP examples/calc-demo.apl
⍝
⍝ APL uses the same arithmetic operators: + - × ÷ *
⍝ Type expressions like: 2+3*4, (10+5)÷3, 2*10

⍝ Open the GTK window with title
⎕GTK 'text APL Calculator Demo'
⎕GTK 'text ===================='
⎕GTK 'text '
⎕GTK 'text Demo calculations:'
⎕GTK 'text 2 + 3 × 4 → ',⍕2+3×4
⎕GTK 'text (10 - 4) ÷ 3 → ',⍕(10-4)÷3
⎕GTK 'text 2 * 10 → ',⍕2*10
⎕GTK 'text 20 + 30 × 2 → ',⍕20+30×2
⎕GTK 'text '
⎕GTK 'text APL evaluates × before +'
⎕GTK 'text just like standard math!'
⎕GTK 'text '

⍝ Interactive calculator loop
∇run_calc
  ⎕GTK 'text Enter expression (or "quit" to exit):'
  input ← ⎕
  
  ⍝ Check for quit
  →(input≡'quit')/done
  
  ⍝ Evaluate input and display result
  ⍝ ⍎ executes the string as APL code
  result ← ⍎input
  ⎕GTK 'text '⍕input,' → ',⍕result
  
  run_calc
∇

⍝ Label for quit
→done

⍝ End
done:
⎕GTK 'text '
⎕GTK 'text Goodbye!'
⎕GTK 'close'
