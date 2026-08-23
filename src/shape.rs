//! Shape — rank, dimensions, and volume of an APL value.
//!
//! Mirrors `src/Shape.hh` / `src/Shape.cc` from the C++ original.

use crate::types::{SAxis, ShapeItem, URank, MAX_RANK};

/// The shape of an APL value.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// the rank (⍴⍴)
    rho_rho: URank,
    /// per-axis lengths (ρ), padded with zeros
    rho: [ShapeItem; MAX_RANK],
    /// product of all dimensions
    volume: ShapeItem,
}

impl Default for Shape {
    fn default() -> Self {
        Shape::scalar()
    }
}

impl Shape {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// shape of a scalar (rank 0)
    #[inline]
    pub fn scalar() -> Shape {
        Shape {
            rho_rho: 0,
            rho: [0; MAX_RANK],
            volume: 1,
        }
    }

    /// shape of a vector of length `len`
    #[inline]
    pub fn vector(len: ShapeItem) -> Shape {
        let mut s = Shape::scalar();
        s.rho_rho = 1;
        s.rho[0] = len;
        s.volume = len;
        s
    }

    /// shape of a matrix (`rows` × `cols`)
    #[inline]
    pub fn matrix(rows: ShapeItem, cols: ShapeItem) -> Shape {
        let mut s = Shape::scalar();
        s.rho_rho = 2;
        s.rho[0] = rows;
        s.rho[1] = cols;
        s.volume = rows * cols;
        s
    }

    /// shape of a cube (`height` × `rows` × `cols`)
    #[inline]
    pub fn cube(height: ShapeItem, rows: ShapeItem, cols: ShapeItem) -> Shape {
        let mut s = Shape::scalar();
        s.rho_rho = 3;
        s.rho[0] = height;
        s.rho[1] = rows;
        s.rho[2] = cols;
        s.volume = height * rows * cols;
        s
    }

    /// arbitrary shape from a slice of dimension lengths
    pub fn from_dims(dims: &[ShapeItem]) -> Result<Shape, crate::types::ErrorCode> {
        if dims.len() > MAX_RANK {
            return Err(crate::types::ErrorCode::LimitError);
        }
        let mut s = Shape::scalar();
        for d in dims {
            s.add_shape_item(*d)?;
        }
        Ok(s)
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    #[inline]
    pub fn get_rank(&self) -> URank {
        self.rho_rho
    }

    #[inline]
    pub fn get_shape_item(&self, axis: SAxis) -> ShapeItem {
        debug_assert!((axis as usize) < self.rho_rho as usize);
        self.rho[axis as usize]
    }

    #[inline]
    pub fn get_volume(&self) -> ShapeItem {
        self.volume
    }

    /// length of the first axis, or 1 for scalars
    #[inline]
    pub fn get_first_shape_item(&self) -> ShapeItem {
        if self.rho_rho == 0 {
            1
        } else {
            self.rho[0]
        }
    }

    /// length of the last axis, or 1 for scalars
    #[inline]
    pub fn get_last_shape_item(&self) -> ShapeItem {
        if self.rho_rho == 0 {
            1
        } else {
            self.rho[self.rho_rho as usize - 1]
        }
    }

    /// alias for `get_last_shape_item()` (number of columns)
    #[inline]
    pub fn get_cols(&self) -> ShapeItem {
        self.get_last_shape_item()
    }

    /// number of rows: volume ÷ cols, or 1 for scalars
    #[inline]
    pub fn get_rows(&self) -> ShapeItem {
        if self.rho_rho == 0 {
            return 1;
        }
        match self.rho[self.rho_rho as usize - 1] {
            0 => {
                let mut count: ShapeItem = 1;
                for i in 0..(self.rho_rho as usize - 1) {
                    count *= self.rho[i];
                }
                count
            }
            cols => self.volume / cols,
        }
    }

    /// true iff any dimension is 0
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.volume == 0 && self.rho_rho > 0
            || self.rho.iter().take(self.rho_rho as usize).any(|&r| r == 0)
    }

    // -----------------------------------------------------------------------
    // Mutators
    // -----------------------------------------------------------------------

    /// modify one dimension
    pub fn set_shape_item(&mut self, axis: SAxis, len: ShapeItem) {
        debug_assert!((axis as usize) < self.rho_rho as usize);
        let idx = axis as usize;
        if self.rho[idx] != 0 {
            self.volume /= self.rho[idx];
            self.rho[idx] = len;
            self.volume *= self.rho[idx];
        } else {
            self.rho[idx] = len;
            self.recompute_volume();
        }
    }

    /// recompute the volume after a zero-length change
    pub fn recompute_volume(&mut self) {
        self.volume = 1;
        for i in 0..self.rho_rho as usize {
            self.volume *= self.rho[i];
        }
    }

    /// add a dimension at the end
    pub fn add_shape_item(&mut self, len: ShapeItem) -> Result<(), crate::types::ErrorCode> {
        if self.rho_rho as usize >= MAX_RANK {
            return Err(crate::types::ErrorCode::LimitError);
        }
        self.rho[self.rho_rho as usize] = len;
        self.rho_rho += 1;
        self.volume *= len;
        Ok(())
    }
}

impl PartialEq for Shape {
    fn eq(&self, other: &Self) -> bool {
        self.rho_rho == other.rho_rho
            && self.rho[..self.rho_rho as usize] == other.rho[..other.rho_rho as usize]
    }
}

impl Eq for Shape {}

impl std::ops::Add for Shape {
    type Output = Shape;
    /// catenate two shapes: this provides higher dims, `lower` the lower ones
    fn add(self, lower: Shape) -> Shape {
        let mut ret = self;
        for i in 0..lower.rho_rho as usize {
            // ignore error: we already checked bounds during construction
            let _ = ret.add_shape_item(lower.rho[i]);
        }
        ret
    }
}
