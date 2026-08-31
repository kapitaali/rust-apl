⍝ Interactive APL Calculator with GTK GUI
⍝
⍝ Run with: cargo run --release --features plugin-gtk < examples/calc-demo.apl
⍝
⍝ Click buttons to enter an expression in the entry field.
⍝ Click Compute to evaluate using ⍎ (execute).
⍝ Close window to exit.

∇run_calc
  ⎕GTK 'text ╔═══════════════════════════════╗'
  ⎕GTK 'append ║    APL Calculator (GTK)       ║'
  ⎕GTK 'append ╠═══════════════════════════════╣'
  ⎕GTK 'append ║  Click buttons to enter expr  ║'
  ⎕GTK 'append ║  Click Compute to evaluate    ║'
  ⎕GTK 'append ╚═══════════════════════════════╝'
  ⎕GTK 'append '

  ⍝ Static demo content
  ⎕GTK 'append Basic arithmetic:'
  ⎕GTK 'append 2 + 3 × 4 = ',⍕2+3×4
  ⎕GTK 'append (10 - 4) ÷ 3 = ',⍕(10-4)÷3
  ⎕GTK 'append 2 * 10 = ',⍕2*10
  ⎕GTK 'append '

  ⎕GTK 'append Calculator buttons:'
  ⎕GTK 'append - Click 0-9, +, -, *, / to enter'
  ⎕GTK 'append - Click Compute to calculate'
  ⎕GTK 'append - Close window to exit'
  ⎕GTK 'append '

  ⎕GTK.WAIT
∇

run_calc
