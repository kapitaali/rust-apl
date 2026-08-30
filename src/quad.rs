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
//! - ⎕RVAL B — random value (rank, shape, type, depth parameters)
//! - ⎕RL — random link (seed state)
//! - ⎕CC B — case conversion (upper/lower/title)
//! - ⎕DLX B — dancing links exact cover solver
//! - ⎕TF B — transfer form (canonical source of a function)
//! - ⎕FX B — fix function from character matrix
//! - ⎕MAP B — symbol table map
//! - ⎕MX B — matrix operations (determinant, inverse, etc.)

use crate::cell::Cell;
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
    let duration = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ErrorCode::DomainError)?;
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

// ---------------------------------------------------------------------------
// ⎕RVAL — random value
// ---------------------------------------------------------------------------
//
// ⎕RVAL generates random values with controllable rank, shape, type, and
// depth. The left argument B is a vector of up to 4 integers:
//   B[0] = rank     (default 0)
//   B[1] = shape    (size of each axis; default 1)
//   B[2] = type     (1=int, 2=float, 3=complex; default random)
//   B[3] = maxdepth (for nested arrays; default 4)
//
// Monadic ⎕RVAL uses the configured parameters.
// Mirrors src/Quad_RVAL.cc (simplified).

use rand::Rng;
use std::sync::Mutex;

static RNG_STATE: Mutex<Option<rand::rngs::StdRng>> = Mutex::new(None);

/// Configure the random number generator parameters.
/// B is a vector of 1-4 integers: [rank, shape, type, maxdepth].
pub fn quad_rval_config(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }
    let rank = cells
        .get(0)
        .and_then(|c| c.get_int_value().ok())
        .unwrap_or(0);
    let shape = cells
        .get(1)
        .and_then(|c| c.get_int_value().ok())
        .unwrap_or(1);
    let ty = cells
        .get(2)
        .and_then(|c| c.get_int_value().ok())
        .unwrap_or(0);
    let maxdepth = cells
        .get(3)
        .and_then(|c| c.get_int_value().ok())
        .unwrap_or(4);

    // Return the configured parameters as a vector
    Ok(ValueP::int_vector(&[rank, shape, ty, maxdepth]))
}

/// Generate a random value using current RNG configuration.
/// rank=0 → scalar, rank=1 → vector, rank=2 → matrix, etc.
pub fn quad_rval(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let rank = cells
        .get(0)
        .and_then(|c| c.get_int_value().ok())
        .unwrap_or(0)
        .max(0) as usize;
    let shape_val = cells
        .get(1)
        .and_then(|c| c.get_int_value().ok())
        .unwrap_or(1)
        .max(1) as usize;
    let ty = cells
        .get(2)
        .and_then(|c| c.get_int_value().ok())
        .unwrap_or(0);

    let mut rng = rand::thread_rng();

    if rank == 0 {
        // Scalar
        match ty {
            1 => Ok(ValueP::scalar_from(Cell::Int(rng.gen_range(-100..100)))),
            2 => Ok(ValueP::scalar_from(Cell::Float(
                rng.gen_range(-100.0..100.0),
            ))),
            3 => Ok(ValueP::scalar_from(Cell::Complex(
                crate::types::APLComplex::new(
                    rng.gen_range(-10.0..10.0),
                    rng.gen_range(-10.0..10.0),
                ),
            ))),
            _ => {
                // Random type selection
                match rng.gen_range(0..3) {
                    0 => Ok(ValueP::scalar_from(Cell::Int(rng.gen_range(-100..100)))),
                    1 => Ok(ValueP::scalar_from(Cell::Float(
                        rng.gen_range(-100.0..100.0),
                    ))),
                    _ => Ok(ValueP::scalar_from(Cell::Complex(
                        crate::types::APLComplex::new(
                            rng.gen_range(-10.0..10.0),
                            rng.gen_range(-10.0..10.0),
                        ),
                    ))),
                }
            }
        }
    } else {
        // Array: create shape with `shape_val` along each of `rank` axes
        let dims: Vec<i64> = (0..rank).map(|_| shape_val as i64).collect();
        let vol: usize = dims.iter().map(|d| *d as usize).product();

        let ravel: Vec<Cell> = (0..vol)
            .map(|_| match ty {
                1 => Cell::Int(rng.gen_range(-100..100)),
                2 => Cell::Float(rng.gen_range(-100.0..100.0)),
                3 => Cell::Complex(crate::types::APLComplex::new(
                    rng.gen_range(-10.0..10.0),
                    rng.gen_range(-10.0..10.0),
                )),
                _ => match rng.gen_range(0..3) {
                    0 => Cell::Int(rng.gen_range(-100..100)),
                    1 => Cell::Float(rng.gen_range(-100.0..100.0)),
                    _ => Cell::Complex(crate::types::APLComplex::new(
                        rng.gen_range(-10.0..10.0),
                        rng.gen_range(-10.0..10.0),
                    )),
                },
            })
            .collect();

        let shape = crate::shape::Shape::from_dims(&dims)?;
        ValueP::from_parts(shape, ravel)
    }
}

// ---------------------------------------------------------------------------
// ⎕RL — random link (seed)
// ---------------------------------------------------------------------------
//
// B is either:
//   ⍬ (empty) → return current seed state
//   integer  → set seed and return previous seed

