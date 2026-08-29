//! Auxiliary Processors (AP) for GNU APL.
//!
//! AP100 is the file server, AP210 is the shared variable server.
//! These are NOT implemented in this port — they require extensive
//! socket communication, shared variable infrastructure, and IPC.
//!
//! This module provides placeholder definitions and documentation
//! for future implementation.

/// Auxiliary Processor types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum APType {
    /// AP100 — File server
    FileServer = 100,
    /// AP210 — Shared variable server
    SharedVariableServer = 210,
}

/// Auxiliary Processor state
#[derive(Debug)]
pub struct AuxiliaryProcessor {
    ap_type: APType,
    // Future fields:
    // - socket listener
    // - shared variable registry
    // - connection pool
}

impl AuxiliaryProcessor {
    /// Create a new auxiliary processor (placeholder)
    pub fn new(ap_type: APType) -> Self {
        Self { ap_type }
    }

    /// Get the AP type
    pub fn ap_type(&self) -> APType {
        self.ap_type
    }

    /// Start the AP (placeholder — not implemented)
    pub fn start(&self) -> Result<(), String> {
        Err(format!(
            "Auxiliary Processor {:?} is not implemented in this port",
            self.ap_type
        ))
    }
}

/// AP100 File Server — handles file I/O operations for APL workspaces
///
/// In GNU APL, AP100 provides:
/// - File open/read/write/close operations
/// - Directory listing
/// - File system operations
///
/// This requires:
/// - TCP socket communication
/// - APL shared variable protocol
/// - File handle management
pub struct AP100;

impl AP100 {
    pub fn new() -> AuxiliaryProcessor {
        AuxiliaryProcessor::new(APType::FileServer)
    }
}

/// AP210 Shared Variable Server — handles inter-process communication
///
/// In GNU APL, AP210 provides:
/// - Shared variable offer/accept
/// - Variable update notifications
/// - Inter-APL-process communication
///
/// This requires:
/// - TCP socket communication
/// - APL shared variable protocol
/// - Variable synchronization
pub struct AP210;

impl AP210 {
    pub fn new() -> AuxiliaryProcessor {
        AuxiliaryProcessor::new(APType::SharedVariableServer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ap_types() {
        let ap100 = AP100::new();
        let ap210 = AP210::new();
        assert_eq!(ap100.ap_type(), APType::FileServer);
        assert_eq!(ap210.ap_type(), APType::SharedVariableServer);
    }

    #[test]
    fn test_ap_start_not_implemented() {
        let ap100 = AP100::new();
        assert!(ap100.start().is_err());
    }
}
