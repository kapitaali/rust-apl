//! Domino `⌹` — matrix inverse (monadic) and matrix divide (dyadic).
//!
//! Mirrors `src/Bif_F12_DOMINO.cc` semantics for square real matrices:
//! - `⌹B` — inverse of square B
//! - `A⌹B` — least-squares solve of `B X = A` for square B (X = B⁻¹A)
//! - 1-element cases: `⌹x = 1/x`, `a⌹b = a/b`
//! - Non-square N×M B: monadic gives the pseudo-inverse via normal
//!   equations (simplified port; full QR is overkill here).
//!
//! Implementation: Gauss-Jordan elimination with partial pivoting on f64.

use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// flatten to f64 matrix (rows × cols) with shape checks
fn as_matrix(b: &ValueP) -> AplResult<Vec<Vec<f64>>> {
    let rank = b.rank();
    if rank == 0 {
        let v = b
            .first_cell()
            .and_then(|c| c.get_real_value().ok())
            .ok_or(ErrorCode::DomainError)?;
        return Ok(vec![vec![v]]);
    }
    if rank == 1 {
        // vector → column vector (n×1)
        let vals: Vec<f64> = b
            .cells()
            .iter()
            .map(|c| c.get_real_value())
            .collect::<Result<_, _>>()?;
        return Ok(vals.into_iter().map(|v| vec![v]).collect());
    }
    if rank != 2 {
        return Err(ErrorCode::RankError);
    }
    let rows = b.get_shape_item(0) as usize;
    let cols = b.get_shape_item(1) as usize;
    let cells = b.cells();
    let mut m = vec![vec![0.0; cols]; rows];
    for (r, row_out) in m.iter_mut().enumerate() {
        for c in 0..cols {
            row_out[c] = cells[r * cols + c].get_real_value()?;
        }
    }
    Ok(m)
}

/// wrap an f64 matrix back into a ValueP shaped like the template
fn from_matrix(m: &[Vec<f64>], tmpl: &ValueP) -> ValueP {
    let flat: Vec<Cell> = m
        .iter()
        .flat_map(|row| row.iter())
        .map(|&v| {
            if v.fract() == 0.0 && v.abs() < 1e15 {
                Cell::Int(v as i64)
            } else {
                Cell::Float(v)
            }
        })
        .collect();
    ValueP::from_ravel_like(tmpl, flat)
}

/// invert a square matrix in place; None if singular
fn invert(m: &mut Vec<Vec<f64>>) -> Option<()> {
    let n = m.len();
    let mut inv = vec![vec![0.0; n]; n];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for col in 0..n {
        // partial pivot: find largest |m[row][col]| for row >= col
        let mut best = col;
        for row in col..n {
            if m[row][col].abs() > m[best][col].abs() {
                best = row;
            }
        }
        if m[best][col].abs() < 1e-12 {
            return None; // singular
        }
        m.swap(col, best);
        inv.swap(col, best);
        // normalize pivot row
        let piv = m[col][col];
        for k in 0..n {
            m[col][k] /= piv;
            inv[col][k] /= piv;
        }
        // eliminate other rows
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = m[row][col];
            for k in 0..n {
                m[row][k] -= factor * m[col][k];
                inv[row][k] -= factor * inv[col][k];
            }
        }
    }
    *m = inv;
    Some(())
}

