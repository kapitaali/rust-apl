//! demo-plugin — proves the F5 plugin path end-to-end.
//!
//! Registers:
//! - STRREV: reverse a character vector (typed accessor round-trip)
//! - BOOM:   panics deliberately (panic containment demo)
//! - SUMI:   sum an int vector

use apl_ext::{apl_extension, AplError, AplExtension, Registrar, XValue};

struct DemoExt;

impl AplExtension for DemoExt {
    fn name(&self) -> &'static str {
        "demo"
    }

    fn register(&self, reg: &mut Registrar) {
        // STRREV 'hello' → 'olleh'
        reg.bind("STRREV", 1, 1, |_ctx, args| {
            let s = args[0].as_string();
            let rev: String = s.chars().rev().collect();
            Ok(XValue::from_str_val(&rev))
        });

        // SUMI 1 2 3 → 6
        reg.bind("SUMI", 1, 1, |_ctx, args| {
            let total: i64 = args[0].as_ints().iter().sum();
            Ok(XValue::from_int(total))
        });

        // PANICME * → DOMAIN ERROR via caught panic
        reg.bind("PANICME", 1, 1, |_ctx, _args| {
            panic!("deliberate demo panic");
            #[allow(unreachable_code)]
            Err(AplError::msg("unreachable"))
        });
    }
}

apl_extension!(|| Box::new(DemoExt));
