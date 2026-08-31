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

/// Storage for the ravel — either materialized (Vec<Cell>) or computed
/// on-the-fly via a fetcher function pointer (Phase 7.4).
#[derive(Clone)]
pub enum CellStorage {
    Materialized(Vec<Cell>),
    /// Lazy array: compute cell at index `i` via `fetcher(i)`.
    /// `cached` is populated on first access (via `OnceLock::get_or_init`).
    /// `proto` is the element type for empty-value / over-take semantics.
    Fetched {
        fetcher: std::sync::Arc<dyn Fn(usize) -> Cell + Send + Sync>,
        cached: std::sync::OnceLock<Vec<Cell>>,
        proto: Cell,
    },
}

impl CellStorage {
    /// Access the ravel as a slice. For `Fetched` variants, this
    /// materializes via the cached cell vector on first call.
    fn ravel(&self, volume: usize) -> &[Cell] {
        match self {
            CellStorage::Materialized(v) => v,
            CellStorage::Fetched {
                fetcher, cached, ..
            } => cached.get_or_init(|| (0..volume).map(|i| fetcher(i)).collect()),
        }
    }
}

/// The actual value data.
#[derive(Clone)]
pub struct ValueInner {
    shape: Shape,
    storage: CellStorage,
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
            storage: CellStorage::Materialized(ravel),
            proto,
        }
    }

    pub fn new_with_proto(shape: Shape, ravel: Vec<Cell>, proto: Cell) -> ValueInner {
        ValueInner {
            shape,
            storage: CellStorage::Materialized(ravel),
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
            storage: CellStorage::Materialized(ravel.into_vec()),
            proto,
        }
    }

    /// Create a lazy (fetcher-based) ValueInner — cells are computed on demand.
    pub fn new_fetched<F>(shape: Shape, fetcher: F, proto: Cell) -> ValueInner
    where
        F: Fn(usize) -> Cell + Send + Sync + 'static,
    {
        ValueInner {
            shape,
            storage: CellStorage::Fetched {
                fetcher: std::sync::Arc::new(fetcher),
                cached: std::sync::OnceLock::new(),
                proto: proto.clone(),
            },
            proto,
        }
    }

    /// the prototype cell (element type)
    pub fn proto(&self) -> &Cell {
        &self.proto
    }

    /// mutable access to the ravel (for selective assignment).
    /// Materializes a `Fetched` storage before returning a mutable slice.
    pub fn ravel_mut(&mut self) -> &mut Vec<Cell> {
        if let CellStorage::Fetched {
            fetcher, cached, ..
        } = &self.storage
        {
            let n = self.shape.get_volume() as usize;
            let ravel: Vec<Cell> = (0..n).map(|i| fetcher(i)).collect();
            let _ = cached.set(ravel.clone());
            self.storage = CellStorage::Materialized(ravel);
        }
        match &mut self.storage {
            CellStorage::Materialized(v) => v,
            _ => unreachable!(),
        }
    }

    /// read-only access to the ravel. Returns a slice; if the storage is
    /// `Fetched`, it materializes via the cached cell vector.
    pub fn cells(&self) -> &[Cell] {
        let volume = self.shape.get_volume() as usize;
        self.storage.ravel(volume)
    }

    /// get a single cell at an index — works for both Materialized and Fetched.
    pub fn cell_at(&self, index: usize) -> Option<Cell> {
        match &self.storage {
            CellStorage::Materialized(v) => v.get(index).cloned(),
            CellStorage::Fetched { fetcher, .. } => Some(fetcher(index)),
        }
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
                .cell_at(0)
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
            && self.inner.element_count() == 0
            && matches!(self.inner.proto(), Cell::Int(_))
    }

    /// true iff this value ≡ '' (empty char vector)
    #[inline]
    pub fn is_str0(&self) -> bool {
        self.is_vector()
            && self.inner.shape.get_cols() == 0
            && self.inner.element_count() == 0
            && matches!(self.inner.proto(), Cell::Char(_))
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
        self.inner.cells()
    }

    #[inline]
    pub fn first_cell(&self) -> Option<&Cell> {
        self.inner.cells().first()
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
                self.inner.cells().to_vec(),
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

    /// build a lazy (fetcher-based) value — cells are computed on demand.
    pub fn from_fetcher<F>(shape: Shape, fetcher: F, proto: Cell) -> Result<ValueP, ErrorCode>
    where
        F: Fn(usize) -> Cell + Send + Sync + 'static,
    {
        let count = shape.get_volume();
        if count < 0 {
            return Err(ErrorCode::LengthError);
        }
        Ok(ValueP {
            inner: Arc::new(ValueInner::new_fetched(shape, fetcher, proto)),
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
    fn test_from_fetcher() {
        // Create a lazy value: cells are computed as i*2
        let shape = Shape::vector(5);
        let v = ValueP::from_fetcher(shape, |i| Cell::int(i as i64 * 2), Cell::int(0)).unwrap();

        // Element count should be correct before materialization
        assert_eq!(v.element_count(), 5);

        // cells() should materialize and return correct values
        assert_eq!(v.cells()[0], Cell::int(0));
        assert_eq!(v.cells()[1], Cell::int(2));
        assert_eq!(v.cells()[2], Cell::int(4));
        assert_eq!(v.cells()[3], Cell::int(6));
        assert_eq!(v.cells()[4], Cell::int(8));

        // After materialization, first_cell should work
        assert_eq!(v.first_cell(), Some(&Cell::int(0)));
    }

    #[test]
    fn test_from_fetcher_matrix() {
        // Create a lazy 3x3 matrix
        let shape = Shape::matrix(3, 3);
        let v = ValueP::from_fetcher(shape, |i| Cell::int(i as i64), Cell::int(0)).unwrap();

        assert_eq!(v.rank(), 2);
        assert_eq!(v.element_count(), 9);
        assert_eq!(v.cells()[0], Cell::int(0));
        assert_eq!(v.cells()[8], Cell::int(8));
    }

    #[test]
    fn test_fetcher_caching() {
        // Verify that the fetcher is called only once per cell via caching
        use std::sync::atomic::{AtomicUsize, Ordering};
        let call_count = std::sync::Arc::new(AtomicUsize::new(0));
        let counter = call_count.clone();

        let shape = Shape::vector(4);
        let v = ValueP::from_fetcher(
            shape,
            move |i| {
                counter.fetch_add(1, Ordering::SeqCst);
                Cell::int(i as i64 * 3)
            },
            Cell::int(0),
        )
        .unwrap();

        // First access
        assert_eq!(v.cells()[0], Cell::int(0));
        assert_eq!(v.cells()[3], Cell::int(9));

        // After materialization, the cached version should be used
        assert_eq!(v.cells()[0], Cell::int(0));

        // The fetcher was called 4 times (once per element during materialization)
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn test_fetcher_materialize() {
        // Test that ravel_mut materializes the fetcher
        let shape = Shape::vector(3);
        let mut v =
            ValueP::from_fetcher(shape, |i| Cell::int(i as i64 + 10), Cell::int(0)).unwrap();

        // ravel_mut should materialize
        let ravel = v.make_mut().ravel_mut();
        assert_eq!(ravel[0], Cell::int(10));
        assert_eq!(ravel[1], Cell::int(11));
        assert_eq!(ravel[2], Cell::int(12));
    }

    #[test]
    fn test_fetcher_with_proto() {
        // Test that proto is preserved for empty values
        let shape = Shape::vector(3);
        let v = ValueP::from_fetcher(
            shape,
            |i| Cell::char('a' as u32 + i as u32),
            Cell::char(' ' as u32),
        )
        .unwrap();

        assert_eq!(v.cells()[0], Cell::char('a' as u32));
        assert_eq!(v.cells()[1], Cell::char('b' as u32));
        assert_eq!(v.cells()[2], Cell::char('c' as u32));
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
