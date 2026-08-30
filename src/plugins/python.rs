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

    fn register(&self, _reg: &mut PluginRegistrar) -> AplResult<()> {
        Ok(())
    }
}

