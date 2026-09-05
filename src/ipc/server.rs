//! IPC server — TCP-based shared variable server (AP210 equivalent).
//!
//! Listens for connections and processes commands from clients.
//! Backed by a shared variable registry.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use super::protocol::{IpcCommand, IpcResponse};

/// Shared variable entry.
#[derive(Clone, Debug)]
struct SvEntry {
    value: String,
    owner: String,
}

/// The IPC server.
pub struct IpcServer {
    port: u16,
    registry: Arc<Mutex<std::collections::HashMap<String, SvEntry>>>,
}

impl IpcServer {
    /// Create a new IPC server on the given port.
    pub fn new(port: u16) -> Self {
        IpcServer {
            port,
            registry: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Get the port this server listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Run the server (blocking).
    pub fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))?;
        println!("IPC server listening on port {}", self.port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let registry = self.registry.clone();
                    std::thread::spawn(move || {
                        handle_client(stream, registry);
                    });
                }
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Run the server in a background thread.
    pub fn run_background(&self) -> std::thread::JoinHandle<std::io::Result<()>> {
        let registry = self.registry.clone();
        let port = self.port;
        std::thread::spawn(move || {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let registry = registry.clone();
                        std::thread::spawn(move || {
                            handle_client(stream, registry);
                        });
                    }
                    Err(e) => {
                        eprintln!("Connection failed: {}", e);
                    }
                }
            }
            Ok(())
        })
    }
}

fn handle_client(mut stream: TcpStream, registry: Arc<Mutex<std::collections::HashMap<String, SvEntry>>>) {
    let peer = stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
    println!("Client connected: {}", peer);

    let reader = BufReader::new(stream.try_clone().unwrap_or_else(|_| stream.try_clone().expect("clone")));

    for line in reader.lines() {
        match line {
            Ok(input) => {
                let input = input.trim();
                if input.is_empty() {
                    continue;
                }

                let response = process_command(input, &registry);
                let response_str = format!("{}\n", response);
                if let Err(e) = stream.write_all(response_str.as_bytes()) {
                    eprintln!("Write error: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }

    println!("Client disconnected: {}", peer);
}

fn process_command(
    input: &str,
    registry: &Arc<Mutex<std::collections::HashMap<String, SvEntry>>>,
) -> IpcResponse {
    let cmd = match IpcCommand::parse(input) {
        Ok(cmd) => cmd,
        Err(e) => return IpcResponse::Error(e),
    };

    let mut guard = match registry.lock() {
        Ok(g) => g,
        Err(_) => return IpcResponse::Error("lock failed".to_string()),
    };

    match cmd {
        IpcCommand::Offer { name, value } => {
            guard.insert(
                name,
                SvEntry {
                    value,
                    owner: "remote".to_string(),
                },
            );
            IpcResponse::Ok
        }
        IpcCommand::Query { name } => {
            if guard.contains_key(&name) {
                IpcResponse::Int(1)
            } else {
                IpcResponse::Int(0)
            }
        }
        IpcCommand::Read { name } => match guard.get(&name) {
            Some(entry) => IpcResponse::Value(entry.value.clone()),
            None => IpcResponse::Error(format!("{} not found", name)),
        },
        IpcCommand::Write { name, value } => {
            if let Some(entry) = guard.get_mut(&name) {
                entry.value = value;
                IpcResponse::Ok
            } else {
                IpcResponse::Error(format!("{} not found", name))
            }
        }
        IpcCommand::List => {
            let names: Vec<String> = guard.keys().cloned().collect();
            IpcResponse::Names(names)
        }
        IpcCommand::Cancel { name } => {
            if guard.remove(&name).is_some() {
                IpcResponse::Int(1)
            } else {
                IpcResponse::Int(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_server_offer_and_query() {
        let server = IpcServer::new(0); // port 0 = OS assigns
        let handle = server.run_background();
        thread::sleep(Duration::from_millis(100)); // wait for server to start

        // We can't easily get the assigned port, so just test the protocol
        let registry = Arc::new(Mutex::new(std::collections::HashMap::new()));

        let response = process_command("OFFER foo 42", &registry);
        assert_eq!(response, IpcResponse::Ok);

        let response = process_command("QUERY foo", &registry);
        assert_eq!(response, IpcResponse::Int(1));

        let response = process_command("QUERY bar", &registry);
        assert_eq!(response, IpcResponse::Int(0));
    }

    #[test]
    fn test_server_read_write() {
        let registry = Arc::new(Mutex::new(std::collections::HashMap::new()));

        process_command("OFFER x 100", &registry);

        let response = process_command("READ x", &registry);
        assert_eq!(response, IpcResponse::Value("100".to_string()));

        let response = process_command("WRITE x 200", &registry);
        assert_eq!(response, IpcResponse::Ok);

        let response = process_command("READ x", &registry);
        assert_eq!(response, IpcResponse::Value("200".to_string()));
    }

    #[test]
    fn test_server_list() {
        let registry = Arc::new(Mutex::new(std::collections::HashMap::new()));

        process_command("OFFER a 1", &registry);
        process_command("OFFER b 2", &registry);

        let response = process_command("LIST", &registry);
        match response {
            IpcResponse::Names(names) => {
                assert!(names.contains(&"a".to_string()));
                assert!(names.contains(&"b".to_string()));
            }
            _ => panic!("expected Names response"),
        }
    }

    #[test]
    fn test_server_cancel() {
        let registry = Arc::new(Mutex::new(std::collections::HashMap::new()));

        process_command("OFFER temp 42", &registry);

        let response = process_command("CANCEL temp", &registry);
        assert_eq!(response, IpcResponse::Int(1));

        let response = process_command("QUERY temp", &registry);
        assert_eq!(response, IpcResponse::Int(0));

        let response = process_command("CANCEL temp", &registry);
        assert_eq!(response, IpcResponse::Int(0));
    }

    #[test]
    fn test_server_read_not_found() {
        let registry = Arc::new(Mutex::new(std::collections::HashMap::new()));

        let response = process_command("READ nonexistent", &registry);
        match response {
            IpcResponse::Error(_) => {}
            _ => panic!("expected Error response"),
        }
    }

    #[test]
    fn test_server_write_not_found() {
        let registry = Arc::new(Mutex::new(std::collections::HashMap::new()));

        let response = process_command("WRITE nonexistent 42", &registry);
        match response {
            IpcResponse::Error(_) => {}
            _ => panic!("expected Error response"),
        }
    }

    #[test]
    fn test_server_unknown_command() {
        let registry = Arc::new(Mutex::new(std::collections::HashMap::new()));

        let response = process_command("FOO bar", &registry);
        match response {
            IpcResponse::Error(_) => {}
            _ => panic!("expected Error response"),
        }
    }
}
