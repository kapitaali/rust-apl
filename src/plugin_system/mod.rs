//! Plugin system — Phase 6 extensible features.
//!
//! Manages static and dynamic plugin registration based on `config.toml`.
//! Static plugins are compiled into the binary; dynamic plugins are loaded
//! at runtime via ⎕LOADSO.

use crate::functions_def::FunctionTable;
use crate::parser::Expr;
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;
use std::collections::HashMap;
use std::sync::Arc;

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
    /// Mutable reference to the environment's hook list, if available.
    pub hooks: Option<&'a mut Vec<Arc<dyn AplPluginHooks>>>,
}

/// Middleware hooks that plugins can implement to intercept operations.
///
/// Hooks are called at key points during evaluation and system command
/// execution. A hook returning `Err` blocks the operation.
pub trait AplPluginHooks: Send + Sync {
    /// Called before evaluating any expression.
    /// Return `Err` to block the evaluation.
    fn before_eval(&self, _expr: &Expr) -> AplResult<()> {
        Ok(())
    }

    /// Called before running any system command.
    /// Return `Err` to block the command.
    fn before_syscmd(&self, _cmd: &str) -> AplResult<()> {
        Ok(())
    }

    /// Called when a system variable is set.
    /// `name` is the full name (e.g., "⎕SEC").
    fn on_sysvar_change(&self, _name: &str, _value: &ValueP) -> AplResult<()> {
        Ok(())
    }
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
    /// Return `Some(hooks)` to register middleware hooks for this plugin.
    fn hooks(&self) -> Option<Arc<dyn AplPluginHooks>> {
        None
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

/// Initialize the plugin system with all static plugins.
/// Called once at interpreter startup.
#[allow(dead_code)]
pub fn init_plugins(
    func_table: &mut FunctionTable,
    sysvars: &mut HashMap<String, ValueP>,
    hooks: &mut Vec<Arc<dyn AplPluginHooks>>,
) -> AplResult<()> {
    let mut reg = PluginRegistrar {
        func_table,
        sysvars,
        hooks: None,
    };

    // Collect hooks from each plugin during registration
    let mut collected_hooks: Vec<Arc<dyn AplPluginHooks>> = Vec::new();

    // Register static plugins based on compile-time features
    #[cfg(feature = "plugin-plot")]
    {
        let plugin = crate::plugins::plot::PlotPlugin::new();
        if let Some(h) = plugin.hooks() {
            collected_hooks.push(h);
        }
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-png")]
    {
        let plugin = crate::plugins::png::PngPlugin::new();
        if let Some(h) = plugin.hooks() {
            collected_hooks.push(h);
        }
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-sql")]
    {
        let plugin = crate::plugins::sql::SqlPlugin::new();
        if let Some(h) = plugin.hooks() {
            collected_hooks.push(h);
        }
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-fft")]
    {
        let plugin = crate::plugins::fft::FftPlugin::new();
        if let Some(h) = plugin.hooks() {
            collected_hooks.push(h);
        }
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-python")]
    {
        let plugin = crate::plugins::python::PythonPlugin::new();
        if let Some(h) = plugin.hooks() {
            collected_hooks.push(h);
        }
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-gtk")]
    {
        let plugin = crate::plugins::gtk::GtkPlugin::new();
        if let Some(h) = plugin.hooks() {
            collected_hooks.push(h);
        }
        plugin.register(&mut reg)?;
    }

    #[cfg(feature = "plugin-cdr")]
    {
        let plugin = crate::plugins::cdr::CdrPlugin::new();
        if let Some(h) = plugin.hooks() {
            collected_hooks.push(h);
        }
        plugin.register(&mut reg)?;
    }

    // Merge collected hooks into the output
    hooks.extend(collected_hooks);

    Ok(())
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
            reg.sysvars
                .insert("⎕TESTVAR".into(), ValueP::scalar_from(Cell::Int(42)));
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
        let mut hooks = Vec::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
            hooks: Some(&mut hooks),
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