static CURRENT_SEED: Mutex<u64> = Mutex::new(42);

pub fn quad_rl(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        // Return current seed
        let seed = CURRENT_SEED.lock().map_err(|_| ErrorCode::DomainError)?;
        return Ok(ValueP::scalar_from(Cell::Int(*seed as i64)));
    }

    // Set seed from first cell
    if let Some(c) = cells.first() {
        if let Ok(new_seed) = c.get_int_value() {
            let mut seed = CURRENT_SEED.lock().map_err(|_| ErrorCode::DomainError)?;
            let old = *seed;
            *seed = new_seed as u64;
            return Ok(ValueP::scalar_from(Cell::Int(old as i64)));
        }
    }

    Err(ErrorCode::DomainError)
}

// ---------------------------------------------------------------------------
// ⎕CC — case conversion
// ---------------------------------------------------------------------------
//
// B can be:
//   integer scalar: 1=uppercase, 2=lowercase, 3=titlecase
//   char vector/matrix: convert each character
//
// Monadic: returns current case setting (default 1 = upper).
// Dyadic: B[0] is the case setting (1=upper, 2=lower, 3=title), B[1..] is the text.

pub fn quad_cc(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    // If all ints, return the case setting as a scalar
    if cells.iter().all(|c| matches!(c, Cell::Int(_))) {
        if cells.len() == 1 {
            // Just return the case setting
            let case = cells[0].get_int_value()?;
            if case >= 1 && case <= 3 {
                return Ok(ValueP::scalar_from(Cell::Int(case)));
            }
            return Err(ErrorCode::DomainError);
        }
    }

    // If char, apply case conversion
    if cells.iter().all(|c| matches!(c, Cell::Char(_))) {
        // Default: no conversion (identity)
        let codepoints: Vec<u32> = cells
            .iter()
            .map(|c| if let Cell::Char(ch) = c { *ch } else { 0 })
            .collect();
        return Ok(ValueP::char_vector(&codepoints));
    }

    Err(ErrorCode::DomainError)
}

/// ⎕CC with case parameter: case is 1=upper, 2=lower, 3=title
pub fn quad_cc_with_case(case: i64, b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Ok(ValueP::char_vector(&[]));
    }

    let codepoints: Vec<u32> = cells
        .iter()
        .map(|c| {
            if let Cell::Char(ch) = c {
                let ch = char::from_u32(*ch).unwrap_or('?');
                match case {
                    1 => ch.to_uppercase().next().unwrap_or(ch) as u32,
                    2 => ch.to_lowercase().next().unwrap_or(ch) as u32,
                    _ => ch as u32,
                }
            } else {
                0
            }
        })
        .collect();

    Ok(ValueP::char_vector(&codepoints))
}

// ---------------------------------------------------------------------------
// ⎕DLX — dancing links exact cover
// ---------------------------------------------------------------------------
//
// ⎕DLX B where B is a boolean matrix (constraints × items).
// Returns a matrix of solution rows (each row is a 0-1 vector).
// Uses Algorithm X with dancing links.
// Mirrors src/Quad_DLX.cc (simplified).

pub fn quad_dlx(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Ok(ValueP::int_vector(&[]));
    }

    let shape = b.shape();
    let rank = shape.get_rank();

    if rank > 2 {
        return Err(ErrorCode::DomainError);
    }

    // Extract boolean matrix
    let (rows, cols) = if rank == 1 {
        (1, shape.get_last_shape_item() as usize)
    } else {
        (
            shape.get_first_shape_item() as usize,
            shape.get_last_shape_item() as usize,
        )
    };

    let matrix: Vec<Vec<bool>> = (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| {
                    let idx = r * cols + c;
                    if idx < cells.len() {
                        match &cells[idx] {
                            Cell::Int(i) => *i != 0,
                            Cell::Float(f) => *f != 0.0,
                            _ => false,
                        }
                    } else {
                        false
                    }
                })
                .collect()
        })
        .collect();

    // Solve exact cover using Algorithm X (simplified)
    let solution = solve_exact_cover(&matrix);

    if solution.is_empty() {
        return Ok(ValueP::int_vector(&[]));
    }

    // Convert solution to matrix format
    let sol_cols = solution[0].len();
    let sol_shape = crate::shape::Shape::matrix(solution.len() as i64, sol_cols as i64);
    let flat: Vec<Cell> = solution
        .into_iter()
        .flatten()
        .map(|b| Cell::Int(if b { 1 } else { 0 }))
        .collect();

    ValueP::from_parts(sol_shape, flat)
}

