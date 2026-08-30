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
            ValueP::char_vector(
                &"fft v0.1.0 (rustfft)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        reg.sysvars.insert(
            "⎕FFT.BACKEND".into(),
            ValueP::scalar_from(Cell::Char('r' as u32)), // 'r'ustfft
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_plugin_info() {
        let plugin = FftPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "fft");
        assert!(info.description.contains("⎕FFT"));
    }

    #[test]
    fn test_fft_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = FftPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        plugin.register(&mut reg).unwrap();

        assert!(sysvars.contains_key("⎕FFT"));
        assert!(sysvars.contains_key("⎕FFT.BACKEND"));
    }
}
