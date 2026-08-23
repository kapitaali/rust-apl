//! Defined functions (∇-style) — a simplified GNU APL function model.
//!
//! A function has:
//! - a **header**: `[result ←] NAME [args]` where args is `X` (monadic),
//!   `Y` (dyadic), or both (`X NAME Y`). Ambivalent: no args.
//! - a **body**: pre-parsed expression list (one per source line).
//!   The value of the last line is returned; an explicit assignment
//!   to the result name (if any) also works.
//! - **locals**: names assigned in the body that shadow globals.
//!
//! Recursion works because calls go through the Environment, which now
//! supports scoping.

use crate::parser::{parse, Expr};
use crate::tokenizer::{tokenize, Tok};

/// one defined function
#[derive(Clone, Debug)]
pub struct DefinedFunction {
    pub name: String,
    /// Some(result-var) if the header declares `Z←NAME ...`
    pub result: Option<String>,
    /// monadic left arg name (from `X NAME Y` header)
    pub arg_left: Option<String>,
    /// dyadic right arg name (from `NAME Y`)
    pub arg_right: Option<String>,
    /// pre-parsed body lines
    pub body: Vec<Expr>,
    /// structured control blocks (:If/:EndIf, :While/:EndWhile), each with
    /// its position in `body` so the interpreter can jump
    pub control: Vec<ControlBlock>,
    /// body indices of :Leave marker lines (no-ops unless a loop sees them)
    pub leave_lines: Vec<usize>,
    /// original source lines of the body (for workspace persistence).
    /// Control-marker lines are included; blank lines are not (they are
    /// dropped during parsing, so body/leave_lines/control indices refer
    /// to the compacted line list, same as `source`).
    pub source: Vec<String>,
    /// true for compiler-generated functions (dfns) whose pseudo-source is
    /// not valid APL — workspace save() must skip them
    pub no_save: bool,
    /// true when this function was created from a dfn `{...}` definition
    pub is_dfn: bool,
    /// true when this dfn references `⍺⍺` or `⍵⍵` (a "dop" - dyadic operator)
    pub is_dop: bool,
    /// left operand function for a dop (the function applied to ⍺)
    pub dop_lo: Option<crate::functions::Prim>,
    /// right operand function for a dop (the function applied to ⍵)
    pub dop_ro: Option<crate::functions::Prim>,
}

/// branch_stack sentinel pushed by :Leave — never a legal branch target
pub const LEAVE_SENTINEL: i64 = i64::MIN;

/// a structured control block extracted from the body at definition time.
/// `start`/`end` are 0-based line indices into `body`; `end` is EXCLUSIVE
/// (points just past :EndIf / :EndWhile). `cond` is the parsed condition
/// from the :If/:While line. For If-blocks with :Else, `else_start` marks
/// the :Else line (body lines start+1..else_start run when true;
/// else_start+1..end-1 run when false).
#[derive(Clone, Debug)]
pub enum ControlBlock {
    /// lines[start] is `:If cond`; body lines start+1..end-1 run when true
    If {
        start: usize,
        end: usize,
        cond: Expr,
        else_start: Option<usize>,
    },
    /// lines[start] is `:While cond`; loop while cond true
    While {
        start: usize,
        end: usize,
        cond: Expr,
    },
    /// lines[start] is `:Repeat`; body runs start+1..until_pos, then the
    /// `:Until cond` line is checked — repeat while cond is FALSE.
    /// `until_pos`/`until_cond` refer to the :Until line; end is past
    /// :EndRepeat. A :Repeat with no :Until loops forever (branch out).
    Repeat {
        start: usize,
        until_pos: Option<usize>,
        until_cond: Option<Expr>,
        end: usize,
    },
}

impl DefinedFunction {
    pub fn arity(&self) -> u8 {
        match (self.arg_left.is_some(), self.arg_right.is_some()) {
            (false, false) => 0,
            (false, true) => 1,
            (true, true) => 2,
            // X NAME without Y is not valid; treat as ambivalent
            (true, false) => 0,
        }
    }
}

/// storage of all defined functions (owned by the Environment)
#[derive(Default, Debug, Clone)]
pub struct FunctionTable {
    funcs: std::collections::HashMap<String, DefinedFunction>,
}

impl FunctionTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<&DefinedFunction> {
        self.funcs.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut DefinedFunction> {
        self.funcs.get_mut(name)
    }

