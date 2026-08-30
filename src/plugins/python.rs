use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

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
            description: "Python pipe (⎕PYTHON)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕PYTHON".into(),
            ValueP::char_vector(
                &"python v0.1.0 (stub)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        reg.sysvars.insert(
            "⎕PYTHON.RUNNING".into(),
            ValueP::scalar_from(Cell::Int(0)), // 0 = not running
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_plugin_info() {
        let plugin = PythonPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "python");
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
        assert!(sysvars.contains_key("⎕PYTHON.RUNNING"));
    }
}
