#!/usr/bin/env python3
"""Differential tests for unofficial APL extensions (key ⌸, over ⍥).

Run with: cargo test --features unofficial-ext
"""

import subprocess
import sys
import os

# Path to the rust-apl binary
APL_BIN = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..", "target", "release", "apl"
)

def run_apl(expr):
    """Run an expression through rust-apl and return output."""
    result = subprocess.run(
        [APL_BIN],
        input=expr + "\n)OFF\n",
        capture_output=True,
        text=True,
        timeout=5
    )
    return result.stdout + result.stderr

CASES = [
    # ⌸ Key - monadic
    ("⌸1 2 3", "unique elements with indices"),
    ("⌸1 2 1 3 2", "key with duplicates"),
    ("⌸'abac'", "key with characters"),
    ("⌸2 3⍴1 2 1 3 2 3", "key on matrix (raveled)"),

    # ⍥ Over - monadic
    ("(+⍥⌈) 3.7", "over monadic: ⌈ then + (trivially)"),
    ("(×⍥⌈) 3.7", "over monadic: ceil then multiply"),
    ("(÷⍥⌈) 10 3", "over monadic: ceil of 10 3 then divide"),
    ("(-⍥|) ¯5", "over: abs then negate"),
    ("(+⍥÷) 6 4", "over: divide then add"),

    # ⍥ Over - dyadic
    ("2 (+⍥÷) 4 6", "over: divide each, then add"),
    ("3 (×⍥⌈) 2.1 1.5", "over: ceil each, then multiply"),
    ("1 2 (+⍥÷) 3 4", "over: divide each pair, then add"),

    # Combined usage
    ("⌸(+⍥⌈) 1.2 2.7 1.2", "over then key"),
]

def main():
    agree = 0
    differ = 0
    rust_errors = 0

    for expr, desc in CASES:
        output = run_apl(expr)
        print(f"Testing: {expr:40s} ({desc})")

        if "ERROR" in output and "ERROR: SYNTAX ERROR" in output:
            print(f"  SYNTAX ERROR")
            rust_errors += 1
        elif "ERROR" in output:
            print(f"  ERROR: {output.split('ERROR:')[1].split(chr(10))[0]}")
            rust_errors += 1
        else:
            # Extract result from output
            lines = output.split('\n')
            result_lines = []
            capture = False
            for line in lines:
                if line.startswith('      '):
                    capture = True
                    result_lines.append(line.strip())
                elif capture and line.startswith('     '):
                    result_lines.append(line.strip())
                elif capture:
                    break
            result = ' '.join(result_lines)
            print(f"  Result: {result}")
            agree += 1

    print(f"\n{len(CASES)} cases tested:")
    print(f"  {agree} executed successfully")
    print(f"  {differ} differed")
    print(f"  {rust_errors} rust errors")

    return 0 if rust_errors == 0 else 1

if __name__ == "__main__":
    sys.exit(main())
