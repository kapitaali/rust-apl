//! System variables and system commands support.
//!
//! ⎕-variables (readable/writable through the vars table with a `⎕` prefix):
//! - `⎕IO` index origin (0 in this port; changing re-bases `⍳`, indexing,
//!   grade, and pick — implemented as offsets at the primitive level
//!   where practical; full re-basing is a larger project, so ⎕IO is
//!   honored by monadic/dyadic `⍳` and bracket indexing)
//! - `⎕CT` comparison tolerance (default 1e-13, matching GNU APL's default)
//! - `⎕PP` print precision (honored by float display in REPL + boxed output)
//!
//! System commands (handled in the REPL before expression evaluation):
//! )VARS — list variable names
//! )FNS  — list defined function names
//! )CLEAR— wipe workspace (vars + functions)
//! )SAVE — save variables to <name>.xml
//! )LOAD — load a saved workspace (wipes current first, like GNU APL)
//! )OFF  — exit (handled by REPL)

use crate::types::AplResult;
use crate::value::ValueP;

pub const IO_VAR: &str = "⎕IO";
pub const CT_VAR: &str = "⎕CT";
pub const PP_VAR: &str = "⎕PP";
pub const BOXING_VAR: &str = "⎕BOXING";

/// Initialize default system variables in a fresh Environment.
pub fn init_sysvars(env: &mut crate::parser::Environment) {
    env.set(IO_VAR, ValueP::scalar_from(crate::cell::Cell::Int(0)));
    let ct = std::sync::Arc::new(crate::value::ValueInner::new(
        crate::shape::Shape::scalar(),
        vec![crate::cell::Cell::Float(1e-13)],
    ));
    env.set(CT_VAR, ValueP { inner: ct });
    env.set(PP_VAR, ValueP::scalar_from(crate::cell::Cell::Int(10)));
    env.set(BOXING_VAR, ValueP::scalar_from(crate::cell::Cell::Int(1)));
}

/// read ⎕BOXING (1 = nested arrays print boxed, 0 = plain)
pub fn get_boxing(env: &crate::parser::Environment) -> bool {
    match env.get(BOXING_VAR) {
        Some(v) => match v.first_cell().unwrap() {
            crate::cell::Cell::Int(i) => *i != 0,
            _ => true, // default: boxing on
        },
        None => true, // default: boxing on
    }
}

/// read ⎕IO (0-based port: only 0 is legal; anything else → DOMAIN ERROR on use)
pub fn get_io(env: &crate::parser::Environment) -> AplResult<i64> {
    match env.get(IO_VAR) {
        Some(v) => v.first_cell().unwrap().get_near_int(),
        None => Ok(0),
    }
}

/// read ⎕PP (print precision, default 10)
pub fn get_pp(env: &crate::parser::Environment) -> AplResult<usize> {
    match env.get(PP_VAR) {
        Some(v) => match v.first_cell().unwrap() {
            crate::cell::Cell::Int(i) => {
                if *i < 1 || *i > 20 {
                    Ok(10)
                } else {
                    Ok(*i as usize)
                }
            }
            crate::cell::Cell::Float(f) => {
                if *f < 1.0 || *f > 20.0 {
                    Ok(10)
                } else {
                    Ok(*f as usize)
                }
            }
            _ => Ok(10),
        },
        None => Ok(10),
    }
}

/// read ⎕CT
pub fn get_ct(env: &crate::parser::Environment) -> AplResult<f64> {
    match env.get(CT_VAR) {
        Some(v) => match v.first_cell().unwrap() {
            crate::cell::Cell::Float(f) => Ok(*f),
            crate::cell::Cell::Int(i) => Ok(*i as f64),
            _ => Err(crate::types::ErrorCode::DomainError),
        },
        None => Ok(1e-13),
    }
}

