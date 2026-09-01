//! GNU APL REPL — the `apl` binary.
//!
//! Reads APL expressions from stdin, evaluates, and prints results
//! (mirrors a minimal `main.cc` + `Command::command_loop()`).

use apl::parser::Environment;
use apl::types::ErrorCode;
use apl::AplError;
use std::io::{self, BufRead, Write};

/// Format a float with ⎕PP significant digits (GNU APL uses %g, not %f).
/// PP=10 means 10 significant digits.
fn float_to_g(v: f64, pp: usize) -> String {
    // Use scientific notation with pp-1 decimals (gives pp significant digits)
    let s = format!("{:.*e}", pp - 1, v);
    if let Some(e_pos) = s.find('e') {
        let mantissa = &s[..e_pos];
        let exp: i32 = s[e_pos + 1..].parse().unwrap_or(0);
        let dot_pos = mantissa.find('.');
        if let Some(dot) = dot_pos {
            let int_part = &mantissa[..dot];
            let frac_part = &mantissa[dot + 1..];
            if exp >= 0 {
                let shift = exp as usize;
                if shift >= frac_part.len() {
                    format!("{}{}", int_part, frac_part) + &"0".repeat(shift - frac_part.len())
                } else {
                    let whole = &frac_part[..shift];
                    let rest = &frac_part[shift..];
                    let rest_trimmed = rest.trim_end_matches('0');
                    if rest_trimmed.is_empty() {
                        format!("{}{}", int_part, whole)
                    } else {
                        format!("{}{}.{}", int_part, whole, rest_trimmed)
                    }
                }
            } else {
                // Negative exponent: 0.xxx form
                let neg_exp = (-exp) as usize;
                if neg_exp == 1 && int_part == "0" {
                    // e.g., 8.775e-1 → 0.8775
                    let frac_trimmed = frac_part.trim_end_matches('0');
                    if frac_trimmed.is_empty() {
                        "0".to_string()
                    } else {
                        format!("0.{}", frac_trimmed)
                    }
                } else if neg_exp == 1 {
                    // e.g., 1.23e-1 → 0.123
                    let frac_trimmed = frac_part.trim_end_matches('0');
                    if frac_trimmed.is_empty() {
                        "0".to_string()
                    } else {
                        format!("0.{}{}", int_part, frac_trimmed)
                    }
                } else {
                    // Larger negative exponent: 0.00...xxx
                    let zeros = "0".repeat(neg_exp - 1);
                    let frac_trimmed = frac_part.trim_end_matches('0');
                    if frac_trimmed.is_empty() {
                        "0".to_string()
                    } else {
                        format!("0.{}{}{}", zeros, int_part, frac_trimmed)
                    }
                }
            }
        } else {
            s
        }
    } else {
        s
    }
}

fn format_value(v: &apl::value::ValueP, pp: usize) -> String {
    // simple one-line formatting for scalars and vectors
    if v.is_scalar() || v.is_vector() {
        let items: Vec<String> = v.cells().iter().map(|c| format_cell(c, pp)).collect();
        items.join("  ")
    } else if v.rank() == 2 {
        // matrix: rows on separate lines
        let cols = v.get_shape_item(1) as usize;
        let cells = v.cells();
        let mut lines = Vec::new();
        for row in 0..(cells.len() / cols.max(1)) {
            let items: Vec<String> = cells[row * cols..(row + 1) * cols]
                .iter()
                .map(|c| format_cell(c, pp))
                .collect();
            lines.push(items.join(" "));
        }
        lines.join("\n")
    } else {
        format!("{:?}", v.shape())
    }
}

/// replace a leading ASCII '-' with a visible minus sign
///
/// Must match boxdisplay::high_minus — this file keeps its own copy of the
/// display logic (see the routing note in main()), so a fix in one place has
/// to be mirrored here.
fn high_minus(s: &str) -> String {
    match s.strip_prefix('-') {
        Some(rest) => format!("−{rest}"), // U+2212 MINUS SIGN
        None => s.to_string(),
    }
}

fn format_cell(c: &apl::cell::Cell, pp: usize) -> String {
    match c {
        apl::cell::Cell::Int(v) => high_minus(&v.to_string()),
        apl::cell::Cell::Float(v) => {
            let s = if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", *v as i64)
            } else if v.abs() >= 1e-10 {
                // GNU APL uses %g (significant digits), not %f (decimal places)
                // PP=10 means 10 significant digits
                float_to_g(*v, pp)
            } else {
                format!("{}", v)
            };
            high_minus(&s)
        }
        apl::cell::Cell::Char(u) => char::from_u32(*u).unwrap_or('?').to_string(),
        apl::cell::Cell::Complex(c) => {
            format!(
                "{}J{}",
                high_minus(&float_to_g(c.re, pp)),
                high_minus(&float_to_g(c.im, pp))
            )
        }
        apl::cell::Cell::Pointer(p) => format_nested(p.value.cells(), pp),
        _ => "<lval>".to_string(),
    }
}