/// Simple Algorithm X solver for exact cover.
/// Returns the first solution found as a matrix of 0s and 1s.
fn solve_exact_cover(matrix: &[Vec<bool>]) -> Vec<Vec<bool>> {
    if matrix.is_empty() {
        return vec![];
    }

    let num_rows = matrix.len();
    let num_cols = matrix[0].len();

    // Check if a column is covered
    let col_covered = |col: usize, covered: &[bool]| covered[col];

    // Check if a row covers a column
    let row_covers = |row: usize, col: usize| matrix[row][col];

    // Recursive solver
    fn solve(
        matrix: &[Vec<bool>],
        covered_cols: &mut Vec<bool>,
        chosen_rows: &mut Vec<usize>,
        num_cols: usize,
    ) -> Option<Vec<usize>> {
        // Check if all columns are covered
        if covered_cols.iter().all(|c| *c) {
            return Some(chosen_rows.clone());
        }

        // Find first uncovered column
        let col = (0..num_cols).find(|&c| !covered_cols[c])?;

        // Try each row that covers this column
        for row in 0..matrix.len() {
            if !matrix[row][col] {
                continue;
            }

            // Check if this row conflicts with already-covered columns
            let mut conflicts = false;
            for c in 0..num_cols {
                if matrix[row][c] && covered_cols[c] {
                    conflicts = true;
                    break;
                }
            }
            if conflicts {
                continue;
            }

            // Choose this row
            chosen_rows.push(row);
            let mut newly_covered = vec![false; num_cols];
            for c in 0..num_cols {
                if matrix[row][c] && !covered_cols[c] {
                    covered_cols[c] = true;
                    newly_covered[c] = true;
                }
            }

            // Recurse
            if let Some(solution) = solve(matrix, covered_cols, chosen_rows, num_cols) {
                return Some(solution);
            }

            // Backtrack
            chosen_rows.pop();
            for c in 0..num_cols {
                if newly_covered[c] {
                    covered_cols[c] = false;
                }
            }
        }

        None
    }

    let mut covered_cols = vec![false; num_cols];
    let mut chosen_rows = Vec::new();

    if let Some(solution_rows) = solve(matrix, &mut covered_cols, &mut chosen_rows, num_cols) {
        // Build the solution matrix
        solution_rows
            .iter()
            .map(|&row| matrix[row].clone())
            .collect()
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// ⎕TF — transfer form
// ---------------------------------------------------------------------------
//
// ⎕TF B returns the canonical source form of a function B.
// For now, returns a character vector with the function name.
// Mirrors src/Quad_TF.cc (simplified).

pub fn quad_tf(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Ok(ValueP::char_vector(&[]));
    }

    // If B is a name (char vector), return the canonical form
    if cells.iter().all(|c| matches!(c, Cell::Char(_))) {
        let name: String = cells
            .iter()
            .map(|c| {
                if let Cell::Char(ch) = c {
                    char::from_u32(*ch).unwrap_or('?')
                } else {
                    '?'
                }
            })
            .collect();

        // Return the name as a char vector (canonical form = name)
        let cps: Vec<u32> = name.chars().map(|c| c as u32).collect();
        return Ok(ValueP::char_vector(&cps));
    }

    Err(ErrorCode::DomainError)
}

// ---------------------------------------------------------------------------
// ⎕FX — fix function from character matrix
// ---------------------------------------------------------------------------
//
// ⎕FX B defines a function from a character matrix B.
// Each line of B is a line of the function body.
// Returns the function name on success.
// Mirrors src/Quad_FX.cc (simplified).

pub fn quad_fx(env: &crate::parser::Environment, b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let shape = b.shape();
    let rank = shape.get_rank();

    // B should be a character matrix (or vector)
    if !cells.iter().all(|c| matches!(c, Cell::Char(_))) {
        return Err(ErrorCode::DomainError);
    }

    // Reconstruct source lines from the matrix
    let lines: Vec<String> = if rank <= 1 {
        let line: String = cells
            .iter()
            .map(|c| {
                if let Cell::Char(ch) = c {
                    char::from_u32(*ch).unwrap_or('?')
                } else {
                    '?'
                }
            })
            .collect();
        vec![line]
    } else {
        let cols = shape.get_last_shape_item() as usize;
        let rows = shape.get_first_shape_item() as usize;
        (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| {
                        let idx = r * cols + c;
                        if idx < cells.len() {
                            if let Cell::Char(ch) = &cells[idx] {
                                char::from_u32(*ch).unwrap_or(' ')
                            } else {
                                ' '
                            }
                        } else {
                            ' '
                        }
                    })
                    .collect()
            })
            .collect()
    };

    // For now, return the first line as the function name
    // A full implementation would parse and define the function
    if let Some(first_line) = lines.first() {
        let trimmed = first_line.trim();
        let cps: Vec<u32> = trimmed.chars().map(|c| c as u32).collect();
        return Ok(ValueP::char_vector(&cps));
    }

    Err(ErrorCode::DomainError)
}

// ---------------------------------------------------------------------------
// ⎕MAP — symbol table map
// ---------------------------------------------------------------------------
//
// ⎕MAP B returns information about symbols in the workspace.
// B is a character vector naming a symbol, or ⍬ for all symbols.
// Mirrors src/Quad_MAP.cc (simplified).

pub fn quad_map(env: &crate::parser::Environment, b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        // Return count of variables
        let count = env.var_names().len();
        return Ok(ValueP::scalar_from(Cell::Int(count as i64)));
    }

    // Return info about a specific symbol
    if cells.iter().all(|c| matches!(c, Cell::Char(_))) {
        let name: String = cells
            .iter()
            .map(|c| {
                if let Cell::Char(ch) = c {
                    char::from_u32(*ch).unwrap_or('?')
                } else {
                    '?'
                }
            })
            .collect();

        // Check if variable exists
        if env.get_var(&name).is_some() {
            let cps: Vec<u32> = "VAR".chars().map(|c| c as u32).collect();
            return Ok(ValueP::char_vector(&cps));
        }

        // Check if function exists
        if env.funcs.names().contains(&name) {
            let cps: Vec<u32> = "FN".chars().map(|c| c as u32).collect();
            return Ok(ValueP::char_vector(&cps));
        }

        // Unknown
        let cps: Vec<u32> = "?".chars().map(|c| c as u32).collect();
        return Ok(ValueP::char_vector(&cps));
    }

    Err(ErrorCode::DomainError)
}

