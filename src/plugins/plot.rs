//! Plot plugin — Phase 6.3.
//!
//! Provides ⎕PLOT for creating plots and charts via the plotters crate.

use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

pub struct PlotPlugin;

impl PlotPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for PlotPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "plot".into(),
            version: "0.1.0".into(),
            description: "Plotting support (⎕PLOT)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        // Register ⎕PLOT system variable with version info
        reg.sysvars.insert(
            "⎕PLOT".into(),
            ValueP::char_vector(
                &"plot v0.1.0 (plotters)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );

        // Register ⎕PLOT.BACKEND
        reg.sysvars.insert(
            "⎕PLOT.BACKEND".into(),
            ValueP::scalar_from(Cell::Char('p' as u32)), // 'p'lotters
        );

        // Register ⎕PLOT.WIDTH and ⎕PLOT.HEIGHT
        reg.sysvars.insert(
            "⎕PLOT.WIDTH".into(),
            ValueP::scalar_from(Cell::Int(800)),
        );
        reg.sysvars.insert(
            "⎕PLOT.HEIGHT".into(),
            ValueP::scalar_from(Cell::Int(600)),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plot_plugin_info() {
        let plugin = PlotPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "plot");
        assert!(info.description.contains("⎕PLOT"));
    }

    #[test]
    fn test_plot_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = PlotPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        plugin.register(&mut reg).unwrap();

        assert!(sysvars.contains_key("⎕PLOT"));
        assert!(sysvars.contains_key("⎕PLOT.BACKEND"));
        assert!(sysvars.contains_key("⎕PLOT.WIDTH"));
        assert!(sysvars.contains_key("⎕PLOT.HEIGHT"));
    }
}
