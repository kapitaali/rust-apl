⍝ APL Calculator Demo — uses ⎕GTK for GUI display
⍝
⍝ Run with: cargo run --release --features plugin-gtk < examples/calc-demo.apl
⍝
⍝ Click buttons to enter expressions in the entry field.
⍝ Close the window to exit.

⍝ ──────────────────────────────────────────
⍝ Static demo content
⍝ ──────────────────────────────────────────

⎕GTK 'text ╔═══════════════════════════════╗'
⎕GTK 'append ║    APL Calculator (GTK)       ║'
⎕GTK 'append ╠═══════════════════════════════╣'
⎕GTK 'append ║  Click buttons to enter expr  ║'
⎕GTK 'append ║  Click Compute to evaluate    ║'
⎕GTK 'append ╚═══════════════════════════════╝'
⎕GTK 'append '

⍝ Basic arithmetic
⎕GTK 'append Basic arithmetic:'
⎕GTK 'append 2 + 3 × 4 = ',⍕2+3×4
⎕GTK 'append (10 - 4) ÷ 3 = ',⍕(10-4)÷3
⎕GTK 'append 2 * 10 = ',⍕2*10
⎕GTK 'append '

⍝ ⍎ Execute demo
⎕GTK 'append Execute (⍎) - evaluate strings as APL:'
⎕GTK 'append ⍎''6 × 7'' = ',⍕⍎'6 × 7'
⎕GTK 'append ⍎''2 + 3'' = ',⍕⍎'2 + 3'
⎕GTK 'append ⍎''⍳10'' = ',⍕⍎'⍳10'
⎕GTK 'append '

⍝ → Branch demo
⎕GTK 'append Branch (→) - conditional jumps:'
⎕GTK 'append →(1=0)/0 → 1+1 = ',⍕((1=0)≠1)+1+1
⎕GTK 'append '

⍝ Recursive factorial
∇r ← fact n
→(n≤0)/4
r ← n × fact n-1
→5
r ← 1
∇

⎕GTK 'append Recursive factorial:'
⎕GTK 'append fact 0 = ',⍕fact 0
⎕GTK 'append fact 5 = ',⍕fact 5
⎕GTK 'append fact 10 = ',⍕fact 10
⎕GTK 'append '

⍝ Calculator buttons
⎕GTK 'append ─────────────────────────────────'
⎕GTK 'append Calculator buttons:'
⎕GTK 'append Click buttons in the GTK window'

⍝ ⎕GTK.WAIT blocks until all GTK windows are closed
⎕GTK.WAIT
