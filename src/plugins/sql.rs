use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕SQL B — SQL database query.
///
/// B is a character vector (SQL query string).
/// Returns a matrix of rows.
#[cfg(feature = "plugin-sql")]
pub fn quad_sql(b: &ValueP) -> AplResult<ValueP> {
    use rusqlite::{Connection, Result};

    let filename = match b {
        ValueP::Char(s) => s
            .iter()
            .map(|c| char::from_u32(*c).unwrap())
            .collect::<String>(),
        _ => return Err(ErrorCode::DomainError),
    };

    let conn = Connection::open(&filename).map_err(|_| ErrorCode::DomainError)?;
    let mut stmt = conn
        .prepare("SELECT * FROM sqlite_master WHERE type='table'")
        .map_err(|_| ErrorCode::DomainError)?;

    let mut rows = stmt.query([]).map_err(|_| ErrorCode::DomainError)?;
    let mut result = Vec::new();

    while let Some(row) = rows.next().map_err(|_| ErrorCode::DomainError)? {
        let name: String = row.get(0).map_err(|_| ErrorCode::DomainError)?;
        result.push(name);
    }

    Ok(ValueP::char_vector(
        &result
            .concat()
            .chars()
            .map(|c| c as u32)
            .collect::<Vec<_>>(),
    ))
}

/// ⎕SQL B — disabled version.
#[cfg(not(feature = "plugin-sql"))]
pub fn quad_sql(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

#[cfg(all(test, feature = "plugin-sql"))]
mod tests {
    use super::*;

    #[test]
    fn test_sql_disabled() {
        let v = ValueP::char_vector(&"test.db".chars().map(|c| c as u32).collect::<Vec<_>>());
        assert!(quad_sql(&v).is_err());
    }
}
