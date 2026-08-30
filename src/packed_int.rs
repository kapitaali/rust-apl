//! Packed integer array storage.
//!
//! Extends the PackedBool concept to integers with variable bit-width.
//! Analyzes the range of values to select the minimum bit-width needed:
//! - 0 bits: all zeros
//! - 1 bit unsigned: values 0 or 1
//! - 4 bits unsigned: values 0..15
//! - 8 bits signed: values -128..127
//! - 8 bits unsigned: values 0..255
//! - 16 bits signed: values -32768..32767
//! - 16 bits unsigned: values 0..65535
//! - 32 bits signed: values i32::MIN..i32::MAX
//! - 32 bits unsigned: values 0..u32::MAX
//! - 64 bits signed: full i64 range

use crate::cell::Cell;

/// A packed integer vector stored as fixed-width integers in u64 words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedInt {
    len: usize,
    bit_width: u8,
    signed: bool,
    bits: Vec<u64>,
}

impl PackedInt {
    pub fn from_values(values: &[i64]) -> Self {
        if values.is_empty() {
            return PackedInt {
                len: 0,
                bit_width: 0,
                signed: false,
                bits: Vec::new(),
            };
        }

        let (min_val, max_val) = values.iter().fold((i64::MAX, i64::MIN), |(min, max), &v| {
            (min.min(v), max.max(v))
        });

        let (bit_width, signed) = Self::select_bit_width(min_val, max_val);
        let mut packed = PackedInt {
            len: values.len(),
            bit_width,
            signed,
            bits: Vec::new(),
        };

        if bit_width == 0 {
            return packed;
        }

        let values_per_word = 64 / bit_width as usize;
        let num_words = (values.len() + values_per_word - 1) / values_per_word;
        packed.bits = vec![0u64; num_words];

        for (i, &v) in values.iter().enumerate() {
            packed.set(i, v);
        }

        packed
    }

    pub fn from_cells(cells: &[Cell]) -> Self {
        let values: Vec<i64> = cells
            .iter()
            .map(|c| c.get_int_value().unwrap_or(0))
            .collect();
        Self::from_values(&values)
    }

    fn select_bit_width(min_val: i64, max_val: i64) -> (u8, bool) {
        if min_val == 0 && max_val == 0 {
            (0, false)
        } else if min_val >= 0 && max_val <= 1 {
            (1, false)
        } else if min_val >= 0 && max_val <= 15 {
            (4, false)
        } else if min_val >= -128 && max_val <= 127 {
            (8, true)
        } else if min_val >= 0 && max_val <= 255 {
            (8, false)
        } else if min_val >= -32768 && max_val <= 32767 {
            (16, true)
        } else if min_val >= 0 && max_val <= 65535 {
            (16, false)
        } else if min_val >= i32::MIN as i64 && max_val <= i32::MAX as i64 {
            (32, true)
        } else if min_val >= 0 && max_val <= u32::MAX as i64 {
            (32, false)
        } else {
            (64, true)
        }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn bit_width(&self) -> u8 { self.bit_width }
    pub fn is_signed(&self) -> bool { self.signed }

    pub fn get(&self, i: usize) -> i64 {
        if i >= self.len {
            panic!("PackedInt index out of bounds: {} >= {}", i, self.len);
        }
        if self.bit_width == 0 { return 0; }

        // Special case: 64-bit values stored one per word
        if self.bit_width == 64 {
            return self.bits[i] as i64;
        }

        let values_per_word = 64 / self.bit_width as usize;
        let word_idx = i / values_per_word;
        let bit_offset = (i % values_per_word) * self.bit_width as usize;
        let mask = (1u64 << self.bit_width) - 1;
        let raw = (self.bits[word_idx] >> bit_offset) & mask;

        if self.signed && self.bit_width < 64 {
            let sign_bit = 1u64 << (self.bit_width - 1);
            if raw & sign_bit != 0 {
                let extend_mask = !((1u64 << self.bit_width) - 1);
                (raw | extend_mask) as i64
            } else {
                raw as i64
            }
        } else {
            raw as i64
        }
    }

    pub fn set(&mut self, i: usize, val: i64) {
        if i >= self.len {
            panic!("PackedInt index out of bounds: {} >= {}", i, self.len);
        }
        if self.bit_width == 0 { return; }

        // Special case: 64-bit values stored one per word
        if self.bit_width == 64 {
            self.bits[i] = val as u64;
            return;
        }

        let values_per_word = 64 / self.bit_width as usize;
        let word_idx = i / values_per_word;
        let bit_offset = (i % values_per_word) * self.bit_width as usize;
        let mask = (1u64 << self.bit_width) - 1;
        let raw = val as u64 & mask;

        let clear_mask = !(mask << bit_offset);
        self.bits[word_idx] = (self.bits[word_idx] & clear_mask) | (raw << bit_offset);
    }

    pub fn to_cells(&self) -> Vec<Cell> {
        (0..self.len).map(|i| Cell::int(self.get(i))).collect()
    }

    pub fn to_values(&self) -> Vec<i64> {
        (0..self.len).map(|i| self.get(i)).collect()
    }

    pub fn memory_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.bits.capacity() * 8
    }

