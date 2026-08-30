use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

pub struct SqlPlugin;

impl SqlPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for SqlPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "sql".into(),
            version: "0.1.0".into(),
            description: "SQL database access (⎕SQL)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕SQL".into(),
            ValueP::char_vector(
                &"sql v0.1.0 (stub)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        reg.sysvars.insert(
            "⎕SQL.BACKEND".into(),
            ValueP::scalar_from(Cell::Char('s' as u32)), // 's'qlite
        );
        reg.sysvars.insert(
            "⎕SQL.CONNECTED".into(),
            ValueP::scalar_from(Cell::Int(0)), // 0 = not connected
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_plugin_info() {
        let plugin = SqlPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "sql");
    }

    #[test]
    fn test_sql_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = SqlPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        plugin.register(&mut reg).unwrap();

        assert!(sysvars.contains_key("⎕SQL"));
        assert!(sysvars.contains_key("⎕SQL.BACKEND"));
        assert!(sysvars.contains_key("⎕SQL.CONNECTED"));
    }
}
