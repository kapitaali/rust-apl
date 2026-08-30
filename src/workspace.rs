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
    // round-trip through define_function on load. Anonymous dfn temporaries
    // (compiler-generated, private-use name prefix) have no valid APL
    // source and are skipped.
    // plugin .so specs first: load() re-issues ⎕LOADSO before function
    // records so Plugin bindings re-register
    for spec in &env.loaded_plugins {
        lines.push(format!("PLG {}", spec));
    }
    for fname in env.funcs.names() {
        let callable = env.funcs.get(&fname).expect("name came from names()");
        // native ⎕NA bindings: persist the declaration text; load() re-parses
        // and re-dlopens (never persist raw addresses)
        if let crate::functions_def::Callable::Native(b) = callable {
            let decl = b.spec.decl_text();
            lines.push(format!("NA {} {}", fname, decl));
            continue;
        }
        // plugin bindings are covered by their PLG record; skip the slot
        if matches!(callable, crate::functions_def::Callable::Plugin(_)) {
            continue;
        }
        let f = callable.interpreted().expect("checked above");
        // skip anonymous dfn temporaries (private-use name prefix)
        if f.name.starts_with(crate::parser::DFNS_PREFIX) {
            continue;
        }
        // named dfns: single-expression body with ⍺/⍵ — persist as a dfn
        // literal via unparse so load() can reconstruct NAME←{body}
        if f.is_dfn && f.body.len() == 1 {
            let body_text = crate::unparse::unparse(&f.body[0]);
            lines.push(format!("DFN {} {}", fname, body_text));
            continue;
        }
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
    // following B-lines; a DFN record is a single-line dfn definition.
    // Index-based walk (no iterator push-back needed).
    #[derive(Debug)]
    enum Record {
        /// name, payload
        Var(String, String),
        /// header, body source lines
        Fn(String, Vec<String>),
        /// dfn name + unparsed body expression
        Dfn(String, String),
        /// ⎕NA binding: apl name + declaration text (re-parsed on load)
        Na(String, String),
        /// plugin .so spec (re-loaded via ⎕LOADSO path on load)
        Plg(String),
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
        } else if let Some(rest) = line.strip_prefix("DFN ") {
            // DFN <name> <body-expr>
            match rest.split_once(' ') {
                Some((name, body)) => records.push(Record::Dfn(name.to_string(), body.to_string())),
                None => return Err(format!("corrupt DFN record: {}", line)),
            }
            i += 1;
        } else if let Some(rest) = line.strip_prefix("NA ") {
            // NA <name> <decl-text> — re-parsed and re-dlopened on load
            match rest.split_once(' ') {
                Some((name, decl)) => records.push(Record::Na(name.to_string(), decl.to_string())),
                None => return Err(format!("corrupt NA record: {}", line)),
            }
            i += 1;
        } else if let Some(rest) = line.strip_prefix("PLG ") {
            // PLG <so-spec> — re-issued as ⎕LOADSO on load
            records.push(Record::Plg(rest.to_string()));
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
            Record::Dfn(name, body) => {
                // reconstruct as NAME←{body}
                let def = format!("{}←{{{}}}", name, body);
                env.eval_line(&def)
                    .map_err(|e| format!("error loading dfn {}: {:?}", name, e))?;
            }
            Record::Na(name, decl) => {
                // re-parse declaration and re-dlopen; never persist addresses
                let spec = crate::ffi::nadecl::parse_na_decl(decl)
                    .map_err(|e| format!("corrupt NA decl {}: {:?}", decl, e))?;
                let binding = crate::ffi::cabi::CAbiBinding::associate(&mut env.lib_cache, spec)
                    .map_err(|e| {
                        format!(
                            "rebinding {} failed: {:?}",
                            name,
                            match e {
                                crate::ffi::cabi::CablError::Load(_) => "library load",
                                crate::ffi::cabi::CablError::Symbol(_) => "symbol lookup",
                                crate::ffi::cabi::CablError::Domain(_) => "signature",
                                crate::ffi::cabi::CablError::Syntax => "syntax",
                            }
                        )
                    })?;
                env.funcs.insert_native(name, binding);
            }
            Record::Plg(spec) => {
                // re-load the plugin .so; insert each binding into the table
                let loaded = crate::ffi::plugin::load_plugin(&mut env.lib_cache, spec)
                    .map_err(|e| format!("plugin {} failed to reload: {}", spec, e))?;
                for b in loaded.bindings {
                    let name = b.apl_name.clone();
                    env.funcs.insert_plugin(&name, b);
                }
                if !env.loaded_plugins.contains(spec) {
                    env.loaded_plugins.push(spec.clone());
                }
            }
        }
    }
    Ok(path.display().to_string())
}