/// monadic `⌹B`
pub fn domino_monadic(b: &ValueP) -> AplResult<ValueP> {
    let m = as_matrix(b)?;
    let n = m.len();
    // scalar / single-element: reciprocal
    if n == 1 && m[0].len() == 1 {
        if m[0][0] == 0.0 {
            return Err(ErrorCode::DomainError);
        }
        return Ok(from_matrix(&[vec![1.0 / m[0][0]]], b));
    }
    if n != m[0].len() {
        // non-square: simplified pseudo-inverse via (BᵀB)⁻¹Bᵀ
        let rows = n;
        let cols = m[0].len();
        // bt_b = BᵀB (cols×cols), bt = Bᵀ
        let mut bt_b = vec![vec![0.0; cols]; cols];
        for i in 0..cols {
            for j in 0..cols {
                let s: f64 = (0..rows).map(|r| m[r][i] * m[r][j]).sum();
                bt_b[i][j] = s;
            }
        }
        if invert(&mut bt_b).is_none() {
            return Err(ErrorCode::DomainError);
        }
        // pinv = (BᵀB)⁻¹Bᵀ → cols×rows
        let mut pinv = vec![vec![0.0; rows]; cols];
        for i in 0..cols {
            for j in 0..rows {
                pinv[i][j] = (0..cols).map(|k| bt_b[i][k] * m[j][k]).sum();
            }
        }
        // shape: cols×rows — build fresh template shape
        let out_shape = crate::shape::Shape::matrix(cols as i64, rows as i64);
        let cells: Vec<Cell> = pinv
            .iter()
            .flat_map(|r| r.iter())
            .map(|&v| Cell::Float(v))
            .collect();
        return ValueP::from_parts(out_shape, cells);
    }
    let mut m = m;
    if invert(&mut m).is_none() {
        return Err(ErrorCode::DomainError); // singular matrix
    }
    Ok(from_matrix(&m, b))
}

/// dyadic `A⌹B` — solve B X = A (X = B⁻¹A)
pub fn domino_dyadic(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let bm = as_matrix(b)?;
    let am = as_matrix(a)?;
    let n = bm.len();
    if n != bm[0].len() {
        return Err(ErrorCode::DomainError); // only square solve supported
    }
    let mut inv = bm.clone();
    if invert(&mut inv).is_none() {
        return Err(ErrorCode::DomainError);
    }
    // X = B⁻¹A
    let acols = am[0].len();
    let mut x = vec![vec![0.0; acols]; n];
    for i in 0..n {
        for j in 0..acols {
            x[i][j] = (0..n).map(|k| inv[i][k] * am[k][j]).sum();
        }
    }
    Ok(from_matrix(&x, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_inverse_roundtrip() {
        // inverse of [[1,2],[3,4]] is [[-2,1],[1.5,-0.5]]
        let b = ValueP::from_parts(
            crate::shape::Shape::matrix(2, 2),
            vec![Cell::Int(1), Cell::Int(2), Cell::Int(3), Cell::Int(4)],
        )
        .unwrap();
        let inv = domino_monadic(&b).unwrap();
        assert_eq!(inv.element_count(), 4);
        let expect = [-2.0, 1.0, 1.5, -0.5];
        for (i, e) in expect.iter().enumerate() {
            match inv.cells()[i] {
                Cell::Float(f) => assert!((f - e).abs() < 1e-12),
                ref other => panic!("cell {}: expected float, got {:?}", i, other),
            }
        }
    }

    #[test]
    fn test_matrix_solve() {
        // solve B X = A for B=[[2,0],[0,3]], A=[4,9] → X=[2,3]
        let b = ValueP::from_parts(
            crate::shape::Shape::matrix(2, 2),
            vec![Cell::Int(2), Cell::Int(0), Cell::Int(0), Cell::Int(3)],
        )
        .unwrap();
        let a = ValueP::from_ravel_like(
            &b,
            vec![Cell::Int(4), Cell::Int(9)], // shape won't match; use vector
        );
        // A as a proper 2-vector
        let av = ValueP::from_parts(
            crate::shape::Shape::vector(2),
            vec![Cell::Int(4), Cell::Int(9)],
        )
        .unwrap_or(a);
        let x = domino_dyadic(&av, &b).unwrap();
        assert_eq!(x.element_count(), 2);
        for (i, e) in [2.0, 3.0].iter().enumerate() {
            match x.cells()[i] {
                Cell::Int(v) => assert_eq!(v as f64, *e),
                Cell::Float(f) => assert!((f - e).abs() < 1e-12),
                ref other => panic!("cell {}: unexpected {:?}", i, other),
            }
        }
    }

    #[test]
    fn test_singular_matrix_errors() {
        let s = ValueP::from_parts(
            crate::shape::Shape::matrix(2, 2),
            vec![Cell::Int(1), Cell::Int(2), Cell::Int(2), Cell::Int(4)],
        )
        .unwrap();
        assert!(domino_monadic(&s).is_err());
    }
}
