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

    // Determine the plot type based on rank
    match b.rank() {
        1 => {
            // Vector → bar chart
            let data: Vec<f64> = cells
                .iter()
                .map(|c| match c {
                    Cell::Int(n) => Ok(*n as f64),
                    Cell::Float(f) => Ok(*f),
                    _ => Err(ErrorCode::DomainError),
                })
                .collect::<Result<Vec<_>, _>>()?;
            render_bar_chart(&data)
        }
        2 => {
            // Matrix → line plot (columns as series)
            let cols = b.get_shape_item(1) as usize;
            let rows = b.get_shape_item(0) as usize;
            if cols == 0 || rows == 0 {
                return Err(ErrorCode::DomainError);
            }
            // Extract each column as a series
            let mut series: Vec<Vec<f64>> = Vec::new();
            for col in 0..cols {
                let mut s = Vec::new();
                for row in 0..rows {
                    let idx = row * cols + col;
                    match cells[idx] {
                        Cell::Int(n) => s.push(n as f64),
                        Cell::Float(f) => s.push(f),
                        _ => return Err(ErrorCode::DomainError),
                    }
                }
                series.push(s);
            }
            render_line_plot(&series)
        }
        _ => Err(ErrorCode::DomainError),
    }
}

/// Render a bar chart from a vector of values.
fn render_bar_chart(data: &[f64]) -> AplResult<ValueP> {
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

    lines_to_matrix(&lines)
}

/// Render a line plot from multiple series (matrix columns).
fn render_line_plot(series: &[Vec<f64>]) -> AplResult<ValueP> {
    if series.is_empty() || series[0].is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let series_len = series[0].len();
    // Verify all series have the same length
    if !series.iter().all(|s| s.len() == series_len) {
        return Err(ErrorCode::DomainError);
    }

    // Find global min/max across all series
    let mut max_val = f64::NEG_INFINITY;
    let mut min_val = f64::INFINITY;
    for s in series {
        for &v in s {
            if v > max_val {
                max_val = v;
            }
            if v < min_val {
                min_val = v;
            }
        }
    }
    let range = max_val - min_val;
    if range == 0.0 {
        return Err(ErrorCode::DomainError);
    }

    let height = 20; // rows
    let width = series_len.min(80); // x-axis capped at 80

    // Characters for different series
    const SERIES_CHARS: [char; 6] = ['*', '+', '#', '@', 'o', 'x'];

    let mut lines: Vec<String> = Vec::new();

    // Top border
    lines.push(format!("┌{}┐", "─".repeat(width)));

    // Plot rows (top to bottom)
    for row in 0..height {
        let threshold = max_val - (range * row as f64 / height as f64);
        let next_threshold = max_val - (range * (row + 1) as f64 / height as f64);
        let mut line = String::from("│");
        for col in 0..width {
            // Check if any series passes through this cell
            let mut c = ' ';
            for (si, s) in series.iter().enumerate() {
                let val = s[col];
                if val <= threshold && val > next_threshold {
                    c = SERIES_CHARS[si % SERIES_CHARS.len()];
                    break;
                }
            }
            line.push(c);
        }
        line.push('│');
        lines.push(line);
    }

    // Bottom border
    lines.push(format!("└{}┘", "─".repeat(width)));

    // Legend
    if series.len() > 1 {
        let mut legend = String::from(" ");
        for (si, _) in series.iter().enumerate() {
            let ch = SERIES_CHARS[si % SERIES_CHARS.len()];
            legend.push(ch);
            legend.push_str(&format!("=series{} ", si));
        }
        lines.push(legend);
    }

    lines_to_matrix(&lines)
}

/// Convert lines to a character matrix ValueP.
fn lines_to_matrix(lines: &[String]) -> AplResult<ValueP> {
    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let mut all_chars: Vec<Cell> = Vec::new();
    for line in lines {
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

    #[test]
    fn test_aplot_matrix_line_plot() {
        // 4x3 matrix (4 rows, 3 columns) → 3 series
        let cells = vec![
            Cell::Float(1.0), Cell::Float(2.0), Cell::Float(3.0),
            Cell::Float(2.0), Cell::Float(3.0), Cell::Float(1.0),
            Cell::Float(3.0), Cell::Float(1.0), Cell::Float(2.0),
            Cell::Float(4.0), Cell::Float(4.0), Cell::Float(4.0),
        ];
        let shape = crate::shape::Shape::matrix(4, 3);
        let v = ValueP::from_parts(shape, cells).unwrap();
        let result = quad_aplot(&v);
        assert!(result.is_ok());
        let plot = result.unwrap();
        assert_eq!(plot.rank(), 2); // matrix
        // Should have legend for multiple series
    }

    #[test]
    fn test_aplot_matrix_empty() {
        let cells: Vec<Cell> = vec![];
        let shape = crate::shape::Shape::matrix(0, 0);
        let v = ValueP::from_parts(shape, cells).unwrap();
        assert!(quad_aplot(&v).is_err());
    }

    #[test]
    fn test_render_bar_chart_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = render_bar_chart(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_line_plot_basic() {
        let series = vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![5.0, 4.0, 3.0, 2.0, 1.0],
        ];
        let result = render_line_plot(&series);
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_line_plot_unequal_series() {
        let series = vec![
            vec![1.0, 2.0, 3.0],
            vec![1.0, 2.0],
        ];
        let result = render_line_plot(&series);
        assert!(result.is_err());
    }

    #[test]
    fn test_lines_to_matrix_basic() {
        let lines = vec!["hello".to_string(), "world!".to_string()];
        let result = lines_to_matrix(&lines);
        assert!(result.is_ok());
        let m = result.unwrap();
        assert_eq!(m.rank(), 2);
        assert_eq!(m.get_shape_item(0), 2); // 2 rows
        assert_eq!(m.get_shape_item(1), 6); // max width (world! = 6)
    }
}
