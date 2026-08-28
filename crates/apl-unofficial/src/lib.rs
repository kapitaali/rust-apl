//! Unofficial APL extensions: ⌸ (key) and ⍥ (over)
//!
//! These primitives are NOT in GNU APL 2.0 but are widely used in
//! Dyalog APL and other implementations. They are provided as a
//! compile-time extension to keep the core interpreter strictly
//! GNU APL compatible.

use apl_ext::{apl_extension, AplExtension, Registrar};

struct UnofficialExt;

impl AplExtension for UnofficialExt {
    fn name(&self) -> &'static str {
        "unofficial"
    }

    fn register(&self, _reg: &mut Registrar) {
        // ⌸ and ⍥ are registered as special forms in the interpreter
        // core (via #[cfg(feature = "unofficial-ext")]) because they
        // need access to the parser and evaluator, not just XValue ops.
        //
        // This crate exists so that --features unofficial-ext has a
        // dependency to activate and a place to put extension-specific
        // tests.
    }
}

apl_extension!(|| Box::new(UnofficialExt));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unofficial_ext_name() {
        let ext = UnofficialExt;
        assert_eq!(ext.name(), "unofficial");
    }

    #[test]
    fn test_unofficial_ext_register_does_not_panic() {
        let ext = UnofficialExt;
        let mut reg = Registrar::new();
        ext.register(&mut reg);
        // register() should not panic and should produce no entries
        // (the actual registration happens in the interpreter core)
        let entries = reg.into_entries();
        assert_eq!(entries.len(), 0);
    }
}