    pub fn insert(&mut self, f: DefinedFunction) {
        self.funcs.insert(f.name.clone(), f);
    }

    pub fn remove(&mut self, name: &str) {
        self.funcs.remove(name);
    }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.funcs.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn clear(&mut self) {
        self.funcs.clear();
    }
}

/// header parse result: (name, result-var, left-arg, right-arg)
pub type HeaderParts = (String, Option<String>, Option<String>, Option<String>);

/// Parse a header line like `Z←ADD X Y`, `DOUBLE X`, `HELLO`, `R←FAC N`
/// into a [HeaderParts].
pub fn parse_header(line: &str) -> Result<HeaderParts, String> {
    let toks = tokenize(line).map_err(|e| format!("header tokenize error: {}", e))?;
    let mut names: Vec<String> = Vec::new();
    let mut has_result_arrow = false;
    let mut result_name: Option<String> = None;

    for t in &toks {
        match t {
            Tok::Name(n) => names.push(n.clone()),
            Tok::Assign => has_result_arrow = true,
            Tok::End => {}
            _ => return Err(format!("unexpected token in header: {:?}", t)),
        }
    }

    if has_result_arrow && !names.is_empty() {
        result_name = Some(names.remove(0));
    }
    if names.is_empty() {
        return Err("no function name in header".into());
    }
    let fname = names.remove(0);

    // remaining: [] | [Y] | [X Y]
    let (arg_left, arg_right) = match names.len() {
        0 => (None, None),
        1 => (None, Some(names[0].clone())),
        2 => (Some(names[0].clone()), Some(names[1].clone())),
        n => return Err(format!("too many names in header ({})", n)),
    };

    Ok((fname, result_name, arg_left, arg_right))
}

/// Define a function from header + body lines.
/// Body lines are tokenized+parsed once here.
pub fn define_function(
    table: &mut FunctionTable,
    header_line: &str,
    body_lines: &[String],
) -> Result<(), String> {
    let (name, result, arg_left, arg_right) = parse_header(header_line)?;

    // compacted line list: control markers + code lines kept in order,
    // blank lines dropped. ALL index spaces (body slots, leave_lines,
    // control blocks, source) refer to this list, so persistence can
    // round-trip the exact same definition regardless of blank lines.
    let mut compacted: Vec<String> = Vec::new();
    for line in body_lines {
        if line.trim().is_empty() {
            continue; // blank line: no body slot
        }
        compacted.push(line.clone());
    }

    // parse each compacted line (control markers become no-op slots)
    let mut body = Vec::new();
    let mut leave_lines = Vec::new();
    for (i, line) in compacted.iter().enumerate() {
        if is_control_marker(line) {
            // control markers occupy a body slot (so indices match
            // scan_control_blocks) but parse to a no-op placeholder
            if line.trim_start().starts_with(":Leave") {
                leave_lines.push(i);
            }
            body.push(Expr::Num(0.0));
            continue;
        }
        let toks = tokenize(line).map_err(|e| format!("{}: {}", e, line))?;
        if matches!(toks.first(), Some(Tok::End)) {
            continue; // whitespace-only line already filtered above
        }
        let (expr, used) = parse(&toks).map_err(|e| format!("{}: {}", e, line))?;
        if !matches!(toks.get(used), Some(Tok::End)) {
            return Err(format!("trailing tokens in body line: {}", line));
        }
        body.push(expr);
    }

    // scan the COMPACTED text so block start/end indices align with body
    // slots and leave_lines (blank-line independent).
    let control = scan_control_blocks(&compacted)?;

    table.insert(DefinedFunction {
        name,
        result,
        arg_left,
        arg_right,
        body,
        control,
        leave_lines,
        source: compacted,
        no_save: false,
        is_dfn: false,
        is_dop: false,
        dop_lo: None,
        dop_ro: None,
    });
    Ok(())
}

