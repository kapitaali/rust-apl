use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

pub struct GtkPlugin;

impl GtkPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for GtkPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "gtk".into(),
            version: "0.1.0".into(),
            description: "GTK GUI (⎕GTK)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕GTK".into(),
            ValueP::char_vector(
                &"gtk v0.1.0 (stub)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        reg.sysvars.insert(
            "⎕GTK.RUNNING".into(),
            ValueP::scalar_from(Cell::Int(0)),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gtk_plugin_info() {
        let plugin = GtkPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "gtk");
    }

    #[test]
    fn test_gtk_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = GtkPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        plugin.register(&mut reg).unwrap();

        assert!(sysvars.contains_key("⎕GTK"));
        assert!(sysvars.contains_key("⎕GTK.RUNNING"));
    }
}
