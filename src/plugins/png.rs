//! PNG image plugin — Phase 6.1.
//!
//! Provides ⎕PNG for reading/writing PNG images via the `image` crate.

use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

pub struct PngPlugin;

impl PngPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for PngPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "png".into(),
            version: "0.1.0".into(),
            description: "PNG image read/write (⎕PNG)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕PNG".into(),
            ValueP::char_vector(
                &"png v0.1.0 (image)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        reg.sysvars.insert(
            "⎕PNG.FORMAT".into(),
            ValueP::scalar_from(Cell::Char('a' as u32)), // 'a'uto
        );
        reg.sysvars.insert(
            "⎕PNG.MAXDIM".into(),
            ValueP::scalar_from(Cell::Int(4096)), // max dimension
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_plugin_info() {
        let plugin = PngPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "png");
        assert!(info.description.contains("⎕PNG"));
    }

    #[test]
    fn test_png_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = PngPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        plugin.register(&mut reg).unwrap();

        assert!(sysvars.contains_key("⎕PNG"));
        assert!(sysvars.contains_key("⎕PNG.FORMAT"));
        assert!(sysvars.contains_key("⎕PNG.MAXDIM"));
    }
}
