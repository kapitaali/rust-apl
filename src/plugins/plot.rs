//! Plot plugin — Phase 6.3.
//!
//! Provides ⎕PLOT for creating plots and charts.

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
            ValueP::char_vector(&"plot v0.1.0".chars().map(|c| c as u32).collect::<Vec<_>>()),
        );

        // Register ⎕PLOT.BACKEND
        reg.sysvars.insert(
            "⎕PLOT.BACKEND".into(),
            ValueP::scalar_from(Cell::Char('p' as u32)), // 'p'lotters
        );

        Ok(())
    }
}
