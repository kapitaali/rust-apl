//! GNU APL REPL — the `apl` binary.
//!
//! Reads APL expressions from stdin, evaluates, and prints results
//! (mirrors a minimal `main.cc` + `Command::command_loop()`).

use apl::parser::Environment;
use apl::types::ErrorCode;
use apl::AplError;
use std::io::{self, BufRead, Write};

/// Format a float with ⎕PP significant digits (GNU APL uses %g, not %f).
/// PP=10 means 10 significant digits.
fn float_to_g(v: f64, pp: usize) -> String {
    // Use scientific notation with pp-1 decimals (gives pp significant digits)
    let s = format!("{:.*e}", pp - 1, v);
    if let Some(e_pos) = s.find('e') {
        let mantissa = &s[..e_pos];
        let exp: i32 = s[e_pos + 1..].parse().unwrap_or(0);
        let dot_pos = mantissa.find('.');
        if let Some(dot) = dot_pos {
            let int_part = &mantissa[..dot];
            let frac_part = &mantissa[dot + 1..];
            if exp >= 0 {
                let shift = exp as usize;
                if shift >= frac_part.len() {
                    format!("{}{}", int_part, frac_part) + &"0".repeat(shift - frac_part.len())
                } else {
                    let whole = &frac_part[..shift];
                    let rest = &frac_part[shift..];
                    let rest_trimmed = rest.trim_end_matches('0');
                    if rest_trimmed.is_empty() {
                        format!("{}{}", int_part, whole)
                    } else {
                        format!("{}{}.{}", int_part, whole, rest_trimmed)
                    }
                }
            } else {
                // Negative exponent: 0.xxx form
                let neg_exp = (-exp) as usize;
                if neg_exp == 1 && int_part == "0" {
                    // e.g., 8.775e-1 → 0.8775
                    let frac_trimmed = frac_part.trim_end_matches('0');
                    if frac_trimmed.is_empty() {
                        "0".to_string()
                    } else {
                        format!("0.{}", frac_trimmed)
                    }
                } else if neg_exp == 1 {
                    // e.g., 1.23e-1 → 0.123
                    let frac_trimmed = frac_part.trim_end_matches('0');
                    if frac_trimmed.is_empty() {
                        "0".to_string()
                    } else {
                        format!("0.{}{}", int_part, frac_trimmed)
                    }
                } else {
                    // Larger negative exponent: 0.00...xxx
                    let zeros = "0".repeat(neg_exp - 1);
                    let frac_trimmed = frac_part.trim_end_matches('0');
                    if frac_trimmed.is_empty() {
                        "0".to_string()
                    } else {
                        format!("0.{}{}{}", zeros, int_part, frac_trimmed)
                    }
                }
            }
        } else {
            s
        }
    } else {
        s
    }
}

fn format_value(v: &apl::value::ValueP, pp: usize) -> String {
    // simple one-line formatting for scalars and vectors
    if v.is_scalar() || v.is_vector() {
        let items: Vec<String> = v.cells().iter().map(|c| format_cell(c, pp)).collect();
        items.join("  ")
    } else if v.rank() == 2 {
        // matrix: rows on separate lines
        let cols = v.get_shape_item(1) as usize;
        let cells = v.cells();
        let mut lines = Vec::new();
        for row in 0..(cells.len() / cols.max(1)) {
            let items: Vec<String> = cells[row * cols..(row + 1) * cols]
                .iter()
                .map(|c| format_cell(c, pp))
                .collect();
            lines.push(items.join(" "));
        }
        lines.join("\n")
    } else {
        format!("{:?}", v.shape())
    }
}

