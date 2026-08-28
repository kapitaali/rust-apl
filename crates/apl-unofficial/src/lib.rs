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
