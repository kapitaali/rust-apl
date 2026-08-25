//! Boxed display of (nested) arrays — a `4⎕CR`-style renderer.
//!
//! Mirrors GNU APL's boxed output:
//! ```text
//! ┏→━━━━━┓
//! ┃1 2 3┃      simple vector in a box
//! ┗━━━━━┛
//!
//! ┏→━━┓ ┏→━━━━┓
//! ┃1 2┃ ┃3 4 5┃    nested: each item gets its own box
//! ┗∼━━┛ ┗∼━━━━┛
//! ```
//!
//! Box characters (matching the C++ PrintBuffer/BoxChar output):
//! - `┏→` top-left for vectors/scalars, `┏↓` / `┏ϵ` variants for matrices /
//!   nested content, `━━` fill, `┓ ┃ ┗┛` edges.
//! - `∼` in the bottom edge marks a *nested* (non-simple) item.
//!
//! We implement the simplified subset: scalar boxes, vector boxes,
//! matrix boxes, and recursive nesting with `∼` markers.

use crate::cell::Cell;
use crate::value::ValueP;

/// render a value as a list of lines (boxed if non-simple), honoring ⎕PP
pub fn render(v: &ValueP) -> Vec<String> {
    render_with_pp(v, 10)
}

/// render honoring print precision (⎕PP): floats show up to `pp` decimals
pub fn render_with_pp(v: &ValueP, pp: usize) -> Vec<String> {
    let is_simple = v.cells().iter().all(|c| c.is_simple_cell());
    if v.rank() == 0 {
        // scalar: simple → bare text; pointer → box around inner
        return match v.first_cell() {
            Some(Cell::Pointer(p)) => {
                let inner = ValueP {
                    inner: p.value.clone(),
                };
                box_lines(&render_with_pp(&inner, pp), '→', is_nested(&inner))
            }
            _ => vec![plain_cell(v.first_cell().unwrap(), pp)],
        };
    }
    if is_simple && v.rank() == 1 {
        // simple vector → single line, no box at top level.
        // Character vectors print with NO separator (GNU APL prints 'hello'
        // as hello, not h e l l o); numeric vectors are space-separated.
        let sep = if all_chars(v.cells()) { "" } else { " " };
        return vec![v
            .cells()
            .iter()
            .map(|c| plain_cell(c, pp))
            .collect::<Vec<_>>()
            .join(sep)];
    }
    if is_simple && v.rank() >= 2 {
        // simple matrix → plain rows (GNU APL prints simple matrices bare)
        let cols = v.get_shape_item(1) as usize;
        let cells = v.cells();
        let sep = if all_chars(cells) { "" } else { " " };
        let mut rows = Vec::new();
        for r in 0..(cells.len() / cols.max(1)) {
            let line: Vec<String> = cells[r * cols..(r + 1) * cols]
                .iter()
                .map(|c| plain_cell(c, pp))
                .collect();
            rows.push(line.join(sep));
        }
        return rows;
    }
    if !is_simple && v.rank() == 1 {
        // nested vector → each element its own box, side by side
        let boxes: Vec<Vec<String>> = v
            .cells()
            .iter()
            .map(|c| match c {
                Cell::Pointer(p) => {
                    let inner = ValueP {
                        inner: p.value.clone(),
                    };
                    let nested = is_nested(&inner);
                    box_lines(&render_with_pp(&inner, pp), '→', nested)
                }
                other => vec![plain_cell(other, pp)],
            })
            .collect();
        return join_horizontal(&boxes);
    }
    if v.rank() == 2 {
        let cols = v.get_shape_item(1) as usize;
        let cells = v.cells();
        let mut rows: Vec<String> = Vec::new();
        for r in 0..(cells.len() / cols.max(1)) {
            let row_cells = &cells[r * cols..(r + 1) * cols];
            let row_boxes: Vec<Vec<String>> = row_cells
                .iter()
                .map(|c| match c {
                    Cell::Pointer(p) => {
                        let inner = ValueP {
                            inner: p.value.clone(),
                        };
                        box_lines(&render_with_pp(&inner, pp), '→', is_nested(&inner))
                    }
                    other => vec![plain_cell(other, pp)],
                })
                .collect();
            rows.extend(join_horizontal(&row_boxes));
        }
        return rows;
    }
    // rank ≥ 3 fallback
    vec![format!("⍴{}", shape_str(v))]
}

