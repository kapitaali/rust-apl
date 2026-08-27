//! GNU APL REPL — the `apl` binary.
//!
//! Reads APL expressions from stdin, evaluates, and prints results
//! (mirrors a minimal `main.cc` + `Command::command_loop()`).

use apl::parser::Environment;
use std::io::{self, BufRead, Write};

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

/// replace a leading ASCII '-' with APL's high minus `¯`
///
/// Must match boxdisplay::high_minus — this file keeps its own copy of the
/// display logic (see the routing note in main()), so a fix in one place has
/// to be mirrored here.
fn high_minus(s: &str) -> String {
    match s.strip_prefix('-') {
        Some(rest) => format!("¯{rest}"),
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
                // ⎕PP precision: up to `pp` decimals, trailing zeros trimmed
                let s = format!("{:.*}", pp, v);
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            } else {
                format!("{}", v)
            };
            high_minus(&s)
        }
        apl::cell::Cell::Char(u) => char::from_u32(*u).unwrap_or('?').to_string(),
        apl::cell::Cell::Complex(c) => {
            format!(
                "{}J{}",
                high_minus(&c.re.to_string()),
                high_minus(&c.im.to_string())
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
                // ⎕PP print precision (default 10)
                let pp = apl::sysvars::get_pp(&env).unwrap_or(10);
                // Default display uses plain rendering (no boxes) to match
                // GNU APL's default output. Boxed display is available via
                // 4⎕CR (the standard APL boxed-representation function).
                let all_chars =
                    !v.cells().is_empty() && v.cells().iter().all(|c| c.is_character_cell());
                if v.rank() >= 2 || all_chars {
                    for l in apl::boxdisplay::render_plain_with_pp(&v, pp) {
                        println!("{}", l);
                    }
                } else {
                    println!("{}", format_value(&v, pp));
                }
            }
            Ok(None) => {} // assignment — no output
            Err(e) => println!("ERROR: {}", e),
        }
    }
}
