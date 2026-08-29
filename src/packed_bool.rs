//! Packed boolean array storage.
//!
//! Booleans in APL are stored as `Cell::Int(0)` or `Cell::Int(1)`, which uses
//! 16 bytes per boolean due to enum alignment. This module packs booleans
//! into bits within `u64` words, reducing memory by ~128x and improving
//! cache locality for large boolean arrays.

use crate::cell::Cell;
use crate::types::ShapeItem;

/// A packed boolean vector stored as bits in `u64` words.
///
/// Bit 0 of word 0 is element 0, bit 1 of word 0 is element 1, etc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedBool {
    len: usize,
    bits: Vec<u64>,
}

impl PackedBool {
    /// Create a new packed boolean vector of the given length, all false.
    pub fn new(len: usize) -> Self {
        let words = (len + 63) / 64;
        PackedBool {
            len,
            bits: vec![0u64; words],
        }
    }

    /// Create from a slice of booleans.
    pub fn from_bools(bools: &[bool]) -> Self {
        let mut pb = Self::new(bools.len());
        for (i, &b) in bools.iter().enumerate() {
            pb.set(i, b);
        }
        pb
    }

    /// Create from a slice of `Cell` values (must all be Int 0 or Int 1).
    pub fn from_cells(cells: &[Cell]) -> Self {
        let mut pb = Self::new(cells.len());
        for (i, cell) in cells.iter().enumerate() {
            let val = cell.get_int_value().unwrap_or(0);
            pb.set(i, val != 0);
        }
        pb
    }

    /// The number of boolean elements.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the boolean at index `i`.
    pub fn get(&self, i: usize) -> bool {
        if i >= self.len {
            panic!("PackedBool index out of bounds: {} >= {}", i, self.len);
        }
        let word = i / 64;
        let bit = i % 64;
        (self.bits[word] >> bit) & 1 != 0
    }

    /// Set the boolean at index `i`.
    pub fn set(&mut self, i: usize, val: bool) {
        if i >= self.len {
            panic!("PackedBool index out of bounds: {} >= {}", i, self.len);
        }
        let word = i / 64;
        let bit = i % 64;
        if val {
            self.bits[word] |= 1u64 << bit;
        } else {
            self.bits[word] &= !(1u64 << bit);
        }
    }

    /// Unpack into a `Vec<Cell>`.
    pub fn to_cells(&self) -> Vec<Cell> {
        (0..self.len)
            .map(|i| Cell::int(if self.get(i) { 1 } else { 0 }))
            .collect()
    }

    /// Unpack into a `Vec<bool>`.
    pub fn to_bools(&self) -> Vec<bool> {
        (0..self.len).map(|i| self.get(i)).collect()
    }

    /// Count the number of true bits (population count).
    pub fn count_ones(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }

    /// Count the number of false bits.
    pub fn count_zeros(&self) -> u32 {
        self.len as u32 - self.count_ones()
    }

    /// Memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.bits.capacity() * 8
    }

    /// Memory usage if stored as `Vec<Cell>`.
    pub fn unpacked_memory_bytes(&self) -> usize {
        self.len * std::mem::size_of::<Cell>()
    }

    /// Memory savings ratio (unpacked / packed).
    pub fn compression_ratio(&self) -> f64 {
        self.unpacked_memory_bytes() as f64 / self.memory_bytes().max(1) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packed_bool_new() {
        let pb = PackedBool::new(100);
        assert_eq!(pb.len(), 100);
        assert!(pb.bits.capacity() >= 2); // 100/64 = 2 words
        for i in 0..100 {
            assert!(!pb.get(i));
        }
    }

    #[test]
    fn test_packed_bool_set_get() {
        let mut pb = PackedBool::new(65); // spans 2 words
        pb.set(0, true);
        pb.set(63, true);
        pb.set(64, true); // crosses word boundary
        assert!(pb.get(0));
        assert!(!pb.get(1));
        assert!(pb.get(63));
        assert!(pb.get(64));
        assert_eq!(pb.len(), 65);
    }

    #[test]
    fn test_packed_bool_from_cells() {
        let cells = vec![Cell::int(1), Cell::int(0), Cell::int(1), Cell::int(1)];
        let pb = PackedBool::from_cells(&cells);
        assert_eq!(pb.len(), 4);
        assert!(pb.get(0));
        assert!(!pb.get(1));
        assert!(pb.get(2));
        assert!(pb.get(3));
    }

    #[test]
    fn test_packed_bool_to_cells() {
        let mut pb = PackedBool::new(4);
        pb.set(0, true);
        pb.set(2, true);
        let cells = pb.to_cells();
        assert_eq!(cells[0], Cell::int(1));
        assert_eq!(cells[1], Cell::int(0));
        assert_eq!(cells[2], Cell::int(1));
        assert_eq!(cells[3], Cell::int(0));
    }

    #[test]
    fn test_packed_bool_count() {
        let mut pb = PackedBool::new(10);
        pb.set(0, true);
        pb.set(3, true);
        pb.set(7, true);
        assert_eq!(pb.count_ones(), 3);
        assert_eq!(pb.count_zeros(), 7);
    }

    #[test]
    fn test_packed_bool_compression() {
        let pb = PackedBool::new(1000);
        let ratio = pb.compression_ratio();
        // Should be roughly 128x (16 bytes per Cell vs 1 bit per bool)
        assert!(ratio > 50.0, "compression ratio {} too low", ratio);
    }

    #[test]
    fn test_packed_bool_empty() {
        let pb = PackedBool::new(0);
        assert!(pb.is_empty());
        assert_eq!(pb.len(), 0);
    }
}
