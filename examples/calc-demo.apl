⍝ APL GTK Calculator Demo
⍝ Event loop that reads ⎕GTKEvent and evaluates expressions

⎕GTK 'calculator'

∇CALCLOOP
  :While 1
    evt ← ⎕GTKEvent
    :If evt≡'WindowClosed'
      →0
    :EndIf
    :If evt≡'Compute'
      expr ← ⎕GTK 'getentry'
      :If 0=≢expr
        ⎕GTK 'append Enter an expression'
      :Else
        result ← ⍎expr
        ⎕GTK 'append ',⍕result
      :EndIf
    :EndIf
  :EndWhile
∇

CALCLOOP
