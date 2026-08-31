⍝ APL Calculator Demo — uses ⎕GTK for GUI display
⍝
⍝ Run with: cargo run --release --features plugin-gtk < examples/calc-demo.apl
⍝
⍝ This demo shows apl + GTK integration with:
⍝ - ⎕GTK GUI window
⍝ - ⍎ execute (evaluate strings as APL)
⍝ - → branch (conditional jumps)
⍝ - Recursive functions

⍝ ──────────────────────────────────────────
⍝ Static demo (all features working)
⍝ ──────────────────────────────────────────

⎕GTK 'text ╔═══════════════════════════════╗'
⎕GTK 'text ║    APL Calculator (GTK)       ║'
⎕GTK 'text ╠═══════════════════════════════╣'
⎕GTK 'text ║  APL + GTK4 Integration Demo  ║'
⎕GTK 'text ╚═══════════════════════════════╝'
⎕GTK 'text '

⍝ Basic arithmetic
⎕GTK 'text Basic arithmetic:'
⎕GTK 'text 2 + 3 × 4 = ',⍕2+3×4
⎕GTK 'text (10 - 4) ÷ 3 = ',⍕(10-4)÷3
⎕GTK 'text 2 * 10 = ',⍕2*10
⎕GTK 'text '

⍝ ⍎ Execute demo
⎕GTK 'text Execute (⍎) - evaluate strings as APL:'
⎕GTK 'text ⍎''6 × 7'' = ',⍕⍎'6 × 7'
⎕GTK 'text ⍎''2 + 3'' = ',⍕⍎'2 + 3'
⎕GTK 'text ⍎''⍳10'' = ',⍕⍎'⍳10'
⎕GTK 'text '

⍝ → Branch demo
⎕GTK 'text Branch (→) - conditional jumps:'
⎕GTK 'text →(1=0)/skip → 1+1 = ',⍕(→(1=0)/0) + 1+1
⎕GTK 'text →(1=1)/skip → 2+2 = ',⍕(→(1=1)/0) + 2+2
⎕GTK 'text '

⍝ Recursive factorial (using line numbers)
∇r ← fact n
→(n≤0)/4
r ← n × fact n-1
→5
r ← 1
∇

⎕GTK 'text Recursive factorial:'
⎕GTK 'text fact 0 = ',⍕fact 0
⎕GTK 'text fact 1 = ',⍕fact 1
⎕GTK 'text fact 5 = ',⍕fact 5
⎕GTK 'text fact 10 = ',⍕fact 10
⎕GTK 'text '

⍝ Close
⎕GTK 'text ─────────────────────────────────'
⎕GTK 'text Close window with ⎕GTK ''close'''
⎕GTK 'close'
