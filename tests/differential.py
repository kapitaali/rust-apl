#!/usr/bin/env python3

"""Differential-test the Rust APL against the reference C++ GNU APL binary.

    python3 tests/differential.py            # every case
    python3 tests/differential.py ⍷ ⌽        # only the named primitives
    python3 tests/differential.py --list     # show primitive tags

Each expression runs in its OWN interpreter invocation, so an empty result or
multi-line output can never misalign the comparison (batching them did, and
cost an hour). Both sides get an explicit `⎕IO←1` first, because the Rust
Environment defaults to ⎕IO=0 while GNU APL defaults to 1 — without pinning
it, every index-returning primitive falsely mismatches.

Probe expressions prefer `,E` (ravel to one line) and `≢⍴E` (rank) so that
boxed-display differences are never what is being compared. NOTE: ravel
itself was broken once, which made correct primitives look wrong — when a
whole primitive's cases fail, sanity-check the PROBE before hunting the
primitive.
"""
import subprocess, sys, os
from collections import OrderedDict

HOME = os.path.expanduser("~")
REF = f"{HOME}/Apps/apl-2.0/src/apl"
RUST = f"{HOME}/Apps/apl-2.0/rust-apl/target/release/apl"

CASES = [
    # ══ previously verified (regression guards) ═══════════════════════════
    ("⍸", "⍸0 1 0 1 1"),
    ("⍸", "⍸0 0 0"),
    ("⍸", "⍸3<1 5 2 7"),
    ("⍷", "1 2⍷1 2 3 1 2"),
    ("⍷", "'ab'⍷'xabyab'"),
    ("⍷", "+/'the'⍷'the cat and the dog'"),
    ("⌷", "2⌷10 20 30"),
    ("⌷", "2 3⌷3 4⍴⍳12"),
    ("⊖", ",⊖2 3⍴⍳6"),
    ("⊖", ",1⊖2 3⍴⍳6"),
    ("⍕", "⍕42"),
    ("⍕", "⍕¯5"),
    ("⍕", "2⍕1.5"),
    ("⍎", "⍎'2+3'"),
    ("⍎", "⍎⍕42"),
    ("⊂", "≢2 2 1 1⊂'abcd'"),
    (",", "≢,2 3⍴⍳6"),
    (",", "≢⍴,2 3⍴⍳6"),
    ("empty", "≢(0⍴0)+1"),

    # ══ NEW: arithmetic / math ════════════════════════════════════════════
    ("⍟", "⍟1"),
    ("⍟", "2⍟8"),
    ("⍟", "10⍟100"),
    ("⍟", "⌊0.5+⍟2.718281828"),
    ("!", "!5"),
    ("!", "2!5"),
    ("○", "⌊1000×○1"),
    ("|", "3|10"),
    ("|", "|¯7"),
    ("|", "¯3|10"),
    ("*", "2*10"),
    ("*", "⌊*1"),
    ("⌈", "⌈2.3"),
    ("⌈", "⌈¯2.3"),
    ("⌈", "3⌈5"),
    ("⌊", "⌊2.7"),
    ("⌊", "⌊¯2.7"),
    ("⌊", "3⌊5"),
    ("÷", "÷4"),
    ("÷", "10÷4"),

    # ══ NEW: set operations ═══════════════════════════════════════════════
    ("∼", "1 2 3 4∼2 4"),
    ("∼", "≢1 2 3∼1 2 3"),
    ("∪", "∪1 2 1 3 2"),
    ("∪", "1 2 3∪3 4 5"),
    ("∩", "1 2 3 4∩2 4 6"),
    ("∩", "≢1 2∩3 4"),
    ("∊", "∊2 2⍴⍳4"),
    ("∈", "2∈1 2 3"),
    ("∈", "5∈1 2 3"),
    ("∈", "1 5∈1 2 3"),

    # ══ NEW: structural ══════════════════════════════════════════════════
    ("⍴", "⍴2 3⍴⍳6"),
    ("⍴", "≢⍴42"),
    ("⌽", "⌽1 2 3"),
    ("⌽", "1⌽1 2 3"),
    ("⌽", "¯1⌽1 2 3"),
    ("⌽", ",⌽2 3⍴⍳6"),
    ("⍉", ",⍉2 3⍴⍳6"),
    ("⍉", "⍴⍉2 3⍴⍳6"),
    ("↑", "3↑1 2 3 4 5"),
    ("↑", "¯2↑1 2 3 4 5"),
    ("↑", "7↑1 2 3"),
    ("↓", "2↓1 2 3 4 5"),
    ("↓", "¯2↓1 2 3 4 5"),
    ("↓", "≢9↓1 2 3"),
    ("⍪", ",⍪1 2 3"),
    ("⍪", "⍴⍪1 2 3"),
    ("⍪", "1 2⍪3 4"),
    ("≢", "≢1 2 3"),
    ("≢", "≢42"),
    ("≢", "1 2 3≢1 2 3"),
    ("≡", "≡1 2 3"),
    ("≡", "1 2 3≡1 2 3"),
    ("≡", "1 2 3≡1 2 4"),

    # ══ NEW: encode / decode ══════════════════════════════════════════════
    ("⊤", "2 2 2⊤5"),
    ("⊤", "10 10⊤42"),
    ("⊥", "2⊥1 0 1"),
    ("⊥", "10⊥1 2 3"),
    ("⊥", "24 60 60⊥1 2 3"),

    # ══ NEW: grade / sort ═════════════════════════════════════════════════
    ("⍋", "⍋3 1 2"),
    ("⍋", "⍋1 2 3"),
    ("⍒", "⍒3 1 2"),
    ("⍒", "⍒1 2 3"),

    # ══ NEW: logical ══════════════════════════════════════════════════════
    ("∧", "1∧1"),
    ("∧", "1∧0"),
    ("∨", "1∨0"),
    ("∨", "0∨0"),
    ("⍲", "0⍲0"),
    ("⍲", "1⍲1"),
    ("⍱", "0⍱0"),
    ("⍱", "1⍱0"),
    ("~", "~1"),
    ("~", "~0 1 0"),

    # ══ NEW: comparison ═══════════════════════════════════════════════════
    ("<", "3<5"),
    ("≤", "3≤3"),
    ("=", "3=3"),
    ("≥", "3≥5"),
    (">", "5>3"),
    ("≠", "3≠3"),

    # ══ NEW: identity ═════════════════════════════════════════════════════
    ("⊣", "1⊣2"),
    ("⊢", "1⊢2"),

    # ══ NEW: enclose / disclose ═══════════════════════════════════════════
    ("⊃", "⊃1 2 3"),
    ("⊃", "≡⊂1 2 3"),
    ("⊂", "≢⊂1 2 3"),

    # ══ NEW: operators (reduce / scan / each) ═════════════════════════════
    ("/", "+/1 2 3 4"),
    ("/", "×/1 2 3 4"),
    ("/", "+/⍳10"),
    ("/", "⌈/3 1 4 1 5"),
    ("/", "-/1 2 3"),
    ("\\", "+\\1 2 3 4"),
    ("\\", "×\\1 2 3 4"),
    ("⌿", ",+⌿2 3⍴⍳6"),
    ("∘.", ",1 2∘.×1 2 3"),
    ("∘.", "⍴1 2∘.×1 2 3"),
    ("f.g", "1 2 3+.×1 2 3"),

    # ══ NEW: replicate / expand ═══════════════════════════════════════════
    ("rep", "2 3/1 2"),
    ("rep", "1 0 1/1 2 3"),

    # ══ NEW: iota / index-of ══════════════════════════════════════════════
    ("⍳", "⍳5"),
    ("⍳", "≢⍳0"),
    ("⍳", "1 2 3⍳2"),
    ("⍳", "1 2 3⍳9"),

    # ══ NEW: index origin sensitivity ═════════════════════════════════════
    ("io", "⍳3"),
    ("io", "⍋3 1 2"),
    ("io", "1 2 3⍳3"),

    # ══ NEW: higher-rank / matrix forms ═══════════════════════════════════
    ("⌽r2", ",⌽2 3⍴⍳6"),
    ("⌽r2", "⍴⌽2 3⍴⍳6"),
    ("⌽r2", ",1⌽2 3⍴⍳6"),
    ("⌽r2", ",¯1⌽2 3⍴⍳6"),
    ("⌽r2", ",⌽3 2⍴⍳6"),
    ("⌽r3", ",⌽2 2 2⍴⍳8"),
    ("⌽r3", "⍴⌽2 2 2⍴⍳8"),

    ("⊖r2", ",⊖2 3⍴⍳6"),
    ("⊖r2", "⍴⊖2 3⍴⍳6"),
    ("⊖r2", ",1⊖2 3⍴⍳6"),
    ("⊖r2", ",¯1⊖2 3⍴⍳6"),
    ("⊖r2", ",⊖3 2⍴⍳6"),
    ("⊖r3", ",⊖2 2 2⍴⍳8"),

    ("⍉r2", ",⍉2 3⍴⍳6"),
    ("⍉r2", "⍴⍉2 3⍴⍳6"),
    ("⍉r2", ",⍉3 2⍴⍳6"),
    ("⍉r2", ",⍉⍉2 3⍴⍳6"),
    ("⍉r3", "⍴⍉2 3 4⍴⍳24"),
    ("⍉r3", ",⍉2 2 2⍴⍳8"),
    ("⍉dy", ",1 2⍉2 3⍴⍳6"),
    ("⍉dy", ",2 1⍉2 3⍴⍳6"),
    ("⍉dy", "⍴2 1⍉2 3⍴⍳6"),
    ("⍉dy", "⍴1 1⍉3 3⍴⍳9"),
    ("⍉dy", ",1 1⍉3 3⍴⍳9"),

    ("↑r2", ",1↑2 3⍴⍳6"),
    ("↑r2", "⍴1↑2 3⍴⍳6"),
    ("↑r2", ",1 2↑2 3⍴⍳6"),
    ("↑r2", "⍴1 2↑2 3⍴⍳6"),
    ("↑r2", ",¯1 ¯2↑2 3⍴⍳6"),
    ("↑r2", "⍴3 4↑2 3⍴⍳6"),
    ("↑r2", ",3 4↑2 3⍴⍳6"),

    ("↓r2", ",1↓2 3⍴⍳6"),
    ("↓r2", "⍴1↓2 3⍴⍳6"),
    ("↓r2", ",1 1↓2 3⍴⍳6"),
    ("↓r2", "⍴1 1↓2 3⍴⍳6"),
    ("↓r2", ",¯1 ¯1↓2 3⍴⍳6"),
    ("↓r2", "⍴5 5↓2 3⍴⍳6"),

    ("⍪r2", ",2 3⍴⍳6"),
    ("⍪r2", "⍴⍪2 3⍴⍳6"),
    ("⍪r2", ",(2 3⍴⍳6)⍪2 3⍴⍳6"),
    ("⍪r2", "⍴(2 3⍴⍳6)⍪2 3⍴⍳6"),

    (",r2", ",(2 3⍴⍳6),2 3⍴⍳6"),
    (",r2", "⍴(2 3⍴⍳6),2 3⍴⍳6"),
    (",r2", "⍴(2 3⍴⍳6),2 1⍴0 0"),

    ("≢r2", "≢2 3⍴⍳6"),
    ("≢r2", "≢3 2⍴⍳6"),
    ("≢r2", "≢2 2 2⍴⍳8"),
    ("≡r2", "≡2 3⍴⍳6"),
    ("≡r2", "(2 3⍴⍳6)≡2 3⍴⍳6"),
    ("≡r2", "(2 3⍴⍳6)≡3 2⍴⍳6"),

    ("⍴r2", "⍴2 3⍴⍳6"),
    ("⍴r2", "⍴2 2 2⍴⍳8"),
    ("⍴r2", "≢⍴2 2 2⍴⍳8"),
    ("⍴r2", ",3 3⍴⍳4"),

    ("/r2", ",+/2 3⍴⍳6"),
    ("/r2", "⍴+/2 3⍴⍳6"),
    ("/r2", ",+⌿2 3⍴⍳6"),
    ("/r2", "⍴+⌿2 3⍴⍳6"),
    ("/r2", ",×/2 3⍴⍳6"),
    ("/r2", ",⌈/2 3⍴⍳6"),
    ("\\r2", ",+\\2 3⍴⍳6"),
    ("\\r2", "⍴+\\2 3⍴⍳6"),

    ("sf r2", ",1+2 3⍴⍳6"),
    ("sf r2", "⍴1+2 3⍴⍳6"),
    ("sf r2", ",(2 3⍴⍳6)×2 3⍴⍳6"),
    ("sf r2", ",-2 3⍴⍳6"),
    ("⍋r2", "⍋3 2⍴1 2 0 1 2 2"),

    ("⌷r2", "1 1⌷2 3⍴⍳6"),
    ("⌷r2", "2 3⌷2 3⍴⍳6"),
    ("idx", ",(2 3⍴⍳6)[1;]"),
    ("idx", ",(2 3⍴⍳6)[;1]"),
    ("idx", "(2 3⍴⍳6)[1;1]"),
    ("idx", "(2 3⍴⍳6)[2;3]"),
    ("idx", "⍴(2 3⍴⍳6)[1;1]"),
    ("idx", "⍴(2 3⍴⍳6)[1;]"),
    ("idx", "⍴(2 3⍴⍳6)[;1]"),
    ("idx", "⍴(2 3⍴⍳6)[1 2;1 2]"),
    ("idx", ",(2 3⍴⍳6)[1 2;1 2]"),
    ("idx", "⍴(2 3⍴⍳6)[1 1;]"),
    ("idx", ",(2 3⍴⍳6)[1 1;]"),
    ("idx", ",(2 3⍴⍳6)[;3 1]"),
    ("idx", "(10 20 30)[2]"),
    ("idx", ",(10 20 30)[1 3]"),
    ("idx", ",(2 2 2⍴⍳8)[1;;]"),
    ("idx", "⍴(2 2 2⍴⍳8)[1;;]"),
    ("idx", "(2 2 2⍴⍳8)[1;2;1]"),

    ("∊r2", "∊2 3⍴⍳6"),
    ("∊r2", "≢∊2 2 2⍴⍳8"),

    ("⊤r2", ",2 2 2⊤5 3"),
    ("⊤r2", "⍴2 2 2⊤5 3"),
    ("⊥r2", "2⊥2 3⍴1 0 1 1 1 0"),

    # ══ NEW: selective assignment (reads back the mutated variable) ═══════
    ("asgn", "V←10 20 30 ⋄ V[2]←99 ⋄ V"),
    ("asgn", "V←10 20 30 ⋄ V[1 3]←7 8 ⋄ V"),
    ("asgn", "V←10 20 30 ⋄ V[1 3]←0 ⋄ V"),
    ("asgn", "M←2 3⍴⍳6 ⋄ M[1;2]←99 ⋄ ,M"),
    ("asgn", "M←2 3⍴⍳6 ⋄ M[1;]←0 0 0 ⋄ ,M"),
    ("asgn", "M←2 3⍴⍳6 ⋄ M[;1]←7 7 ⋄ ,M"),
    ("asgn", "M←2 3⍴⍳6 ⋄ M[1 2;1 2]←100 ⋄ ,M"),
    ("asgn", "M←2 3⍴⍳6 ⋄ M[2;3]←0 ⋄ ,M"),
    ("asgn", "M←2 3⍴⍳6 ⋄ M[;]←0 ⋄ ,M"),
    ("asgn", "M←2 3⍴⍳6 ⋄ M[1;2]←99 ⋄ ⍴M"),
    ("asgn", "C←2 2 2⍴⍳8 ⋄ C[1;2;1]←0 ⋄ ,C"),
    ("asgn", "C←2 2 2⍴⍳8 ⋄ C[1;;]←0 ⋄ ,C"),

    # ══ NEW: rank operator ⍤ ══════════════════════════════════════════════
    ("⍤", ",(⌽⍤1)2 3⍴⍳6"),
    ("⍤", "⍴(⌽⍤1)2 3⍴⍳6"),
    ("⍤", ",(⌽⍤2)2 3⍴⍳6"),
    ("⍤", ",(⌽⍤3)2 3⍴⍳6"),
    ("⍤", ",(≢⍤1)2 3⍴⍳6"),
    ("⍤", "(≢⍤2)2 3⍴⍳6"),
    ("⍤", "⍴(≢⍤1)2 3⍴⍳6"),
    ("⍤", ",(⌽⍤1)1 2 3"),
    ("⍤", ",(⌽⍤1)2 2 2⍴⍳8"),
    ("⍤", ",(≢⍤1)2 2 2⍴⍳8"),
    ("⍤", "⍴(≢⍤1)2 2 2⍴⍳8"),
    ("⍤", ",(+⍤1)2 3⍴⍳6"),
    ("⍤", ",(-⍤1)2 3⍴⍳6"),
    ("⍣", "double←{2×⍵} ⋄ double⍣3 5"),
    ("⍣", "square←{⍵×⍵} ⋄ square⍣2 3"),
    ("∘", "(2 2⍴1 2 3 4)∘(2 2⍴5 6 7 8)"),
    ("⍤", ",(⍳⍤0)3"),

    # ══ NEW: dyadic rank with separate left/right ranks ═════════════════
    ("⍤2", ",1 2 3(,⍤0 1)4 5 6"),
    ("⍤2", "⍴1 2 3(,⍤0 1)4 5 6"),
    ("⍤2", ",1 2 3(,⍤1 0)4 5 6"),
    ("⍤2", "⍴1 2 3(,⍤1 0)4 5 6"),
    ("⍤2", ",1 2 3(,⍤0 0)4 5 6"),
    ("⍤2", "⍴1 2 3(,⍤0 0)4 5 6"),
    ("⍤2", ",(,⍤1 0)2 3⍴⍳6"),
    ("⍤2", "⍴(,⍤1 0)2 3⍴⍳6"),
    ("⍤2", ",(2 2⍴1 2 3 4)(,⍤1 1)2 2⍴5 6 7 8"),
    ("⍤2", "⍴(2 2⍴1 2 3 4)(,⍤1 1)2 2⍴5 6 7 8"),

    # ══ NEW: selective assignment through selectors ════════════════════
    ("sel", "V←1 2 3 4 5 ⋄ (2↑V)←9 8 ⋄ V"),
    ("sel", "V←1 2 3 4 5 ⋄ (2↓V)←9 9 9 ⋄ V"),
    ("sel", "V←1 2 3 4 5 ⋄ (3↓V)←99 99 ⋄ V"),
    ("sel", "V←1 2 3 4 5 ⋄ (3⌽V)←10 20 30 40 50 ⋄ V"),
    ("sel", "V←1 2 3 4 5 ⋄ (⌽V)←10 20 30 40 50 ⋄ V"),
    ("sel", "V←1 2 3 4 5 ⋄ (3⍴V)←99 99 99 ⋄ V"),
    ("sel", "V←1 2 3 4 5 ⋄ (2⍴V)←99 99 ⋄ V"),
    ("sel", "M←2 3⍴⍳6 ⋄ (2 3↑M)←99 ⋄ M"),
    ("sel", "M←2 3⍴⍳6 ⋄ (2 3↓M)←99 ⋄ M"),
    ("sel", "M←2 3⍴⍳6 ⋄ (2 3⍴M)←100 ⋄ M"),
    ("sel", "V←1 2 3 4 5 ⋄ (5↑V)←10 20 30 40 50 ⋄ V"),
    ("sel", "V←1 2 3 4 5 ⋄ (6↑V)←10 20 30 40 50 60 ⋄ V"),
    ("sel", "M←2 3⍴⍳6 ⋄ (1 2⌷M)←99 ⋄ M"),
    ("sel", "M←2 3⍴⍳6 ⋄ (2 3⌷M)←88 ⋄ M"),
    ("sel", "M←2 3⍴⍳6 ⋄ (1 2⌷M)←99 ⋄ (2 3⌷M)←88 ⋄ M"),

    # ══ NEW: display parity (nested arrays print boxed by default) ════
    ("disp", "N←(1 2)(3 4 5) ⋄ ≢⍴N"),
    ("disp", "N←⊂(1 2)(3 4 5) ⋄ ≢⍴N"),
    # 4⎕CR boxed display
    ("cr", "⍴4⎕CR(1 2)(3 4 5)"),
    ("cr", ",4⎕CR 1 2 3"),
    ("cr", "⍴4⎕CR⊂(1 2)(3 4 5)"),

    # ══ NEW: 1⎕CR (ravel) ═════════════════════════════════════════════════
    ("cr1", "1⎕CR(1 2)(3 4 5)"),
    ("cr1", "1⎕CR 2 3⍴⍳6"),

    # ══ NEW: zilde ⍬ ═══════════════════════════════════════════════════════
    ("zilde", "⍴⍬"),
    ("zilde", "≢⍬"),
    ("zilde", "⍬≡0⍴0"),
    ("zilde", "1+⍬"),

    # ══ NEW: power operator ⍣ ═════════════════════════════════════════════
    ("⍣", "×⍣3 5"),
    ("⍣", "×⍣0 5"),
    ("⍣", "×⍣1 5"),

    # ══ NEW: complex numbers ═════════════════════════════════════════════
    ("J", "1J2+2J3"),
    ("J", "1J2×2J3"),
    ("J", "9○1J2"),
    ("J", "11○1J2"),

    # ══ NEW: circle function (hyperbolic + inverse trig) ═══════════════════
    ("○", "1○0.5"),
    ("○", "2○0.5"),
    ("○", "3○0.5"),
    ("○", "4○0.5"),
    ("○", "5○0.5"),
    ("○", "6○0.5"),
    ("○", "7○0.5"),
    ("○", "¯1○0.5"),
    ("○", "¯3○0.5"),
    ("○", "¯4○0.5"),
    ("○", "¯5○0.5"),
    ("○", "¯6○0.5"),
    ("○", "¯7○0.5"),
    ("⍸dy", "1 2 3⍸0.5 1.5 2.5 3.5"),
    ("⍸dy", "1 2 3⍸1 2 3"),
    # ══ NEW: nested strand with mixed simple/nested items ═══════════════
    ("nstr", "≢,(1 2)(3 (4 5))"),
    ("nstr", "⍴(1 2)(3 (4 5))"),
    ("nstr", "≡(1 2)(3 (4 5))"),
]

