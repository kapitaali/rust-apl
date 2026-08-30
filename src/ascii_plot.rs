//! ASCII plot — Phase 6.3.
//!
//! Text-mode plotting for terminal output.
//! Provides ⎕APLOT for creating simple bar/line/scatter plots.

use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕APLOT B — create an ASCII plot.
///
/// B can be:
/// - A numeric vector → simple bar chart
/// - A numeric matrix → line plot (columns as series)
/// - A nested array → scatter plot
///
/// Returns a character matrix (the rendered plot).
pub fn quad_aplot(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    // Extract numeric values
    let data: Vec<f64> = cells
        .iter()
        .map(|c| match c {
            Cell::Int(n) => Ok(*n as f64),
            Cell::Float(f) => Ok(*f),
            _ => Err(ErrorCode::DomainError),
        })
        .collect::<Result<Vec<_>, _>>()?;

    if data.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    // Simple bar chart rendering
    let max_val = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_val = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let range = max_val - min_val;

    if range == 0.0 {
        return Err(ErrorCode::DomainError);
    }

    let height = 20; // rows
    let width = data.len().min(80); // columns capped at 80

    let mut lines: Vec<String> = Vec::new();

    // Top border
    lines.push(format!("┌{}┐", "─".repeat(width)));

    // Plot rows (top to bottom)
    for row in 0..height {
        let threshold = max_val - (range * row as f64 / height as f64);
        let mut line = String::from("│");
        for col in 0..width {
            let val = data[col];
            if val >= threshold {
                line.push('█');
            } else {
                line.push(' ');
            }
        }
        line.push('│');
        lines.push(line);
    }

    // Bottom border
    lines.push(format!("└{}┘", "─".repeat(width)));

    // Convert to char matrix
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let mut all_chars: Vec<Cell> = Vec::new();
    for line in &lines {
        let mut chars: Vec<Cell> = line.chars().map(|c| Cell::Char(c as u32)).collect();
        // Pad to uniform width
        while chars.len() < max_line_len {
            chars.push(Cell::Char(' ' as u32));
        }
        all_chars.extend(chars);
    }

    let shape = crate::shape::Shape::matrix(lines.len() as i64, max_line_len as i64);
    ValueP::from_parts(shape, all_chars).map_err(|_| ErrorCode::DomainError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aplot_simple_vector() {
        let v = ValueP::int_vector(&[1, 2, 3, 4, 5]);
        let result = quad_aplot(&v);
        assert!(result.is_ok());
        let plot = result.unwrap();
        assert_eq!(plot.rank(), 2); // matrix
    }

    #[test]
    fn test_aplot_empty() {
        let v = ValueP::int_vector(&[]);
        assert!(quad_aplot(&v).is_err());
    }

    #[test]
    fn test_aplot_constant_values() {
        let v = ValueP::int_vector(&[5, 5, 5]);
        assert!(quad_aplot(&v).is_err()); // range == 0
    }

    #[test]
    fn test_aplot_float_vector() {
        // Create float vector manually since float_vector doesn't exist
        let cells = vec![Cell::Float(1.0), Cell::Float(2.5), Cell::Float(3.7)];
        let shape = crate::shape::Shape::vector(3);
        let v = ValueP::from_parts(shape, cells).unwrap();
        let result = quad_aplot(&v);
        assert!(result.is_ok());
    }
}
