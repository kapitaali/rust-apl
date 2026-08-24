//! Tests for the ⎕NA declaration parser (Phase F2).
//!
//! Acceptance: every example line from the Dyalog 19.0 ⎕NA documentation
//! parses or rejects as documented.

use crate::ffi::nadecl::*;
use crate::types::ErrorCode;

fn ok(s: &str) -> CAbiSpec {
    parse_na_decl(s).unwrap_or_else(|e| panic!("expected Ok for {:?}, got {:?}", s, e))
}

fn is_syntax(s: &str) {
    match parse_na_decl(s) {
        Err(ErrorCode::SyntaxError) => {}
        other => panic!("expected SyntaxError for {:?}, got {:?}", s, other),
    }
}

fn is_domain(s: &str) {
    match parse_na_decl(s) {
        Err(ErrorCode::DomainError) => {}
        other => panic!("expected DomainError for {:?}, got {:?}", s, other),
    }
}

// ---- the canonical example from the doc --------------------------------

#[test]
fn test_divide() {
    let spec = ok("F8 math|divide I4 I4");
    assert_eq!(spec.library, "math");
    assert_eq!(spec.symbol, "divide");
    let r = spec.result.unwrap();
    assert_eq!(r.leaf, LeafType::Float);
    assert_eq!(r.width, Width::W8);
    assert_eq!(spec.args.len(), 2);
    for a in &spec.args {
        assert_eq!(a.leaf, LeafType::Int);
        assert_eq!(a.width, Width::W4);
    }
}

#[test]
fn test_named_association() {
    // 'div' ⎕NA 'F8 math|divide I4 I4' — same decl, name comes from LEFT arg
    let _ = ok("F8   math|divide       I4        I4 "); // extra ws tolerated
}

#[test]
fn test_no_result() {
    let spec = ok("glu32|gluPerspective F8 F8 F8 F8");
    assert!(spec.result.is_none());
    assert_eq!(spec.symbol, "gluPerspective");
    assert_eq!(spec.args.len(), 4);
}

// ---- direction / special markers ----------------------------------------

#[test]
fn test_directions() {
    let spec = ok("lib|f <I2 >C =T");
    assert_eq!(spec.args.len(), 3);
    assert_eq!(spec.args[0].dir, Direction::In);
    assert_eq!(spec.args[1].dir, Direction::Out);
    assert_eq!(spec.args[2].dir, Direction::InOut);
}

#[test]
fn test_strings() {
    let spec = ok("lib|f <0T >0T[] =#C");
    assert_eq!(spec.args[0].special, Special::NulTerm);
    assert_eq!(spec.args[1].special, Special::NulTerm);
    assert!(spec.args[1].array_open);
    assert_eq!(spec.args[2].special, Special::ByteCounted);
}

#[test]
fn test_utf() {
    let a = ok("lib|f >0UTF8[]");
    assert_eq!(a.args[0].leaf, LeafType::Utf8);
    let b = ok("lib|f <0UTF16[]");
    assert_eq!(b.args[0].leaf, LeafType::Utf16);
}

// ---- arrays --------------------------------------------------------------

#[test]
fn test_fixed_array() {
    let spec = ok("lib|f I[10] U U[]");
    assert_eq!(spec.args[0].array_len, Some(10));
    assert!(spec.args[1].array_len.is_none());
    assert!(!spec.args[1].array_open);
    assert!(spec.args[2].array_open);
}

#[test]
fn test_scalar_vs_vector_distinction() {
    // FooScalar vs FooVector pattern from the doc
    let scalar = ok("mydll|foo <T");
    assert!(!scalar.args[0].array_open && scalar.args[0].array_len.is_none());
    let vector = ok("mydll|foo <T[]");
    assert!(vector.args[0].array_open);
}

// ---- structures ----------------------------------------------------------

#[test]
fn test_struct_basic() {
    let spec = ok("mydll.foo U <{F8 I2}[]");
    assert_eq!(spec.library, "mydll"); // '.' separator also accepted
    assert_eq!(spec.symbol, "foo");
    let st = &spec.args[1];
    assert!(st.is_struct);
    assert!(st.array_open);
    assert_eq!(st.members.len(), 2);
    assert_eq!(st.members[0].leaf, LeafType::Float);
    assert_eq!(st.members[1].leaf, LeafType::Int);
}

