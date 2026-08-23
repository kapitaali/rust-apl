//! GNU APL REPL — the `apl` binary.
//!
//! Reads APL expressions from stdin, evaluates, and prints results
//! (mirrors a minimal `main.cc` + `Command::command_loop()`).

use apl::parser::Environment;
use std::io::{self, BufRead, Write};

fn format_value(v: &apl::value::ValueP) -> String {
    // simple one-line formatting for scalars and vectors
    if v.is_scalar() || v.is_vector() {
        let items: Vec<String> = v.cells().iter().map(format_cell).collect();
        items.join("  ")
    } else if v.rank() == 2 {
        // matrix: rows on separate lines
        let cols = v.get_shape_item(1) as usize;
        let cells = v.cells();
        let mut lines = Vec::new();
        for row in 0..(cells.len() / cols.max(1)) {
            let items: Vec<String> = cells[row * cols..(row + 1) * cols]
                .iter()
                .map(format_cell)
                .collect();
            lines.push(items.join(" "));
        }
        lines.join("\n")
    } else {
        format!("{:?}", v.shape())
    }
}

fn format_cell(c: &apl::cell::Cell) -> String {
    match c {
        apl::cell::Cell::Int(v) => v.to_string(),
        apl::cell::Cell::Float(v) => {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", *v as i64)
            } else if v.abs() >= 1e-10 {
                // honor ⎕PP-ish precision: up to 10 significant digits,
                // trimming trailing zeros
                let s = format!("{:.10}", v);
                let t = s.trim_end_matches('0').trim_end_matches('.');
                t.to_string()
            } else {
                format!("{}", v)
            }
        }
        apl::cell::Cell::Char(u) => char::from_u32(*u).unwrap_or('?').to_string(),
        apl::cell::Cell::Complex(c) => {
            format!("{}J{}", c.re, c.im)
        }
        apl::cell::Cell::Pointer(p) => format_nested(p.value.cells()),
        _ => "<lval>".to_string(),
    }
}

/// format the cells of a nested value: simple scalars inline, deeper
/// nesting recurses. A single scalar shows bare; vectors show space-
/// separated; higher rank shows rows.
fn format_nested(cells: &[apl::cell::Cell]) -> String {
    // all-simple vector → space-separated inline
    if cells.iter().all(|c| c.is_simple_cell()) {
        if cells.len() == 1 {
            return format_cell(&cells[0]);
        }
        return cells.iter().map(format_cell).collect::<Vec<_>>().join(" ");
    }
    // mixed/nested → recurse per element
    cells
        .iter()
        .map(|c| match c {
            apl::cell::Cell::Pointer(p) => format_nested(p.value.cells()),
            other => format_cell(other),
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
                    Err(e) => println!("ERROR: {}", e),
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
                // nested/matrix values get boxed display (4⎕CR-style)
                let has_pointer = v.cells().iter().any(|c| c.is_pointer_cell());
                if has_pointer || v.rank() >= 2 {
                    for l in apl::boxdisplay::render(&v) {
                        println!("{}", l);
                    }
                } else {
                    println!("{}", format_value(&v));
                }
            }
            Ok(None) => {} // assignment — no output
            Err(e) => println!("ERROR: {}", e),
        }
    }
}