fn is_nested(v: &ValueP) -> bool {
    !v.cells().iter().all(|c| c.is_simple_cell())
}

/// true iff every cell is a character (a simple character array, which
/// GNU APL prints without separators)
fn all_chars(cells: &[Cell]) -> bool {
    !cells.is_empty() && cells.iter().all(|c| matches!(c, Cell::Char(_)))
}

fn plain_cell(c: &Cell, pp: usize) -> String {
    match c {
        Cell::Int(i) => i.to_string(),
        Cell::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else if f.abs() >= 1e-10 {
                // ⎕PP: up to `pp` decimals, trailing zeros trimmed
                format!("{:.*}", pp, f)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            } else {
                format!("{}", f)
            }
        }
        Cell::Char(u) => char::from_u32(*u).unwrap_or('?').to_string(),
        Cell::Complex(z) => format!("{}J{}", z.re, z.im),
        _ => "<lval>".to_string(),
    }
}

/// wrap already-rendered `lines` in a box. `arrow` selects `┏→` (content on
/// one line) vs `┏↓`-style (we use → everywhere except matrices, matching
/// the common GNU APL look). `nested` puts ∼ marks in the bottom edge.
fn box_lines(lines: &[String], arrow: char, nested: bool) -> Vec<String> {
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let mut out = Vec::with_capacity(lines.len() + 2);
    // GNU APL ground truth (UserCommand.tc): content "]FOO" (w=4) gives
    // ┏→━━━┓ / ┃]FOO┃ / ┗━━━━┛ — all 6 = w+2 chars. Top = ┏→ + (w-1) fills.
    let fills = width.saturating_sub(1);
    out.push(format!("┏{}{}┓", arrow, "━".repeat(fills)));
    for l in lines {
        let pad = width - l.chars().count();
        out.push(format!("┃{}{}┃", l, " ".repeat(pad)));
    }
    // bottom: ┗ + w fills + ┛ = w+2; ∼ marks nested content
    let mut bottom = String::from("┗");
    for k in 0..width {
        if nested && k == 0 {
            bottom.push('∼');
        } else {
            bottom.push('━');
        }
    }
    bottom.push('┛');
    out.push(bottom);
    out
}

/// place boxes side by side, vertically centered-ish (top aligned like GNU APL)
fn join_horizontal(boxes: &[Vec<String>]) -> Vec<String> {
    let height = boxes.iter().map(|b| b.len()).max().unwrap_or(0);
    let widths: Vec<usize> = boxes
        .iter()
        .map(|b| b.iter().map(|l| l.chars().count()).max().unwrap_or(0))
        .collect();
    let mut out = Vec::with_capacity(height);
    for r in 0..height {
        let mut line = String::new();
        for (k, b) in boxes.iter().enumerate() {
            let cell_line = b.get(r).cloned().unwrap_or_default();
            let pad = widths[k] - cell_line.chars().count();
            line.push_str(&cell_line);
            line.push_str(&" ".repeat(pad));
            line.push(' ');
        }
        // trim one trailing space
        out.push(line.trim_end().to_string());
    }
    out
}

