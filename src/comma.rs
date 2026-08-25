//! `,` — monadic ravel and dyadic catenate.
//!
//! Monadic `,B` ravels B to a vector (nested cells stay nested — use ∊ to
//! flatten). Dyadic `A,B` joins A and B along a new trailing axis: both
//! args are promoted to rank ≥ 1 first (scalars become 1-element vectors),
//! leading shapes must agree (LENGTH ERROR otherwise), and mixed simple
//! types promote Char → Int → Float per Dyalog.

use crate::cell::{Cell, PointerCellData};
use crate::shape::Shape;
use crate::types::ErrorCode;
use crate::value::ValueP;

/// monadic `,B` — ravel to a vector of the same cells
pub fn ravel(b: &ValueP) -> Result<ValueP, ErrorCode> {
    Ok(ValueP::from_ravel_like(b, b.cells().to_vec()))
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum SimpleType {
    Char,
    Int,
    Float,
}

fn lift(c: &Cell, t: SimpleType) -> Cell {
    match (c, t) {
        (Cell::Char(ch), SimpleType::Int) => Cell::Int(*ch as i64),
        (Cell::Char(ch), SimpleType::Float) => Cell::Float(*ch as f64),
        (Cell::Int(i), SimpleType::Float) => Cell::Float(*i as f64),
        _ => c.clone(),
    }
}

/// dyadic `A,B` — catenate along the last axis
pub fn catenate(a: &ValueP, b: &ValueP) -> Result<ValueP, ErrorCode> {
    // rank-0 args become 1-element vectors
    let av = if a.is_scalar() {
        ValueP::from_ravel_like(a, a.cells().to_vec()).reshape_vector()
    } else {
        ValueP::from_ravel_like(a, a.cells().to_vec())
    };
    let bv = if b.is_scalar() {
        ValueP::from_ravel_like(b, b.cells().to_vec()).reshape_vector()
    } else {
        ValueP::from_ravel_like(b, b.cells().to_vec())
    };

    let ra = av.rank() as usize;
    let rb = bv.rank() as usize;

    // pad the lower-rank arg with leading 1s so ranks agree
    let (dims_a, dims_b) = {
        let mut da: Vec<i64> = (0..ra).map(|i| av.get_shape_item(i as i16)).collect();
        let mut db: Vec<i64> = (0..rb).map(|i| bv.get_shape_item(i as i16)).collect();
        let target = da.len().max(db.len());
        while da.len() < target {
            da.insert(0, 1);
        }
        while db.len() < target {
            db.insert(0, 1);
        }
        (da, db)
    };

    let n_axes = dims_a.len();
    // all axes except the LAST must agree exactly
    for i in 0..n_axes.saturating_sub(1) {
        if dims_a[i] != dims_b[i] {
            return Err(ErrorCode::LengthError);
        }
    }

    let cat_len = *dims_a.last().unwrap_or(&0) + *dims_b.last().unwrap_or(&0);

    // result shape: common leading dims + concatenated last axis
    let mut out_dims = dims_a[..n_axes.saturating_sub(1)].to_vec();
    out_dims.push(cat_len);
    let shape = Shape::from_dims(&out_dims)?;

    // dominant simple type across BOTH ravels; None keeps cells as-is
    // (nested content or uniform type). Fold with Option-in-Option:
    // outer None = "nested present, give up", inner = running max.
    let mut promo: Option<Option<SimpleType>> = Some(None);
    for c in a.cells().iter().chain(b.cells().iter()) {
        let ct = match c {
            Cell::Int(_) => SimpleType::Int,
            Cell::Float(_) => SimpleType::Float,
            Cell::Char(_) => SimpleType::Char,
            _ => {
                promo = None; // nested cell — no promotion at all
                break;
            }
        };
        if let Some(acc @ Some(_)) = &mut promo {
            *acc = match *acc {
                Some(prev) => Some(prev.max(ct)),
                None => Some(ct),
            };
        }
    }
    let promo = promo.flatten();

    let mut ravel_out: Vec<Cell> =
        Vec::with_capacity(av.element_count() as usize + bv.element_count() as usize);
    match promo {
        None => {
            // keep everything as-is (uniform types and/or nested)
            ravel_out.extend(av.cells().iter().cloned());
            ravel_out.extend(bv.cells().iter().cloned());
        }
        Some(t) => {
            // mixed simple types: promote both sides; nested cells would
            // have made promo None already
            ravel_out.extend(av.cells().iter().map(|c| lift(c, t)));
            ravel_out.extend(bv.cells().iter().map(|c| lift(c, t)));
        }
    }

    Ok(ValueP {
        inner: std::sync::Arc::new(crate::value::ValueInner::new(shape, ravel_out)),
    })
}

impl ValueP {
    /// helper: same value forced to rank ≥1 (scalar → 1-element vector)
    fn reshape_vector(self) -> ValueP {
        if self.rank() == 0 {
            ValueP {
                inner: std::sync::Arc::new(crate::value::ValueInner::new(
                    Shape::vector(1),
                    self.cells().to_vec(),
                )),
            }
        } else {
            self
        }
    }
}

// re-export PointerCellData so nested-cell construction sites compile even
// when this module is the only user in some feature combos
#[allow(unused_imports)]
use PointerCellData as _PointerCellData;
