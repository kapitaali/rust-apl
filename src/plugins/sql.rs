//! SQL database plugin — Phase 6.1.
//!
//! Provides ⎕SQL for database access.

use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::AplResult;
use crate::value::ValueP;

pub struct SqlPlugin;

impl SqlPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for SqlPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "sql".into(),
            version: "0.1.0".into(),
            description: "SQL database access (⎕SQL)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕SQL".into(),
            ValueP::char_vector(&"sql v0.1.0".chars().map(|c| c as u32).collect::<Vec<_>>()),
        );
        reg.sysvars.insert(
            "⎕SQL.BACKEND".into(),
            ValueP::scalar_from(Cell::Char('s' as u32)), // 's'qlite
        );
        Ok(())
    }
}