fn shape_str(v: &ValueP) -> String {
    let dims: Vec<String> = (0..v.rank() as usize)
        .map(|a| v.get_shape_item(a as i16).to_string())
        .collect();
    dims.join(" ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::PointerCellData;
    use crate::shape::Shape;
    use crate::value::ValueInner;
    use std::sync::Arc;

    fn env_vec(line: &str) -> ValueP {
        let mut env = crate::parser::Environment::new();
        env.eval_line(line).unwrap();
        env.get("N").unwrap().clone()
    }

    #[test]
    fn test_simple_vector_bare() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let lines = render(&v);
        assert_eq!(lines, vec!["1 2 3"]);
    }

    #[test]
    fn test_char_vector_has_no_separators() {
        // GNU APL prints 'hello' as hello, not h e l l o
        let cps: Vec<u32> = "hello".chars().map(|c| c as u32).collect();
        let v = ValueP::char_vector(&cps);
        let lines = render(&v);
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn test_char_matrix_rows_have_no_separators() {
        let cps: Vec<Cell> = "abcd".chars().map(|c| Cell::Char(c as u32)).collect();
        let v = ValueP::from_parts(Shape::matrix(2, 2), cps).unwrap();
        let lines = render(&v);
        assert_eq!(lines, vec!["ab", "cd"]);
    }

    #[test]
    fn test_enclosed_vector_boxed() {
        // ⊂1 2 3 → box with all edges equal width
        let mut env = crate::parser::Environment::new();
        env.eval_line("Z←⊂1 2 3").unwrap();
        let v = env.get("Z").unwrap().clone();
        let lines = render(&v);
        assert_eq!(lines.len(), 3);
        for l in &lines {
            assert_eq!(l.chars().count(), lines[0].chars().count(), "ragged: {}", l);
        }
        assert!(lines[0].starts_with("┏→"));
        assert!(lines[1].contains("1 2 3"));
        assert!(lines[2].starts_with('┗'));
    }

    #[test]
    fn test_nested_vector_side_by_side() {
        // (1 2)(3 4 5): two boxes side by side, aligned
        let n = env_vec("N←(1 2)(3 4 5)");
        let lines = render(&n);
        assert_eq!(lines.len(), 3);
        let w = lines[0].chars().count();
        for l in &lines {
            assert_eq!(l.chars().count(), w, "misaligned row: {}", l);
        }
        // both boxes on the top line
        assert_eq!(lines[0].matches('┏').count(), 2);
        assert_eq!(lines[2].matches('┗').count(), 2);
    }

    #[test]
    fn test_nested_bottom_edge_has_tilde() {
        // a nested item's own bottom edge carries ∼ (per GNU APL Pick.tc):
        // build ⊂(1 2)(3 4) — enclosing a nested vector marks it nested
        let inner = ValueP::from_parts(
            Shape::vector(2),
            vec![
                Cell::Pointer(PointerCellData {
                    value: Arc::new(ValueInner::new(
                        Shape::vector(2),
                        vec![Cell::Int(1), Cell::Int(2)],
                    )),
                }),
                Cell::Pointer(PointerCellData {
                    value: Arc::new(ValueInner::new(
                        Shape::vector(2),
                        vec![Cell::Int(3), Cell::Int(4)],
                    )),
                }),
            ],
        )
        .unwrap();
        let enc = ValueP::nested(inner);
        let lines = render(&enc);
        assert!(lines[lines.len() - 1].contains('∼'), "nested mark missing");
    }

    #[test]
    fn test_mixed_matrix_rows() {
        // 2 2⍴1 'a' 2 'b': all cells SIMPLE (scalar strand items flatten),
        // so GNU APL renders it as a plain matrix — no boxes. Verified
        // against reference APL output "1 a / 2 b".
        let m = env_vec("N←2 2⍴1 'a' 2 'b'");
        let lines = render(&m);
        assert_eq!(lines.len(), 2); // one plain text line per row
        assert!(lines.iter().all(|l| !l.contains('┏')));
        assert!(lines[0].contains('1') && lines[0].contains('a'));
        assert!(lines[1].contains('2') && lines[1].contains('b'));
    }
}
