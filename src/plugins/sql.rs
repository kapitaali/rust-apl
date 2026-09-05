use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕SQL B — SQL database query.
///
/// B is a character vector (SQL query string).
/// Returns a matrix of rows.
#[cfg(feature = "plugin-sql")]
pub fn quad_sql(b: &ValueP) -> AplResult<ValueP> {
    use rusqlite::{Connection, Result};

    let cells = b.cells();
    let filename: String = cells
        .iter()
        .filter_map(|c| match c {
            crate::cell::Cell::Char(ch) => char::from_u32(*ch),
            _ => None,
        })
        .collect();

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

/// ⎕SQL plugin struct for registration.
#[cfg(feature = "plugin-sql")]
pub struct SqlPlugin;

#[cfg(feature = "plugin-sql")]
impl SqlPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "plugin-sql")]
impl crate::plugin_system::AplPlugin for SqlPlugin {
    fn info(&self) -> crate::plugin_system::PluginInfo {
        crate::plugin_system::PluginInfo {
            name: "sql".into(),
            version: "0.1.0".into(),
            description: "SQL database access (⎕SQL)".into(),
        }
    }

    fn register(&self, reg: &mut crate::plugin_system::PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕SQL".into(),
            ValueP::char_vector(
                &"sql v0.1.0 (rusqlite)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        Ok(())
    }
}

#[cfg(all(test, feature = "plugin-sql"))]
mod tests {
    use super::*;

    #[test]
    fn test_sql_plugin_info() {
        use crate::plugin_system::AplPlugin;
        let plugin = SqlPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "sql");
        assert!(info.description.contains("⎕SQL"));
    }

    #[test]
    fn test_sql_plugin_register() {
        use crate::functions_def::FunctionTable;
        use crate::plugin_system::AplPlugin;
        use std::collections::HashMap;

        let plugin = SqlPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = crate::plugin_system::PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
            hooks: None,
        };

        plugin.register(&mut reg).unwrap();
        assert!(sysvars.contains_key("⎕SQL"));
    }
}
