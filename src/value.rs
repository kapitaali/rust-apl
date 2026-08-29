//! Value — an APL array (Shape + ravel of Cells).
//!
//! Mirrors `src/Value.hh` / `src/Value.cc` / `src/Value_P.hh` from the C++
//! original. Uses reference counting (`Arc`) for shared ownership with
//! copy-on-write isolation (mirroring the C++ `Value_P::isolate()`).

use std::sync::Arc;

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{APLInteger, ErrorCode, ShapeItem, Unicode};

/// A reference-counted APL value (mirrors C++ `Value_P`).
#[derive(Clone)]
pub struct ValueP {
    pub(crate) inner: Arc<ValueInner>,
}

/// The actual value data.
#[derive(Clone)]
pub struct ValueInner {
    shape: Shape,
    ravel: Vec<Cell>,
    /// prototype cell: type of the elements (Int(0), Char(' '), Float(0),
    /// Pointer(...)). Used for over-take padding and empty-value prototypes
    /// (mirrors C++ `Value::get_cproto()`).
    proto: Cell,
}

impl ValueInner {
    pub fn new(shape: Shape, ravel: Vec<Cell>) -> ValueInner {
        let proto = ravel
            .first()
            .cloned()
            .unwrap_or_else(|| crate::cell::Cell::int(0));
        ValueInner {
            shape,
            ravel,
            proto,
        }
    }

    pub fn new_with_proto(shape: Shape, ravel: Vec<Cell>, proto: Cell) -> ValueInner {
        ValueInner {
            shape,
            ravel,
            proto,
        }
    }

    /// Create a ValueInner from a SmallVec-backed ravel (avoids heap allocation for ≤8 elements)
    pub fn new_with_smallvec(shape: Shape, ravel: smallvec::SmallVec<[Cell; 8]>) -> ValueInner {
        let proto = ravel
            .first()
            .cloned()
            .unwrap_or_else(|| crate::cell::Cell::int(0));
        ValueInner {
            shape,
            ravel: ravel.into_vec(),
            proto,
        }
    }

    /// the prototype cell (element type)
    pub fn proto(&self) -> &Cell {
        &self.proto
    }

    /// mutable access to the ravel (for selective assignment)
    pub fn ravel_mut(&mut self) -> &mut Vec<Cell> {
        &mut self.ravel
    }

    /// read-only access to the ravel
    pub fn cells(&self) -> &[Cell] {
        &self.ravel
    }

    /// the shape of this value
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// true iff this value has rank 0 (a scalar)
    pub fn is_scalar_shape(&self) -> bool {
        self.shape.get_rank() == 0
    }

    /// the number of ravel elements
    pub fn element_count(&self) -> i64 {
        self.shape.get_volume()
    }
}

impl ValueP {
    // -----------------------------------------------------------------------
    // Constructors (mirror Value_P constructors)
    // -----------------------------------------------------------------------

    /// a new scalar with an uninitialized ravel (IntCell(0))
    pub fn scalar() -> ValueP {
        ValueP {
            inner: Arc::new(ValueInner::new(Shape::scalar(), vec![Cell::int(0)])),
        }
    }

    /// a new scalar from a cell
    pub fn scalar_from(cell: Cell) -> ValueP {
        ValueP {
            inner: Arc::new(ValueInner::new(Shape::scalar(), vec![cell])),
        }
    }

    /// a new vector of length `len` filled with IntCell(0)
    pub fn vector(len: ShapeItem) -> ValueP {
        ValueP {
            inner: Arc::new(ValueInner::new(
                Shape::vector(len),
                vec![Cell::int(0); len as usize],
            )),
        }
    }

    /// a new matrix
    pub fn matrix(rows: ShapeItem, cols: ShapeItem) -> ValueP {
        let n = rows * cols;
        ValueP {
            inner: Arc::new(ValueInner::new(
                Shape::matrix(rows, cols),
                vec![Cell::int(0); n.max(0) as usize],
            )),
        }
    }

    /// a new general value from a shape
    pub fn from_shape(shape: Shape) -> Result<ValueP, crate::types::ErrorCode> {
        let count = shape.get_volume();
        if count < 0 {
            return Err(crate::types::ErrorCode::ValueError);
        }
        Ok(ValueP {
            inner: Arc::new(ValueInner::new(shape, vec![Cell::int(0); count as usize])),
        })
    }

