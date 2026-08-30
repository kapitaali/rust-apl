//! Plugin system — Phase 6 extensible features.
//!
//! Manages static and dynamic plugin registration based on `config.toml`.
//! Static plugins are compiled into the binary; dynamic plugins are loaded
//! at runtime via ⎕LOADSO.

use std::collections::HashMap;
use std::sync::Arc;

use crate::functions_def::FunctionTable;
use crate::types::AplResult;
use crate::value::ValueP;

/// Plugin metadata.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

/// Registrar passed to plugins during registration.
pub struct PluginRegistrar<'a> {
    pub func_table: &'a mut FunctionTable,
    pub sysvars: &'a mut HashMap<String, ValueP>,
}

/// Core trait for all plugins.
pub trait AplPlugin: Send + Sync {
    fn info(&self) -> PluginInfo;
    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()>;
    fn init(&self) -> AplResult<()> {
        Ok(())
    }
    fn shutdown(&self) -> AplResult<()> {
        Ok(())
    }
}

/// Plugin registry.
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn AplPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        PluginRegistry {
            plugins: Vec::new(),
        }
    }

    pub fn register(&mut self, plugin: Arc<dyn AplPlugin>) {
        self.plugins.push(plugin);
    }

    pub fn plugins(&self) -> &[Arc<dyn AplPlugin>] {
        &self.plugins
    }

    pub fn find(&self, name: &str) -> Option<&Arc<dyn AplPlugin>> {
        self.plugins.iter().find(|p| p.info().name == name)
    }

    pub fn init_all(&self) -> AplResult<()> {
        for p in &self.plugins {
            p.init()?;
        }
        Ok(())
    }

    pub fn shutdown_all(&self) -> AplResult<()> {
        for p in &self.plugins {
            p.shutdown()?;
        }
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global plugin registry.
use std::sync::OnceLock;
static PLUGIN_REGISTRY: OnceLock<PluginRegistry> = OnceLock::new();

/// Initialize the global plugin registry.
pub fn init_plugin_registry() -> &'static PluginRegistry {
    PLUGIN_REGISTRY.get_or_init(PluginRegistry::new)
}

/// Get the global plugin registry.
pub fn plugin_registry() -> Option<&'static PluginRegistry> {
    PLUGIN_REGISTRY.get()
}

/// Initialize the plugin system with all static plugins.
/// Called once at interpreter startup.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;
    use crate::functions_def::FunctionTable;
    use std::collections::HashMap;

    struct TestPlugin;

    impl AplPlugin for TestPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                name: "test".into(),
                version: "0.0.1".into(),
                description: "Test plugin".into(),
            }
        }

        fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
            reg.sysvars.insert(
                "⎕TESTVAR".into(),
                ValueP::scalar_from(Cell::Int(42)),
            );
            Ok(())
        }
    }

    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();
        let plugin = Arc::new(TestPlugin);
        registry.register(plugin);

        assert_eq!(registry.plugins().len(), 1);
        assert_eq!(registry.plugins()[0].info().name, "test");
    }

    #[test]
    fn test_plugin_find_missing() {
        let registry = PluginRegistry::new();
        assert!(registry.find("nonexistent").is_none());
    }

    #[test]
    fn test_plugin_registrar() {
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        let plugin = TestPlugin;
        plugin.register(&mut reg).unwrap();

        assert!(reg.sysvars.contains_key("⎕TESTVAR"));
        assert_eq!(
            reg.sysvars.get("⎕TESTVAR").unwrap().first_cell().unwrap(),
            &Cell::Int(42)
        );
    }

    #[test]
    fn test_plugin_info() {
        let plugin = TestPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "test");
        assert_eq!(info.version, "0.0.1");
        assert_eq!(info.description, "Test plugin");
    }
}
/// Initialize the plugin system with all static plugins.
/// Called once at interpreter startup.
#[allow(dead_code)]
pub fn init_plugins(func_table: &mut FunctionTable, sysvars: &mut HashMap<String, ValueP>) -> AplResult<()> {
    let mut reg = PluginRegistrar { func_table, sysvars };

    // Register static plugins based on compile-time features
    #[cfg(feature = "plugin-plot")]
    {
        let plugin = crate::plugins::plot::PlotPlugin::new();
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-png")]
    {
        let plugin = crate::plugins::png::PngPlugin::new();
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-sql")]
    {
        let plugin = crate::plugins::sql::SqlPlugin::new();
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-fft")]
    {
        let plugin = crate::plugins::fft::FftPlugin::new();
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-python")]
    {
        let plugin = crate::plugins::python::PythonPlugin::new();
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-gtk")]
    {
        let plugin = crate::plugins::gtk::GtkPlugin::new();
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-cdr")]
    {
        let plugin = crate::plugins::cdr::CdrPlugin::new();
        plugin.register(&mut reg)?;
    }

    Ok(())
}