#[test]
fn test_struct_with_alignment_note() {
    // {I4 [pad]} — struct members parse individually
    let spec = ok("kernel32|GlobalMemoryStatusEx ={U4 U4 U8 U8 U8 U8 U8 U8}");
    assert_eq!(spec.args[0].members.len(), 8);
}

#[test]
fn test_struct_array_fixed() {
    let spec = ok("x|f <{F8 I2}[3]");
    assert_eq!(spec.args[0].array_len, Some(3));
}

// ---- real-world example lines from the Dyalog doc -------------------------

#[test]
fn test_registry_family() {
    for line in [
        "I4 advapi32|RegCloseKey P",
        "I4 advapi32|RegCreateKeyEx* P <0T U4 <0T U4 U4 P >P >U4",
        "I4 advapi32|RegOpenKey* P <0T >P",
        "P dyalog32|STRNCPY P P P",
    ] {
        ok(line);
    }
}

#[test]
fn test_kernel_user_gdi() {
    for line in [
        "U4 kernel32|GetLastError",
        "P kernel32|GetEnvironmentStrings",
        "U4 kernel32|GetTempPath* U4 >0T",
        "I4 user32|MessageBox* P <0T <0T U4",
        "I4 user32|ShowWindow P I4",
        "U4 gdi32|GetPixel P I4 I4",
        "opengl32|glClearColor F4 F4 F4 F4",
        "opengl32|glClearDepth F8",
    ] {
        ok(line);
    }
}

#[test]
fn test_star_suffix_symbols() {
    // '*' after symbol names (decorates stdcall exports) must not confuse
    // the parser — it's part of the SYMBOL word.
    let spec = ok("I4 advapi32|RegOpenKey* P <0T >P");
    assert_eq!(spec.symbol, "RegOpenKey*");
}

#[test]
fn test_pathname_with_dirs() {
    let spec = ok("F8 c:\\mydir\\mydll|foo I4");
    assert_eq!(spec.library, "c:\\mydir\\mydll");
    assert_eq!(spec.symbol, "foo");
}

// ---- rejections ------------------------------------------------------------

#[test]
fn test_empty_rejected() {
    is_syntax("");
    is_syntax("   ");
}

#[test]
fn test_bad_type_letter() {
    is_syntax("Q foo I4");
    is_syntax("F8 lib|foo Q4");
}

#[test]
fn test_unclosed_struct() {
    is_syntax("{F8 I2 foo");
}

#[test]
fn test_unclosed_array() {
    is_syntax("F8 lib|foo I4[");
    is_syntax("F8 lib|foo I4[4");
}

#[test]
fn test_bad_width() {
    is_syntax("I3 lib|foo"); // 3 not a legal width
    is_domain("F8 lib|f F16"); // float can't be 16 bytes
}

#[test]
fn test_illegal_widths_by_type() {
    is_domain("I4 lib|f F16"); // float max 8
    is_domain("I4 lib|f C8"); // char max 4
}

#[test]
fn test_d16_unsupported_v1() {
    is_domain("D16 lib|dec16 D16");
}

#[test]
fn test_nabla_deferred_v2() {
    is_domain("∇ lib|callback ∇");
}

#[test]
fn test_special_without_direction() {
    is_domain("0T lib|foo"); // special requires a direction marker
}

#[test]
fn test_out_result_rejected() {
    is_domain(">I4 lib|foo");
}

#[test]
fn test_too_many_args() {
    let decl = "lib|f I4 I4 I4 I4 I4 I4 I4 I4 I4 I4 I4 I4 I4"; // 13
    match parse_na_decl(decl) {
        Err(ErrorCode::SyntaxError) => {}
        other => panic!("expected SyntaxError, got {:?}", other),
    }
}

#[test]
fn test_twelve_args_ok() {
    ok("lib|f I4 I4 I4 I4 I4 I4 I4 I4 I4 I4 I4 I4");
}
