use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PYTHON B — Python pipe (shell-out approach).
///
/// B is a character vector (Python code).
/// The code is executed via `python3 -c {code}`.
/// Stdout from Python is parsed and converted to a ValueP.
pub fn quad_python(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    // Build the Python code string
    let code = cells_to_string(cells)?;

    // Execute Python via shell
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(&code)
        .output()
        .map_err(|e| {
            eprintln!("⎕PYTHON: failed to spawn python3: {}", e);
            ErrorCode::DomainError
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("⎕PYTHON: Python error: {}", stderr);
        return Err(ErrorCode::DomainError);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();

    if stdout.is_empty() {
        // Return zilde
        return Ok(ValueP::int_vector(&[]));
    }

    // Try to parse as JSON first
    if stdout.starts_with('[') {
        // Try JSON array parsing
        match parse_json_array(stdout) {
            Ok(v) => return Ok(v),
            Err(_) => {}
        }
    }

    // Try to parse as a single number
    if let Ok(n) = stdout.parse::<i64>() {
        return Ok(ValueP::scalar_from(crate::cell::Cell::Int(n)));
    }
    if let Ok(f) = stdout.parse::<f64>() {
        return Ok(ValueP::scalar_from(crate::cell::Cell::Float(f)));
    }

    // Otherwise return as character vector
    Ok(ValueP::char_vector(
        &stdout.chars().map(|c| c as u32).collect::<Vec<_>>(),
    ))
}

/// Python plugin — registers ⎕PYTHON-related system variables.
pub struct PythonPlugin;

impl PythonPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for PythonPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "python".into(),
            version: "0.1.0".into(),
            description: "Python interop (⎕PYTHON)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕PYTHON".into(),
            ValueP::char_vector(
                &"python v0.1.0 (shell-out)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        reg.sysvars.insert(
            "⎕PYTHON.BACKEND".into(),
            ValueP::scalar_from(Cell::Char('s' as u32)), // 's'hell-out
        );
        Ok(())
    }
}

fn cells_to_string(cells: &[crate::cell::Cell]) -> Result<String, ErrorCode> {
    let mut s = String::new();
    for (i, c) in cells.iter().enumerate() {
        match c {
            crate::cell::Cell::Char(ch) => {
                if let Some(ch) = char::from_u32(*ch) {
                    s.push(ch);
                }
            }
            crate::cell::Cell::Int(n) => {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(&n.to_string());
            }
            crate::cell::Cell::Float(f) => {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(&f.to_string());
            }
            _ => return Err(ErrorCode::DomainError),
        }
    }
    Ok(s)
}

fn parse_json_array(s: &str) -> Result<ValueP, ErrorCode> {
    // Simple JSON array parser (no external deps)
    // Handles: [1, 2, 3], [1.5, 2.5], [[1,2],[3,4]]
    let trimmed = s.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(ErrorCode::DomainError);
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    let inner = inner.trim();

    if inner.is_empty() {
        return Ok(ValueP::int_vector(&[]));
    }

    // Check if nested (2D)
    if inner.contains('[') {
        // Parse as nested array
        let mut rows = Vec::new();
        let mut depth = 0;
        let mut start = 0;
        for (i, c) in inner.char_indices() {
            match c {
                '[' => {
                    if depth == 0 {
                        start = i;
                    }
                    depth += 1;
                }
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        let row_str = &inner[start..=i];
                        rows.push(parse_json_array(row_str)?);
                    }
                }
                _ => {}
            }
        }
        // Nest each row
        let mut all_cells = Vec::new();
        for row in &rows {
            all_cells.push(crate::cell::Cell::Pointer(crate::cell::PointerCellData {
                value: row.clone_inner_arc(),
            }));
        }
        return Ok(
            ValueP::from_parts(crate::shape::Shape::vector(rows.len() as i64), all_cells)
                .map_err(|_| ErrorCode::DomainError)?,
        );
    }

    // Parse flat array
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    // Try to parse as integers first
    let mut ints = Vec::new();
    let mut all_ints = true;
    for part in &parts {
        if let Ok(n) = part.parse::<i64>() {
            ints.push(crate::cell::Cell::Int(n));
        } else {
            all_ints = false;
            break;
        }
    }

    if all_ints {
        return Ok(
            ValueP::from_parts(crate::shape::Shape::vector(parts.len() as i64), ints)
                .map_err(|_| ErrorCode::DomainError)?,
        );
    }

    // Try as floats
    let mut floats = Vec::new();
    for part in &parts {
        if let Ok(f) = part.parse::<f64>() {
            floats.push(crate::cell::Cell::Float(f));
        } else {
            return Err(ErrorCode::DomainError);
        }
    }

    Ok(
        ValueP::from_parts(crate::shape::Shape::vector(parts.len() as i64), floats)
            .map_err(|_| ErrorCode::DomainError)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quad_python_simple_eval() {
        let code =
            ValueP::char_vector(&"print(6 * 7)".chars().map(|c| c as u32).collect::<Vec<_>>());
        let result = quad_python(&code);
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v.first_cell(), Some(&crate::cell::Cell::Int(42)));
    }

    #[test]
    fn test_quad_python_json_array() {
        let code = ValueP::char_vector(
            &"import json; print(json.dumps([1, 2, 3]))"
                .chars()
                .map(|c| c as u32)
                .collect::<Vec<_>>(),
        );
        let result = quad_python(&code);
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v.element_count(), 3);
        assert_eq!(v.cells()[0], crate::cell::Cell::Int(1));
        assert_eq!(v.cells()[1], crate::cell::Cell::Int(2));
        assert_eq!(v.cells()[2], crate::cell::Cell::Int(3));
    }

    #[test]
    fn test_quad_python_string_output() {
        let code = ValueP::char_vector(
            &"print('hello')"
                .chars()
                .map(|c| c as u32)
                .collect::<Vec<_>>(),
        );
        let result = quad_python(&code);
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v.cells()[0], crate::cell::Cell::Char('h' as u32));
        assert_eq!(v.cells()[4], crate::cell::Cell::Char('o' as u32));
    }

    #[test]
    fn test_quad_python_empty_code() {
        let code = ValueP::char_vector(&[]);
        assert!(quad_python(&code).is_err());
    }

    #[test]
    fn test_python_plugin_info() {
        let plugin = PythonPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "python");
        assert!(info.description.contains("⎕PYTHON"));
    }

    #[test]
    fn test_python_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = PythonPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        plugin.register(&mut reg).unwrap();

        assert!(sysvars.contains_key("⎕PYTHON"));
        assert!(sysvars.contains_key("⎕PYTHON.BACKEND"));
    }
}
