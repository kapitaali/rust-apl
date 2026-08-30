//! Security level enforcement (Phase 5.5).
//!
//! ⎕SEC controls the security level:
//! - 0 = normal: all operations allowed
//! - 1 = restricted: ⍎, ⎕NA, ⎕LOADSO, )COPY, )INP are blocked
//! - 2 = locked down: additionally )SAVE, )LOAD, )OUT, file I/O blocked

use crate::cell::Cell;
use crate::parser::Environment;
use crate::value::ValueP;

/// Security levels
pub const SEC_NORMAL: i64 = 0;
pub const SEC_RESTRICTED: i64 = 1;
pub const SEC_LOCKED: i64 = 2;

/// Read ⎕SEC security level.
pub fn get_sec(env: &Environment) -> i64 {
    match env.get(crate::sysvars::SEC_VAR) {
        Some(v) => match v.first_cell() {
            Some(Cell::Int(i)) => *i,
            _ => SEC_NORMAL,
        },
        None => SEC_NORMAL,
    }
}

/// Check whether an operation is allowed at the current security level.
/// Returns Ok(()) if allowed, Err(SecurityError) if blocked.
pub fn check_sec(env: &Environment, operation: &str) -> Result<(), String> {
    let sec = get_sec(env);

    let blocked_at = match operation {
        // ⍎ execute
        "EXECUTE" => SEC_RESTRICTED,
        // ⎕NA native function interface
        "NA" => SEC_RESTRICTED,
        // ⎕LOADSO load shared object
        "LOADSO" => SEC_RESTRICTED,
        // )COPY copy workspace
        "COPY" => SEC_RESTRICTED,
        // )INP input session
        "INP" => SEC_RESTRICTED,
        // )SAVE save workspace
        "SAVE" => SEC_LOCKED,
        // )LOAD load workspace
        "LOAD" => SEC_LOCKED,
        // )OUT save session
        "OUT" => SEC_LOCKED,
        // file I/O
        "FIO" => SEC_LOCKED,
        // all other operations allowed at all levels
        _ => -1,
    };

    if blocked_at >= 0 && sec >= blocked_at {
        Err(format!(
            "SECURITY ERROR: {} is blocked at ⎕SEC={} (requires ⎕SEC<{})",
            operation, sec, blocked_at
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_sec_default() {
        let env = Environment::new();
        assert_eq!(get_sec(&env), SEC_NORMAL);
    }

    #[test]
    fn test_get_sec_after_init() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        assert_eq!(get_sec(&env), SEC_NORMAL);
    }

    #[test]
    fn test_check_sec_normal() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);

        // At ⎕SEC=0, everything allowed
        assert!(check_sec(&env, "EXECUTE").is_ok());
        assert!(check_sec(&env, "NA").is_ok());
        assert!(check_sec(&env, "LOADSO").is_ok());
        assert!(check_sec(&env, "COPY").is_ok());
        assert!(check_sec(&env, "SAVE").is_ok());
        assert!(check_sec(&env, "LOAD").is_ok());
        assert!(check_sec(&env, "OUT").is_ok());
        assert!(check_sec(&env, "FIO").is_ok());
    }

    #[test]
    fn test_check_sec_restricted() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.set(
            crate::sysvars::SEC_VAR,
            ValueP::scalar_from(Cell::Int(SEC_RESTRICTED)),
        );

        // At ⎕SEC=1, ⍎/⎕NA/⎕LOADSO/)COPY/)INP blocked
        assert!(check_sec(&env, "EXECUTE").is_err());
        assert!(check_sec(&env, "NA").is_err());
        assert!(check_sec(&env, "LOADSO").is_err());
        assert!(check_sec(&env, "COPY").is_err());
        assert!(check_sec(&env, "INP").is_err());

        // )SAVE/)LOAD/)OUT/)FIO still allowed
        assert!(check_sec(&env, "SAVE").is_ok());
        assert!(check_sec(&env, "LOAD").is_ok());
        assert!(check_sec(&env, "OUT").is_ok());
        assert!(check_sec(&env, "FIO").is_ok());
    }

    #[test]
    fn test_check_sec_locked() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.set(
            crate::sysvars::SEC_VAR,
            ValueP::scalar_from(Cell::Int(SEC_LOCKED)),
        );

        // At ⎕SEC=2, everything blocked
        assert!(check_sec(&env, "EXECUTE").is_err());
        assert!(check_sec(&env, "NA").is_err());
        assert!(check_sec(&env, "LOADSO").is_err());
        assert!(check_sec(&env, "COPY").is_err());
        assert!(check_sec(&env, "SAVE").is_err());
        assert!(check_sec(&env, "LOAD").is_err());
        assert!(check_sec(&env, "OUT").is_err());
        assert!(check_sec(&env, "FIO").is_err());
    }

    #[test]
    fn test_check_sec_unknown_operation() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.set(
            crate::sysvars::SEC_VAR,
            ValueP::scalar_from(Cell::Int(SEC_LOCKED)),
        );
        // Unknown operations are always allowed
        assert!(check_sec(&env, "UNKNOWN_OP").is_ok());
    }
}
