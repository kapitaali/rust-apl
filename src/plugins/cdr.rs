use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕CDR B — CDR binary interchange format.
///
/// If B is a character vector, read a CDR file.
/// If B is an array, serialize to CDR binary format.
#[cfg(feature = "plugin-cdr")]
pub fn quad_cdr(b: &ValueP) -> AplResult<ValueP> {
    match b {
        ValueP::Char(s) => {
            let filename: String = s.iter().map(|c| char::from_u32(*c).unwrap()).collect();
            let data = std::fs::read(&filename).map_err(|_| ErrorCode::DomainError)?;
            Ok(ValueP::int_vector(
                &data.iter().map(|p| *p as i64).collect::<Vec<_>>(),
            ))
        }
        _ => {
            // Serialize: write cells to binary
            let mut buf = Vec::new();
            for cell in b.cells() {
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
    }
}

/// ⎕CDR B — disabled version.
#[cfg(not(feature = "plugin-cdr"))]
pub fn quad_cdr(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
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
            &"nonexistent.cdr".chars().map(|c| c as u32).collect::<Vec<_>>(),
        );
        assert!(quad_cdr(&v).is_err());
    }
}