fn ws_path(name: &str) -> PathBuf {
    PathBuf::from(format!("{}.aplws", name))
}

/// One-line variable encoding. Returns None for values we can't round-trip.
///
/// Format for any-rank simple numeric/char/complex arrays:
///   `AI <dims-comma>;<vals-comma>`   int array (rank ≥ 0)
///   `AF <dims-comma>;<vals-comma>`   float array
///   `AC <dims-comma>;<vals-comma>`   char array (u32 codepoints)
///   `AX <dims-comma>;<re,im;...>`    complex array (real,imag pairs)
///   `AN <dims-comma>;<child1>|...`   nested array (enclosed children)
/// Scalars serialize with an empty dim list (`I`/`F`/`C` legacy forms are
/// still parsed for backward compatibility).
fn serialize_var(v: &ValueP) -> Option<String> {
    let cells = v.cells();

    // Check if all cells are pointers (nested array)
    if cells.iter().all(|c| c.is_pointer_cell()) {
        let dims: Vec<String> = (0..v.rank() as usize)
            .map(|k| v.get_shape_item(k as i16).to_string())
            .collect();
        let children: Vec<String> = cells
            .iter()
            .map(|c| {
                if let Cell::Pointer(p) = c {
                    serialize_var(&ValueP {
                        inner: p.value.clone(),
                    })
                } else {
                    None
                }
            })
            .collect::<Option<Vec<_>>>()?;
        return Some(format!("AN {};{}", dims.join(","), children.join("|")));
    }

    let kind = match cells.first()? {
        Cell::Int(_) => "AI",
        Cell::Float(_) => "AF",
        Cell::Char(_) => "AC",
        Cell::Complex(_) => "AX",
        _ => return None,
    };
    // mixed int/float arrays: promote to AF
    let kind = if kind == "AI" && cells.iter().any(|c| matches!(c, Cell::Float(_))) {
        "AF"
    } else {
        kind
    };

    let dims: Vec<String> = (0..v.rank() as usize)
        .map(|k| v.get_shape_item(k as i16).to_string())
        .collect();
    let vals: Vec<String> = cells
        .iter()
        .map(|c| match (kind, c) {
            ("AI", Cell::Int(i)) => Ok(i.to_string()),
            ("AF", Cell::Int(i)) => Ok((*i as f64).to_string()),
            ("AF", Cell::Float(f)) => Ok(f.to_string()),
            ("AC", Cell::Char(cp)) => Ok(cp.to_string()),
            ("AX", Cell::Complex(c)) => Ok(format!("{}J{}", c.re, c.im)),
            _ => Err(()),
        })
        .collect::<Result<_, _>>()
        .ok()?;
    Some(format!("{} {};{}", kind, dims.join(","), vals.join(",")))
}

fn parse_dims(s: &str) -> Result<Vec<i64>, String> {
    if s.is_empty() {
        return Ok(vec![]); // scalar
    }
    s.split(',')
        .map(|d| d.parse().map_err(|_| "bad dim".to_string()))
        .collect()
}

