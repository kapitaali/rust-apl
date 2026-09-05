//! IPC protocol definitions.
//!
//! Simple text-based protocol for shared variable operations:
//! - OFFER <name> <value>  → OK / ERROR
//! - QUERY <name>         → 1 / 0 / ERROR
//! - READ <name>           → <value> / ERROR
//! - WRITE <name> <value>  → OK / ERROR
//! - LIST                  → <name1> <name2> ... / ERROR
//! - CANCEL <name>         → 1 / 0 / ERROR

use std::fmt;

/// Commands sent from client to server.
#[derive(Debug, Clone, PartialEq)]
pub enum IpcCommand {
    /// Offer a variable: OFFER <name> <value>
    Offer { name: String, value: String },
    /// Query if variable exists: QUERY <name>
    Query { name: String },
    /// Read variable value: READ <name>
    Read { name: String },
    /// Write variable value: WRITE <name> <value>
    Write { name: String, value: String },
    /// List all offered variables: LIST
    List,
    /// Withdraw an offer: CANCEL <name>
    Cancel { name: String },
}

/// Responses sent from server to client.
#[derive(Debug, Clone, PartialEq)]
pub enum IpcResponse {
    /// Operation successful
    Ok,
    /// Value response
    Value(String),
    /// Integer response (for queries)
    Int(i64),
    /// List of names
    Names(Vec<String>),
    /// Error with message
    Error(String),
}

impl fmt::Display for IpcResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcResponse::Ok => write!(f, "OK"),
            IpcResponse::Value(v) => write!(f, "{}", v),
            IpcResponse::Int(n) => write!(f, "{}", n),
            IpcResponse::Names(names) => {
                let joined = names.join(" ");
                write!(f, "{}", joined)
            }
            IpcResponse::Error(msg) => write!(f, "ERROR {}", msg),
        }
    }
}

impl IpcCommand {
    /// Parse a command from a string.
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("empty command".to_string());
        }

        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        let cmd = parts[0].to_uppercase();

        match cmd.as_str() {
            "OFFER" => {
                if parts.len() < 3 {
                    return Err("OFFER requires name and value".to_string());
                }
                Ok(IpcCommand::Offer {
                    name: parts[1].to_string(),
                    value: parts[2].to_string(),
                })
            }
            "QUERY" => {
                if parts.len() < 2 {
                    return Err("QUERY requires name".to_string());
                }
                Ok(IpcCommand::Query {
                    name: parts[1].to_string(),
                })
            }
            "READ" => {
                if parts.len() < 2 {
                    return Err("READ requires name".to_string());
                }
                Ok(IpcCommand::Read {
                    name: parts[1].to_string(),
                })
            }
            "WRITE" => {
                if parts.len() < 3 {
                    return Err("WRITE requires name and value".to_string());
                }
                Ok(IpcCommand::Write {
                    name: parts[1].to_string(),
                    value: parts[2].to_string(),
                })
            }
            "LIST" => Ok(IpcCommand::List),
            "CANCEL" => {
                if parts.len() < 2 {
                    return Err("CANCEL requires name".to_string());
                }
                Ok(IpcCommand::Cancel {
                    name: parts[1].to_string(),
                })
            }
            _ => Err(format!("unknown command: {}", cmd)),
        }
    }
}

impl fmt::Display for IpcCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcCommand::Offer { name, value } => write!(f, "OFFER {} {}", name, value),
            IpcCommand::Query { name } => write!(f, "QUERY {}", name),
            IpcCommand::Read { name } => write!(f, "READ {}", name),
            IpcCommand::Write { name, value } => write!(f, "WRITE {} {}", name, value),
            IpcCommand::List => write!(f, "LIST"),
            IpcCommand::Cancel { name } => write!(f, "CANCEL {}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_offer() {
        let cmd = IpcCommand::parse("OFFER foo 42").unwrap();
        assert_eq!(
            cmd,
            IpcCommand::Offer {
                name: "foo".to_string(),
                value: "42".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_query() {
        let cmd = IpcCommand::parse("QUERY foo").unwrap();
        assert_eq!(
            cmd,
            IpcCommand::Query {
                name: "foo".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_read() {
        let cmd = IpcCommand::parse("READ foo").unwrap();
        assert_eq!(
            cmd,
            IpcCommand::Read {
                name: "foo".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_write() {
        let cmd = IpcCommand::parse("WRITE foo 42").unwrap();
        assert_eq!(
            cmd,
            IpcCommand::Write {
                name: "foo".to_string(),
                value: "42".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_list() {
        let cmd = IpcCommand::parse("LIST").unwrap();
        assert_eq!(cmd, IpcCommand::List);
    }

    #[test]
    fn test_parse_cancel() {
        let cmd = IpcCommand::parse("CANCEL foo").unwrap();
        assert_eq!(
            cmd,
            IpcCommand::Cancel {
                name: "foo".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_unknown() {
        let result = IpcCommand::parse("FOO bar");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty() {
        let result = IpcCommand::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_offer_missing_value() {
        let result = IpcCommand::parse("OFFER foo");
        assert!(result.is_err());
    }

    #[test]
    fn test_response_display_ok() {
        assert_eq!(format!("{}", IpcResponse::Ok), "OK");
    }

    #[test]
    fn test_response_display_value() {
        assert_eq!(
            format!("{}", IpcResponse::Value("hello".to_string())),
            "hello"
        );
    }

    #[test]
    fn test_response_display_int() {
        assert_eq!(format!("{}", IpcResponse::Int(42)), "42");
    }

    #[test]
    fn test_response_display_names() {
        assert_eq!(
            format!(
                "{}",
                IpcResponse::Names(vec!["a".to_string(), "b".to_string()])
            ),
            "a b"
        );
    }

    #[test]
    fn test_response_display_error() {
        assert_eq!(
            format!("{}", IpcResponse::Error("bad".to_string())),
            "ERROR bad"
        );
    }
}
