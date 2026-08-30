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

    fn register(&self, _reg: &mut PluginRegistrar) -> AplResult<()> {
        Ok(())
    }
}