/// Render an evaluation result exactly the way a session displays it: boxed
/// when boxing is on and the value is nested, plain for flat matrices and
/// character output, simple one-line formatting otherwise. Both the
/// interactive REPL and the RIDE gateway evaluate path use this so the two
/// can never drift apart again.
fn render_session_value(v: &apl::value::ValueP, pp: usize, boxing: bool) -> String {
    let all_chars = !v.cells().is_empty() && v.cells().iter().all(|c| c.is_character_cell());
    let has_pointer = v.cells().iter().any(|c| c.is_pointer_cell());
    if v.rank() >= 2 || all_chars || (has_pointer && boxing) {
        if boxing && has_pointer {
            apl::boxdisplay::render_with_pp(v, pp).join("\n")
        } else {
            apl::boxdisplay::render_plain_with_pp(v, pp).join("\n")
        }
    } else {
        format_value(v, pp)
    }
}

/// replace a leading ASCII '-' with a visible minus sign
///
/// Must match boxdisplay::high_minus — this file keeps its own copy of the
/// display logic (see the routing note in main()), so a fix in one place has
/// to be mirrored here.
fn high_minus(s: &str) -> String {
    match s.strip_prefix('-') {
        Some(rest) => format!("−{rest}"), // U+2212 MINUS SIGN
        None => s.to_string(),
    }
}

fn format_cell(c: &apl::cell::Cell, pp: usize) -> String {
    match c {
        apl::cell::Cell::Int(v) => high_minus(&v.to_string()),
        apl::cell::Cell::Float(v) => {
            let s = if v.fract() == 0.0 && v.abs() < 1e15 {
                format!("{}", *v as i64)
            } else if v.abs() >= 1e-10 {
                // GNU APL uses %g (significant digits), not %f (decimal places)
                // PP=10 means 10 significant digits
                float_to_g(*v, pp)
            } else {
                format!("{}", v)
            };
            high_minus(&s)
        }
        apl::cell::Cell::Char(u) => char::from_u32(*u).unwrap_or('?').to_string(),
        apl::cell::Cell::Complex(c) => {
            format!(
                "{}J{}",
                high_minus(&float_to_g(c.re, pp)),
                high_minus(&float_to_g(c.im, pp))
            )
        }
        apl::cell::Cell::Pointer(p) => format_nested(p.value.cells(), pp),
        _ => "<lval>".to_string(),
    }
}

