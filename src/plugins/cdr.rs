use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

pub struct CdrPlugin;

impl CdrPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for CdrPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "cdr".into(),
            version: "0.1.0".into(),
            description: "CDR binary interchange (⎕CDR)".into(),
        }
    }

    fn register(&self, _reg: &mut PluginRegistrar) -> AplResult<()> {
        Ok(())
    }
}