fn deserialize_var(payload: &str) -> Result<ValueP, String> {
    let (kind, rest) = payload.split_once(' ').ok_or("corrupt var payload")?;
    // generic any-rank forms
    if matches!(kind, "AI" | "AF" | "AC" | "AX") {
        let (dim_str, val_str) = rest.split_once(';').ok_or("corrupt array payload")?;
        let dims = parse_dims(dim_str)?;
        let shape =
            crate::shape::Shape::from_dims(&dims).map_err(|e| format!("shape error: {:?}", e))?;
        let cells: Vec<Cell> = val_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| match kind {
                "AI" => s
                    .parse::<i64>()
                    .map(Cell::Int)
                    .map_err(|_| "bad int".to_string()),
                "AF" => s
                    .parse::<f64>()
                    .map(Cell::Float)
                    .map_err(|_| "bad float".to_string()),
                "AC" => s
                    .parse::<u32>()
                    .map(Cell::Char)
                    .map_err(|_| "bad char".to_string()),
                "AX" => {
                    let parts: Vec<&str> = s.split('J').collect();
                    if parts.len() != 2 {
                        return Err("bad complex".to_string());
                    }
                    let re = parts[0].parse::<f64>().map_err(|_| "bad complex re")?;
                    let im = parts[1].parse::<f64>().map_err(|_| "bad complex im")?;
                    Ok(Cell::Complex(crate::types::APLComplex::new(re, im)))
                }
                _ => unreachable!(),
            })
            .collect::<Result<_, _>>()?;
        return Ok(ValueP::from_parts(shape, cells).map_err(|e| format!("shape error: {:?}", e))?);
    }
    // nested array format: AN <dims-comma>;<child1>|...
    if kind == "AN" {
        let (dim_str, val_str) = rest.split_once(';').ok_or("corrupt nested payload")?;
        let dims = parse_dims(dim_str)?;
        let shape =
            crate::shape::Shape::from_dims(&dims).map_err(|e| format!("shape error: {:?}", e))?;
        let cells: Vec<Cell> = val_str
            .split('|')
            .map(|child_payload| {
                let child = deserialize_var(child_payload)?;
                Ok(Cell::pointer(child.inner.clone()))
            })
            .collect::<Result<_, String>>()?;
        return Ok(ValueP::from_parts(shape, cells).map_err(|e| format!("shape error: {:?}", e))?);
    }
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
        // Nested arrays are now supported, so this should succeed
        env.eval_line("N←(1 2)(3 4)").unwrap();
        assert!(save(&env, "test_ws_unsupported").is_ok());
        let path = save(&env, "test_ws_unsupported").unwrap();
        let mut env2 = fresh();
        load(&mut env2, "test_ws_unsupported").unwrap();
        assert!(env2.get("N").is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_load_complex_values() {
        let mut env = fresh();
        env.eval_line("C←1J2 2J3 3J4").unwrap();

        let path = save(&env, "test_ws_complex").unwrap();
        let mut env2 = fresh();
        load(&mut env2, "test_ws_complex").unwrap();
        let c = env2.eval_line("C").unwrap().unwrap();
        assert_eq!(c.element_count(), 3);
        assert_eq!(c.cells()[0], Cell::complex(1.0, 2.0));
        assert_eq!(c.cells()[1], Cell::complex(2.0, 3.0));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_load_nested_arrays() {
        let mut env = fresh();
        env.eval_line("N←(1 2)(3 4 5)").unwrap();

        let path = save(&env, "test_ws_nested").unwrap();
        let mut env2 = fresh();
        load(&mut env2, "test_ws_nested").unwrap();
        let n = env2.eval_line("N").unwrap().unwrap();
        assert_eq!(n.rank(), 1);
        assert_eq!(n.element_count(), 2);
        // first child should be 1 2
        if let Cell::Pointer(p) = &n.cells()[0] {
            assert_eq!(p.value.element_count(), 2);
        } else {
            panic!("expected pointer cell");
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_load_matrix_and_rank3() {
        let mut env = fresh();
        env.eval_line("M←2 3⍴⍳6").unwrap();
        env.eval_line("T←2 2 2⍴⍳8").unwrap();
        env.eval_line("FM←2 2⍴÷2 4 5 8").unwrap();

        let path = save(&env, "test_ws_rank").unwrap();
        let mut env2 = fresh();
        load(&mut env2, "test_ws_rank").unwrap();

        // matrix shape + ravel survive
        let m = eval_val(&mut env2, "M+0");
        assert_eq!(m.rank(), 2);
        assert_eq!(m.get_shape_item(0), 2);
        assert_eq!(m.get_shape_item(1), 3);
        assert_eq!(
            m.cells(),
            &[
                Cell::Int(0),
                Cell::Int(1),
                Cell::Int(2),
                Cell::Int(3),
                Cell::Int(4),
                Cell::Int(5)
            ]
        );
        // rank-3
        let t = eval_val(&mut env2, "T+0");
        assert_eq!(t.rank(), 3);
        assert_eq!(t.element_count(), 8);
        assert_eq!(t.cells()[7], Cell::Int(7));
        // float matrix (⎕PP-independent: full f64 precision stored)
        let fm = eval_val(&mut env2, "FM+0");
        assert_eq!(fm.cells()[1], Cell::Float(0.25));
        assert_eq!(fm.cells()[3], Cell::Float(0.125));
        let _ = std::fs::remove_file(path);
    }

    /// helper: evaluate a line that must produce a value
    fn eval_val(env: &mut Environment, line: &str) -> ValueP {
        env.eval_line(line).expect("eval failed").expect("a result")
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

    #[test]
    fn test_save_load_dfn_roundtrip() {
        let mut env = fresh();
        env.eval_line("AVG←{(+/⍵)÷⍴⍵}").unwrap();
        // verify in-session
        assert_eq!(
            env.eval_line("AVG 10 20 30").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(20))
        );

        let path = save(&env, "test_ws_dfn").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("DFN AVG"), "dfn record missing: {}", text);

        let mut env2 = fresh();
        load(&mut env2, "test_ws_dfn").unwrap();
        assert_eq!(
            env2.eval_line("AVG 10 20 30")
                .unwrap()
                .unwrap()
                .first_cell(),
            Some(&Cell::Int(20))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_dfn_guarded() {
        let mut env = fresh();
        env.eval_line("ABS←{⍵<0:(-⍵) ⋄ ⍵}").unwrap();
        assert_eq!(
            env.eval_line("ABS ¯5").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(5))
        );
        assert_eq!(
            env.eval_line("ABS 3").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(3))
        );
    }

    #[test]
    fn test_dfn_guarded_multi() {
        let mut env = fresh();
        env.eval_line("CLASSIFY←{⍵<0:(-1) ⋄ ⍵>0:(1) ⋄ 0}").unwrap();
        assert_eq!(
            env.eval_line("CLASSIFY ¯7").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(-1))
        );
        assert_eq!(
            env.eval_line("CLASSIFY 9").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(1))
        );
        assert_eq!(
            env.eval_line("CLASSIFY 0").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(0))
        );
    }

    #[test]
    fn test_dfn_recursive_nontrivial() {
        let mut env = fresh();
        env.eval_line("FAC←{⍵=0:(1) ⋄ ⍵×∇ ⍵-1}").unwrap();
        assert_eq!(
            env.eval_line("FAC 5").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(120))
        );
        assert_eq!(
            env.eval_line("FAC 0").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(1))
        );
    }

    #[test]
    fn test_dfn_recursive_fib() {
        let mut env = fresh();
        env.eval_line("FIB←{⍵≤1:(1) ⋄ (∇ ⍵-1)+∇ ⍵-2}").unwrap();
        assert_eq!(
            env.eval_line("FIB 10").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(89))
        );
    }

    #[test]
    fn test_dfn_nested() {
        let mut env = fresh();
        env.eval_line("F←{⍵+{⍵+1}⍵}").unwrap();
        assert_eq!(
            env.eval_line("F 3").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(7))
        );
    }
}