/// format the cells of a nested value: simple scalars inline, deeper
/// nesting recurses. A single scalar shows bare; vectors show space-
/// separated; higher rank shows rows.
fn format_nested(cells: &[apl::cell::Cell], pp: usize) -> String {
    // all-simple vector → space-separated inline
    if cells.iter().all(|c| c.is_simple_cell()) {
        if cells.len() == 1 {
            return format_cell(&cells[0], pp);
        }
        return cells
            .iter()
            .map(|c| format_cell(c, pp))
            .collect::<Vec<_>>()
            .join(" ");
    }
    // mixed/nested → recurse per element
    cells
        .iter()
        .map(|c| match c {
            apl::cell::Cell::Pointer(p) => format_nested(p.value.cells(), pp),
            other => format_cell(other, pp),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quiet = args.iter().any(|a| a == "-q" || a == "--quiet");
    let serve = args
        .iter()
        .position(|a| a == "--serve")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u16>().ok());

    let ride = args.iter().any(|a| a == "--ride");

    if let Some(port) = serve {
        serve_mode(port);
        return;
    }

    if ride {
        ride_mode();
        return;
    }

    if !quiet {
        println!("GNU APL 2.0 (Rust) — experimental REPL");
        println!("Enter APL expressions, or )OFF to exit.");
    }
    let stdin = io::stdin();
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);

    // Initialize plugins (Phase 6)
    if let Err(e) = apl::plugin_system::init_plugins(
        &mut env.funcs,
        &mut std::collections::HashMap::new(),
        &mut env.hooks,
    ) {
        eprintln!("Warning: plugin initialization failed: {}", e);
    }

    // function definition mode state: Some(header) while inside ∇ editing
    let mut def_header: Option<String> = None;
    let mut def_body: Vec<String> = Vec::new();

    loop {
        // prompt: inside a function definition show line numbers
        match &def_header {
            Some(_) => print!("[{}] ", def_body.len() + 1),
            None => print!("      "),
        }
        io::stdout().flush().ok();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }
        let trimmed = line.trim_end();
        // system commands: )CMD — handled before anything else
        if trimmed.starts_with(')') || trimmed.starts_with(']') {
            let cmd = trimmed.chars().skip(1).collect::<String>();
            match apl::sysvars::syscmd(&cmd, &mut env) {
                None => break, // )OFF
                Some(lines) => {
                    for l in lines {
                        if !l.is_empty() {
                            println!("{}", l);
                        }
                    }
                }
            }
            continue;
        }
        if trimmed == ")OFF" || trimmed == ")off" {
            break;
        }

        // function definition mode: ∇HEADER starts, lone ∇ (or ∇) ends
        if def_header.is_none() && trimmed.starts_with('∇') && trimmed.len() > 1 {
            def_header = Some(
                trimmed
                    .chars()
                    .skip(1)
                    .collect::<String>()
                    .trim()
                    .to_string(),
            );
            def_body.clear();
            continue;
        }
        if def_header.is_some() {
            if trimmed == "∇" || trimmed.is_empty() {
                // end of definition: compile and install
                let header = def_header.take().unwrap();
                match apl::functions_def::define_function(&mut env.funcs, &header, &def_body) {
                    Ok(()) => println!(
                        "{} defined",
                        header.split_whitespace().next().unwrap_or("?")
                    ),
                    Err(e) => {
                        let rich = AplError::with_message(ErrorCode::SyntaxError, e)
                            .with_source_line(trimmed.to_string());
                        println!("ERROR: {}", rich);
                    }
                }
            } else {
                def_body.push(trimmed.to_string());
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        match env.eval_line(trimmed) {
            Ok(Some(v)) => {
                // ⎕PP print precision and ⎕BOXING style, as in a session
                let pp = apl::sysvars::get_pp(&env).unwrap_or(10);
                let boxing = apl::sysvars::get_boxing(&env);
                println!("{}", render_session_value(&v, pp, boxing));
            }
            Ok(None) => {} // assignment — no output
            Err(e) => {
                // Display error with source line for context
                let rich = AplError::from(e).with_source_line(trimmed.to_string());
                println!("ERROR: {}", rich);
            }
        }
    }

    // After REPL loop ends, wait for any GTK windows to close
    // This prevents the main thread from killing the GTK thread prematurely
    #[cfg(feature = "plugin-gtk")]
    {
        apl::plugins::gtk::gtk_wait_timeout(u64::MAX);
    }
}

/// Serve mode: listen on a TCP port, speak the RIDE binary-framed protocol.
/// Framing: [4 bytes BE total length][4 bytes "RIDE"][JSON payload]
/// This matches the protocol used by the RIDE editor (src/cn.js in the RIDE repo).
fn serve_mode(port: u16) {
    use std::net::TcpListener;

    let listener = match TcpListener::bind(format!("127.0.0.1:{port}")) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("apl server: cannot bind port {port}: {e}");
            std::process::exit(1);
        }
    };
    println!("APL server listening on port {port}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || handle_client(stream));
            }
            Err(e) => eprintln!("Connection failed: {e}"),
        }
    }
}

/// Frame a JSON payload for the RIDE protocol.
fn frame(payload: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + payload.len());
    let total_len = (8 + payload.len()) as u32;
    buf.extend_from_slice(&total_len.to_be_bytes());
    buf.extend_from_slice(b"RIDE");
    buf.extend_from_slice(payload.as_bytes());
    buf
}

