use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕GTK B — GTK GUI (stub — full GTK4 integration pending).
pub fn quad_gtk(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// GTK plugin — registers ⎕GTK-related system variables.
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
            description: "GTK4 GUI (⎕GTK) — stub".into(),
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
        assert!(info.description.contains("⎕GTK"));
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
    }
}
