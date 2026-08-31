⍝ APL Calculator Demo — uses ⎕GTK for GUI display
⍝
⍝ Run with: cargo run --release --features plugin-gtk < examples/calc-demo.apl
⍝
⍝ This demo shows apl + GTK integration with:
⍝ - ⎕GTK GUI window with calculator buttons
⍝ - ⎕GTK.WAIT (keeps window open until user closes it)
⍝ - ⍎ execute (evaluate strings as APL)
⍝ - → branch (conditional jumps)
⍝ - Recursive functions

⍝ ──────────────────────────────────────────
⍝ Static demo (all features working)
⍝ ──────────────────────────────────────────

⎕GTK 'text ╔═══════════════════════════════╗'
⎕GTK 'append ║    APL Calculator (GTK)       ║'
⎕GTK 'append ╠═══════════════════════════════╣'
⎕GTK 'append ║  APL + GTK4 Integration Demo  ║'
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
⎕GTK 'append →(1=0)/skip → 1+1 = ',⍕(→(1=0)/0) + 1+1
⎕GTK 'append →(1=1)/skip → 2+2 = ',⍕(→(1=1)/0) + 2+2
⎕GTK 'append '

⍝ Recursive factorial (using line numbers)
∇r ← fact n
→(n≤0)/4
r ← n × fact n-1
→5
r ← 1
∇

⎕GTK 'append Recursive factorial:'
⎕GTK 'append fact 0 = ',⍕fact 0
⎕GTK 'append fact 1 = ',⍕fact 1
⎕GTK 'append fact 5 = ',⍕fact 5
⎕GTK 'append fact 10 = ',⍕fact 10
⎕GTK 'append '

⍝ Calculator buttons
⎕GTK 'append ─────────────────────────────────'
⎕GTK 'append Calculator buttons:'
⎕GTK 'append Click buttons in the GTK window'
⎕GTK 'append or type in the entry field'

⍝ ⎕GTK.WAIT blocks until all GTK windows are closed
⎕GTK.WAIT
