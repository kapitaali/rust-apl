//! PNG image plugin — Phase 6.1.
//!
//! Provides ⎕PNG for reading/writing PNG images.

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
            ValueP::char_vector(&"png v0.1.0".chars().map(|c| c as u32).collect::<Vec<_>>()),
        );
        reg.sysvars.insert(
            "⎕PNG.FORMAT".into(),
            ValueP::scalar_from(Cell::Char('a' as u32)), // 'a'uto
        );
        Ok(())
    }
}