fn handle_client(mut stream: std::net::TcpStream) {
    use std::io::{Read, Write};

    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let _ = apl::plugin_system::init_plugins(
        &mut env.funcs,
        &mut std::collections::HashMap::new(),
        &mut env.hooks,
    );

    let mut buf = [0u8; 4096];
    let mut acc = Vec::new();

    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }

        // Process all complete frames in the accumulator.
        loop {
            if acc.len() < 8 {
                break;
            }
            let frame_len = u32::from_be_bytes([acc[0], acc[1], acc[2], acc[3]]) as usize;
            if acc.len() < frame_len {
                break; // incomplete frame
            }
            // Skip the 4-byte "RIDE" magic, extract JSON payload.
            let payload = String::from_utf8_lossy(&acc[8..frame_len]).to_string();
            acc.drain(0..frame_len);

            if payload.starts_with('[') {
                // JSON command: ["Name", {...}]
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload) {
                    if let Some(arr) = val.as_array() {
                        let cmd = arr[0].as_str().unwrap_or("");
                        let args = arr.get(1).cloned().unwrap_or(serde_json::Value::Null);
                        handle_command(&mut stream, &mut env, cmd, &args);
                    }
                }
            } else if payload.starts_with("SupportedProtocols=") {
                // Handshake step 1: client sends supported protocols
                let _ = stream.write_all(&frame("UsingProtocol=2"));
            }
        }
    }
}

fn handle_command(
    stream: &mut std::net::TcpStream,
    env: &mut Environment,
    cmd: &str,
    args: &serde_json::Value,
) {
    use std::io::Write;

    match cmd {
        "Identify" => {
            let reply = serde_json::json!(["ReplyIdentify", {
                "identity": 1,
                "version": "GNU APL 2.0 (Rust)",
                "protocolVersion": 2
            }]);
            let _ = stream.write_all(&frame(&reply.to_string()));
        }
        "Connect" => {
            let reply = serde_json::json!(["ReplyConnect", {
                "remoteId": args["remoteId"],
                "protocolVersion": 2
            }]);
            let _ = stream.write_all(&frame(&reply.to_string()));
        }
        "GetWindowLayout" => {
            let reply = serde_json::json!(["ReplyGetWindowLayout", {
                "windows": []
            }]);
            let _ = stream.write_all(&frame(&reply.to_string()));
        }
        "Execute" => {
            let text = args["text"].as_str().unwrap_or("");
            let expr = text.trim();
            if expr.is_empty() {
                return;
            }
            // System commands ( )… and ]… ) run locally, exactly like in a
            // session; their output is "system command output" (type 4).
            if expr.starts_with(')') || expr.starts_with(']') {
                let cmd = expr.chars().skip(1).collect::<String>();
                if let Some(lines) = apl::sysvars::syscmd(&cmd, env) {
                    let out = lines.join("\n");
                    if !out.is_empty() {
                        let output = serde_json::json!(["AppendSessionOutput", {
                            "result": out,
                            "group": 0,
                            "type": 4
                        }]);
                        let _ = stream.write_all(&frame(&output.to_string()));
                    }
                }
                return;
            }
            match env.eval_line(expr) {
                Ok(Some(v)) => {
                    // Same display rules as the REPL, so boxing shows here too.
                    let pp = apl::sysvars::get_pp(env).unwrap_or(10);
                    let boxing = apl::sysvars::get_boxing(env);
                    let result = render_session_value(&v, pp, boxing);
                    let output = serde_json::json!(["AppendSessionOutput", {
                        "result": result,
                        "group": 0,
                        "type": 2
                    }]);
                    let _ = stream.write_all(&frame(&output.to_string()));
                }
                Ok(None) => {
                    // Assignment: APL produces no output for it — stay silent.
                }
                Err(e) => {
                    let rich = AplError::from(e).with_source_line(expr.to_string());
                    let output = serde_json::json!(["AppendSessionOutput", {
                        "result": format!("ERROR {rich}"),
                        "group": 0,
                        "type": 5
                    }]);
                    let _ = stream.write_all(&frame(&output.to_string()));
                }
            }
        }
        _ => {
            // Unknown command — ignore
        }
    }
}

