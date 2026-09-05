//! Security level plugin — ⎕SEC extension.
//!
//! Provides a coarse security sandbox. Plugs into the middleware hooks
//! to block sensitive operations at higher ⎕SEC levels.
//!
//! ⎕SEC levels:
//! - 0 (Normal): all operations allowed
//! - 1 (Restricted): ⍎, ⎕NA, ⎕LOADSO, )COPY, )INP blocked
//! - 2 (Locked): additionally )SAVE, )LOAD, )OUT, )CONTINUE, ⎕FIO blocked

use crate::cell::Cell;
use crate::parser::Expr;
use crate::plugin_system::{AplPlugin, AplPluginHooks, PluginInfo, PluginRegistrar};
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;
use std::sync::Arc;

/// Security levels
const SEC_NORMAL: i64 = 0;
#[allow(dead_code)]
const SEC_RESTRICTED: i64 = 1;
#[allow(dead_code)]
const SEC_LOCKED: i64 = 2;

/// Reads the current ⎕SEC level from sysvars.
/// Returns SEC_NORMAL (0) if not set or invalid.
fn get_sec_level(sysvars: &std::collections::HashMap<String, ValueP>) -> i64 {
    match sysvars.get("⎕SEC") {
        Some(v) => match v.first_cell() {
            Some(Cell::Int(i)) if *i >= 0 && *i <= 2 => *i,
            _ => SEC_NORMAL,
        },
        None => SEC_NORMAL,
    }
}

