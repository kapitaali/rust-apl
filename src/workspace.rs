//! Workspace save/load — `)SAVE name` / `)LOAD name`.
//!
//! Mirrors the workspace-persistence role of GNU APL's `Workspace.cc` /
//! `SystemVariable` / symbol export (greatly simplified). The on-disk
//! format is a line-oriented text file:
//!
//! ```text
//! APLWS1
//! V <name> <kind> <payload>
//! ...
//! FN <header>
//! B <body-line>
//! ... (body lines until next FN/EOF)
//! ```
//!
//! Variables serialize as one line each: scalar Int (`I`), scalar Float
//! (`F`), char vector (`C`), or int vector (`VI`) — the value kinds a
//! REPL session realistically accumulates. Nested/mixed values fall back
//! to an error message rather than silently truncating. Functions store
//! their raw header + body lines so redefinition goes through
//! define_function (control structures included).

use crate::cell::Cell;
use crate::parser::Environment;
use crate::value::ValueP;
use std::path::PathBuf;

/// serialize + write the workspace to `<name>.aplws`
pub fn save(env: &Environment, name: &str) -> Result<String, String> {
    let mut lines = vec!["APLWS1".to_string()];

    let mut names = env.var_names();
    names.sort();
    for n in names {
        // skip system variables: they are re-seeded by init_sysvars
        if n.starts_with('⎕') {
            continue;
        }
        let v = env.get(&n).expect("name came from var_names");
        match serialize_var(v) {
            Some(payload) => {
                lines.push(format!("V {} {}", n, payload));
            }
            None => {
                return Err(format!(
                    "cannot save variable {} (unsupported shape/kind)",
                    n
                ))
            }
        }
    }

    // functions: DefinedFunction retains raw source lines, so definitions
    // round-trip through define_function on load.
    for fname in env.funcs.names() {
        let f = env.funcs.get(&fname).expect("name came from names()");
        // reconstruct header: [result←]NAME [left] [right]
        let mut header = String::new();
        if let Some(r) = &f.result {
            header.push_str(r);
            header.push('←');
        }
        header.push_str(&fname);
        if let Some(l) = &f.arg_left {
            header.push(' ');
            header.push_str(l);
        }
        if let Some(r) = &f.arg_right {
            header.push(' ');
            header.push_str(r);
        }
        lines.push(format!("FN {}", header));
        for src in &f.source {
            lines.push(format!("B {}", src));
        }
    }

    let path = ws_path(name);
    std::fs::write(&path, lines.join("\n") + "\n")
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(path.display().to_string())
}

/// read + restore a workspace from `<name>.aplws`
pub fn load(env: &mut Environment, name: &str) -> Result<String, String> {
    let path = ws_path(name);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let mut lines = text.lines();
    match lines.next() {
        Some("APLWS1") => {}
        _ => return Err("not a valid workspace file".to_string()),
    }

    // wipe first, like GNU APL )LOAD
    env.clear_workspace();
    crate::sysvars::init_sysvars(env);

    // Parse records: a V-line is self-contained; an FN record owns all
    // following B-lines. Index-based walk (no iterator push-back needed).
    #[derive(Debug)]
    enum Record {
        /// name, payload
        Var(String, String),
        /// header, body source lines
        Fn(String, Vec<String>),
    }
    let all: Vec<&str> = text.lines().collect();
    let mut records: Vec<Record> = Vec::new();
    let mut i = 1; // past APLWS1
    while i < all.len() {
        let line = all[i];
        if let Some(rest) = line.strip_prefix("V ") {
            match rest.split_once(' ') {
                Some((n, payload)) => records.push(Record::Var(n.to_string(), payload.to_string())),
                None => return Err(format!("corrupt V record: {}", line)),
            }
            i += 1;
        } else if let Some(header) = line.strip_prefix("FN ") {
            let mut body: Vec<String> = Vec::new();
            i += 1;
            while i < all.len() {
                match all[i].strip_prefix("B ") {
                    Some(src) => {
                        body.push(src.to_string());
                        i += 1;
                    }
                    None => break, // next record starts; outer loop takes it
                }
            }
            records.push(Record::Fn(header.to_string(), body));
        } else {
            return Err(format!("corrupt workspace line: {}", line));
        }
    }

    for rec in &records {
        match rec {
            Record::Var(n, payload) => {
                let v = deserialize_var(payload)?;
                env.set(n, v);
            }
            Record::Fn(header, body) => {
                crate::functions_def::define_function(&mut env.funcs, header, body)
                    .map_err(|e| format!("in {}: {}", header, e))?;
            }
        }
    }
    Ok(path.display().to_string())
}

fn ws_path(name: &str) -> PathBuf {
    PathBuf::from(format!("{}.aplws", name))
}

/// One-line variable encoding. Returns None for values we can't round-trip.
fn serialize_var(v: &ValueP) -> Option<String> {
    let cells = v.cells();
    if v.is_scalar() {
        match cells.first()? {
            Cell::Int(i) => return Some(format!("I {}", i)),
            Cell::Float(f) => return Some(format!("F {}", f)),
            Cell::Char(c) => return Some(format!("C {}", *c)),
            _ => return None,
        }
    }
    if v.rank() == 1 {
        // int vector
        if cells.iter().all(|c| matches!(c, Cell::Int(_))) {
            let ints: Vec<String> = cells
                .iter()
                .map(|c| match c {
                    Cell::Int(i) => i.to_string(),
                    _ => unreachable!("checked above"),
                })
                .collect();
            return Some(format!("VI {}", ints.join(",")));
        }
        // char vector ('abc') — chars stored as u32 codepoints
        if cells.iter().all(|c| matches!(c, Cell::Char(_))) {
            let esc: Vec<String> = cells
                .iter()
                .filter_map(|c| match c {
                    Cell::Char(cp) => Some(cp.to_string()),
                    _ => None,
                })
                .collect();
            return Some(format!("VC {}", esc.join(",")));
        }
    }
    None
}