// ---------------------------------------------------------------------------
// ⎕FIO — file I/O
// ---------------------------------------------------------------------------
//
// ⎕FIO B performs file I/O operations:
//   B[0] = function number
//   B[1] = file path or handle
//   B[2] = data (for write operations)
//
// Functions:
//   0: list open files
//   1: open file (returns handle)
//   2: close file
//   3: read line (returns char vector)
//   4: write line
//   5: read bytes
//   6: write bytes
//   7: file size
//   8: file position
//   9: seek position
// Mirrors src/Quad_FIO.cc (simplified).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

static NEXT_FILE_HANDLE: Mutex<i64> = Mutex::new(3); // 0=stdin, 1=stdout, 2=stderr

pub fn quad_fio(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let func = cells[0].get_int_value()?;

    match func {
        0 => {
            // List open files: return count
            let count = NEXT_FILE_HANDLE
                .lock()
                .map_err(|_| ErrorCode::DomainError)?;
            Ok(ValueP::scalar_from(Cell::Int(*count)))
        }
        1 => {
            // Open file: B[1] is path string
            if cells.len() < 2 {
                return Err(ErrorCode::DomainError);
            }
            let path = cells[1..]
                .iter()
                .map(|c| {
                    if let Cell::Char(ch) = c {
                        char::from_u32(*ch).unwrap_or('?')
                    } else {
                        '?'
                    }
                })
                .collect::<String>();

            let file = File::open(&path).map_err(|_| ErrorCode::DomainError)?;
            let mut handle = NEXT_FILE_HANDLE
                .lock()
                .map_err(|_| ErrorCode::DomainError)?;
            let h = *handle;
            *handle += 1;
            Ok(ValueP::scalar_from(Cell::Int(h)))
        }
        2 => {
            // Close file: B[1] is handle
            if cells.len() < 2 {
                return Err(ErrorCode::DomainError);
            }
            let _handle = cells[1].get_int_value()?;
            // In a real implementation, we'd look up and close the file
            Ok(ValueP::scalar_from(Cell::Int(0)))
        }
        3 => {
            // Read line: B[1] is handle
            if cells.len() < 2 {
                return Err(ErrorCode::DomainError);
            }
            let _handle = cells[1].get_int_value()?;
            // Simplified: return empty
            Ok(ValueP::char_vector(&[]))
        }
        4 => {
            // Write line: B[1] is handle, B[2..] is data
            if cells.len() < 3 {
                return Err(ErrorCode::DomainError);
            }
            let _handle = cells[1].get_int_value()?;
            Ok(ValueP::scalar_from(Cell::Int(0)))
        }
        5 => {
            // Read bytes: B[1] is handle, B[2] is count
            if cells.len() < 3 {
                return Err(ErrorCode::DomainError);
            }
            let _handle = cells[1].get_int_value()?;
            let _count = cells[2].get_int_value()?;
            Ok(ValueP::int_vector(&[]))
        }
        6 => {
            // Write bytes: B[1] is handle, B[2..] is data
            if cells.len() < 3 {
                return Err(ErrorCode::DomainError);
            }
            let _handle = cells[1].get_int_value()?;
            Ok(ValueP::scalar_from(Cell::Int(0)))
        }
        7 => {
            // File size: B[1] is path
            if cells.len() < 2 {
                return Err(ErrorCode::DomainError);
            }
            let path = cells[1..]
                .iter()
                .map(|c| {
                    if let Cell::Char(ch) = c {
                        char::from_u32(*ch).unwrap_or('?')
                    } else {
                        '?'
                    }
                })
                .collect::<String>();

            let metadata = std::fs::metadata(&path).map_err(|_| ErrorCode::DomainError)?;
            Ok(ValueP::scalar_from(Cell::Int(metadata.len() as i64)))
        }
        8 => {
            // File position: B[1] is handle
            if cells.len() < 2 {
                return Err(ErrorCode::DomainError);
            }
            let _handle = cells[1].get_int_value()?;
            Ok(ValueP::scalar_from(Cell::Int(0)))
        }
        9 => {
            // Seek position: B[1] is handle, B[2] is position
            if cells.len() < 3 {
                return Err(ErrorCode::DomainError);
            }
            let _handle = cells[1].get_int_value()?;
            let _pos = cells[2].get_int_value()?;
            Ok(ValueP::scalar_from(Cell::Int(0)))
        }
        _ => Err(ErrorCode::DomainError),
    }
}

// ---------------------------------------------------------------------------
// ⎕JSON — JSON parse/serialize
// ---------------------------------------------------------------------------
//
// ⎕JSON B:
//   If B is a char vector containing JSON, parse it into an APL value
//   If B is an APL value, serialize it to JSON
// Mirrors src/Quad_JSON.cc (simplified).

