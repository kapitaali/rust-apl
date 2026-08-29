//! Key `⌸` — groups array elements into unique values + indices.
//!
//! Dyalog APL: ⌸B returns a 2-column matrix where column 1 is the unique
//! elements of B (in order of first appearance) and column 2 is a nested
//! vector of 1-based indices where each element appears.
//!
//! Example: ⌸ 'abac' → a ⟨1 3⟩ ⋄ b ⟨2⟩ ⋄ c ⟨4⟩

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Cell wrapper usable as a HashMap key.
#[derive(Clone)]
struct CellKey(Cell);

impl PartialEq for CellKey {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Cell::Int(a), Cell::Int(b)) => a == b,
            (Cell::Float(a), Cell::Float(b)) => a.to_bits() == b.to_bits(),
            (Cell::Char(a), Cell::Char(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for CellKey {}

impl Hash for CellKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match &self.0 {
            Cell::Int(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Cell::Float(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
            Cell::Char(v) => {
                2u8.hash(state);
                v.hash(state);
            }
            _ => {}
        }
    }
}

/// Monadic ⌸B — key of B's ravel elements.
pub fn key_monadic(b: &ValueP) -> AplResult<ValueP> {
    let elems = b.cells();
    if elems.is_empty() {
        return Ok(ValueP::int_vector(&[]));
    }

    // Build map: unique cell → list of 1-based positions
    // Use Vec to preserve insertion order (first appearance)
    let mut order: Vec<CellKey> = Vec::new();
    let mut map: HashMap<CellKey, Vec<i64>> = HashMap::new();
    for (idx, c) in elems.iter().enumerate() {
        if matches!(c, Cell::Pointer(_)) {
            return Err(ErrorCode::DomainError);
        }
        let key = CellKey(c.clone());
        if !map.contains_key(&key) {
            order.push(key.clone());
        }
        map.entry(key).or_default().push(idx as i64 + 1);
    }

    // Build result: 2-column matrix [unique_value, indices_vector]
    let nunique = order.len();
    let mut ravel: Vec<Cell> = Vec::with_capacity(nunique * 2);
    for key in &order {
        let val_cell = key.0.clone();
        let positions = map.get(key).unwrap();
        let idx_vec = ValueP::int_vector(positions);
        ravel.push(val_cell);
        ravel.push(Cell::pointer(idx_vec.inner.clone()));
    }

    Ok(ValueP::from_parts(Shape::matrix(nunique as i64, 2), ravel)?)
}

/// Dyadic A⌸B — key with A applied to B first.
///
/// Dyalog semantics: apply the function A to each element of B, then
/// group the results. A must be a function value (Prim) that can be
/// applied monadically to each element.
///
/// Example: ⍴⌸(1 2)(3 4 5)(6 7) → 2 ⟨1 3⟩ ⋄ 3 ⟨2⟩
pub fn key_dyad(a: &ValueP, b: &ValueP) -> AplResult<ValueP> {
    let elems = b.cells();
    if elems.is_empty() {
        return Ok(ValueP::int_vector(&[]));
    }

    // A must be a function value — represented as a single-char scalar
    // whose codepoint matches a Prim glyph. This is a simplified approach:
    // in a full implementation, A⌸B would require a proper function table
    // lookup or a callback mechanism.
    // For now, we require A to be a Char scalar representing a known Prim.
    if !a.is_scalar() {
        return Err(ErrorCode::DomainError);
    }

    let glyph = a
        .first_cell()
        .unwrap()
        .get_char_value()
        .map_err(|_| ErrorCode::DomainError)?;

    // Map glyph to Prim — only include variants that exist in the enum
    let prim = match glyph {
        c if c == '⍴' as u32 => Some(crate::functions::Prim::Rho),
        c if c == '⍳' as u32 => Some(crate::functions::Prim::Iota),
        c if c == '⍕' as u32 => Some(crate::functions::Prim::Format),
        c if c == '≡' as u32 => Some(crate::functions::Prim::Depth),
        c if c == '⌊' as u32 => Some(crate::functions::Prim::Floor),
        c if c == '⌈' as u32 => Some(crate::functions::Prim::Ceiling),
        c if c == '∣' as u32 => Some(crate::functions::Prim::Magnitude),
        c if c == '⊣' as u32 => Some(crate::functions::Prim::Left),
        c if c == '⊢' as u32 => Some(crate::functions::Prim::Right),
        _ => None,
    };

    let prim = match prim {
        Some(p) => p,
        None => return Err(ErrorCode::DomainError),
    };

    // Apply prim monadically to each element of B
    let mut transformed: Vec<crate::cell::Cell> = Vec::with_capacity(elems.len());
    for elem in elems {
        match elem {
            crate::cell::Cell::Pointer(p) => {
                let val = ValueP {
                    inner: p.value.clone(),
                };
                let result = prim.eval_monadic(&val)?;
                if result.is_scalar() {
                    transformed.push(result.first_cell().unwrap().clone());
                } else {
                    // For non-scalar results, keep as pointer
                    transformed.push(Cell::pointer(result.inner.clone()));
                }
            }
            _ => {
                let val = ValueP::scalar_from(elem.clone());
                let result = prim.eval_monadic(&val)?;
                if result.is_scalar() {
                    transformed.push(result.first_cell().unwrap().clone());
                } else {
                    transformed.push(Cell::pointer(result.inner.clone()));
                }
            }
        }
    }

    // Build a ValueP from the transformed cells, then key_monadic
    let transformed_vp = ValueP::from_parts(
        crate::shape::Shape::vector(transformed.len() as i64),
        transformed,
    )?;

    key_monadic(&transformed_vp)
}
