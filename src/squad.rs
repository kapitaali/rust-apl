//! Squad ⌷ (general index) and Rotate1 ⊖ (first-axis reverse/rotate).
//!
//! Mirrors `Bif_F2_INDEX` and `Bif_F12_ROTATE1` in C++ (simplified).

use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// A⌷B — general index: select element(s) from B using A as indices.
///
/// A is a vector of length equal to rank(B). Each element selects along
/// the corresponding axis. Returns the scalar at the given position.
/// Indices are 0-based (honor ⎕IO at the call site).
pub fn squad(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    if a.rank() > 1 {
        return Err(ErrorCode::RankError);
    }
    let ac = a.cells();
    let rank = b.rank() as usize;
    if ac.len() != rank {
        return Err(ErrorCode::RankError);
    }
    // Convert indices to 0-based (already done by caller via ⎕IO shift)
    let indices: Vec<i64> = ac
        .iter()
        .map(|c| c.get_int_value())
        .collect::<Result<Vec<_>, _>>()?;
    // Bounds check
    for (i, &idx) in indices.iter().enumerate() {
        let axis_len = if i < rank {
            b.get_shape_item(i as i16)
        } else {
            1
        };
        if idx < 0 || idx >= axis_len {
            return Err(ErrorCode::IndexError);
        }
    }
    // Compute linear offset from indices
    let mut offset: i64 = 0;
    let mut stride: i64 = 1;
    for (i, &idx) in indices.iter().enumerate().rev() {
        offset += idx * stride;
        if i > 0 {
            stride *= b.get_shape_item(i as i16);
        }
    }
    let cell = b.cells()[offset as usize].clone();
    Ok(ValueP::scalar_from(cell))
}

/// ⊖B — reverse along first axis.
pub fn reverse_first(b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    if rank <= 1 {
        // For vectors, reverse_first is same as reverse_last
        return crate::rotate::reverse(b);
    }
    let m = b.get_shape_item(0); // first axis length
    let inner = b.element_count() / m.max(1);
    let cells = b.cells();
    let mut out = Vec::with_capacity(b.element_count() as usize);
    // Reverse the order of "rows" (each row has `inner` elements)
    for r in (0..m as usize).rev() {
        for k in 0..inner as usize {
            out.push(cells[r * inner as usize + k].clone());
        }
    }
    // Build result with explicit shape
    let dims: Vec<i64> = (0..rank).map(|i| b.get_shape_item(i as i16)).collect();
    let shape = Shape::from_dims(&dims)?;
    ValueP::from_parts(shape, out)
}

/// A⊖B — rotate along first axis.
pub fn rotate_first(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let rank = b.rank();
    if rank <= 1 {
        // For vectors, rotate_first is same as rotate_last
        return crate::rotate::rotate(a, b);
    }
    let m = b.get_shape_item(0); // first axis length
    let inner = b.element_count() / m.max(1);
    let cells = b.cells();
    // Global shift?
    let shift = if a.element_count() == 1 {
        a.first_cell()
            .and_then(|c| c.get_int_value().ok())
            .ok_or(ErrorCode::DomainError)?
    } else {
        return Err(ErrorCode::DomainError); // per-line shifts not yet supported
    };
    let mut out = Vec::with_capacity(b.element_count() as usize);
    for r in 0..m as usize {
        let mut src_r = r as i64 + shift;
        while src_r < 0 {
            src_r += m;
        }
        while src_r >= m {
            src_r -= m;
        }
        for k in 0..inner as usize {
            out.push(cells[src_r as usize * inner as usize + k].clone());
        }
    }
    Ok(ValueP::from_ravel_like(b, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    #[test]
    fn test_squad_vector() {
        let b = ValueP::int_vector(&[10, 20, 30]);
        let a = ValueP::int_vector(&[1]); // 0-based index
        let r = squad(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 20);
    }

    #[test]
    fn test_squad_matrix() {
        // 2x3 matrix: [[1 2 3] [4 5 6]]
        let b = ValueP::from_parts(
            Shape::matrix(2, 3),
            vec![
                Cell::Int(1),
                Cell::Int(2),
                Cell::Int(3),
                Cell::Int(4),
                Cell::Int(5),
                Cell::Int(6),
            ],
        )
        .unwrap();
        // Index [1, 2] → row 1, col 2 → value 6
        let a = ValueP::int_vector(&[1, 2]);
        let r = squad(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 6);
        // Index [0, 0] → 1
        let a = ValueP::int_vector(&[0, 0]);
        let r = squad(&a, &b).unwrap();
        assert_eq!(r.first_cell().unwrap().get_int_value().unwrap(), 1);
    }

    #[test]
    fn test_squad_out_of_bounds() {
        let b = ValueP::int_vector(&[10, 20, 30]);
        let a = ValueP::int_vector(&[5]);
        assert!(squad(&a, &b).is_err());
    }

    #[test]
    fn test_reverse_first_matrix() {
        // 2x3: [[1 2 3] [4 5 6]] → [[4 5 6] [1 2 3]]
        let b = ValueP::from_parts(
            Shape::matrix(2, 3),
            vec![
                Cell::Int(1),
                Cell::Int(2),
                Cell::Int(3),
                Cell::Int(4),
                Cell::Int(5),
                Cell::Int(6),
            ],
        )
        .unwrap();
        let r = reverse_first(&b).unwrap();
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![4, 5, 6, 1, 2, 3]);
    }

    #[test]
    fn test_rotate_first_matrix() {
        // 2x3: [[1 2 3] [4 5 6]] → rotate first axis by 1 → [[4 5 6] [1 2 3]]
        let b = ValueP::from_parts(
            Shape::matrix(2, 3),
            vec![
                Cell::Int(1),
                Cell::Int(2),
                Cell::Int(3),
                Cell::Int(4),
                Cell::Int(5),
                Cell::Int(6),
            ],
        )
        .unwrap();
        let a = ValueP::int_vector(&[1]);
        let r = rotate_first(&a, &b).unwrap();
        let ints: Vec<i64> = r
            .cells()
            .iter()
            .map(|c| c.get_int_value().unwrap())
            .collect();
        assert_eq!(ints, vec![4, 5, 6, 1, 2, 3]);
    }
}