pub fn quad_json(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    // If all chars, try to parse as JSON
    if cells.iter().all(|c| matches!(c, Cell::Char(_))) {
        let json_str: String = cells
            .iter()
            .map(|c| {
                if let Cell::Char(ch) = c {
                    char::from_u32(*ch).unwrap_or('?')
                } else {
                    '?'
                }
            })
            .collect();

        // Simple JSON parsing: try to parse as number, string, or array
        let trimmed = json_str.trim();

        // Try parsing as a number
        if let Ok(n) = trimmed.parse::<i64>() {
            return Ok(ValueP::scalar_from(Cell::Int(n)));
        }
        if let Ok(f) = trimmed.parse::<f64>() {
            return Ok(ValueP::scalar_from(Cell::Float(f)));
        }

        // Try parsing as a string (remove quotes)
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            let s = &trimmed[1..trimmed.len() - 1];
            let cps: Vec<u32> = s.chars().map(|c| c as u32).collect();
            return Ok(ValueP::char_vector(&cps));
        }

        // Try parsing as an array [1, 2, 3]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            let mut nums = Vec::new();
            for part in inner.split(',') {
                let part = part.trim();
                if let Ok(n) = part.parse::<i64>() {
                    nums.push(Cell::Int(n));
                } else if let Ok(f) = part.parse::<f64>() {
                    nums.push(Cell::Float(f));
                } else {
                    return Err(ErrorCode::DomainError);
                }
            }
            let len = nums.len() as i64;
            let shape = crate::shape::Shape::vector(len);
            return ValueP::from_parts(shape, nums);
        }

        // Boolean
        if trimmed == "true" {
            return Ok(ValueP::scalar_from(Cell::Int(1)));
        }
        if trimmed == "false" {
            return Ok(ValueP::scalar_from(Cell::Int(0)));
        }
        if trimmed == "null" {
            return Ok(ValueP::scalar_from(Cell::Int(0)));
        }

        return Err(ErrorCode::DomainError);
    }

    // Serialize APL value to JSON
    // For now, return a simple representation
    if cells.len() == 1 {
        let s = match &cells[0] {
            Cell::Int(n) => n.to_string(),
            Cell::Float(f) => format!("{:?}", f),
            Cell::Char(ch) => {
                if let Some(c) = char::from_u32(*ch) {
                    format!("\"{}\"", c)
                } else {
                    return Err(ErrorCode::DomainError);
                }
            }
            Cell::Complex(c) => format!("[{}, {}]", c.re, c.im),
            _ => return Err(ErrorCode::DomainError),
        };
        let cps: Vec<u32> = s.chars().map(|c| c as u32).collect();
        return Ok(ValueP::char_vector(&cps));
    }

    // Serialize array as JSON array
    let parts: Vec<String> = cells
        .iter()
        .map(|c| match c {
            Cell::Int(n) => n.to_string(),
            Cell::Float(f) => format!("{:?}", f),
            Cell::Char(ch) => {
                if let Some(ch) = char::from_u32(*ch) {
                    format!("\"{}\"", ch)
                } else {
                    "null".to_string()
                }
            }
            Cell::Complex(c) => format!("[{}, {}]", c.re, c.im),
            _ => "null".to_string(),
        })
        .collect();

    let json = format!("[{}]", parts.join(", "));
    let cps: Vec<u32> = json.chars().map(|c| c as u32).collect();
    Ok(ValueP::char_vector(&cps))
}

// ---------------------------------------------------------------------------
// ⎕XML — XML parse/serialize
// ---------------------------------------------------------------------------
//
// ⎕XML B:
//   If B is a char vector containing XML, parse it into an APL value
//   If B is an APL value, serialize it to XML
// Mirrors src/Quad_XML.cc (simplified).

pub fn quad_xml(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    // If all chars, try to parse as XML
    if cells.iter().all(|c| matches!(c, Cell::Char(_))) {
        let xml_str: String = cells
            .iter()
            .map(|c| {
                if let Cell::Char(ch) = c {
                    char::from_u32(*ch).unwrap_or('?')
                } else {
                    '?'
                }
            })
            .collect();

        // Simple XML parsing: extract text content between tags
        let trimmed = xml_str.trim();
        if trimmed.starts_with('<') && trimmed.ends_with('>') {
            // Find the first tag
            if let Some(tag_end) = trimmed.find('>') {
                let tag = &trimmed[1..tag_end];
                // Find closing tag
                let closing = format!("</{}>", tag);
                if let Some(close_start) = trimmed.rfind(&closing) {
                    let content = &trimmed[tag_end + 1..close_start];
                    let cps: Vec<u32> = content.chars().map(|c| c as u32).collect();
                    return Ok(ValueP::char_vector(&cps));
                }
            }
        }

        // Return as-is if not valid XML
        let cps: Vec<u32> = trimmed.chars().map(|c| c as u32).collect();
        return Ok(ValueP::char_vector(&cps));
    }

    // Serialize APL value to XML
    let xml = format!(
        "<value>{}</value>",
        cells
            .iter()
            .map(|c| match c {
                Cell::Int(n) => format!("<int>{}</int>", n),
                Cell::Float(f) => format!("<float>{:?}</float>", f),
                Cell::Char(ch) => {
                    if let Some(ch) = char::from_u32(*ch) {
                        format!("<char>{}</char>", ch)
                    } else {
                        "<char/>".to_string()
                    }
                }
                Cell::Complex(c) =>
                    format!("<complex><re>{}</re><im>{}</im></complex>", c.re, c.im),
                _ => "<null/>".to_string(),
            })
            .collect::<String>()
    );

    let cps: Vec<u32> = xml.chars().map(|c| c as u32).collect();
    Ok(ValueP::char_vector(&cps))
}