fn deserialize_var(payload: &str) -> Result<ValueP, String> {
    let (kind, rest) = payload.split_once(' ').ok_or("corrupt var payload")?;
    match kind {
        "I" => Ok(ValueP::scalar_from(Cell::Int(
            rest.parse().map_err(|_| "bad int")?,
        ))),
        "F" => Ok(ValueP::scalar_from(Cell::Float(
            rest.parse().map_err(|_| "bad float")?,
        ))),
        "C" => Ok(ValueP::scalar_from(Cell::Char(
            rest.parse().map_err(|_| "bad char")?,
        ))),
        "VI" => {
            let ints: Vec<i64> = rest
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().map_err(|_| "bad int vector"))
                .collect::<Result<_, _>>()?;
            let cells: Vec<Cell> = ints.into_iter().map(Cell::Int).collect();
            ValueP::from_parts(crate::shape::Shape::vector(cells.len() as i64), cells)
                .map_err(|e| format!("shape error: {:?}", e))
        }
        "VC" => {
            let cps: Vec<u32> = rest
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().map_err(|_| "bad char vector"))
                .collect::<Result<_, _>>()?;
            let cells: Vec<Cell> = cps.into_iter().map(Cell::Char).collect();
            ValueP::from_parts(crate::shape::Shape::vector(cells.len() as i64), cells)
                .map_err(|e| format!("shape error: {:?}", e))
        }
        _ => Err(format!("unknown var kind {}", kind)),
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Environment {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env
    }

    #[test]
    fn test_save_load_roundtrip_vars() {
        let mut env = fresh();
        env.eval_line("X←42").unwrap();
        env.eval_line("Y←3.5").unwrap();
        env.eval_line("V←⍳5").unwrap();
        env.eval_line("S←'HELLO'").unwrap();

        let path = save(&env, "test_ws_roundtrip").unwrap();
        assert!(path.ends_with(".aplws"));

        let mut env2 = fresh();
        load(&mut env2, "test_ws_roundtrip").unwrap();
        assert_eq!(
            env2.eval_line("X").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(42))
        );
        assert_eq!(
            env2.eval_line("Y").unwrap().unwrap().first_cell(),
            Some(&Cell::Float(3.5))
        );
        let v = env2.eval_line("V+0").unwrap().unwrap();
        assert_eq!(v.element_count(), 5);
        let s = env2.eval_line("S").unwrap().unwrap();
        assert_eq!(s.element_count(), 5);
        assert!(matches!(s.first_cell().unwrap(), Cell::Char(c) if *c == 'H' as u32));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_load_wipes_existing() {
        let mut env = fresh();
        env.set("OLD", ValueP::int_vector(&[9]));
        let path = save(&env, "test_ws_wipe").unwrap();

        let mut env2 = fresh();
        env2.set("KEEPME", ValueP::int_vector(&[1]));
        load(&mut env2, "test_ws_wipe").unwrap();
        assert!(env2.get("KEEPME").is_none()); // wiped by LOAD
        assert!(env2.get("OLD").is_some()); // restored from file
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_unsupported_value_errors() {
        let mut env = fresh();
        env.eval_line("N←(1 2)(3 4)").unwrap(); // nested — unsupported
        assert!(save(&env, "test_ws_nested").is_err());
    }

    #[test]
    fn test_load_missing_file() {
        let mut env = fresh();
        assert!(load(&mut env, "definitely_missing_ws_12345").is_err());
    }

    #[test]
    fn test_save_load_function_roundtrip() {
        let mut env = fresh();
        crate::functions_def::define_function(
            &mut env.funcs,
            "R←FINDSTOP N",
            &[
                "R←0".to_string(),
                "I←1".to_string(),
                ":While 1".to_string(),
                "R←R+I".to_string(),
                ":If I≥N".to_string(),
                ":Leave".to_string(),
                ":EndIf".to_string(),
                "I←I+1".to_string(),
                ":EndWhile".to_string(),
            ],
        )
        .unwrap();
        let path = save(&env, "test_ws_fns").unwrap();

        let mut env2 = fresh();
        load(&mut env2, "test_ws_fns").unwrap();
        // the function survives and still executes (incl. :Leave)
        assert_eq!(
            env2.eval_line("FINDSTOP 4").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(10))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_load_multiple_functions_and_vars() {
        let mut env = fresh();
        env.set("Z", ValueP::int_vector(&[7]));
        crate::functions_def::define_function(&mut env.funcs, "R←ADD A B", &["R←A+B".to_string()])
            .unwrap();
        crate::functions_def::define_function(&mut env.funcs, "DOUBLE X", &["X+X".to_string()])
            .unwrap();

        let path = save(&env, "test_ws_multi").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("\nFN ").count(), 2); // both functions stored

        let mut env2 = fresh();
        load(&mut env2, "test_ws_multi").unwrap();
        assert_eq!(
            env2.eval_line("3 ADD 4").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(7))
        );
        assert_eq!(
            env2.eval_line("DOUBLE 21").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(42))
        );
        assert!(env2.get("Z").is_some());
        let _ = std::fs::remove_file(path);
    }
}