/// detect `:If` / `:While` ... `:EndIf` / `:EndWhile` in raw body lines.
/// Returns blocks with start = the :If/:While line index, end = index just
/// past the matching :EndIf/:EndWhile, and the parsed condition.
fn scan_control_blocks(lines: &[String]) -> Result<Vec<ControlBlock>, String> {
    let mut out: Vec<ControlBlock> = Vec::new();
    // stack entries: (kind, start_idx, marker_pos) where kind 0=:If, 1=:While,
    // 2=:Repeat; marker_pos holds the recorded :Else or :Until line.
    let mut stack: Vec<(u8, usize, Option<usize>)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with(":If") {
            stack.push((0, i, None));
        } else if t.starts_with(":While") {
            stack.push((1, i, None));
        } else if t.starts_with(":Repeat") {
            stack.push((2, i, None));
        } else if t.starts_with(":Else") || t.starts_with(":Until") {
            if let Some(top) = stack.last_mut() {
                top.2 = Some(i);
            }
        } else if t.starts_with(":EndIf") {
            if let Some((0, s, else_pos)) = stack.pop() {
                let cond = parse_control_cond(&lines[s], ":If")?;
                out.push(ControlBlock::If {
                    start: s,
                    end: i + 1,
                    cond,
                    else_start: else_pos,
                });
            }
        } else if t.starts_with(":EndWhile") {
            if let Some((1, s, _)) = stack.pop() {
                let cond = parse_control_cond(&lines[s], ":While")?;
                out.push(ControlBlock::While {
                    start: s,
                    end: i + 1,
                    cond,
                });
            }
        } else if t.starts_with(":EndRepeat") {
            if let Some((2, s, until_pos)) = stack.pop() {
                let until_cond = match until_pos {
                    Some(u) => Some(parse_control_cond(&lines[u], ":Until")?),
                    None => None,
                };
                out.push(ControlBlock::Repeat {
                    start: s,
                    until_pos,
                    until_cond,
                    end: i + 1,
                });
            }
        }
    }
    if !stack.is_empty() {
        return Err("unterminated :If/:While/:Repeat block".to_string());
    }
    Ok(out)
}

/// parse the condition text after `:If`/`:While` (and before any ⋄)
fn parse_control_cond(line: &str, kw: &str) -> Result<Expr, String> {
    let rest = line.trim_start()[kw.len()..].trim();
    // cut at first diamond (statement separators after the condition)
    let cond_text = match rest.find('⋄') {
        Some(p) => &rest[..p],
        None => rest,
    };
    let toks = tokenize(cond_text).map_err(|e| format!("{} in {}", e, line))?;
    let (expr, used) = parse(&toks).map_err(|e| format!("{} in {}", e, line))?;
    if !matches!(toks.get(used), Some(Tok::End)) {
        return Err(format!("trailing tokens in {} condition", kw));
    }
    Ok(expr)
}

/// true if body line `i` is a control marker (:If/:While/:End.../:Else/:Repeat/:Until/:Leave)
pub fn is_control_marker(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with(":If")
        || t.starts_with(":While")
        || t.starts_with(":EndIf")
        || t.starts_with(":EndWhile")
        || t.starts_with(":Else")
        || t.starts_with(":Repeat")
        || t.starts_with(":Until")
        || t.starts_with(":EndRepeat")
        || t.starts_with(":Leave")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header_ambivalent() {
        let (n, r, l, rr) = parse_header("HELLO").unwrap();
        assert_eq!(n, "HELLO");
        assert_eq!(r, None);
        assert_eq!(l, None);
        assert_eq!(rr, None);
    }

    #[test]
    fn test_parse_header_monadic() {
        let (n, r, l, rr) = parse_header("DOUBLE X").unwrap();
        assert_eq!(n, "DOUBLE");
        assert_eq!(r, None);
        assert_eq!(l, None);
        assert_eq!(rr.unwrap(), "X");
    }

    #[test]
    fn test_parse_header_dyadic_result() {
        let (n, r, l, rr) = parse_header("R←ADD A B").unwrap();
        assert_eq!(n, "ADD");
        assert_eq!(r.unwrap(), "R");
        assert_eq!(l.unwrap(), "A");
        assert_eq!(rr.unwrap(), "B");
    }

    #[test]
    fn test_define_and_lookup() {
        let mut table = FunctionTable::new();
        define_function(&mut table, "R←FAC N", &["R←N".to_string(), "R".to_string()]).unwrap();
        assert!(table.get("FAC").is_some());
        let f = table.get("FAC").unwrap();
        assert_eq!(f.result.as_deref(), Some("R"));
        assert_eq!(f.arg_right.as_deref(), Some("N"));
        assert_eq!(f.body.len(), 2);
    }

    #[test]
    fn test_define_blank_body_lines_skipped() {
        let mut table = FunctionTable::new();
        define_function(
            &mut table,
            "F",
            vec!["".to_string(), "1+1".to_string()].as_slice(),
        )
        .unwrap();
        assert_eq!(table.get("F").unwrap().body.len(), 1);
    }
}