    pub fn unpacked_memory_bytes(&self) -> usize {
        self.len * std::mem::size_of::<Cell>()
    }

    pub fn compression_ratio(&self) -> f64 {
        self.unpacked_memory_bytes() as f64 / self.memory_bytes().max(1) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packed_int_empty() {
        let pi = PackedInt::from_values(&[]);
        assert_eq!(pi.len(), 0);
        assert_eq!(pi.bit_width(), 0);
    }

    #[test]
    fn test_packed_int_all_zeros() {
        let pi = PackedInt::from_values(&[0, 0, 0, 0]);
        assert_eq!(pi.len(), 4);
        assert_eq!(pi.bit_width(), 0);
        for i in 0..4 { assert_eq!(pi.get(i), 0); }
    }

    #[test]
    fn test_packed_int_single_bit() {
        let pi = PackedInt::from_values(&[0, 1, 0, 1, 1]);
        assert_eq!(pi.bit_width(), 1);
        assert!(!pi.is_signed());
        assert_eq!(pi.get(0), 0);
        assert_eq!(pi.get(1), 1);
        assert_eq!(pi.get(2), 0);
        assert_eq!(pi.get(3), 1);
        assert_eq!(pi.get(4), 1);
    }

    #[test]
    fn test_packed_int_4bit() {
        let pi = PackedInt::from_values(&[0, 5, 10, 15, 3, 7]);
        assert_eq!(pi.bit_width(), 4);
        assert_eq!(pi.get(0), 0);
        assert_eq!(pi.get(1), 5);
        assert_eq!(pi.get(2), 10);
        assert_eq!(pi.get(3), 15);
        assert_eq!(pi.get(4), 3);
        assert_eq!(pi.get(5), 7);
    }

    #[test]
    fn test_packed_int_8bit_unsigned() {
        let pi = PackedInt::from_values(&[0, 100, 200, 255]);
        assert_eq!(pi.bit_width(), 8);
        assert!(!pi.is_signed());
        assert_eq!(pi.get(0), 0);
        assert_eq!(pi.get(1), 100);
        assert_eq!(pi.get(2), 200);
        assert_eq!(pi.get(3), 255);
    }

    #[test]
    fn test_packed_int_8bit_signed() {
        let pi = PackedInt::from_values(&[-100, 0, 50, -128, 127]);
        assert_eq!(pi.bit_width(), 8);
        assert!(pi.is_signed());
        assert_eq!(pi.get(0), -100);
        assert_eq!(pi.get(1), 0);
        assert_eq!(pi.get(2), 50);
        assert_eq!(pi.get(3), -128);
        assert_eq!(pi.get(4), 127);
    }

    #[test]
    fn test_packed_int_16bit() {
        let pi = PackedInt::from_values(&[0, 1000, -1000, 32767, -32768]);
        assert_eq!(pi.bit_width(), 16);
        assert!(pi.is_signed());
        assert_eq!(pi.get(0), 0);
        assert_eq!(pi.get(1), 1000);
        assert_eq!(pi.get(2), -1000);
        assert_eq!(pi.get(3), 32767);
        assert_eq!(pi.get(4), -32768);
    }

    #[test]
    fn test_packed_int_32bit() {
        let pi = PackedInt::from_values(&[0, 100000, -100000, i32::MAX as i64, i32::MIN as i64]);
        assert_eq!(pi.bit_width(), 32);
        assert!(pi.is_signed());
        assert_eq!(pi.get(0), 0);
        assert_eq!(pi.get(1), 100000);
        assert_eq!(pi.get(2), -100000);
        assert_eq!(pi.get(3), i32::MAX as i64);
        assert_eq!(pi.get(4), i32::MIN as i64);
    }

    #[test]
    fn test_packed_int_64bit() {
        let pi = PackedInt::from_values(&[0, i64::MAX, i64::MIN, 12345678901234]);
        assert_eq!(pi.bit_width(), 64);
        assert!(pi.is_signed());
        assert_eq!(pi.get(0), 0);
        assert_eq!(pi.get(1), i64::MAX);
        assert_eq!(pi.get(2), i64::MIN);
        assert_eq!(pi.get(3), 12345678901234);
    }

    #[test]
    fn test_packed_int_from_cells() {
        let cells = vec![Cell::int(10), Cell::int(20), Cell::int(30)];
        let pi = PackedInt::from_cells(&cells);
        assert_eq!(pi.len(), 3);
        assert_eq!(pi.get(0), 10);
        assert_eq!(pi.get(1), 20);
        assert_eq!(pi.get(2), 30);
    }

    #[test]
    fn test_packed_int_to_cells() {
        let pi = PackedInt::from_values(&[1, 2, 3, 4, 5]);
        let cells = pi.to_cells();
        assert_eq!(cells.len(), 5);
        assert_eq!(cells[0], Cell::int(1));
        assert_eq!(cells[4], Cell::int(5));
    }

    #[test]
    fn test_packed_int_set() {
        let mut pi = PackedInt::from_values(&[0, 0, 0, 0]);
        // Note: all zeros means bit_width=0, so set is a no-op. Test with non-zero.
        let mut pi = PackedInt::from_values(&[0, 1, 0, 1]);
        pi.set(0, 1);
        pi.set(1, 0);
        pi.set(2, 1);
        pi.set(3, 0);
        assert_eq!(pi.get(0), 1);
        assert_eq!(pi.get(1), 0);
        assert_eq!(pi.get(2), 1);
        assert_eq!(pi.get(3), 0);
    }

    #[test]
    fn test_packed_int_compression() {
        let values: Vec<i64> = (0..1000).map(|i| (i % 16) as i64).collect();
        let pi = PackedInt::from_values(&values);
        assert_eq!(pi.bit_width(), 4);
        let ratio = pi.compression_ratio();
        assert!(ratio > 4.0, "Expected >4x compression, got {}", ratio);
    }

    #[test]
    fn test_packed_int_roundtrip() {
        let original = vec![-5, 0, 5, 100, -100, 1000, -1000, 100000];
        let pi = PackedInt::from_values(&original);
        let recovered = pi.to_values();
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_packed_int_roundtrip_unsigned() {
        let original = vec![0, 1, 100, 200, 255];
        let pi = PackedInt::from_values(&original);
        assert!(!pi.is_signed());
        let recovered = pi.to_values();
        assert_eq!(original, recovered);
    }
}