/// Ride mode: connect to a RIDE server (like stride) as a client.
/// Reads RIDE_INIT from environment to determine where to connect.
fn ride_mode() {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let ride_init = std::env::var("RIDE_INIT").unwrap_or_default();
    // Parse "CONNECT:host:port" format
    let parts: Vec<&str> = ride_init.split(':').collect();
    if parts.len() != 3 || parts[0] != "CONNECT" {
        eprintln!("apl --ride: RIDE_INIT must be in format CONNECT:host:port");
        eprintln!("  e.g., RIDE_INIT=CONNECT:localhost:4502");
        std::process::exit(1);
    }

    let host = parts[1];
    let port: u16 = parts[2].parse().unwrap_or(4502);
    let addr = format!("{host}:{port}");

    println!("APL RIDE client connecting to {addr}...");

    let mut stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("apl --ride: cannot connect to {addr}: {e}");
            std::process::exit(1);
        }
    };

    println!("Connected to RIDE server at {addr}");

    // Perform handshake
    if !perform_ride_handshake(&mut stream) {
        eprintln!("apl --ride: handshake failed");
        std::process::exit(1);
    }

    println!("Handshake complete. Waiting for commands...");

    // Initialize interpreter
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let _ = apl::plugin_system::init_plugins(
        &mut env.funcs,
        &mut std::collections::HashMap::new(),
        &mut env.hooks,
    );

    // Process commands from the server
    let mut buf = [0u8; 4096];
    let mut acc = Vec::new();

    loop {
        match stream.read(&mut buf) {
            Ok(0) => {
                println!("Server closed connection");
                break;
            }
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }

        // Process all complete frames
        loop {
            if acc.len() < 8 {
                break;
            }
            let frame_len = u32::from_be_bytes([acc[0], acc[1], acc[2], acc[3]]) as usize;
            if acc.len() < frame_len {
                break; // incomplete frame
            }
            let payload = String::from_utf8_lossy(&acc[8..frame_len]).to_string();
            acc.drain(0..frame_len);

            if payload.starts_with('[') {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload) {
                    if let Some(arr) = val.as_array() {
                        let cmd = arr[0].as_str().unwrap_or("");
                        let args = arr.get(1).cloned().unwrap_or(serde_json::Value::Null);
                        handle_command(&mut stream, &mut env, cmd, &args);
                    }
                }
            }
        }
    }
}

/// Perform the RIDE handshake as a client.
fn perform_ride_handshake(stream: &mut std::net::TcpStream) -> bool {
    use std::io::{Read, Write};

    // Step 1: Send SupportedProtocols=2
    if stream.write_all(b"SupportedProtocols=2").is_err() {
        return false;
    }

    // Step 2: Read UsingProtocol=2
    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf) {
        Ok(0) => return false,
        Ok(n) => n,
        Err(_) => return false,
    };
    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.contains("UsingProtocol=2") {
        return false;
    }

    // Step 3: Send ["Identify", {...}]
    let identify = serde_json::json!(["Identify", {
        "apiVersion": 1,
        "identity": 1
    }]);
    let identify_frame = frame(&identify.to_string());
    if stream.write_all(&identify_frame).is_err() {
        return false;
    }

    // Step 4: Read ["ReplyIdentify", {...}]
    let n = match stream.read(&mut buf) {
        Ok(0) => return false,
        Ok(n) => n,
        Err(_) => return false,
    };
    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.contains("ReplyIdentify") {
        return false;
    }

    // Step 5: Send ["Connect", {"remoteId":2}]
    let connect = serde_json::json!(["Connect", {"remoteId":2}]);
    let connect_frame = frame(&connect.to_string());
    if stream.write_all(&connect_frame).is_err() {
        return false;
    }

    // Step 6: Read ["ReplyConnect", {...}]
    let n = match stream.read(&mut buf) {
        Ok(0) => return false,
        Ok(n) => n,
        Err(_) => return false,
    };
    let response = String::from_utf8_lossy(&buf[..n]);
    if !response.contains("ReplyConnect") {
        return false;
    }

    true
}
