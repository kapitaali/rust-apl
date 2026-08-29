//! Tests for the ∇ (nabla) function editor.
//!
//! The ∇ editor allows defining functions interactively:
//! - `∇HEADER` starts a function definition
//! - Body lines follow
//! - `∇` alone ends the definition

use apl::functions_def::define_function;
use apl::parser::Environment;

fn eval_one(env: &mut Environment, line: &str) -> apl::value::ValueP {
    env.eval_line(line).unwrap().unwrap()
}

#[test]
fn test_nabla_monadic() {
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let body = vec!["X×2".to_string()];
    define_function(&mut env.funcs, "DOUBLE X", &body).unwrap();
    let result = eval_one(&mut env, "DOUBLE 21");
    assert_eq!(result.first_cell().unwrap(), &apl::cell::Cell::Int(42));
}

#[test]
fn test_nabla_dyadic() {
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let body = vec!["A+B".to_string()];
    define_function(&mut env.funcs, "SUM A B", &body).unwrap();
    let result = eval_one(&mut env, "5 SUM 7");
    assert_eq!(result.first_cell().unwrap(), &apl::cell::Cell::Int(12));
}

#[test]
fn test_nabla_with_result() {
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let body = vec!["R←N×2".to_string()];
    define_function(&mut env.funcs, "R←FACT N", &body).unwrap();
    let result = eval_one(&mut env, "FACT 6");
    assert_eq!(result.first_cell().unwrap(), &apl::cell::Cell::Int(12));
}

#[test]
fn test_nabla_multiline() {
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let body = vec![
        "R←1".to_string(),
        "I←N".to_string(),
        ":While I>1".to_string(),
        "R←R×I".to_string(),
        "I←I-1".to_string(),
        ":EndWhile".to_string(),
    ];
    define_function(&mut env.funcs, "R←FAC N", &body).unwrap();
    let result = eval_one(&mut env, "FAC 5");
    assert_eq!(result.first_cell().unwrap(), &apl::cell::Cell::Int(120));
}

#[test]
fn test_nabla_recursive() {
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let body = vec![
        ":If N≤1".to_string(),
        "R←1".to_string(),
        ":Else".to_string(),
        "R←N×∇ N-1".to_string(),
        ":EndIf".to_string(),
    ];
    define_function(&mut env.funcs, "R←FAC N", &body).unwrap();
    let result = eval_one(&mut env, "FAC 5");
    assert_eq!(result.first_cell().unwrap(), &apl::cell::Cell::Int(120));
}

#[test]
fn test_nabla_ambivalent() {
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    // Ambivalent function: no args in header, body uses ⍵
    // In APL, ambivalent functions can be called monadically (FN B) or dyadically (A FN B)
    let body = vec!["⍵+1".to_string()];
    define_function(&mut env.funcs, "INCR", &body).unwrap();
    let result = eval_one(&mut env, "INCR 5");
    assert_eq!(result.first_cell().unwrap(), &apl::cell::Cell::Int(6));
}

#[test]
fn test_nabla_with_branch() {
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let body = vec![
        ":If N=0".to_string(),
        "→1".to_string(),
        ":EndIf".to_string(),
        "R←N×2".to_string(),
        "R".to_string(),
    ];
    define_function(&mut env.funcs, "R←EVEN N", &body).unwrap();
    let result = eval_one(&mut env, "EVEN 4");
    assert_eq!(result.first_cell().unwrap(), &apl::cell::Cell::Int(8));
}
