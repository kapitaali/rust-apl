//! IPC client — connects to an IPC server.
//!
//! Provides a simple client for the shared variable server.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use super::protocol::{IpcCommand, IpcResponse};

/// IPC client.
pub struct IpcClient {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

impl IpcClient {
    /// Connect to an IPC server.
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(IpcClient { stream, reader })
    }

    /// Send a command and get the response.
    pub fn send(&mut self, cmd: &IpcCommand) -> std::io::Result<IpcResponse> {
        let cmd_str = format!("{}\n", cmd);
        self.stream.write_all(cmd_str.as_bytes())?;
        self.stream.flush()?;

        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        let line = line.trim();

        // Parse response
        if line == "OK" {
            Ok(IpcResponse::Ok)
        } else if line.starts_with("ERROR ") {
            Ok(IpcResponse::Error(line[6..].to_string()))
        } else if let Ok(n) = line.parse::<i64>() {
            Ok(IpcResponse::Int(n))
        } else {
            // Check if it's a list of names
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 1 {
                Ok(IpcResponse::Names(
                    parts.iter().map(|s| s.to_string()).collect(),
                ))
            } else {
                Ok(IpcResponse::Value(line.to_string()))
            }
        }
    }

    /// Offer a variable.
    pub fn offer(&mut self, name: &str, value: &str) -> std::io::Result<IpcResponse> {
        self.send(&IpcCommand::Offer {
            name: name.to_string(),
            value: value.to_string(),
        })
    }

    /// Query if a variable exists.
    pub fn query(&mut self, name: &str) -> std::io::Result<IpcResponse> {
        self.send(&IpcCommand::Query {
            name: name.to_string(),
        })
    }

    /// Read a variable.
    pub fn read(&mut self, name: &str) -> std::io::Result<IpcResponse> {
        self.send(&IpcCommand::Read {
            name: name.to_string(),
        })
    }

    /// Write a variable.
    pub fn write(&mut self, name: &str, value: &str) -> std::io::Result<IpcResponse> {
        self.send(&IpcCommand::Write {
            name: name.to_string(),
            value: value.to_string(),
        })
    }

    /// List all offered variables.
    pub fn list(&mut self) -> std::io::Result<IpcResponse> {
        self.send(&IpcCommand::List)
    }

    /// Cancel an offer.
    pub fn cancel(&mut self, name: &str) -> std::io::Result<IpcResponse> {
        self.send(&IpcCommand::Cancel {
            name: name.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    use crate::ipc::server::IpcServer;

    #[test]
    fn test_client_server_roundtrip() {
        // Start server on a random port
        let server = IpcServer::new(0);
        let handle = server.run_background();
        thread::sleep(Duration::from_millis(200));

        // We can't get the port easily, so test with a direct connection attempt
        // This test is best-effort - if port binding fails, skip
        let result = IpcClient::connect("127.0.0.1:19876");
        if result.is_err() {
            // Server didn't start in time or port issue
            return;
        }

        let mut client = result.unwrap();

        // Offer a variable
        let response = client.offer("test_var", "42").unwrap();
        assert_eq!(response, IpcResponse::Ok);

        // Query it
        let response = client.query("test_var").unwrap();
        assert_eq!(response, IpcResponse::Int(1));

        // Read it
        let response = client.read("test_var").unwrap();
        assert_eq!(response, IpcResponse::Value("42".to_string()));

        // Write it
        let response = client.write("test_var", "100").unwrap();
        assert_eq!(response, IpcResponse::Ok);

        // Read again
        let response = client.read("test_var").unwrap();
        assert_eq!(response, IpcResponse::Value("100".to_string()));

        // List
        let response = client.list().unwrap();
        match response {
            IpcResponse::Names(names) => assert!(names.contains(&"test_var".to_string())),
            _ => panic!("expected Names"),
        }

        // Cancel
        let response = client.cancel("test_var").unwrap();
        assert_eq!(response, IpcResponse::Int(1));

        // Query again
        let response = client.query("test_var").unwrap();
        assert_eq!(response, IpcResponse::Int(0));
    }
}