/// Check if an expression is blocked at the given security level.
/// Returns Err with a descriptive message if blocked.
fn check_expr(expr: &Expr, sec: i64) -> AplResult<()> {
    // Only restricted level and above applies to expressions
    if sec < SEC_RESTRICTED {
        return Ok(());
    }
    // Check for execute (⍎) and native association (⎕NA)
    match expr {
        Expr::Monadic(crate::functions::Prim::Execute, _) => Err(ErrorCode::SecurityError),
        Expr::QuadNa(_, _) => Err(ErrorCode::SecurityError),
        Expr::QuadLoadSo(_) => Err(ErrorCode::SecurityError),
        Expr::QuadFio(_) => {
            // File I/O is blocked at level 2
            if sec >= SEC_LOCKED {
                Err(ErrorCode::SecurityError)
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

/// Check if a system command is blocked at the given security level.
fn check_syscmd(cmd: &str, sec: i64) -> AplResult<()> {
    let cmd_upper = cmd.trim().to_uppercase();
    // Extract the command word (before any arguments)
    let cmd_word = cmd_upper.split_whitespace().next().unwrap_or("");

    match sec {
        SEC_RESTRICTED => match cmd_word {
            "COPY" | "IN" | "INP" => Err(ErrorCode::SecurityError),
            _ => Ok(()),
        },
        SEC_LOCKED => match cmd_word {
            "COPY" | "IN" | "INP" | "SAVE" | "LOAD" | "OUT" | "CONTINUE" => {
                Err(ErrorCode::SecurityError)
            }
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

pub struct SecurityPlugin;

impl SecurityPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for SecurityPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "security".into(),
            version: "0.1.0".into(),
            description: "Security level (⎕SEC) — sandboxing extension".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        // Register ⎕SEC system variable with default level 0
        reg.sysvars
            .insert("⎕SEC".into(), ValueP::scalar_from(Cell::Int(0)));
        // Register version info
        reg.sysvars.insert(
            "⎕SEC.VERSION".into(),
            ValueP::char_vector(
                &"security v0.1.0 (⎕SEC)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        Ok(())
    }

    /// Return the security hooks.
    fn hooks(&self) -> Option<Arc<dyn AplPluginHooks>> {
        Some(Arc::new(SecurityHooks))
    }
}

/// Middleware hooks for enforcing ⎕SEC.
struct SecurityHooks;

impl AplPluginHooks for SecurityHooks {
    fn before_eval(&self, expr: &Expr) -> AplResult<()> {
        let sec = SEC_LEVEL.with(|s| *s.borrow());
        check_expr(expr, sec)
    }

    fn before_syscmd(&self, cmd: &str) -> AplResult<()> {
        let sec = SEC_LEVEL.with(|s| *s.borrow());
        check_syscmd(cmd, sec)
    }

    fn on_sysvar_change(&self, name: &str, value: &ValueP) -> AplResult<()> {
        if name == "⎕SEC" {
            let level = match value.first_cell() {
                Some(Cell::Int(i)) if *i >= 0 && *i <= 2 => *i,
                _ => SEC_NORMAL,
            };
            SEC_LEVEL.with(|s| *s.borrow_mut() = level);
        }
        Ok(())
    }
}

/// Thread-local storage for the current ⎕SEC level.
/// Updated by on_sysvar_change, read by before_eval/before_syscmd.
use std::cell::RefCell;
thread_local! {
    static SEC_LEVEL: RefCell<i64> = RefCell::new(SEC_NORMAL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_plugin_info() {
        let plugin = SecurityPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "security");
        assert!(info.description.contains("⎕SEC"));
    }

    #[test]
    fn test_security_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = SecurityPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
            hooks: None,
        };

        plugin.register(&mut reg).unwrap();
        assert!(sysvars.contains_key("⎕SEC"));
        assert!(sysvars.contains_key("⎕SEC.VERSION"));
        assert_eq!(
            sysvars.get("⎕SEC").unwrap().first_cell().unwrap(),
            &Cell::Int(0)
        );
    }

    #[test]
    fn test_security_plugin_hooks() {
        let plugin = SecurityPlugin;
        let hooks = plugin.hooks();
        assert!(hooks.is_some());
    }

    #[test]
    fn test_check_expr_normal_level() {
        // At level 0, all expressions allowed
        let expr = Expr::Num(42.0);
        assert!(check_expr(&expr, SEC_NORMAL).is_ok());
    }

    #[test]
    fn test_check_expr_restricted_blocks_execute() {
        // At level 1, ⍎ is blocked
        let expr = Expr::Monadic(crate::functions::Prim::Execute, Box::new(Expr::Num(42.0)));
        assert!(check_expr(&expr, SEC_RESTRICTED).is_err());
    }

    #[test]
    fn test_check_syscmd_normal_level() {
        // At level 0, all commands allowed
        assert!(check_syscmd("COPY foo", SEC_NORMAL).is_ok());
        assert!(check_syscmd("SAVE foo", SEC_NORMAL).is_ok());
    }

    #[test]
    fn test_check_syscmd_restricted_blocks_copy() {
        // At level 1, )COPY and )INP are blocked
        assert!(check_syscmd("COPY foo", SEC_RESTRICTED).is_err());
        assert!(check_syscmd("INP foo", SEC_RESTRICTED).is_err());
        // But SAVE is still allowed
        assert!(check_syscmd("SAVE foo", SEC_RESTRICTED).is_ok());
    }

    #[test]
    fn test_check_syscmd_locked_blocks_save() {
        // At level 2, )SAVE and )LOAD are blocked
        assert!(check_syscmd("SAVE foo", SEC_LOCKED).is_err());
        assert!(check_syscmd("LOAD foo", SEC_LOCKED).is_err());
        assert!(check_syscmd("OUT foo", SEC_LOCKED).is_err());
        assert!(check_syscmd("CONTINUE", SEC_LOCKED).is_err());
        // But simple queries are still allowed
        assert!(check_syscmd("VARS", SEC_LOCKED).is_ok());
    }

    #[test]
    fn test_get_sec_level_default() {
        let sysvars = std::collections::HashMap::new();
        assert_eq!(get_sec_level(&sysvars), SEC_NORMAL);
    }

    #[test]
    fn test_get_sec_level_with_value() {
        use std::collections::HashMap;
        let mut sysvars = HashMap::new();
        sysvars.insert("⎕SEC".into(), ValueP::scalar_from(Cell::Int(1)));
        assert_eq!(get_sec_level(&sysvars), 1);
    }
}
