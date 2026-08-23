//! System variables and system commands support.
//!
//! ⎕-variables (readable/writable through the vars table with a `⎕` prefix):
//! - `⎕IO` index origin (0 in this port; changing re-bases `⍳`, indexing,
//!   grade, and pick — implemented as offsets at the primitive level
//!   where practical; full re-basing is a larger project, so ⎕IO is
//!   honored by monadic/dyadic `⍳` and bracket indexing)
//! - `⎕CT` comparison tolerance (default 1e-13, matching GNU APL's default)
//! - `⎕PP` print precision (display only for now)
//!
//! System commands (handled in the REPL before expression evaluation):
//! )VARS — list variable names
//! )FNS  — list defined function names
//! )CLEAR— wipe workspace (vars + functions)
//! )SAVE — save variables to <name>.aplws
//! )LOAD — load a saved workspace (wipes current first, like GNU APL)
//! )OFF  — exit (handled by REPL)

use crate::types::AplResult;
use crate::value::ValueP;

pub const IO_VAR: &str = "⎕IO";
pub const CT_VAR: &str = "⎕CT";
pub const PP_VAR: &str = "⎕PP";

/// Initialize default system variables in a fresh Environment.
pub fn init_sysvars(env: &mut crate::parser::Environment) {
    env.set(IO_VAR, ValueP::scalar_from(crate::cell::Cell::Int(0)));
    let ct = std::sync::Arc::new(crate::value::ValueInner::new(
        crate::shape::Shape::scalar(),
        vec![crate::cell::Cell::Float(1e-13)],
    ));
    env.set(CT_VAR, ValueP { inner: ct });
    env.set(PP_VAR, ValueP::scalar_from(crate::cell::Cell::Int(10)));
}

/// read ⎕IO (0-based port: only 0 is legal; anything else → DOMAIN ERROR on use)
pub fn get_io(env: &crate::parser::Environment) -> AplResult<i64> {
    match env.get(IO_VAR) {
        Some(v) => v.first_cell().unwrap().get_near_int(),
        None => Ok(0),
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
            Some(vec![names.join("  ")])
        }
        "FNS" => {
            let names = env.funcs.names();
            Some(vec![names.join("  ")])
        }
        "CLEAR" => {
            env.clear_workspace();
            init_sysvars(env);
            Some(vec!["CLEAR WORKSPACE".to_string()])
        }
        "SAVE" | "LOAD" => {
            let name = parts.next().unwrap_or("");
            if name.is_empty() {
                return Some(vec![format!(
                    "{} REQUIRES A WORKSPACE NAME: ){} NAME",
                    cmd, cmd
                )]);
            }
            let result = match cmd.as_str() {
                "SAVE" => crate::workspace::save(env, name),
                _ => crate::workspace::load(env, name),
            };
            match result {
                Ok(path) => Some(vec![format!("{} {} ({})", cmd, name.to_uppercase(), path)]),
                Err(e) => Some(vec![format!("ERROR: {}", e)]),
            }
        }
        "OFF" => None, // caller exits
        "" => Some(vec!["(empty system command)".to_string()]),
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
        // sysvars + user vars, sorted
        assert!(out[0].contains("A"));
        assert!(out[0].contains("B"));
        assert!(out[0].contains("⎕IO"));

        let out = syscmd("fns", &mut env).unwrap(); // case-insensitive
        assert!(out[0].contains("F"));
    }

    #[test]
    fn test_syscmd_clear() {
        let mut env = crate::parser::Environment::new();
        init_sysvars(&mut env);
        env.set("ZZZ", ValueP::int_vector(&[1]));
        let out = syscmd("CLEAR", &mut env).unwrap();
        assert_eq!(out[0], "CLEAR WORKSPACE");
        assert!(env.get("ZZZ").is_none());
        assert!(env.funcs.names().is_empty());
    }

    #[test]
    fn test_syscmd_off_and_unknown() {
        let mut env = crate::parser::Environment::new();
        assert!(syscmd("OFF", &mut env).is_none());
        let out = syscmd("NOPE", &mut env).unwrap();
        assert!(out[0].starts_with("UNKNOWN"));
    }
}
