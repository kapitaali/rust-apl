⍝ APL Calculator Demo — uses ⎕GTK for GUI display
⍝
⍝ Run with: cargo run --release --features plugin-gtk < examples/calc-demo.apl
⍝
⍝ This demo shows the ⎕GTK system displaying calculation results.
⍝ APL arithmetic: + add, - subtract, × multiply, ÷ divide, * power

⍝ Display header
⎕GTK 'text APL Calculator Demo'
⎕GTK 'text ===================='
⎕GTK 'text '
⎕GTK 'text Demo calculations:'

⍝ Show calculations
⎕GTK 'text 2 + 3 × 4 → ',⍕2+3×4
⎕GTK 'text (10 - 4) ÷ 3 → ',⍕(10-4)÷3
⎕GTK 'text 2 * 10 → ',⍕2*10
⎕GTK 'text 20 + 30 × 2 → ',⍕20+30×2
⎕GTK 'text '

⍝ Show table of common operations
⎕GTK 'text Common operations:'
⎕GTK 'text 5 + 5 → ',⍕5+5
⎕GTK 'text 100 ÷ 4 → ',⍕100÷4
⎕GTK 'text 7 × 8 → ',⍕7×8
⎕GTK 'text 3 * 4 → ',⍕3*4
⎕GTK 'text '

⍝ Footer
⎕GTK 'text (window will close shortly)'
⎕GTK 'close'
