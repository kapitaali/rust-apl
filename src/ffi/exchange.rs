//! XArray — the stable foreign-function exchange format.
//!
//! Mirrors Dyalog's `A` type (the Auxiliary-Processor array) deliberately:
//! any signature declaring `A` is already speaking this wire format. All
//! ValueP ⇄ XArray conversion lives in THIS module — one audit point per
//! direction. Nothing else in the tree may touch XCell internals.
//!
//! Ownership: an XArray owns its cell buffer AND its direct nested children
//! (freed transitively on Drop). A Nested cell's `ptr` is an index into the
//! OWNING XArray's child table — never a raw pointer across the boundary,
//! so foreign code can't forge or dangle them. Foreign callees must not
//! free anything received from us; we free nothing received from them
//! except through their own documented allocator callbacks (F3).

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::ShapeItem;
use crate::value::{ValueInner, ValueP};
use std::sync::Arc;

/// Bump when XArray/XCell layout changes in a breaking way.
pub const EXCHANGE_ABI: u32 = 1;

pub const MAX_RANK: usize = 8;

/// cell type tag for XTaggedCell
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellTag {
    Int = 0,
    Float = 1,
    Char = 2,
    /// `ptr` = index into the owning XArray's child table
    Nested = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union XCell {
    pub int: i64,
    pub float: f64,
    pub chr: u32,
    pub ptr: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XTaggedCell {
    pub tag: CellTag,
    pub cell: XCell,
}

impl XTaggedCell {
    pub fn int(v: i64) -> Self {
        XTaggedCell {
            tag: CellTag::Int,
            cell: XCell { int: v },
        }
    }
    pub fn float(v: f64) -> Self {
        XTaggedCell {
            tag: CellTag::Float,
            cell: XCell { float: v },
        }
    }
    pub fn char(c: u32) -> Self {
        XTaggedCell {
            tag: CellTag::Char,
            cell: XCell { chr: c },
        }
    }
}

/// A POD APL array crossing the .so boundary.
pub struct XArray {
    abi_version: u32,
    rank: u32,
    dims: [u64; MAX_RANK],
    elem_count: u64,
    cells: *mut XTaggedCell,
    len_alloc: usize,
    /// direct nested children, indexed by Nested-cell ptr values
    children: Vec<XArray>,
}

impl std::fmt::Debug for XArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "XArray(rank={}, dims={:?}, n={}, children={})",
            self.rank,
            self.dims_vec(),
            self.elem_count,
            self.children.len()
        )
    }
}

impl XArray {
    /// Build from rank, dims, ravel, and direct children.
    pub fn build(
        rank: usize,
        dims: &[u64],
        ravel: Vec<XTaggedCell>,
        children: Vec<XArray>,
    ) -> Result<Box<XArray>, String> {
        if rank > MAX_RANK {
            return Err(format!("rank {} exceeds MAX_RANK {}", rank, MAX_RANK));
        }
        let count: u64 = dims.iter().product();
        if count != ravel.len() as u64 {
            return Err(format!(
                "dims product {} != ravel length {}",
                count,
                ravel.len()
            ));
        }
        // every Nested cell must index an existing child
        for t in &ravel {
            if t.tag == CellTag::Nested {
                let idx = unsafe { t.cell.ptr as usize };
                if idx >= children.len() {
                    return Err(format!("nested index {} out of range", idx));
                }
            }
        }
        let mut d = [0u64; MAX_RANK];
        d[..rank].copy_from_slice(&dims[..rank]);
        // Force an EXACT-size, non-dangling allocation: pad empty ravels to
        // one element so the raw buffer is always valid to free.
        let mut cells = ravel;
        if cells.is_empty() {
            cells.push(XTaggedCell::int(0));
        }
        cells.shrink_to_fit();
        debug_assert_eq!(cells.len(), cells.capacity());
        let len_alloc = cells.len();
        let ptr = cells.as_mut_ptr();
        std::mem::forget(cells); // ownership moves into the returned XArray
        Ok(Box::new(XArray {
            abi_version: EXCHANGE_ABI,
            rank: rank as u32,
            dims: d,
            elem_count: count,
            cells: ptr,
            len_alloc,
            children,
        }))
    }

    pub fn scalar(c: XTaggedCell) -> Result<Box<XArray>, String> {
        XArray::build(0, &[], vec![c], Vec::new())
    }

    /// Validate structure on entry from foreign code.
    pub fn check_abi(&self) -> Result<(), String> {
        if self.abi_version != EXCHANGE_ABI {
            return Err(format!(
                "XArray ABI mismatch: got {}, expected {}",
                self.abi_version, EXCHANGE_ABI
            ));
        }
        if self.rank as usize > MAX_RANK {
            return Err(format!("rank {} out of range", self.rank));
        }
        let count: u64 = self.dims[..self.rank as usize].iter().product();
        if count != self.elem_count {
            return Err(format!(
                "dims product {} != elem_count {}",
                count, self.elem_count
            ));
        }
        for t in self.elems() {
            if t.tag == CellTag::Nested {
                let idx = unsafe { t.cell.ptr as usize };
                if idx >= self.children.len() {
                    return Err(format!("nested index {} out of range", idx));
                }
                self.children[idx].check_abi()?;
            }
        }
        Ok(())
    }

