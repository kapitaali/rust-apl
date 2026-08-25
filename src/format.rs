//! Format ⍕ (monadic: value → character representation) and Where ⍸
//! (monadic: indices of 1s in a boolean vector).
//!
//! Mirrors `Bif_F12_FORMAT` and `Bif_F12_INDEX_OF` (⍸ portion) in C++.

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// build a character vector from a Rust string (Cell::Char holds codepoints)
fn char_vec_from_str(s: &str) -> ValueP {
    let cps: Vec<u32> = s.chars().map(|c| c as u32).collect();
    ValueP::char_vector(&cps)
}

/// ⍕B — format: render B as its character representation.
///
/// Scalars and vectors become a simple character vector. Matrices (rank ≥ 2)
/// become a character matrix whose rows are the rendered lines, right-padded
/// to a common width.
pub fn format(b: &ValueP) -> AplResult<ValueP> {
    format_with_pp(b, 10)
}

/// ⍕B honoring print precision (⎕PP).
pub fn format_with_pp(b: &ValueP, pp: usize) -> AplResult<ValueP> {
    let lines = crate::boxdisplay::render_with_pp(b, pp);
    if lines.len() <= 1 {
        // scalar / vector / empty → simple character vector
        let text = lines.first().map(String::as_str).unwrap_or("");
        return Ok(char_vec_from_str(text));
    }
    // multiple lines → character matrix, rows padded to the widest line
    let rows = lines.len() as i64;
    let cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i64;
    let mut ravel = Vec::with_capacity((rows * cols) as usize);
    for line in &lines {
        let mut n = 0i64;
        for ch in line.chars() {
            ravel.push(Cell::Char(ch as u32));
            n += 1;
        }
        while n < cols {
            ravel.push(Cell::Char(' ' as u32));
            n += 1;
        }
    }
    ValueP::from_parts(Shape::matrix(rows, cols), ravel)
}

/// A⍕B — dyadic format: format B with A specifying width and decimals.
///
/// A is either a scalar (decimals only, width auto) or a 2-element vector
/// `width decimals`. Every element of B is rendered in a field of that
/// width with that many decimal places, and the results are catenated into
/// one character vector (rank ≤ 1) or a character matrix (rank ≥ 2).
pub fn format_dyadic(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let ac = a.cells();
    let (width, decimals) = match ac.len() {
        1 => (0i64, ac[0].get_int_value()?),
        2 => (ac[0].get_int_value()?, ac[1].get_int_value()?),
        _ => return Err(ErrorCode::LengthError),
    };
    if decimals < 0 || width < 0 {
        return Err(ErrorCode::DomainError);
    }
    let dec = decimals as usize;

    // render each cell into its field
    let mut fields: Vec<String> = Vec::with_capacity(b.element_count() as usize);
    for c in b.cells() {
        let f = match c {
            Cell::Int(i) => format!("{:.*}", dec, *i as f64),
            Cell::Float(f) => format!("{:.*}", dec, f),
            Cell::Char(ch) => ch.to_string(),
            _ => return Err(ErrorCode::DomainError),
        };
        // APL renders negatives with the high minus ¯
        let f = if let Some(stripped) = f.strip_prefix('-') {
            format!("¯{stripped}")
        } else {
            f
        };
        if width > 0 {
            let w = width as usize;
            let len = f.chars().count();
            if len > w {
                // field too narrow → DOMAIN ERROR (GNU APL signals this)
                return Err(ErrorCode::DomainError);
            }
            fields.push(format!("{}{}", " ".repeat(w - len), f));
        } else {
            fields.push(f);
        }
    }

    if b.rank() <= 1 {
        // vector → one character vector, fields separated by a space when
        // no explicit width was given
        let sep = if width > 0 { "" } else { " " };
        return Ok(char_vec_from_str(&fields.join(sep)));
    }

    // matrix → each row of B becomes one row of characters
    let cols = b.get_shape_item(b.rank() as i16 - 1) as usize;
    let rows = fields.len() / cols.max(1);
    let sep = if width > 0 { "" } else { " " };
    let mut lines: Vec<String> = Vec::with_capacity(rows);
    for r in 0..rows {
        lines.push(fields[r * cols..(r + 1) * cols].join(sep));
    }
    let out_cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i64;
    let mut ravel = Vec::with_capacity(rows * out_cols as usize);
    for line in &lines {
        let mut n = 0i64;
        for ch in line.chars() {
            ravel.push(Cell::Char(ch as u32));
            n += 1;
        }
        while n < out_cols {
            ravel.push(Cell::Char(' ' as u32));
            n += 1;
        }
    }
    ValueP::from_parts(Shape::matrix(rows as i64, out_cols), ravel)
}

