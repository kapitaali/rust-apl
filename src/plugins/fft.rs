use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

pub struct FftPlugin;

impl FftPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for FftPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "fft".into(),
            version: "0.1.0".into(),
            description: "FFT (⎕FFT)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕FFT".into(),
            ValueP::char_vector(&"fft v0.1.0".chars().map(|c| c as u32).collect::<Vec<_>>()),
        );
        Ok(())
    }
}