NOISE = ("GNU APL", "Enter APL", "end-of-input", "end of input", "Goodbye")


def run_one(binary, expr, is_ref):
    """Run one expression; return its output text ('<empty>' if none)."""
    script = f"⎕IO←1\n{expr}\n)OFF\n"
    try:
        p = subprocess.run(
            [binary, "--script"] if is_ref else [binary],
            input=script, capture_output=True, text=True, timeout=30,
        )
    except subprocess.TimeoutExpired:
        return "<TIMEOUT>"
    out = []
    for raw in p.stdout.split("\n"):
        s = raw.strip()
        if not s or any(n in s for n in NOISE) or s.startswith("***"):
            continue
        out.append(s)
    return " | ".join(out) if out else "<empty>"


def norm(s):
    return " ".join(s.split())


def is_error(s):
    """True if the output reports an APL error (either dialect's wording)."""
    u = s.upper()
    return "ERROR" in u


def same(ref, rust):
    """Agreement test.

    Both sides erroring counts as agreement even when the surrounding text
    differs: GNU APL echoes the offending line with a caret, while this REPL
    prints a bare `ERROR: LENGTH ERROR`. What matters is that both rejected
    the expression, and with the same error CLASS when we can tell.
    """
    if is_error(ref) and is_error(rust):
        for kind in ("LENGTH", "RANK", "DOMAIN", "SYNTAX", "INDEX", "VALUE"):
            in_ref, in_rust = kind in ref.upper(), kind in rust.upper()
            if in_ref != in_rust:
                return False
        return True
    return norm(ref) == norm(rust)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if "--list" in sys.argv:
        tags = OrderedDict((p, 0) for p, _ in CASES)
        for p, _ in CASES:
            tags[p] += 1
        print(" ".join(f"{t}({n})" for t, n in tags.items()))
        return 0

    cases = [c for c in CASES if not args or c[0] in args]
    print(f"running {len(cases)} cases against the reference...\n")

    bad, errs = [], []
    for prim, expr in cases:
        r = run_one(REF, expr, True)
        u = run_one(RUST, expr, False)
        if is_error(u) and not is_error(r):
            errs.append((prim, expr, r, u))
        elif not same(r, u):
            bad.append((prim, expr, r, u))

    total = len(cases)
    print(f"{total} cases: {total - len(bad) - len(errs)} agree, "
          f"{len(bad)} differ, {len(errs)} rust-only errors\n")

    def table(title, rows):
        print(f"=== {title} ===")
        print(f"{'prim':6} {'expression':30} {'reference':22} rust")
        print("-" * 96)
        for prim, expr, r, u in rows:
            print(f"{prim:6} {expr:30} {r[:20]:22} {u[:28]}")
        print()

    if errs:
        table("RUST ERRORS (reference succeeds)", errs)
    if bad:
        table("MISMATCHES", bad)
    if not bad and not errs:
        print("All cases agree with the reference implementation.")
    return 1 if (bad or errs) else 0


if __name__ == "__main__":
    sys.exit(main())
