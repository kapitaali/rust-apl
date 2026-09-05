//! IPC (Inter-Process Communication) module.
//!
//! Implements a TCP-based shared variable server (AP210 equivalent).
//! Allows multiple APL processes to share variables over the network.

pub mod client;
pub mod protocol;
pub mod server;

pub use client::IpcClient;
pub use protocol::{IpcCommand, IpcResponse};
pub use server::IpcServer;
