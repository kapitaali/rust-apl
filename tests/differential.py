#!/usr/bin/env python3
"""Differential-test the Rust APL against the reference C++ GNU APL binary.

Each expression is run in its OWN interpreter invocation, so an empty result
or multi-line output can never misalign the comparison (batching them did).
Both sides get an explicit `⎕IO←1` first, because the Rust Environment
defaults to ⎕IO=0 while GNU APL defaults to 1.

Matrices are raveled with `,` in the case list and rank probed separately via
`≢⍴E`, so boxed-display differences are never what we're comparing.
"""
import subprocess, sys, os

HOME = os.path.expanduser("~")
REF = f"{HOME}/Apps/apl-2.0/src/apl"
RUST = f"{HOME}/Apps/apl-2.0/rust-apl/target/release/apl"

CASES = [
    ("⍸", "⍸0 1 0 1 1"),
    ("⍸", "⍸0 0 0"),
    ("⍸", "⍸1 1 1"),
    ("⍸", "⍸3<1 5 2 7"),
    ("⍸", "≢⍸0 1 0 1"),
    ("⍸", ",⍸2 2⍴0 1 1 0"),

    ("⍷", "1 2⍷1 2 3 1 2"),
    ("⍷", "'ab'⍷'xabyab'"),
    ("⍷", "+/'the'⍷'the cat and the dog'"),
    ("⍷", "2⍷1 2 3 2"),
    ("⍷", "1 1⍷1 1 1"),
    ("⍷", "9 9⍷1 2 3"),
    ("⍷", "1 2 3 4⍷1 2"),
    ("⍷", ",3 4⍷2 3⍴1 2 3 3 4 5"),
    ("⍷", "≢⍴3 4⍷2 3⍴1 2 3 3 4 5"),
    ("⍷", "⍸1 2⍷1 2 3 1 2"),

    ("⌷", "2⌷10 20 30"),
    ("⌷", "1⌷10 20 30"),
    ("⌷", "3⌷10 20 30"),
    ("⌷", "2 3⌷3 4⍴⍳12"),
    ("⌷", "1 1⌷2 3⍴⍳6"),
    ("⌷", "2 2⌷2 3⍴⍳6"),

    ("⊖", ",⊖2 3⍴⍳6"),
    ("⊖", "≢⍴⊖2 3⍴⍳6"),
    ("⊖", ",1⊖2 3⍴⍳6"),
    ("⊖", ",2⊖2 3⍴⍳6"),
    ("⊖", ",¯1⊖2 3⍴⍳6"),
    ("⊖", "⊖1 2 3"),
    ("⊖", "1⊖1 2 3"),
    ("⊖", ",⊖3 2⍴⍳6"),

    ("⍕", "⍕42"),
    ("⍕", "⍕1 2 3"),
    ("⍕", "≢⍕1 2 3"),
    ("⍕", "≢⍕42"),
    ("⍕", "⍕¯5"),
    ("⍕", "≢⍴⍕2 2⍴⍳4"),

    ("⍕d", "2⍕1.5"),
    ("⍕d", "2⍕1.5 2.25"),
    ("⍕d", "6 2⍕1.5"),
    ("⍕d", "0⍕3.7"),
    ("⍕d", "1⍕¯2.5"),
    ("⍕d", "≢6 2⍕1.5"),

    ("⍎", "⍎'2+3'"),
    ("⍎", "⍎'1 2 3'"),
    ("⍎", "⍎⍕42"),
    ("⍎", ",⍎'2 3⍴⍳6'"),
    ("⍎", "⍎'⍳4'"),
    ("⍎", "1+⍎'2'"),

    # partition, already fixed — kept as a regression guard
    ("⊂", "≢1 1 2 2⊂'abcd'"),
    ("⊂", "≢2 2 1 1⊂'abcd'"),
    ("⊂", "≢1 1 2 2 1⊂'abcde'"),
]

NOISE = ("GNU APL", "Enter APL", "end-of-input", "end of input", "Goodbye")


def run_one(binary, expr, is_ref):
    """Run a single expression, return its output text (may be '' or an error)."""
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
    # collapse whitespace; the reference pads numeric columns differently
    return " ".join(s.split())


def main():
    only = sys.argv[1] if len(sys.argv) > 1 else None
    cases = [c for c in CASES if only is None or c[0] == only]

    bad, errs = [], []
    for prim, expr in cases:
        r = run_one(REF, expr, True)
        u = run_one(RUST, expr, False)
        if "ERROR" in u and "ERROR" not in r:
            errs.append((prim, expr, r, u))
        elif norm(r) != norm(u):
            bad.append((prim, expr, r, u))

    total = len(cases)
    print(f"{total} cases: {total - len(bad) - len(errs)} agree, "
          f"{len(bad)} differ, {len(errs)} rust-only errors\n")

    if errs:
        print("=== RUST ERRORS (reference succeeds) ===")
        print(f"{'prim':5} {'expression':32} {'reference':20} rust")
        print("-" * 92)
        for prim, expr, r, u in errs:
            print(f"{prim:5} {expr:32} {r[:18]:20} {u[:30]}")
        print()

    if bad:
        print("=== MISMATCHES ===")
        print(f"{'prim':5} {'expression':32} {'reference':20} rust")
        print("-" * 92)
        for prim, expr, r, u in bad:
            print(f"{prim:5} {expr:32} {r[:18]:20} {u[:30]}")
        print()

    if not bad and not errs:
        print("All cases agree with the reference implementation.")
    return 1 if (bad or errs) else 0


if __name__ == "__main__":
    sys.exit(main())
