use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕CDR B — CDR binary interchange format.
///
/// If B is a character vector, read a CDR file.
/// If B is an array, serialize to CDR binary format.
#[cfg(feature = "plugin-cdr")]
pub fn quad_cdr(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();

    // Check if this is a char vector (read mode) — all cells are Char
    let all_chars = cells.iter().all(|c| matches!(c, Cell::Char(_)));
    if all_chars && !cells.is_empty() {
        let filename: String = cells
            .iter()
            .filter_map(|c| match c {
                Cell::Char(ch) => char::from_u32(*ch),
                _ => None,
            })
            .collect();
        let data = std::fs::read(&filename).map_err(|_| ErrorCode::DomainError)?;
        return Ok(ValueP::int_vector(
            &data.iter().map(|p| *p as i64).collect::<Vec<_>>(),
        ));
    }

    // Serialize: write cells to binary
    let mut buf = Vec::new();
    for cell in cells {
        match cell {
            Cell::Int(n) => {
                buf.push(0x01);
                buf.extend_from_slice(&n.to_le_bytes());
            }
            Cell::Float(f) => {
                buf.push(0x02);
                buf.extend_from_slice(&f.to_le_bytes());
            }
            Cell::Char(c) => {
                buf.push(0x03);
                buf.extend_from_slice(&(*c as u32).to_le_bytes());
            }
            Cell::Complex(c) => {
                buf.push(0x04);
                buf.extend_from_slice(&c.re.to_le_bytes());
                buf.extend_from_slice(&c.im.to_le_bytes());
            }
            _ => return Err(ErrorCode::DomainError),
        }
    }
    Ok(ValueP::int_vector(
        &buf.iter().map(|p| *p as i64).collect::<Vec<_>>(),
    ))
}

/// ⎕CDR B — disabled version.
#[cfg(not(feature = "plugin-cdr"))]
pub fn quad_cdr(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// ⎕CDR plugin struct for registration.
#[cfg(feature = "plugin-cdr")]
pub struct CdrPlugin;

#[cfg(feature = "plugin-cdr")]
impl CdrPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "plugin-cdr")]
impl AplPlugin for CdrPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "cdr".into(),
            version: "0.1.0".into(),
            description: "CDR binary interchange (⎕CDR)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕CDR".into(),
            ValueP::char_vector(&"cdr v0.1.0".chars().map(|c| c as u32).collect::<Vec<_>>()),
        );
        Ok(())
    }
}

#[cfg(all(test, feature = "plugin-cdr"))]
mod tests {
    use super::*;

    #[test]
    fn test_cdr_empty() {
        let v = ValueP::int_vector(&[]);
        assert!(quad_cdr(&v).is_ok());
    }

    #[test]
    fn test_cdr_scalar() {
        let v = ValueP::scalar_from(Cell::Int(42));
        assert!(quad_cdr(&v).is_ok());
    }

    #[test]
    fn test_cdr_read_missing_file() {
        let v = ValueP::char_vector(
            &"nonexistent.cdr"
                .chars()
                .map(|c| c as u32)
                .collect::<Vec<_>>(),
        );
        assert!(quad_cdr(&v).is_err());
    }
}