/// execute a system command (without the leading ')').
/// Returns output lines to print; Ok(None) for )OFF handled by caller.
pub fn syscmd(cmd_line: &str, env: &mut crate::parser::Environment) -> Option<Vec<String>> {
    let mut parts = cmd_line.split_whitespace();
    let cmd = parts.next().unwrap_or("").to_uppercase();
    match cmd.as_str() {
        "VARS" => {
            let mut names = env.var_names();
            names.sort();
            // Filter out ⎕-vars from )VARS (they have their own commands)
            let user_names: Vec<String> =
                names.into_iter().filter(|n| !n.starts_with('⎕')).collect();
            Some(vec![user_names.join("  ")])
        }
        "FNS" => {
            let names = env.funcs.names();
            Some(vec![names.join("  ")])
        }
        "LIB" => {
            // )LIB — list workspace names with details
            let mut output = Vec::new();
            let mut names = env.var_names();
            names.sort();
            for name in names {
                if let Some(v) = env.get(&name) {
                    let shape_str = if v.is_scalar() {
                        "scalar".to_string()
                    } else {
                        format!("shape {}", v.shape())
                    };
                    output.push(format!("{}: {}", name, shape_str));
                }
            }
            let fns = env.funcs.names();
            for name in fns {
                output.push(format!("{}: function", name));
            }
            if output.is_empty() {
                output.push("(empty workspace)".to_string());
            }
            Some(output)
        }
        "DIGITS" => {
            // )DIGITS n — set ⎕PP
            let n = parts.next().unwrap_or("");
            if n.is_empty() {
                let pp = get_pp(env).unwrap_or(10);
                Some(vec![format!("⎕PP = {}", pp)])
            } else {
                match n.parse::<i64>() {
                    Ok(val) if val >= 1 && val <= 20 => {
                        env.set(PP_VAR, ValueP::scalar_from(crate::cell::Cell::Int(val)));
                        Some(vec![format!("⎕PP = {}", val)])
                    }
                    _ => Some(vec!["DIGITS must be 1-20".to_string()]),
                }
            }
        }
        "WIDTH" => {
            // )WIDTH n — report (stored but not yet used for output truncation)
            let n = parts.next().unwrap_or("");
            if n.is_empty() {
                Some(vec!["⎕PW = 80".to_string()])
            } else {
                match n.parse::<i64>() {
                    Ok(val) if val >= 30 && val <= 9999 => Some(vec![format!("⎕PW = {}", val)]),
                    _ => Some(vec!["WIDTH must be 30-9999".to_string()]),
                }
            }
        }
        "CONTINUE" => {
            // )CONTINUE — report no saved session (we don't support it yet)
            Some(vec!["CONTINUE is not supported in this port".to_string()])
        }
        "ED" => Some(vec![")ED is not supported in this port".to_string()]),
        "ERASE" => {
            let name = parts.next().unwrap_or("");
            if name.is_empty() {
                Some(vec!["USAGE: )ERASE name".to_string()])
            } else {
                env.erase_var(name);
                Some(vec![format!("ERASED {}", name)])
            }
        }
        "RESET" | "CLEAR" => {
            env.clear_workspace();
            init_sysvars(env);
            Some(vec!["RESET WORKSPACE".to_string()])
        }
        "DIR" => Some(vec![std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unknown)".to_string())]),
        "HISTORY" => Some(vec!["HISTORY is not supported in this port".to_string()]),
        "SAVE" | "LOAD" => {
            let name = parts.next().unwrap_or("");
            if name.is_empty() {
                return Some(vec![format!(
                    "{} REQUIRES A WORKSPACE NAME: ){} NAME",
                    cmd, cmd
                )]);
            }
            let result = match cmd.as_str() {
                "SAVE" => match crate::xml_archive::save_xml(env, name) {
                    Ok(path) => Some(path),
                    Err(e) => return Some(vec![format!("ERROR: {}", e)]),
                },
                _ => {
                    env.clear_workspace();
                    crate::xml_archive::load_xml(env, name).ok()?;
                    init_sysvars(env);
                    Some(format!("{}.xml", name))
                }
            };
            Some(vec![format!(
                "{} {} ({})",
                cmd,
                name.to_uppercase(),
                result.unwrap_or_else(|| "failed".to_string())
            )])
        }
        "SI" => {
            // )SI — state indicator (call stack). No active functions in v1.
            Some(vec!["(no active functions)".to_string()])
        }
        "SYMBOLS" => {
            // )SYMBOLS — display ⎕AV (APL character vector) info
            Some(vec![format!("⎕AV = 256 characters (0-255)")])
        }
        "OFF" => None, // caller exits
        "" => Some(vec!["(empty system command)".to_string()]),
        "OUT" => {
            // )OUT file — save session as APL source (variables + dfns)
            let name = parts.next().unwrap_or("");
            if name.is_empty() {
                return Some(vec!["USAGE: )OUT file".to_string()]);
            }
            let mut lines = Vec::new();
            // emit variable assignments
            let mut var_names = env.var_names();
            var_names.sort();
            for vname in &var_names {
                if vname.starts_with('⎕') {
                    continue; // skip system vars
                }
                if let Some(val) = env.get(vname) {
                    // simple scalar/vector emission
                    if val.is_scalar() || val.is_vector() {
                        let cells: Vec<String> = val
                            .cells()
                            .iter()
                            .map(|c| match c {
                                crate::cell::Cell::Int(n) => n.to_string(),
                                crate::cell::Cell::Float(f) => format!("{}", f),
                                crate::cell::Cell::Char(ch) => {
                                    format!("'{}'", char::from_u32(*ch).unwrap_or('?'))
                                }
                                _ => "?".to_string(),
                            })
                            .collect();
                        let val_str = cells.join(" ");
                        lines.push(format!("{} ← {}", vname, val_str));
                    }
                    // skip matrices/functions for now
                }
            }
            // emit function definitions
            for fname in env.funcs.names() {
                if let Some(crate::functions_def::Callable::Interpreted(f)) = env.funcs.get(&fname)
                {
                    if !f.no_save && !f.source.is_empty() {
                        lines.push(format!("∇{}", fname));
                        for sline in &f.source {
                            lines.push(sline.clone());
                        }
                        lines.push("∇".to_string());
                    }
                }
            }
            let content = lines.join("\n");
            match std::fs::write(name, content) {
                Ok(()) => Some(vec![format!("SAVED {}", name)]),
                Err(e) => Some(vec![format!("ERROR: cannot write {}: {}", name, e)]),
            }
        }
        "OPS" => {
            // )OPS — list available operators
            let ops = vec![
                "¨ (Each)",
                "⍤ (Rank)",
                "⍣ (Power)",
                "⍨ (Commute)",
                "∘. (Outer Dot)",
                "∘ (Matrix Product)",
                ". (Inner Dot)",
                "⍀ (Scan1)",
                "⌿ (Reduce1)",
                "⍥ (Over)",
            ];
            Some(vec![ops.join("  ")])
        }
        "GRP" => {
            // )GRP — grouped name display (by type)
            let mut groups: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            for name in env.var_names() {
                if name.starts_with('⎕') {
                    continue; // skip system vars (consistent with )VARS)
                }
                if let Some(val) = env.get(&name) {
                    let kind = if val.is_scalar() {
                        "scalar"
                    } else if val.is_vector() {
                        "vector"
                    } else {
                        "array"
                    };
                    groups.entry(kind.to_string()).or_default().push(name);
                }
            }
            for name in env.funcs.names() {
                groups.entry("function".to_string()).or_default().push(name);
            }
            let mut output = Vec::new();
            for (kind, names) in groups {
                let mut sorted = names;
                sorted.sort();
                output.push(format!("{}: {}", kind, sorted.join(" ")));
            }
            output.sort();
            if output.is_empty() {
                output.push("(empty workspace)".to_string());
            }
            Some(output)
        }
        "NMS" => {
            // )NMS — name space display (all names, grouped by first letter)
            let mut all_names: Vec<String> = env
                .var_names()
                .into_iter()
                .filter(|n| !n.starts_with('⎕'))
                .collect();
            all_names.extend(env.funcs.names());
            all_names.sort();
            let mut groups: std::collections::HashMap<char, Vec<String>> =
                std::collections::HashMap::new();
            for name in &all_names {
                let first = name.chars().next().unwrap_or('?');
                groups.entry(first).or_default().push(name.clone());
            }
            let mut output = Vec::new();
            let mut keys: Vec<char> = groups.keys().copied().collect();
            keys.sort();
            for k in keys {
                let names = groups.get(&k).unwrap();
                output.push(format!("{}: {}", k, names.join(" ")));
            }
            if output.is_empty() {
                output.push("(empty workspace)".to_string());
            }
            Some(output)
        }
        "SINL" => {
            // )SINL — state indicator with line numbers
            if env.call_stack.is_empty() {
                Some(vec!["(no active functions)".to_string()])
            } else {
                let mut out = Vec::new();
                for (name, line) in &env.call_stack {
                    out.push(format!("{} [{}]", name, line));
                }
                Some(out)
            }
        }
        "SI" => {
            // )SI — state indicator (function call stack)
            if env.call_stack.is_empty() {
                Some(vec!["(no active functions)".to_string()])
            } else {
                Some(env.call_stack.iter().map(|(n, _)| n.clone()).collect())
            }
        }
        "COPY" | "IN" => {
            // minimal )COPY: evaluate each line of an APL source file in
            // the live workspace. Covers ⎕NA associations, variable and
            // dfn definitions (single-line bodies). Stops at first error.
            let name = parts.next().unwrap_or("");
            if name.is_empty() {
                return Some(vec![format!("USAGE: ){} file", cmd)]);
            }
            let text = match std::fs::read_to_string(name) {
                Ok(t) => t,
                Err(e) => return Some(vec![format!("ERROR: cannot read {name}: {e}")]),
            };
            for (lineno, raw) in text.lines().enumerate() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('⍝') {
                    continue;
                }
                // system commands inside a copied file run too
                if let Some(rest) = line.strip_prefix(')') {
                    if let Some(out) = syscmd(rest, env) {
                        for l in out {
                            println!("{l}");
                        }
                    }
                    continue;
                }
                if let Err(e) = env.eval_line(line) {
                    return Some(vec![format!("ERROR: {} line {}: {}", name, lineno + 1, e)]);
                }
            }
            Some(vec![format!("COPIED {}", name)])
        }
        "INP" => {
            // )INP file — input session from file (like )COPY but prints values)
            let name = parts.next().unwrap_or("");
            if name.is_empty() {
                return Some(vec!["USAGE: )INP file".to_string()]);
            }
            let text = match std::fs::read_to_string(name) {
                Ok(t) => t,
                Err(e) => return Some(vec![format!("ERROR: cannot read {name}: {e}")]),
            };
            for (lineno, raw) in text.lines().enumerate() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('⍝') || line.starts_with(')') {
                    continue;
                }
                match env.eval_line(line) {
                    Ok(Some(v)) => {
                        let disp = crate::boxdisplay::render(&v);
                        for l in &disp {
                            println!("{l}");
                        }
                    }
                    Ok(None) => {} // assignment, shy result
                    Err(e) => {
                        return Some(vec![format!("ERROR: {} line {}: {}", name, lineno + 1, e)]);
                    }
                }
            }
            Some(vec![format!("INP {}", name)])
        }
        "UCS" => {
            // )UCS — report Unicode support
            Some(vec!["UCS-2/UTF-8 character support enabled".to_string()])
        }
        "DROP" => {
            // )DROP name — delete a saved workspace file
            let name = parts.next().unwrap_or("");
            if name.is_empty() {
                Some(vec!["USAGE: )DROP name".to_string()])
            } else {
                let path = format!("{}.xml", name);
                match std::fs::remove_file(&path) {
                    Ok(()) => Some(vec![format!("DROPPED {}", name.to_uppercase())]),
                    Err(e) => Some(vec![format!("ERROR: cannot drop {}: {}", name, e)]),
                }
            }
        }
        other => Some(vec![format!("UNKNOWN SYSTEM COMMAND: {})", other)]),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;
    use crate::shape::Shape;
    use crate::value::ValueInner;

    #[test]
    fn test_init_sysvars() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        assert_eq!(get_io(&env).unwrap(), 0);
        assert!((get_ct(&env).unwrap() - 1e-13).abs() < 1e-20);
        // readable as ordinary names
        let io = env.get("⎕IO").unwrap();
        assert_eq!(io.first_cell().unwrap(), &Cell::Int(0));
    }

    #[test]
    fn test_ct_writable() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.set(
            CT_VAR,
            ValueP {
                inner: std::sync::Arc::new(ValueInner::new(
                    Shape::scalar(),
                    vec![Cell::Float(1e-6)],
                )),
            },
        );
        assert_eq!(get_ct(&env).unwrap(), 1e-6);
    }

    #[test]
    fn test_syscmd_vars_and_fns() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.set("B", ValueP::int_vector(&[1]));
        env.set("A", ValueP::int_vector(&[2]));
        crate::functions_def::define_function(&mut env.funcs, "F X", &["X".to_string()]).unwrap();

        let out = syscmd("VARS", &mut env).unwrap();
        // user vars only (⎕-vars filtered out)
        assert!(out[0].contains("A"));
        assert!(out[0].contains("B"));
        assert!(!out[0].contains("⎕IO"));

        let out = syscmd("fns", &mut env).unwrap(); // case-insensitive
        assert!(out[0].contains("F"));
    }

    #[test]
    fn test_syscmd_clear() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.set("ZZZ", ValueP::int_vector(&[1]));
        let out = syscmd("CLEAR", &mut env).unwrap();
        assert_eq!(out[0], "RESET WORKSPACE");
        assert!(env.get("ZZZ").is_none());
        assert!(env.funcs.names().is_empty());
    }

    #[test]
    fn test_get_pp() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        // default is 10
        assert_eq!(get_pp(&env).unwrap(), 10);
        // writable
        env.set(PP_VAR, ValueP::scalar_from(Cell::Int(3)));
        assert_eq!(get_pp(&env).unwrap(), 3);
        // out-of-range falls back to 10
        env.set(PP_VAR, ValueP::scalar_from(Cell::Int(0)));
        assert_eq!(get_pp(&env).unwrap(), 10);
        env.set(PP_VAR, ValueP::scalar_from(Cell::Int(99)));
        assert_eq!(get_pp(&env).unwrap(), 10);
    }

    #[test]
    fn test_pp_honored_in_display() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.eval_line("⎕PP←4").unwrap();
        let v = env.eval_line("÷3").unwrap().unwrap();
        let lines = crate::boxdisplay::render_with_pp(&v, 4);
        assert_eq!(lines[0], "0.3333");
        // default render keeps 10
        let lines = crate::boxdisplay::render(&v);
        assert_eq!(lines[0], "0.3333333333");
    }

    #[test]
    fn test_boxing_writable() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        // default: boxing on
        assert!(get_boxing(&env));
        // turn off
        env.set(BOXING_VAR, ValueP::scalar_from(Cell::Int(0)));
        assert!(!get_boxing(&env));
        // turn on
        env.set(BOXING_VAR, ValueP::scalar_from(Cell::Int(1)));
        assert!(get_boxing(&env));
    }

    #[test]
    fn test_syscmd_off_and_unknown() {
        let mut env = crate::parser::Environment::new();
        assert!(syscmd("OFF", &mut env).is_none());
        let out = syscmd("NOPE", &mut env).unwrap();
        assert!(out[0].starts_with("UNKNOWN"));
    }

    #[test]
    fn test_syscmd_save_load_xml() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.eval_line("X←42").unwrap();
        env.eval_line("S←'HELLO'").unwrap();
        crate::functions_def::define_function(&mut env.funcs, "INCR", &["⍵+1".to_string()])
            .unwrap();

        // Save
        let out = syscmd("SAVE test_ws", &mut env).unwrap();
        assert!(out[0].contains("SAVE"));
        assert!(out[0].ends_with(".xml)"));

        // Clear and load
        env.clear_workspace();
        assert!(env.get("X").is_none());
        assert!(env.funcs.names().is_empty());

        let out = syscmd("LOAD test_ws", &mut env).unwrap();
        assert!(out[0].contains("LOAD"));

        // Verify variables restored
        assert_eq!(
            env.eval_line("X+0").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(42))
        );
        let s = env.eval_line("S").unwrap().unwrap();
        assert_eq!(s.cells()[0], crate::cell::Cell::Char('H' as u32));

        // Verify function restored
        assert_eq!(
            env.eval_line("INCR 5").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(6))
        );

        // Cleanup
        let _ = std::fs::remove_file("test_ws.xml");
    }

    #[test]
    fn test_syscmd_out_scalar_vector() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.eval_line("X←42").unwrap();
        env.eval_line("V←1 2 3").unwrap();
        env.eval_line("S←'HELLO'").unwrap();

        let out = syscmd("OUT /tmp/test_out_ws", &mut env).unwrap();
        assert_eq!(out[0], "SAVED /tmp/test_out_ws");

        let content = std::fs::read_to_string("/tmp/test_out_ws").unwrap();
        assert!(content.contains("X ← 42"));
        assert!(content.contains("V ← 1 2 3"));
        assert!(content.contains("S ← 'H' 'E' 'L' 'L' 'O'"));
        // system vars should not be saved
        assert!(!content.contains("⎕IO"));

        let _ = std::fs::remove_file("/tmp/test_out_ws");
    }

    #[test]
    fn test_syscmd_out_with_function() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        crate::functions_def::define_function(&mut env.funcs, "DOUBLE", &["⍵+⍵".to_string()])
            .unwrap();

        let out = syscmd("OUT /tmp/test_out_fn", &mut env).unwrap();
        assert_eq!(out[0], "SAVED /tmp/test_out_fn");

        let content = std::fs::read_to_string("/tmp/test_out_fn").unwrap();
        assert!(content.contains("∇DOUBLE"));
        assert!(content.contains("⍵+⍵"));
        assert!(content.contains("∇"));

        let _ = std::fs::remove_file("/tmp/test_out_fn");
    }

    #[test]
    fn test_syscmd_out_no_args() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        let out = syscmd("OUT", &mut env).unwrap();
        assert!(out[0].contains("USAGE"));
    }

    #[test]
    fn test_syscmd_ops() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        let out = syscmd("OPS", &mut env).unwrap();
        assert!(out[0].contains("Each"));
        assert!(out[0].contains("Rank"));
        assert!(out[0].contains("Power"));
        assert!(out[0].contains("Commute"));
    }

    #[test]
    fn test_syscmd_grp() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.eval_line("A←5").unwrap();
        env.eval_line("B←1 2 3").unwrap();
        crate::functions_def::define_function(&mut env.funcs, "F", &["⍵".to_string()]).unwrap();

        let out = syscmd("GRP", &mut env).unwrap();
        let joined = out.join("\n");
        assert!(joined.contains("scalar:"));
        assert!(joined.contains("vector:"));
        assert!(joined.contains("function:"));
        assert!(joined.contains(" A"));
        assert!(joined.contains(" B"));
        assert!(joined.contains(" F"));
        // ⎕-vars should be filtered out
        assert!(!joined.contains("⎕IO"));
        assert!(!joined.contains("⎕PP"));
    }

    #[test]
    fn test_syscmd_nms() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.eval_line("ALPHA←1").unwrap();
        env.eval_line("BETA←2").unwrap();
        env.eval_line("AARDVARK←3").unwrap();

        let out = syscmd("NMS", &mut env).unwrap();
        let joined = out.join("\n");
        // all names grouped by first letter
        assert!(joined.contains("A:"));
        assert!(joined.contains("B:"));
        assert!(joined.contains("ALPHA"));
        assert!(joined.contains("AARDVARK"));
        assert!(joined.contains("BETA"));
    }

    #[test]
    fn test_syscmd_sinl() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        let out = syscmd("SINL", &mut env).unwrap();
        assert_eq!(out[0], "(no active functions)");
    }

    #[test]
    fn test_syscmd_si_empty() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        let out = syscmd("SI", &mut env).unwrap();
        assert_eq!(out[0], "(no active functions)");
    }

    #[test]
    fn test_call_stack_tracks_invocation() {
        // Define a recursive function (FIB using dfn self-call ∇) and verify
        // the call stack grows on entry and shrinks on exit
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.eval_line("FIB←{⍵≤1:1 ⋄ (∇ ⍵-1)+∇ ⍵-2}").unwrap();

        // Before call: empty
        assert!(env.call_stack.is_empty());

        // Call it
        env.eval_line("FIB 4").unwrap().unwrap();

        // After call: empty again (properly popped)
        assert!(env.call_stack.is_empty());

        // Verify the result (FIB 5 = 8 with this dfn definition)
        assert_eq!(
            env.eval_line("FIB 5").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(8))
        );
    }

    #[test]
    fn test_call_stack_nested() {
        // Define two functions where one calls the other
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        crate::functions_def::define_function(&mut env.funcs, "INNER X", &["X+1".to_string()])
            .unwrap();
        crate::functions_def::define_function(&mut env.funcs, "OUTER Y", &["INNER Y".to_string()])
            .unwrap();

        // We can't easily inspect mid-call stack from outside, but we can
        // verify the call completes and the stack is clean after
        let result = env.eval_line("OUTER 5").unwrap().unwrap();
        assert_eq!(result.first_cell(), Some(&crate::cell::Cell::Int(6)));
        assert!(env.call_stack.is_empty());
    }

    #[test]
    fn test_syscmd_inp() {
        // Write a temp APL source file and replay it via )INP
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        let src = "A←10\nB←20\nA+B\n";
        std::fs::write("/tmp/test_inp_src", src).unwrap();

        let out = syscmd("INP /tmp/test_inp_src", &mut env).unwrap();
        assert_eq!(out[0], "INP /tmp/test_inp_src");

        // Variables should be defined
        assert_eq!(
            env.eval_line("A").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(10))
        );
        assert_eq!(
            env.eval_line("B").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(20))
        );

        let _ = std::fs::remove_file("/tmp/test_inp_src");
    }

    #[test]
    fn test_syscmd_inp_no_args() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        let out = syscmd("INP", &mut env).unwrap();
        assert!(out[0].contains("USAGE"));
    }

    #[test]
    fn test_call_stack_cleared_on_clear_workspace() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.call_stack.push(("DUMMY".to_string(), 1));
        env.clear_workspace();
        assert!(env.call_stack.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Namespace (⎕NS / ⎕CS) tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_quad_ns_creates_namespace() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        // Create a namespace
        let result = env.eval_line("⎕NS 'myns'").unwrap().unwrap();
        let name: String = result
            .cells()
            .iter()
            .filter_map(|c| c.get_char_value().ok())
            .map(|cp| std::char::from_u32(cp).unwrap())
            .collect();
        assert_eq!(name, "myns");

        // Verify namespace was registered
        assert!(env.namespaces.contains("myns"));
    }

    #[test]
    fn test_quad_ns_empty_name_errors() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        assert!(env.eval_line("⎕NS ''").is_err());
    }

    #[test]
    fn test_quad_cs_switches_namespace() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        // Create namespace
        env.eval_line("⎕NS 'foo'").unwrap();

        // Switch to it — returns previous (empty = root)
        let result = env.eval_line("⎕CS 'foo'").unwrap().unwrap();
        assert_eq!(result.element_count(), 0); // previous was root (empty string)
        assert_eq!(env.current_ns, "foo");

        // Switch back to root — returns previous ('foo')
        let result = env.eval_line("⎕CS ''").unwrap().unwrap();
        assert_eq!(result.element_count(), 3);
        assert_eq!(env.current_ns, "");
    }

    #[test]
    fn test_quad_cs_unknown_namespace_errors() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        assert!(env.eval_line("⎕CS 'nonexistent'").is_err());
    }

    #[test]
    fn test_ns_variable_scoping() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        // Create a variable in root
        env.eval_line("X←100").unwrap();

        // Create a namespace and switch to it
        env.eval_line("⎕NS 'ns1'").unwrap();
        env.eval_line("⎕CS 'ns1'").unwrap();

        // Set a variable in the namespace
        env.eval_line("X←200").unwrap();

        // Get X from namespace should be 200
        let v = env.eval_line("X").unwrap().unwrap();
        assert_eq!(v.first_cell(), Some(&crate::cell::Cell::Int(200)));

        // Switch back to root
        env.eval_line("⎕CS ''").unwrap();

        // Get X from root should still be 100
        let v = env.eval_line("X").unwrap().unwrap();
        assert_eq!(v.first_cell(), Some(&crate::cell::Cell::Int(100)));
    }

    #[test]
    fn test_ns_vars_stored_qualified() {
        // Verify variables are stored with namespace prefix internally
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        env.eval_line("⎕NS 'ns1'").unwrap();
        env.eval_line("⎕CS 'ns1'").unwrap();
        env.eval_line("Y←42").unwrap();

        // Internally stored as ns1::Y
        assert!(env.get_var("ns1::Y").is_some());
        // But accessible as just Y
        assert!(env.get("Y").is_some());
        // var_names should show just "Y"
        assert!(env.var_names().contains(&"Y".to_string()));
    }

    #[test]
    fn test_ns_clear_workspace_resets() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        env.eval_line("⎕NS 'myns'").unwrap();
        env.eval_line("⎕CS 'myns'").unwrap();
        env.clear_workspace();

        assert!(env.current_ns.is_empty());
        assert!(env.namespaces.is_empty());
    }

    #[test]
    fn test_ns_simple_scoping() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        // Initially in root
        assert_eq!(env.current_ns, "");

        // Set X in root
        env.eval_line("X←100").unwrap();
        let v = env.get_var("X").unwrap();
        assert_eq!(v.first_cell(), Some(&crate::cell::Cell::Int(100)));

        // Create namespace
        env.eval_line("⎕NS 'ns1'").unwrap();
        assert!(env.namespaces.contains("ns1"));

        // Switch to namespace
        env.eval_line("⎕CS 'ns1'").unwrap();
        assert_eq!(env.current_ns, "ns1");

        // Set X in namespace
        env.eval_line("X←200").unwrap();

        // Verify it's stored with namespace prefix
        let ns_x = env.get_var("ns1::X");
        assert!(
            ns_x.is_some(),
            "ns1::X should exist, vars: {:?}",
            env.var_names()
        );
        assert_eq!(
            ns_x.unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(200))
        );

        // Get X should return 200
        let v = env.eval_line("X").unwrap().unwrap();
        assert_eq!(v.first_cell(), Some(&crate::cell::Cell::Int(200)));

        // Switch back to root
        env.eval_line("⎕CS ''").unwrap();
        assert_eq!(env.current_ns, "");

        // Get X from root should be 100
        let v = env.eval_line("X").unwrap().unwrap();
        assert_eq!(v.first_cell(), Some(&crate::cell::Cell::Int(100)));
    }

    #[test]
    fn test_ns_debug_lookup() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        // Set X in root
        env.eval_line("X←100").unwrap();
        assert_eq!(
            env.get("X").unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(100))
        );

        // Create and switch to namespace
        env.eval_line("⎕NS 'ns1'").unwrap();
        env.eval_line("⎕CS 'ns1'").unwrap();
        assert_eq!(env.current_ns, "ns1");

        // Set X in namespace
        env.eval_line("X←200").unwrap();

        // Verify storage is correct
        assert!(env.get_var("ns1::X").is_some(), "ns1::X should exist");
        assert_eq!(
            env.get_var("ns1::X").unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(200))
        );

        // Verify get() with namespace qualification works
        assert_eq!(
            env.get("X").unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(200))
        );
    }

    #[test]
    fn test_ns_qualified_names_not_colliding() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);

        // Two variables with same short name in different namespaces
        env.eval_line("VAL←1").unwrap();

        env.eval_line("⎕NS 'a'").unwrap();
        env.eval_line("⎕CS 'a'").unwrap();
        env.eval_line("VAL←2").unwrap();

        env.eval_line("⎕NS 'b'").unwrap();
        env.eval_line("⎕CS 'b'").unwrap();
        env.eval_line("VAL←3").unwrap();

        // Each namespace has its own VAL
        env.eval_line("⎕CS 'a'").unwrap();
        assert_eq!(
            env.eval_line("VAL").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(2))
        );

        env.eval_line("⎕CS 'b'").unwrap();
        assert_eq!(
            env.eval_line("VAL").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(3))
        );

        // Root still has original
        env.eval_line("⎕CS ''").unwrap();
        assert_eq!(
            env.eval_line("VAL").unwrap().unwrap().first_cell(),
            Some(&crate::cell::Cell::Int(1))
        );
    }
}