    pub fn elems(&self) -> &[XTaggedCell] {
        unsafe { std::slice::from_raw_parts(self.cells, self.elem_count as usize) }
    }

    pub fn dims_vec(&self) -> Vec<u64> {
        self.dims[..self.rank as usize].to_vec()
    }

    pub fn rank(&self) -> usize {
        self.rank as usize
    }

    pub fn elem_count(&self) -> u64 {
        self.elem_count
    }

    pub fn children_count(&self) -> usize {
        self.children.len()
    }

    pub fn child(&self, idx: usize) -> Option<&XArray> {
        self.children.get(idx)
    }

    /// Read back a scalar's payload (test/diagnostic helper).
    pub fn scalar_int(&self) -> Option<i64> {
        if self.elem_count == 1 && self.elems()[0].tag == CellTag::Int {
            Some(unsafe { self.elems()[0].cell.int })
        } else {
            None
        }
    }

    pub fn scalar_float(&self) -> Option<f64> {
        if self.elem_count == 1 && self.elems()[0].tag == CellTag::Float {
            Some(unsafe { self.elems()[0].cell.float })
        } else {
            None
        }
    }
}

impl Drop for XArray {
    fn drop(&mut self) {
        if !self.cells.is_null() && self.len_alloc > 0 {
            unsafe {
                // reconstruct the Vec we forgot in build() so the buffer frees
                let v = Vec::from_raw_parts(self.cells, self.len_alloc, self.len_alloc);
                drop(v);
            }
            self.cells = std::ptr::null_mut();
        }
        // children drop recursively via Vec<Box<_>>
    }
}

// ---------------------------------------------------------------------------
// ValueP → XArray
// ---------------------------------------------------------------------------

fn shape_dims(v: &ValueP) -> (usize, Vec<u64>) {
    let shape = v.shape();
    let rank = shape.get_rank() as usize;
    let dims: Vec<u64> = (0..rank)
        .map(|ax| shape.get_shape_item(ax as i16) as u64)
        .collect();
    (rank, dims)
}

/// Convert an APL value to a heap-owned XArray tree.
///
/// Pointer cells become Nested cells; each nested value becomes a direct
/// child of the array that references it. Complex/Lval cells are rejected.
pub fn value_to_xarray(v: &ValueP) -> Result<Box<XArray>, String> {
    let mut children: Vec<XArray> = Vec::new();
    let ravel = flatten_level(v, &mut children)?;
    let (rank, dims) = shape_dims(v);
    XArray::build(rank, &dims, ravel, children)
}

/// Walk one level; nested values recurse into fresh subtrees.
fn flatten_level(v: &ValueP, children: &mut Vec<XArray>) -> Result<Vec<XTaggedCell>, String> {
    let mut out = Vec::with_capacity(v.element_count() as usize);
    for c in v.cells() {
        match c {
            Cell::Int(i) => out.push(XTaggedCell::int(*i)),
            Cell::Float(f) => out.push(XTaggedCell::float(*f)),
            Cell::Char(ch) => out.push(XTaggedCell::char(*ch)),
            Cell::Pointer(p) => {
                let child_v = ValueP {
                    inner: p.value.clone(),
                };
                let child = value_to_xarray(&child_v)?;
                let idx = children.len() as u64;
                children.push(*child);
                out.push(XTaggedCell {
                    tag: CellTag::Nested,
                    cell: XCell { ptr: idx },
                });
            }
            other => {
                return Err(format!("ffi: unsupported cell {:?} in exchange", other));
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// XArray → ValueP
// ---------------------------------------------------------------------------

/// Convert a foreign XArray tree back into an APL value.
pub fn xarray_to_value(x: &XArray) -> Result<ValueP, String> {
    x.check_abi()?;
    rebuild(x)
}

fn rebuild(x: &XArray) -> Result<ValueP, String> {
    let mut cells: Vec<Cell> = Vec::with_capacity(x.elem_count as usize);
    for t in x.elems() {
        match t.tag {
            CellTag::Int => unsafe { cells.push(Cell::int(t.cell.int)) },
            CellTag::Float => unsafe { cells.push(Cell::float(t.cell.float)) },
            CellTag::Char => unsafe {
                // Cell::char takes a Unicode (u32 code point) directly
                cells.push(Cell::char(t.cell.chr));
            },
            CellTag::Nested => {
                let idx = unsafe { t.cell.ptr as usize };
                let child = x
                    .child(idx)
                    .ok_or_else(|| format!("nested index {} out of range", idx))?;
                let inner = rebuild(child)?;
                cells.push(Cell::Pointer(crate::cell::PointerCellData {
                    value: inner.inner,
                }));
            }
        }
    }
    let dims: Vec<ShapeItem> = x.dims_vec().iter().map(|&d| d as ShapeItem).collect();
    let shape =
        Shape::from_dims(&dims).map_err(|e| format!("bad shape from foreign array: {:?}", e))?;
    Ok(ValueP {
        inner: Arc::new(ValueInner::new(shape, cells)),
    })
}