/// ⍸B — where: indices of the 1s in boolean B (0-based; caller adds ⎕IO).
///
/// B must be a boolean array. For rank ≤ 1 the result is a vector of
/// positions. For higher rank the result is a nested vector of index
/// vectors, one per set element.
pub fn where_indices(b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    let cells = b.cells();

    // validate booleans up front
    for c in cells {
        let v = c.get_int_value()?;
        if v != 0 && v != 1 {
            return Err(ErrorCode::DomainError);
        }
    }

    if rank <= 1 {
        let mut out = Vec::new();
        for (i, c) in cells.iter().enumerate() {
            if c.get_int_value()? == 1 {
                out.push(Cell::Int(i as i64));
            }
        }
        let n = out.len() as i64;
        return ValueP::from_parts(Shape::vector(n), out);
    }

    // rank ≥ 2: each hit becomes an enclosed index vector
    let dims: Vec<i64> = (0..rank).map(|i| b.get_shape_item(i as i16)).collect();
    let mut out = Vec::new();
    for (lin, c) in cells.iter().enumerate() {
        if c.get_int_value()? != 1 {
            continue;
        }
        // decode the linear position into per-axis subscripts
        let mut rem = lin as i64;
        let mut subs = vec![0i64; dims.len()];
        for ax in (0..dims.len()).rev() {
            let d = dims[ax].max(1);
            subs[ax] = rem % d;
            rem /= d;
        }
        let idx = ValueP::int_vector(&subs);
        out.push(ValueP::nested(idx).first_cell().unwrap().clone());
    }
    let n = out.len() as i64;
    ValueP::from_parts(Shape::vector(n), out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars_of(v: &ValueP) -> String {
        v.cells()
            .iter()
            .map(|c| match c {
                Cell::Char(ch) => char::from_u32(*ch).unwrap_or('?'),
                _ => '?',
            })
            .collect()
    }

    #[test]
    fn test_format_scalar() {
        let v = ValueP::scalar_from(Cell::Int(42));
        let r = format(&v).unwrap();
        assert_eq!(chars_of(&r), "42");
        assert_eq!(r.rank(), 1);
    }

    #[test]
    fn test_format_vector() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let r = format(&v).unwrap();
        assert_eq!(chars_of(&r), "1 2 3");
    }

    #[test]
    fn test_format_matrix_is_char_matrix() {
        let v = ValueP::from_parts(
            Shape::matrix(2, 2),
            vec![Cell::Int(1), Cell::Int(2), Cell::Int(3), Cell::Int(4)],
        )
        .unwrap();
        let r = format(&v).unwrap();
        assert_eq!(r.rank(), 2);
        assert_eq!(r.get_shape_item(0), 2);
        // every cell is a character
        assert!(r.cells().iter().all(|c| matches!(c, Cell::Char(_))));
    }

    #[test]
    fn test_format_dyadic_decimals() {
        // 2⍕1.5 2.25 → "1.50 2.25"
        let a = ValueP::scalar_from(Cell::Int(2));
        let b = ValueP::from_parts(Shape::vector(2), vec![Cell::Float(1.5), Cell::Float(2.25)])
            .unwrap();
        let r = format_dyadic(&a, &b).unwrap();
        assert_eq!(chars_of(&r), "1.50 2.25");
    }

    #[test]
    fn test_format_dyadic_width_decimals() {
        // 6 2⍕1.5 → "  1.50"
        let a = ValueP::int_vector(&[6, 2]);
        let b = ValueP::scalar_from(Cell::Float(1.5));
        let r = format_dyadic(&a, &b).unwrap();
        assert_eq!(chars_of(&r), "  1.50");
    }

    #[test]
    fn test_format_dyadic_negative_high_minus() {
        let a = ValueP::scalar_from(Cell::Int(1));
        let b = ValueP::scalar_from(Cell::Float(-2.5));
        let r = format_dyadic(&a, &b).unwrap();
        assert_eq!(chars_of(&r), "¯2.5");
    }

    #[test]
    fn test_format_dyadic_too_narrow_errors() {
        let a = ValueP::int_vector(&[2, 2]);
        let b = ValueP::scalar_from(Cell::Float(123.456));
        assert!(format_dyadic(&a, &b).is_err());
    }

    #[test]
    fn test_where_vector() {
        // ⍸0 1 0 1 1 → 1 3 4 (0-based)
        let b = ValueP::int_vector(&[0, 1, 0, 1, 1]);
        let r = where_indices(&b).unwrap();
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![1, 3, 4]);
    }

    #[test]
    fn test_where_all_zeros_is_empty() {
        let b = ValueP::int_vector(&[0, 0, 0]);
        let r = where_indices(&b).unwrap();
        assert_eq!(r.element_count(), 0);
    }

    #[test]
    fn test_where_rejects_non_boolean() {
        let b = ValueP::int_vector(&[0, 2, 1]);
        assert!(where_indices(&b).is_err());
    }

    #[test]
    fn test_where_matrix_gives_index_vectors() {
        // 2x2 with 1s at [0,1] and [1,0]
        let b = ValueP::from_parts(
            Shape::matrix(2, 2),
            vec![Cell::Int(0), Cell::Int(1), Cell::Int(1), Cell::Int(0)],
        )
        .unwrap();
        let r = where_indices(&b).unwrap();
        assert_eq!(r.element_count(), 2);
        // each element is an enclosed 2-element index vector
        for c in r.cells() {
            match c {
                Cell::Pointer(p) => {
                    let inner = ValueP {
                        inner: p.value.clone(),
                    };
                    assert_eq!(inner.element_count(), 2);
                }
                other => panic!("expected nested index vector, got {other:?}"),
            }
        }
    }
}