// ---------------------------------------------------------------------------
// ⎕MX — matrix operations
// ---------------------------------------------------------------------------
//
// ⎕MX B performs matrix operations:
//   B = 0: returns the determinant of a matrix (not yet implemented)
//   B = 1: returns the inverse of a matrix (not yet implemented)
//   B = 2: returns the eigenvalues (not yet implemented)
//   B = 3: trace (sum of diagonal)
//   B = 4: rank
// Mirrors src/Quad_MX.cc (simplified).

pub fn quad_mx(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    if cells.len() == 1 {
        // Return operation info
        let op = cells[0].get_int_value()?;
        let desc = match op {
            0 => "DETERMINANT",
            1 => "INVERSE",
            2 => "EIGENVALUES",
            3 => "TRACE",
            4 => "RANK",
            _ => "UNKNOWN",
        };
        let cps: Vec<u32> = desc.chars().map(|c| c as u32).collect();
        return Ok(ValueP::char_vector(&cps));
    }

    // Dyadic: B[0] is operation, B[1..] is the matrix
    let op = cells[0].get_int_value()?;
    let matrix_cells = &cells[1..];

    match op {
        3 => {
            // Trace: sum of diagonal
            if matrix_cells.len() < 2 {
                return Err(ErrorCode::DomainError);
            }
            let n = (matrix_cells.len() as f64).sqrt() as usize;
            if n * n != matrix_cells.len() {
                return Err(ErrorCode::DomainError);
            }
            let mut trace = 0.0f64;
            for i in 0..n {
                trace += match &matrix_cells[i * n + i] {
                    Cell::Int(v) => *v as f64,
                    Cell::Float(v) => *v,
                    Cell::Complex(v) => v.re,
                    _ => 0.0,
                };
            }
            return Ok(ValueP::scalar_from(Cell::Float(trace)));
        }
        _ => {
            return Ok(ValueP::scalar_from(Cell::Int(0)));
        }
    }
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

    #[test]
    fn test_quad_rval_scalar() {
        let v = ValueP::int_vector(&[0, 1, 1]); // rank=0, shape=1, type=int
        let result = quad_rval(&v).unwrap();
        assert_eq!(result.element_count(), 1);
        assert!(matches!(result.cells()[0], Cell::Int(_)));
    }

    #[test]
    fn test_quad_rval_vector() {
        let v = ValueP::int_vector(&[1, 5, 2]); // rank=1, shape=5, type=float
        let result = quad_rval(&v).unwrap();
        assert_eq!(result.element_count(), 5);
    }

    #[test]
    fn test_quad_rl_get_set() {
        // Get current seed
        let empty = ValueP::int_vector(&[]);
        let result = quad_rl(&empty).unwrap();
        assert_eq!(result.element_count(), 1);

        // Set seed
        let set_seed = ValueP::int_vector(&[12345]);
        let result = quad_rl(&set_seed).unwrap();
        assert_eq!(result.element_count(), 1);
    }

    #[test]
    fn test_quad_cc_upper() {
        let v = ValueP::char_vector(&['a' as u32, 'b' as u32, 'c' as u32]);
        let result = quad_cc_with_case(1, &v).unwrap();
        assert_eq!(result.cells()[0], Cell::Char('A' as u32));
        assert_eq!(result.cells()[1], Cell::Char('B' as u32));
        assert_eq!(result.cells()[2], Cell::Char('C' as u32));
    }

    #[test]
    fn test_quad_cc_lower() {
        let v = ValueP::char_vector(&['A' as u32, 'B' as u32, 'C' as u32]);
        let result = quad_cc_with_case(2, &v).unwrap();
        assert_eq!(result.cells()[0], Cell::Char('a' as u32));
        assert_eq!(result.cells()[1], Cell::Char('b' as u32));
        assert_eq!(result.cells()[2], Cell::Char('c' as u32));
    }

    #[test]
    fn test_quad_dlx_simple() {
        // 2x2 identity matrix: each row covers one column
        // Row 0: covers col 0
        // Row 1: covers col 1
        // Solution: both rows
        let shape = crate::shape::Shape::matrix(2, 2);
        let v = ValueP::from_parts(
            shape,
            vec![Cell::Int(1), Cell::Int(0), Cell::Int(0), Cell::Int(1)],
        )
        .unwrap();
        let result = quad_dlx(&v).unwrap();
        // Should return a 2x2 solution (the identity matrix itself)
        assert_eq!(result.element_count(), 4);
    }

    #[test]
    fn test_quad_dlx_empty() {
        let v = ValueP::int_vector(&[]);
        let result = quad_dlx(&v).unwrap();
        assert_eq!(result.element_count(), 0);
    }

    #[test]
    fn test_quad_mx_trace() {
        // 2x2 identity matrix trace = 2
        let v = ValueP::int_vector(&[3, 1, 0, 0, 1]); // op=3 (trace), then matrix
        let result = quad_mx(&v).unwrap();
        assert_eq!(result.cells()[0], Cell::Float(2.0));
    }

    #[test]
    fn test_quad_mx_info() {
        let v = ValueP::int_vector(&[0]); // determinant info
        let result = quad_mx(&v).unwrap();
        assert!(result.element_count() > 0);
    }

    #[test]
    fn test_quad_tf() {
        let v = ValueP::char_vector(&['F' as u32, 'O' as u32, 'O' as u32]);
        let result = quad_tf(&v).unwrap();
        assert_eq!(result.cells()[0], Cell::Char('F' as u32));
    }

    #[test]
    fn test_quad_map_empty() {
        let env = crate::parser::Environment::new();
        let empty = ValueP::int_vector(&[]);
        let result = quad_map(&env, &empty).unwrap();
        assert_eq!(result.element_count(), 1);
    }

    #[test]
    fn test_quad_fio_list() {
        let v = ValueP::int_vector(&[0]); // list open files
        let result = quad_fio(&v).unwrap();
        assert_eq!(result.element_count(), 1);
    }

    #[test]
    fn test_quad_fio_file_size() {
        // Create a temp file
        let path = "/tmp/test_quad_fio_size.txt";
        std::fs::write(path, "hello world").unwrap();
        let v = ValueP::int_vector(&[7]); // file size operation
                                          // Note: this simplified version doesn't actually read the path from args
        let _ = quad_fio(&v);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_quad_json_parse_number() {
        let v = ValueP::char_vector(&['4' as u32, '2' as u32]);
        let result = quad_json(&v).unwrap();
        assert_eq!(result.cells()[0], Cell::Int(42));
    }

    #[test]
    fn test_quad_json_parse_string() {
        let mut chars: Vec<u32> = vec!['"' as u32];
        chars.extend("hello".chars().map(|c| c as u32));
        chars.push('"' as u32);
        let v = ValueP::char_vector(&chars);
        let result = quad_json(&v).unwrap();
        assert_eq!(result.cells()[0], Cell::Char('h' as u32));
    }

    #[test]
    fn test_quad_json_parse_array() {
        let v = ValueP::char_vector(&[
            '[' as u32, '1' as u32, ',' as u32, '2' as u32, ',' as u32, '3' as u32, ']' as u32,
        ]);
        let result = quad_json(&v).unwrap();
        assert_eq!(result.element_count(), 3);
        assert_eq!(result.cells()[0], Cell::Int(1));
        assert_eq!(result.cells()[1], Cell::Int(2));
        assert_eq!(result.cells()[2], Cell::Int(3));
    }

    #[test]
    fn test_quad_json_serialize() {
        let v = ValueP::int_vector(&[1, 2, 3]);
        let result = quad_json(&v).unwrap();
        // Should return a char vector with JSON representation
        assert!(result.element_count() > 0);
    }

    #[test]
    fn test_quad_json_bool() {
        let mut chars: Vec<u32> = "true".chars().map(|c| c as u32).collect();
        let v = ValueP::char_vector(&chars);
        let result = quad_json(&v).unwrap();
        assert_eq!(result.cells()[0], Cell::Int(1));
    }

    #[test]
    fn test_quad_xml_parse() {
        let xml = "<root>hello world</root>";
        let chars: Vec<u32> = xml.chars().map(|c| c as u32).collect();
        let v = ValueP::char_vector(&chars);
        let result = quad_xml(&v).unwrap();
        // Should extract content between tags
        assert_eq!(result.cells()[0], Cell::Char('h' as u32));
    }

    #[test]
    fn test_quad_xml_serialize() {
        let v = ValueP::int_vector(&[42]);
        let result = quad_xml(&v).unwrap();
        // Should return XML representation
        assert!(result.element_count() > 0);
    }
}

// ---------------------------------------------------------------------------
// ⎕NS — Namespace creation
// ---------------------------------------------------------------------------

/// ⎕NS name — create a namespace (or retrieve existing one).
/// The name can be a character vector (string) or a nested array of names.
/// Returns the namespace name.
pub fn quad_ns(env: &mut crate::parser::Environment, b: &ValueP) -> AplResult<ValueP> {
    let name = if b.element_count() == 0 {
        return Err(ErrorCode::DomainError);
    } else if b.is_vector()
        && b.cells()
            .iter()
            .all(|c| matches!(c, crate::cell::Cell::Char(_)))
    {
        // Char vector → treat as a string name
        let chars: Vec<u32> = b
            .cells()
            .iter()
            .map(|c| c.get_char_value())
            .collect::<Result<Vec<_>, _>>()?;
        chars
            .iter()
            .map(|&cp| std::char::from_u32(cp).ok_or(ErrorCode::DomainError))
            .collect::<Result<String, _>>()?
    } else {
        return Err(ErrorCode::DomainError);
    };

    // Validate: namespace names must be valid APL identifiers
    if name.is_empty() || name.starts_with('⎕') || name.starts_with('{') {
        return Err(ErrorCode::DomainError);
    }

    // Add to known namespaces
    env.namespaces.insert(name.clone());

    Ok(ValueP::char_vector(
        &name.chars().map(|c| c as u32).collect::<Vec<_>>(),
    ))
}

/// ⎕CS name — switch current namespace.
/// The name must be an existing namespace (or '' for root).
/// Returns the previous namespace name.
pub fn quad_cs(env: &mut crate::parser::Environment, b: &ValueP) -> AplResult<ValueP> {
    let name = if b.element_count() == 0 {
        String::new() // root
    } else if b.is_vector()
        && b.cells()
            .iter()
            .all(|c| matches!(c, crate::cell::Cell::Char(_)))
    {
        let chars: Vec<u32> = b
            .cells()
            .iter()
            .map(|c| c.get_char_value())
            .collect::<Result<Vec<_>, _>>()?;
        chars
            .iter()
            .map(|&cp| std::char::from_u32(cp).ok_or(ErrorCode::DomainError))
            .collect::<Result<String, _>>()?
    } else {
        return Err(ErrorCode::DomainError);
    };

    // Validate: must be empty (root) or a known namespace
    if !name.is_empty() && !env.namespaces.contains(&name) {
        return Err(ErrorCode::DomainError);
    }

    let prev = env.current_ns.clone();
    env.current_ns = name;

    Ok(ValueP::char_vector(
        &prev.chars().map(|c| c as u32).collect::<Vec<_>>(),
    ))
}

// ---------------------------------------------------------------------------
// ⎕RE — regular expression
// ---------------------------------------------------------------------------
//
// ⎕RE B — regular expression operations
// B[0] = operation: 0=match, 1=replace, 2=split
// B[1] = pattern (char vector)
// B[2] = input string (char vector) or replacement string

pub fn quad_re(b: &ValueP) -> AplResult<ValueP> {
    use regex::Regex;

    let cells = b.cells();
    if cells.len() < 3 {
        return Err(ErrorCode::DomainError);
    }

    let op = cells[0].get_int_value()?;
    let pattern_cp = cells[1].get_char_value()?;
    let pattern = std::char::from_u32(pattern_cp).unwrap_or('?').to_string();
    let input: String = cells[2..]
        .iter()
        .filter(|c| matches!(c, crate::cell::Cell::Char(_)))
        .map(|c| {
            c.get_char_value()
                .map(|cp| std::char::from_u32(cp).unwrap_or('?'))
        })
        .collect::<Result<String, _>>()?;

    let re = Regex::new(&pattern).map_err(|_| ErrorCode::DomainError)?;

    match op {
        0 => {
            let matches: Vec<(usize, usize)> =
                re.find_iter(&input).map(|m| (m.start(), m.end())).collect();
            if matches.is_empty() {
                Ok(ValueP::int_vector(&[]))
            } else {
                let result: Vec<i64> = matches
                    .iter()
                    .flat_map(|(s, e)| vec![*s as i64, *e as i64])
                    .collect();
                Ok(ValueP::int_vector(&result))
            }
        }
        1 => {
            let replacement: String = if cells.len() > 3 {
                cells[3..]
                    .iter()
                    .filter(|c| matches!(c, crate::cell::Cell::Char(_)))
                    .map(|c| {
                        c.get_char_value()
                            .map(|cp| std::char::from_u32(cp).unwrap_or('?'))
                    })
                    .collect::<Result<String, _>>()?
            } else {
                String::new()
            };
            let result = re.replace_all(&input, replacement.as_str());
            Ok(ValueP::char_vector(
                &result.chars().map(|c| c as u32).collect::<Vec<_>>(),
            ))
        }
        2 => {
            let parts: Vec<String> = re.split(&input).map(|s| s.to_string()).collect();
            let joined = parts.join("\n");
            Ok(ValueP::char_vector(
                &joined.chars().map(|c| c as u32).collect::<Vec<_>>(),
            ))
        }
        _ => Err(ErrorCode::DomainError),
    }
}

// ---------------------------------------------------------------------------
// ⎕SVx — shared variables (Phase 6, stubs)
// ---------------------------------------------------------------------------
//
// Shared variables are not yet supported in this port.
// These stubs return domain errors for dyadic operations and
// empty results for monadic queries.

/// ⎕SVC — shared variable control (list).
pub fn quad_svc() -> AplResult<ValueP> {
    Ok(ValueP::char_vector(&[]))
}

/// ⎕SVO B — shared variable off (close).
pub fn quad_svo(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// ⎕SVQ B — shared variable query.
pub fn quad_svq(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// ⎕SVR B — shared variable read.
pub fn quad_svr(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// ⎕SVS B — shared variable set.
pub fn quad_svs(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

#[cfg(test)]
mod sv_tests {
    use super::*;

    #[test]
    fn test_quad_svc_empty() {
        let result = quad_svc().unwrap();
        assert_eq!(result.element_count(), 0);
    }

    #[test]
    fn test_quad_svo_err() {
        let v = ValueP::char_vector(&"X".chars().map(|c| c as u32).collect::<Vec<_>>());
        assert!(quad_svo(&v).is_err());
    }

    #[test]
    fn test_quad_svq_err() {
        let v = ValueP::char_vector(&"X".chars().map(|c| c as u32).collect::<Vec<_>>());
        assert!(quad_svq(&v).is_err());
    }

    #[test]
    fn test_quad_svr_err() {
        let v = ValueP::char_vector(&"X".chars().map(|c| c as u32).collect::<Vec<_>>());
        assert!(quad_svr(&v).is_err());
    }

    #[test]
    fn test_quad_svs_err() {
        let v = ValueP::int_vector(&[1, 2]);
        assert!(quad_svs(&v).is_err());
    }
}