/// format the cells of a nested value: simple scalars inline, deeper
/// nesting recurses. A single scalar shows bare; vectors show space-
/// separated; higher rank shows rows.
fn format_nested(cells: &[apl::cell::Cell], pp: usize) -> String {
    // all-simple vector → space-separated inline
    if cells.iter().all(|c| c.is_simple_cell()) {
        if cells.len() == 1 {
            return format_cell(&cells[0], pp);
        }
        return cells
            .iter()
            .map(|c| format_cell(c, pp))
            .collect::<Vec<_>>()
            .join(" ");
    }
    // mixed/nested → recurse per element
    cells
        .iter()
        .map(|c| match c {
            apl::cell::Cell::Pointer(p) => format_nested(p.value.cells(), pp),
            other => format_cell(other, pp),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    println!("GNU APL 2.0 (Rust) — experimental REPL");
    println!("Enter APL expressions, or )OFF to exit.");
    let stdin = io::stdin();
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);

    // Initialize plugins (Phase 6)
    if let Err(e) =
        apl::plugin_system::init_plugins(&mut env.funcs, &mut std::collections::HashMap::new())
    {
        eprintln!("Warning: plugin initialization failed: {}", e);
    }

    // function definition mode state: Some(header) while inside ∇ editing
    let mut def_header: Option<String> = None;
    let mut def_body: Vec<String> = Vec::new();

    loop {
        // prompt: inside a function definition show line numbers
        match &def_header {
            Some(_) => print!("[{}] ", def_body.len() + 1),
            None => print!("      "),
        }
        io::stdout().flush().ok();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }
        let trimmed = line.trim_end();
        // system commands: )CMD — handled before anything else
        if trimmed.starts_with(')') {
            let cmd = trimmed.chars().skip(1).collect::<String>();
            match apl::sysvars::syscmd(&cmd, &mut env) {
                None => break, // )OFF
                Some(lines) => {
                    for l in lines {
                        if !l.is_empty() {
                            println!("{}", l);
                        }
                    }
                }
            }
            continue;
        }
        if trimmed == ")OFF" || trimmed == ")off" {
            break;
        }

        // function definition mode: ∇HEADER starts, lone ∇ (or ∇) ends
        if def_header.is_none() && trimmed.starts_with('∇') && trimmed.len() > 1 {
            def_header = Some(
                trimmed
                    .chars()
                    .skip(1)
                    .collect::<String>()
                    .trim()
                    .to_string(),
            );
            def_body.clear();
            continue;
        }
        if def_header.is_some() {
            if trimmed == "∇" || trimmed.is_empty() {
                // end of definition: compile and install
                let header = def_header.take().unwrap();
                match apl::functions_def::define_function(&mut env.funcs, &header, &def_body) {
                    Ok(()) => println!(
                        "{} defined",
                        header.split_whitespace().next().unwrap_or("?")
                    ),
                    Err(e) => {
                        let rich = AplError::with_message(ErrorCode::SyntaxError, e)
                            .with_source_line(trimmed.to_string());
                        println!("ERROR: {}", rich);
                    }
                }
            } else {
                def_body.push(trimmed.to_string());
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        match env.eval_line(trimmed) {
            Ok(Some(v)) => {
                // ⎕PP print precision (default 10)
                let pp = apl::sysvars::get_pp(&env).unwrap_or(10);
                // ⎕BOXING: 1 = boxed display, 0 = plain (GNU APL default: 1)
                let boxing = apl::sysvars::get_boxing(&env);
                let all_chars =
                    !v.cells().is_empty() && v.cells().iter().all(|c| c.is_character_cell());
                let has_pointer = v.cells().iter().any(|c| c.is_pointer_cell());
                if v.rank() >= 2 || all_chars || (has_pointer && boxing) {
                    if boxing && has_pointer {
                        for l in apl::boxdisplay::render_with_pp(&v, pp) {
                            println!("{}", l);
                        }
                    } else {
                        for l in apl::boxdisplay::render_plain_with_pp(&v, pp) {
                            println!("{}", l);
                        }
                    }
                } else {
                    println!("{}", format_value(&v, pp));
                }
            }
            Ok(None) => {} // assignment — no output
            Err(e) => {
                // Display error with source line for context
                let rich = AplError::from(e).with_source_line(trimmed.to_string());
                println!("ERROR: {}", rich);
            }
        }
    }

    // After REPL loop ends, wait for any GTK windows to close
    // This prevents the main thread from killing the GTK thread prematurely
    #[cfg(feature = "plugin-gtk")]
    {
        apl::plugins::gtk::gtk_wait_timeout(u64::MAX);
    }
}