    /// a character vector from a string of unicode code points.
    /// Prototype is Char(' ') so empty '' keeps its character-ness.
    pub fn char_vector(chars: &[Unicode]) -> ValueP {
        ValueP {
            inner: Arc::new(ValueInner::new_with_proto(
                Shape::vector(chars.len() as ShapeItem),
                chars.iter().map(|&c| Cell::char(c)).collect(),
                Cell::char(' ' as u32),
            )),
        }
    }

    /// an integer vector from a slice
    pub fn int_vector(vals: &[APLInteger]) -> ValueP {
        ValueP {
            inner: Arc::new(ValueInner::new(
                Shape::vector(vals.len() as ShapeItem),
                vals.iter().map(|&v| Cell::int(v)).collect(),
            )),
        }
    }

    /// ⍳B — index generator (monadic iota)
    ///
    /// Returns a vector 0..B if B ≥ 0, else errors.
    pub fn iota(n: APLInteger) -> Result<ValueP, crate::types::ErrorCode> {
        if n < 0 {
            return Err(crate::types::ErrorCode::DomainError);
        }
        Ok(Self::int_vector(&(0..n).collect::<Vec<_>>()))
    }

    // -----------------------------------------------------------------------
    // Type tests (mirror Value::is_*)
    // -----------------------------------------------------------------------

    #[inline]
    pub fn is_scalar(&self) -> bool {
        self.inner.shape.get_rank() == 0
    }

    /// true iff rank-0 with a simple (non-pointer) cell
    #[inline]
    pub fn is_scalar_simple(&self) -> bool {
        self.is_scalar()
            && self
                .inner
                .ravel
                .first()
                .map_or(false, |c| !c.is_pointer_cell())
    }

    #[inline]
    pub fn is_vector(&self) -> bool {
        self.inner.shape.get_rank() == 1
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.shape.is_empty()
    }

    /// true iff this value ≡ ⍬ (empty numeric vector)
    #[inline]
    pub fn is_zilde(&self) -> bool {
        self.is_vector()
            && self.inner.shape.get_cols() == 0
            && matches!(self.inner.ravel.first(), Some(Cell::Int(_)))
    }

    /// true iff this value ≡ '' (empty char vector)
    #[inline]
    pub fn is_str0(&self) -> bool {
        self.is_vector()
            && self.inner.shape.get_cols() == 0
            && matches!(self.inner.ravel.first(), Some(Cell::Char(_)))
    }

    /// clone the shared inner (used by ffi/plugin code to nest values
    /// without deep-copying)
    pub fn clone_inner_arc(&self) -> std::sync::Arc<ValueInner> {
        self.inner.clone()
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    #[inline]
    pub fn shape(&self) -> &Shape {
        &self.inner.shape
    }

    #[inline]
    pub fn rank(&self) -> u32 {
        self.inner.shape.get_rank()
    }

    #[inline]
    pub fn element_count(&self) -> ShapeItem {
        self.inner.shape.get_volume()
    }

    #[inline]
    pub fn get_shape_item(&self, axis: i16) -> ShapeItem {
        self.inner.shape.get_shape_item(axis)
    }

    #[inline]
    pub fn cells(&self) -> &[Cell] {
        &self.inner.ravel
    }

    #[inline]
    pub fn first_cell(&self) -> Option<&Cell> {
        self.inner.ravel.first()
    }

    // -----------------------------------------------------------------------
    // Copy-on-write (mirror Value_P::isolate / isolate_deep)
    // -----------------------------------------------------------------------

    /// number of owners of this value's data
    #[inline]
    pub fn owner_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// true if this handle exclusively owns its data
    #[inline]
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.inner) == 1
    }

    /// deep-clone the data so this handle becomes the sole owner.
    pub fn isolate(&mut self) {
        if !self.is_unique() {
            self.inner = Arc::new(ValueInner::new_with_proto(
                self.inner.shape,
                self.inner.ravel.clone(),
                self.inner.proto.clone(),
            ));
        }
    }

    /// get a mutable reference, cloning if shared (COW)
    pub fn make_mut(&mut self) -> &mut ValueInner {
        self.isolate();
        Arc::make_mut(&mut self.inner)
    }

    /// dyadic + over two values (element-wise with scalar extension)
    pub fn add(a: &ValueP, b: &ValueP) -> Result<ValueP, crate::types::ErrorCode> {
        crate::functions::elementwise(a, b, crate::cell::bif_add)
    }

    /// build a new value with the same shape, from a full ravel
    pub fn from_ravel_like(model: &ValueP, ravel: Vec<Cell>) -> ValueP {
        ValueP {
            inner: Arc::new(ValueInner::new(*model.shape(), ravel)),
        }
    }

    /// nest a value inside a scalar PointerCell (for building nested arrays)
    pub fn nested(v: ValueP) -> ValueP {
        ValueP {
            inner: Arc::new(ValueInner::new(
                Shape::scalar(),
                vec![Cell::Pointer(crate::cell::PointerCellData {
                    value: v.inner,
                })],
            )),
        }
    }

    /// if this is a scalar whose only cell is a Pointer, return the nested value;
    /// otherwise return self (used by pick / disclose).
    pub fn disclose(&self) -> ValueP {
        if self.is_scalar() {
            if let Some(Cell::Pointer(p)) = self.first_cell() {
                return ValueP {
                    inner: p.value.clone(),
                };
            }
        }
        self.clone()
    }

    /// build a value from a SmallVec-backed ravel (avoids heap allocation for small arrays ≤8 elements)
    pub fn from_smallvec(
        shape: Shape,
        ravel: smallvec::SmallVec<[Cell; 8]>,
    ) -> Result<ValueP, ErrorCode> {
        let want = shape.get_volume();
        if want < 0 || want as usize != ravel.len() {
            return Err(ErrorCode::LengthError);
        }
        Ok(ValueP {
            inner: Arc::new(ValueInner::new(shape, ravel.into_vec())),
        })
    }

    /// build a value from an explicit shape and ravel
    pub fn from_parts(shape: Shape, ravel: Vec<Cell>) -> Result<ValueP, ErrorCode> {
        let want = shape.get_volume();
        if want < 0 || want as usize != ravel.len() {
            return Err(ErrorCode::LengthError);
        }
        Ok(ValueP {
            inner: Arc::new(ValueInner::new(shape, ravel)),
        })
    }
}

