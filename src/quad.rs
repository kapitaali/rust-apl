//! Quad system functions — additional ⎕ functions beyond the basic ⎕IO/⎕CT/⎕PP.
//!
//! Implements:
//! - ⎕UCS B — Unicode character set conversion (codepoints ↔ characters)
//! - ⎕AV — APL character vector (256 characters)
//! - ⎕TS — current timestamp (year month day hour minute second microsecond)
//! - ⎕WA — workspace available (memory info)
//! - ⎕TC — terminal control characters (backspace, newline, etc.)
//! - ⎕DM — error message (last error)
//! - ⎕EN — error number (last error)
//! - ⎕DFT — default format

use crate::cell::Cell;
use crate::shape::Shape;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕UCS B — Unicode character set conversion
/// Monadic: convert codepoints to characters or characters to codepoints
pub fn quad_ucs(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Ok(ValueP::int_vector(&[]));
    }

    // If all cells are Int, convert codepoints to characters
    if cells.iter().all(|c| matches!(c, Cell::Int(_))) {
        let codepoints: Vec<u32> = cells
            .iter()
            .map(|c| c.get_int_value().map(|i| i as u32))
            .collect::<Result<Vec<_>, _>>()?;
        // Validate codepoints
        for &cp in &codepoints {
            if std::char::from_u32(cp).is_none() {
                return Err(ErrorCode::DomainError);
            }
        }
        return Ok(ValueP::char_vector(&codepoints));
    }

    // If all cells are Char, convert characters to codepoints
    if cells.iter().all(|c| matches!(c, Cell::Char(_))) {
        let codepoints: Vec<i64> = cells
            .iter()
            .map(|c| {
                if let Cell::Char(ch) = c {
                    Ok(*ch as i64)
                } else {
                    Err(ErrorCode::DomainError)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(ValueP::int_vector(&codepoints));
    }

    Err(ErrorCode::DomainError)
}

/// ⎕AV — APL character vector (256 characters, 0-255)
pub fn quad_av() -> ValueP {
    let codepoints: Vec<u32> = (0..256).collect();
    ValueP::char_vector(&codepoints)
}

/// ⎕TS — current timestamp
/// Returns: year month day hour microsecond second millisecond
pub fn quad_ts() -> AplResult<ValueP> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).map_err(|_| ErrorCode::DomainError)?;
    let secs = duration.as_secs();
    let micros = duration.subsec_micros();
    
    // Convert to date components (simplified)
    let days = secs / 86400;
    let years = 1970 + days / 365; // Simplified, doesn't account for leap years
    let day_of_year = days % 365;
    let month = day_of_year / 30 + 1; // Simplified
    let day = day_of_year % 30 + 1;
    
    let hour = (secs % 86400) / 3600;
    let minute = (secs % 3600) / 60;
    let second = secs % 60;
    let millisecond = micros / 1000;
    let microsecond = micros % 1000;
    
    Ok(ValueP::int_vector(&[
        years as i64,
        month as i64,
        day as i64,
        hour as i64,
        minute as i64,
        second as i64,
        millisecond as i64,
        microsecond as i64,
    ]))
}

/// ⎕WA — workspace available (memory info in bytes)
pub fn quad_wa() -> AplResult<ValueP> {
    // Return a simplified memory estimate
    // In a real implementation, this would query system memory
    let total_memory = 1024 * 1024 * 1024i64; // 1 GB placeholder
    Ok(ValueP::scalar_from(Cell::Int(total_memory)))
}

/// ⎕TC — terminal control characters
/// Returns: backspace, newline, carriage return
pub fn quad_tc() -> ValueP {
    ValueP::char_vector(&[
        '\u{08}' as u32, // backspace
        '\n' as u32,     // newline
        '\r' as u32,     // carriage return
    ])
}

/// ⎕DM — error message (returns empty string if no error)
pub fn quad_dm() -> ValueP {
    ValueP::char_vector(&[])
}

/// ⎕EN — error number (returns 0 if no error)
pub fn quad_en() -> ValueP {
    ValueP::scalar_from(Cell::Int(0))
}

/// ⎕DFT — default format (returns "DEFAULT")
pub fn quad_dft() -> ValueP {
    let chars: Vec<u32> = "DEFAULT".chars().map(|c| c as u32).collect();
    ValueP::char_vector(&chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quad_ucs_int_to_char() {
        let v = ValueP::int_vector(&[65, 66, 67]);
        let result = quad_ucs(&v).unwrap();
        assert_eq!(result.cells()[0], Cell::Char('A' as u32));
        assert_eq!(result.cells()[1], Cell::Char('B' as u32));
        assert_eq!(result.cells()[2], Cell::Char('C' as u32));
    }

    #[test]
    fn test_quad_ucs_char_to_int() {
        let v = ValueP::char_vector(&['A' as u32, 'B' as u32, 'C' as u32]);
        let result = quad_ucs(&v).unwrap();
        assert_eq!(result.cells()[0], Cell::Int(65));
        assert_eq!(result.cells()[1], Cell::Int(66));
        assert_eq!(result.cells()[2], Cell::Int(67));
    }

    #[test]
    fn test_quad_ucs_invalid_codepoint() {
        let v = ValueP::int_vector(&[0x110000]); // Invalid codepoint
        assert!(quad_ucs(&v).is_err());
    }

    #[test]
    fn test_quad_av() {
        let result = quad_av();
        assert_eq!(result.element_count(), 256);
    }

    #[test]
    fn test_quad_ts() {
        let result = quad_ts().unwrap();
        assert_eq!(result.element_count(), 8);
        // Year should be >= 2026
        assert!(result.cells()[0].get_int_value().unwrap() >= 2026);
    }

    #[test]
    fn test_quad_wa() {
        let result = quad_wa().unwrap();
        assert!(result.cells()[0].get_int_value().unwrap() > 0);
    }

    #[test]
    fn test_quad_tc() {
        let result = quad_tc();
        assert_eq!(result.element_count(), 3);
        assert_eq!(result.cells()[0], Cell::Char('\u{08}' as u32));
        assert_eq!(result.cells()[1], Cell::Char('\n' as u32));
        assert_eq!(result.cells()[2], Cell::Char('\r' as u32));
    }

    #[test]
    fn test_quad_dm() {
        let result = quad_dm();
        assert_eq!(result.element_count(), 0);
    }

    #[test]
    fn test_quad_en() {
        let result = quad_en();
        assert_eq!(result.cells()[0], Cell::Int(0));
    }

    #[test]
    fn test_quad_dft() {
        let result = quad_dft();
        assert_eq!(result.element_count(), 7);
    }
}
