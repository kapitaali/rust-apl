//! Encode `A⊤B` and Decode `A⊥B` — base representation conversions.
//!
//! **Decode** `A⊥B`: evaluates a polynomial in base A.
//!   `2⊥1 0 1 1` = 1×2³ + 0×2² + 1×2¹ + 1×2⁰ = 11
//!   `10⊥1 2 3` = 123 (base-10 digits)
//!   When A is a vector, each element is the base for the corresponding
//!   position (mixed-radix): `24 60 0⊥1 2 0` = 1×(60×1) + 2×1 + 0 = 62
//!
//! **Encode** `A⊤B`: converts B to representation in bases A.
//!   `2⊤5` = `1 0 1` (binary representation of 5)
//!   `10⊤123` = `1 2 3`
//!   `2 2 2 2⊤11` = `1 0 1 1`

use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// `A⊥B` — decode (base value). A = bases, B = coefficients.
pub fn decode(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let a_cells = a.cells();
    let b_cells = b.cells();

    // If both are scalars, simple: a^1 * b
    if a_cells.len() == 1 && b_cells.len() == 1 {
        let base = cell_to_f64(&a_cells[0])?;
        let val = cell_to_f64(&b_cells[0])?;
        return Ok(ValueP::scalar_from(Cell::Float(base * val)));
    }

    // A is the bases vector, B is the digits vector.
    // Scalar A extended to match B length (scalar extension):
    if a_cells.len() == 1 && b_cells.len() > 1 {
        let base = cell_to_f64(&a_cells[0])?;
        // Rank ≥ 2: B's FIRST axis holds the digits and each COLUMN is an
        // independent number, so the result is one value per column.
        // (`2⊥2 3⍴1 0 1 1 1 0` is `3 1 2`, not a single 46 from the ravel.)
        if b.rank() >= 2 {
            let rows = b.get_shape_item(0) as usize;
            let cols = (b.element_count() as usize) / rows.max(1);
            let mut weights = vec![1.0_f64; rows];
            for i in (0..rows.saturating_sub(1)).rev() {
                weights[i] = weights[i + 1] * base;
            }
            let mut out = Vec::with_capacity(cols);
            let all_int = b_cells.iter().all(|c| matches!(c, Cell::Int(_)));
            for j in 0..cols {
                let mut acc = 0.0_f64;
                for i in 0..rows {
                    acc += cell_to_f64(&b_cells[i * cols + j])? * weights[i];
                }
                out.push(if all_int && acc.fract() == 0.0 && acc.abs() < 1e18 {
                    Cell::Int(acc as i64)
                } else {
                    Cell::Float(acc)
                });
            }
            return ValueP::from_parts(crate::shape::Shape::vector(cols as i64), out);
        }
        let n = b_cells.len();
        let mut weights = vec![1.0_f64; n];
        for i in (0..n - 1).rev() {
            weights[i] = weights[i + 1] * base;
        }
        let mut result = 0.0_f64;
        for i in 0..n {
            let val = cell_to_f64(&b_cells[i])?;
            result += val * weights[i];
        }
        let all_int = b_cells.iter().all(|c| matches!(c, Cell::Int(_)));
        if all_int && result.fract() == 0.0 && result.abs() < 1e18 {
            return Ok(ValueP::scalar_from(Cell::Int(result as i64)));
        }
        return Ok(ValueP::scalar_from(Cell::Float(result)));
    }

    // A is the bases vector, B is the digits vector — same length.
    // Result = sum over i of B[i] * product of A[i+1..end]
    // (rightmost digit has weight 1 = A[0]^0, but APL convention:
    // A⊥B with A=[a0,a1,...,an] and B=[b0,b1,...,bn]:
    // result = b0*(a1*a2*...*an) + b1*(a2*...*an) + ... + bn-1*an + bn
    // i.e. weight[i] = product of A[i+1..end]
    if a_cells.len() != b_cells.len() {
        return Err(ErrorCode::LengthError);
    }

    let n = a_cells.len();
    let mut weights = vec![1.0_f64; n];
    // weights[n-1] = 1 (rightmost = weight 1)
    for i in (0..n - 1).rev() {
        let base = cell_to_f64(&a_cells[i + 1])?;
        weights[i] = weights[i + 1] * base;
    }

    let mut result = 0.0_f64;
    for i in 0..n {
        let val = cell_to_f64(&b_cells[i])?;
        result += val * weights[i];
    }

    // Return int if all inputs were ints and result is whole
    let all_int = a_cells.iter().all(|c| matches!(c, Cell::Int(_)))
        && b_cells.iter().all(|c| matches!(c, Cell::Int(_)));
    if all_int && result.fract() == 0.0 && result.abs() < 1e18 {
        Ok(ValueP::scalar_from(Cell::Int(result as i64)))
    } else {
        Ok(ValueP::scalar_from(Cell::Float(result)))
    }
}