impl std::fmt::Debug for ValueP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.shape())?;
        f.write_str("(")?;
        for (i, c) in self.cells().iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            match c {
                Cell::Int(v) => write!(f, "{}", v)?,
                Cell::Float(v) => write!(f, "{}", v)?,
                Cell::Char(v) => write!(f, "{}", char::from_u32(*v).unwrap_or('?'))?,
                Cell::Complex(c) => write!(f, "({}J{})", c.re, c.im)?,
                _ => write!(f, "<nested>")?,
            }
        }
        f.write_str(")")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar() {
        let v = ValueP::scalar();
        assert!(v.is_scalar());
        assert_eq!(v.element_count(), 1);
    }

    #[test]
    fn test_vector_iota() {
        let v = ValueP::iota(5).unwrap();
        assert_eq!(v.element_count(), 5);
        match v.first_cell() {
            Some(Cell::Int(0)) => {}
            _ => panic!("expected first cell to be 0"),
        }
    }

    #[test]
    fn test_cow_isolation() {
        let mut a = ValueP::iota(3).unwrap();
        let b = a.clone();
        assert!(!a.is_unique());
        a.isolate();
        assert!(a.is_unique());
        drop(b);
        assert!(a.is_unique());
    }

    #[test]
    fn test_add_elementwise() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[10, 20, 30]);
        let c = ValueP::add(&a, &b).unwrap();
        assert_eq!(
            c.cells(),
            &[Cell::int(11), Cell::int(22), Cell::int(33)][..]
        );
    }

    #[test]
    fn test_add_scalar_extension() {
        let a = ValueP::scalar_from(Cell::int(1));
        let b = ValueP::int_vector(&[10, 20, 30]);
        let c = ValueP::add(&a, &b).unwrap();
        assert_eq!(
            c.cells(),
            &[Cell::int(11), Cell::int(21), Cell::int(31)][..]
        );
    }

    #[test]
    fn test_length_error() {
        let a = ValueP::int_vector(&[1, 2, 3]);
        let b = ValueP::int_vector(&[1, 2]);
        assert_eq!(
            ValueP::add(&a, &b).unwrap_err(),
            crate::types::ErrorCode::LengthError
        );
    }

    #[test]
    fn test_zilde() {
        let z = ValueP::char_vector(&[]);
        assert!(z.is_empty());
    }

    #[test]
    fn test_from_smallvec() {
        use smallvec::SmallVec;
        let mut sv: SmallVec<[Cell; 8]> = SmallVec::new();
        sv.push(Cell::int(1));
        sv.push(Cell::int(2));
        sv.push(Cell::int(3));
        let v = ValueP::from_smallvec(Shape::vector(3), sv).unwrap();
        assert_eq!(v.element_count(), 3);
        assert_eq!(v.cells()[0], Cell::int(1));
        assert_eq!(v.cells()[1], Cell::int(2));
        assert_eq!(v.cells()[2], Cell::int(3));
    }
}