/// `A⊤B` — encode (representation). A = bases, B = values.
pub fn encode(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let a_cells = a.cells();
    let b_cells = b.cells();

    // Single base, single value: 2⊤5 → 1 0 1
    if a_cells.len() == 1 {
        let base = cell_to_f64(&a_cells[0])? as i64;
        if base <= 0 {
            return Err(ErrorCode::DomainError);
        }
        let mut digits = Vec::new();
        for bc in b_cells.iter() {
            let val = cell_to_f64(bc)? as i64;
            let mut d = val;
            let mut row = Vec::new();
            if d == 0 {
                row.push(Cell::Int(0));
            }
            while d != 0 {
                row.push(Cell::Int(((d % base) + base) % base)); // handle negative
                d /= base;
            }
            row.reverse();
            // pad to match max width if multiple values
            digits.push(row);
        }
        // If single value, return as vector
        if b_cells.len() == 1 {
            let row = digits.pop().unwrap();
            return Ok(ValueP {
                inner: std::sync::Arc::new(crate::value::ValueInner::new(
                    crate::shape::Shape::vector(row.len() as i64),
                    row,
                )),
            });
        }
        // Multiple values: matrix (each row = one value's digits)
        let max_len = digits.iter().map(|d| d.len()).max().unwrap_or(1);
        // left-pad shorter rows with zeros
        let mut matrix = Vec::new();
        for row in &mut digits {
            while row.len() < max_len {
                row.insert(0, Cell::Int(0));
            }
            matrix.extend(row.iter().cloned());
        }
        let shape = crate::shape::Shape::from_dims(&[b_cells.len() as i64, max_len as i64])
            .map_err(|_| ErrorCode::DomainError)?;
        return Ok(ValueP {
            inner: std::sync::Arc::new(crate::value::ValueInner::new(shape, matrix)),
        });
    }

    // Multiple bases (mixed-radix): A=[a0,a1,...,an] ⊤ B
    // Each element of B is encoded independently.
    // For scalar B: result = digits of B in mixed radix
    if b_cells.len() == 1 {
        let val = cell_to_f64(&b_cells[0])? as i64;
        let n = a_cells.len();
        let mut result = vec![Cell::Int(0); n];
        let mut d = val.abs();
        for i in (0..n).rev() {
            let base = cell_to_f64(&a_cells[i])? as i64;
            if base <= 0 {
                return Err(ErrorCode::DomainError);
            }
            result[i] = Cell::Int(d % base);
            d /= base;
        }
        return Ok(ValueP {
            inner: std::sync::Arc::new(crate::value::ValueInner::new(
                crate::shape::Shape::vector(n as i64),
                result,
            )),
        });
    }

    // General case: each element of B is encoded with the full A bases, and
    // the result is a (n_bases × n_values) matrix — one COLUMN per value.
    //
    // Build it column-by-column into the right row-major slots: digit i of
    // value j belongs at row i, column j, i.e. linear index i*n_values + j.
    // (The old code pushed digits sequentially in reverse per value while
    // claiming this shape, so the layout was transposed AND reversed:
    // `2 2 2⊤5 3` gave 1 0 1 1 1 0 instead of 1 0 0 1 1 1.)
    let n_bases = a_cells.len();
    let n_values = b_cells.len();
    let mut result_cells = vec![Cell::Int(0); n_bases * n_values];
    for (j, bc) in b_cells.iter().enumerate() {
        let val = cell_to_f64(bc)? as i64;
        let mut d = val.abs();
        // least significant digit first, so walk rows bottom-up
        for i in (0..n_bases).rev() {
            let base = cell_to_f64(&a_cells[i])? as i64;
            if base <= 0 {
                return Err(ErrorCode::DomainError);
            }
            result_cells[i * n_values + j] = Cell::Int(d % base);
            d /= base;
        }
    }
    let shape = crate::shape::Shape::from_dims(&[n_bases as i64, n_values as i64])
        .map_err(|_| ErrorCode::DomainError)?;
    Ok(ValueP {
        inner: std::sync::Arc::new(crate::value::ValueInner::new(shape, result_cells)),
    })
}

fn cell_to_f64(c: &Cell) -> Result<f64, ErrorCode> {
    match c {
        Cell::Int(i) => Ok(*i as f64),
        Cell::Float(f) => Ok(*f),
        _ => Err(ErrorCode::DomainError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_binary() {
        // 2⊥1 0 1 1 = 11 (using repeated base 2)
        let a = ValueP::int_vector(&[2, 2, 2, 2]);
        let b = ValueP::int_vector(&[1, 0, 1, 1]);
        let r = decode(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_near_int().unwrap(), 11);
    }

    #[test]
    fn test_decode_decimal() {
        // 10⊥1 2 3 = 123
        let a = ValueP::int_vector(&[10, 10, 10]);
        let b = ValueP::int_vector(&[1, 2, 3]);
        let r = decode(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_near_int().unwrap(), 123);
    }

    #[test]
    fn test_encode_binary() {
        // 2⊤5 = 1 0 1
        let a = ValueP::int_vector(&[2]);
        let b = ValueP::int_vector(&[5]);
        let r = encode(&a, &b).unwrap();
        let cells: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| match c {
                Cell::Int(i) => *i,
                _ => panic!(),
            })
            .collect();
        assert_eq!(cells, vec![1, 0, 1]);
    }

    #[test]
    fn test_encode_decimal() {
        // 10⊤123 = 1 2 3
        let a = ValueP::int_vector(&[10]);
        let b = ValueP::int_vector(&[123]);
        let r = encode(&a, &b).unwrap();
        let cells: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| match c {
                Cell::Int(i) => *i,
                _ => panic!(),
            })
            .collect();
        assert_eq!(cells, vec![1, 2, 3]);
    }
}
