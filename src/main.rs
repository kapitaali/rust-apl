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
                        if !handle_command(&mut stream, &mut env, cmd, &args) {
                            break;
                        }
                    }
                }
            } else if payload.starts_with("SupportedProtocols=") {
                // Handshake step 1: client sends supported protocols
                let _ = stream.write_all(&frame("UsingProtocol=2"));
            }
        }
    }
}

/// Send one framed RIDE message. Returns false when the peer is gone —
/// callers must end the session instead of writing into a broken pipe.
fn send_ride(stream: &mut std::net::TcpStream, payload: &str) -> bool {
    use std::io::Write;
    if let Err(e) = stream.write_all(&frame(payload)) {
        eprintln!("ride: write failed ({e}); closing session");
        return false;
    }
    if let Err(e) = stream.flush() {
        eprintln!("ride: flush failed ({e}); closing session");
        return false;
    }
    true
}

/// Full interpreter description for ReplyIdentify (protocol.md). RIDE reads
/// `arch[0]` and `version` with no guards, so both must be non-empty —
/// a minimal reply crashes the peer.
fn ride_identify() -> String {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    let user = std::env::var("USER").unwrap_or_default();
    serde_json::json!(["ReplyIdentify", {
        "apiVersion": 1,
        "Port": 0,
        "IPAddress": "",
        "Vendor": "rust-apl",
        "Language": "APL",
        "version": "GNU APL 2.0 (Rust)",
        "Machine": hostname,
        "arch": "Unicode/64",
        "Project": "CLEAR WS",
        "Process": "apl",
        "User": user,
        "pid": std::process::id() as i64,
        "token": "",
        "date": "",
        "platform": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
    }])
    .to_string()
}

/// Status-bar snapshot for InterpreterStatus. ⎕IO is live; the interpreter
/// has no trap/thread/SI accounting, so those report idle values.
fn send_interpreter_status(stream: &mut std::net::TcpStream, env: &mut Environment) -> bool {
    let io = env.get_io().unwrap_or(1);
    let msg = serde_json::json!(["InterpreterStatus", {
        "IO": io, "DQ": 0, "WA": 0, "SI": 0, "TRAP": 0, "ML": 1,
        "NumThreads": 1, "TID": 0, "CompactCount": 0, "GarbageCount": 0,
    }]);
    send_ride(stream, &msg.to_string())
}

/// Handle one RIDE message from the peer. Returns false when the session
/// must end (Exit/Disconnect, or the peer went away mid-write).
fn handle_command(
    stream: &mut std::net::TcpStream,
    env: &mut Environment,
    cmd: &str,
    args: &serde_json::Value,
) -> bool {
    match cmd {
        "Identify" => {
            // The peer announces itself and waits for our description.
            if !send_ride(stream, &ride_identify()) {
                return false;
            }
            // Advertise readiness: RIDE queues session lines until it sees
            // SetPromptType with type>0 (or HadError).
            send_ride(
                stream,
                &serde_json::json!(["SetPromptType", {"type": 1}]).to_string(),
            )
        }
        // "Connect" has no reply in the protocol — it just opens the
        // session (the old ReplyConnect was stride-only and real RIDE
        // clients ignore unknown replies, so stay silent like Kap does).
        "Connect" => true,
        // Legacy/setup queries we have nothing for; Kap ignores these too
        // and RIDE carries on regardless.
        "GetWindowLayout" => true,
        "GetSyntaxInformation" => true,
        "GetLog" => true,
        "SetPW" => true,
        "GetLanguageBar" => {
            let reply = serde_json::json!(["ReplyGetLanguageBar", {"entries": []}]);
            send_ride(stream, &reply.to_string())
        }
        "GetKeyboardLayout" => {
            let reply = serde_json::json!(["ReplyGetKeyboardLayout", {"keyMappings": {}}]);
            send_ride(stream, &reply.to_string())
        }
        "GetConfiguration" => {
            let names: Vec<String> = args["names"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|n| n.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let cfgs: Vec<serde_json::Value> = names
                .iter()
                .map(|n| serde_json::json!({"name": n, "value": ""}))
                .collect();
            let reply = serde_json::json!(["ReplyGetConfiguration", {"configurations": cfgs}]);
            send_ride(stream, &reply.to_string())
        }
        "Subscribe" => {
            let wants_status = args["status"]
                .as_array()
                .map(|a| a.iter().any(|s| s.as_str() == Some("statusfields")))
                .unwrap_or(false);
            if wants_status {
                send_interpreter_status(stream, env)
            } else {
                true
            }
        }
        "Exit" | "Disconnect" => false,
        "Execute" => {
            let text = args["text"].as_str().unwrap_or("");
            let expr = text.trim();
            if expr.is_empty() {
                return true;
            }
            // RIDE shows the line in the session only when it is echoed.
            if !send_ride(
                stream,
                &serde_json::json!(["EchoInput", {"input": text, "group": 0}]).to_string(),
            ) {
                return false;
            }
            // No prompt while evaluating; RIDE holds queued lines until
            // SetPromptType with type>0 comes back.
            if !send_ride(
                stream,
                &serde_json::json!(["SetPromptType", {"type": 0}]).to_string(),
            ) {
                return false;
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
                        if !send_ride(stream, &output.to_string()) {
                            return false;
                        }
                    }
                }
                return send_ride(
                    stream,
                    &serde_json::json!(["SetPromptType", {"type": 1}]).to_string(),
                );
            }
            let evaluated = match env.eval_line(expr) {
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
                    send_ride(stream, &output.to_string())
                }
                Ok(None) => {
                    // Assignment: APL produces no output for it — stay silent.
                    true
                }
                Err(e) => {
                    let rich = AplError::from(e).with_source_line(expr.to_string());
                    let output = serde_json::json!(["AppendSessionOutput", {
                        "result": format!("ERROR {rich}"),
                        "group": 0,
                        "type": 5
                    }]);
                    send_ride(stream, &output.to_string())
                }
            };
            if !evaluated {
                return false;
            }
            // Keep the status bar truthful when ⎕IO changes mid-session.
            send_interpreter_status(stream, env);
            send_ride(
                stream,
                &serde_json::json!(["SetPromptType", {"type": 1}]).to_string(),
            )
        }
        // Replies from a stride-like peer answering our own Identify.
        "ReplyIdentify" | "ReplyConnect" => true,
        _ => {
            println!("ride: ignoring unhandled message {cmd}");
            true
        }
    }
}

/// Ride mode: connect to a RIDE peer (Dyalog RIDE, or stride) as the interpreter.
/// Reads RIDE_INIT from environment to determine where to connect.
fn ride_mode() {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let ride_init = std::env::var("RIDE_INIT").unwrap_or_default();
    let Some((host, port)) = parse_ride_init(&ride_init) else {
        eprintln!("apl --ride: RIDE_INIT must be in format CONNECT:host:port");
        eprintln!("  e.g., RIDE_INIT=CONNECT:localhost:4502");
        std::process::exit(1);
    };
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

    // Handshake in frames, like Kap and Dyalog RIDE do: raw text would
    // desync the peer's framed parser and get the connection dropped.
    for hello in ["SupportedProtocols=2", "UsingProtocol=2"] {
        if stream.write_all(&frame(hello)).is_err() {
            eprintln!("apl --ride: handshake failed (peer went away)");
            std::process::exit(1);
        }
    }
    stream.flush().ok();

    // NOTE: no Identify of our own here. The peer announces itself first
    // and the main loop answers with ReplyIdentify. Sending our own
    // Identify upfront crashes Dyalog RIDE: its Identify handler indexes
    // arch[0] with no guards, and only ReplyIdentify carries full fields.
    println!("Handshake sent. Waiting for commands...");

    // Initialize interpreter
    let mut env = Environment::new();
    apl::sysvars::init_sysvars(&mut env);
    let _ = apl::plugin_system::init_plugins(
        &mut env.funcs,
        &mut std::collections::HashMap::new(),
        &mut env.hooks,
    );

    // Announce readiness up front (Kap does the same): RIDE queues
    // session lines until SetPromptType with type>0.
    if !send_ride(
        &mut stream,
        &serde_json::json!(["SetPromptType", {"type": 1}]).to_string(),
    ) {
        eprintln!("apl --ride: peer went away during handshake");
        std::process::exit(1);
    }

    // Unified framed read loop: the peer pipelines handshake + setup
    // messages, so every frame goes through one accumulator (handles
    // coalesced and split TCP segments). Anything without the RIDE magic
    // ends the session instead of desyncing it.
    let mut buf = [0u8; 4096];
    let mut acc = Vec::new();

    'session: loop {
        match stream.read(&mut buf) {
            Ok(0) => {
                println!("RIDE disconnected");
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
            if frame_len < 8 {
                eprintln!("ride: bad frame length {frame_len}; closing session");
                break 'session;
            }
            if acc.len() < frame_len {
                break; // incomplete frame
            }
            if &acc[4..8] != b"RIDE" {
                eprintln!("ride: bad frame magic; closing session");
                break 'session;
            }
            let payload = String::from_utf8_lossy(&acc[8..frame_len]).to_string();
            acc.drain(0..frame_len);

            if payload.starts_with('[') {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload) {
                    if let Some(arr) = val.as_array() {
                        let cmd = arr[0].as_str().unwrap_or("");
                        let args = arr.get(1).cloned().unwrap_or(serde_json::Value::Null);
                        if !handle_command(&mut stream, &mut env, cmd, &args) {
                            break 'session;
                        }
                    }
                }
            } else if payload == "SupportedProtocols=2" {
                // Peer's handshake opener; ours was already sent. Nothing to do.
            } else if payload.starts_with("UsingProtocol=") {
                // Protocol version agreed.
            } else {
                println!("ride: ignoring handshake text {payload:?}");
            }
        }
    }
}

/// Parse RIDE_INIT=CONNECT:host:port (set by the RIDE side when it spawns
/// the interpreter, or by hand for a listening RIDE). Split from the right
/// so IPv6 hosts survive.
fn parse_ride_init(ride_init: &str) -> Option<(String, u16)> {
    let rest = ride_init.strip_prefix("CONNECT:")?;
    let (host, port) = rest.rsplit_once(':')?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port.parse::<u16>().ok()?))
}

#[cfg(test)]
mod ride_tests {
    use super::*;

    #[test]
    fn ride_init_parses_host_port() {
        assert_eq!(
            parse_ride_init("CONNECT:localhost:4502"),
            Some(("localhost".to_string(), 4502))
        );
        assert_eq!(
            parse_ride_init("CONNECT:127.0.0.1:4502"),
            Some(("127.0.0.1".to_string(), 4502))
        );
    }

    #[test]
    fn ride_init_rejects_garbage() {
        assert_eq!(parse_ride_init(""), None);
        assert_eq!(parse_ride_init("SERVE:127.0.0.1:4502"), None);
        assert_eq!(parse_ride_init("CONNECT::4502"), None);
        assert_eq!(parse_ride_init("CONNECT:host:notaport"), None);
    }

    #[test]
    fn reply_identify_satisfies_ride() {
        // RIDE indexes arch[0] and reads version with no guards.
        let v: serde_json::Value = serde_json::from_str(&ride_identify()).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr[0].as_str().unwrap(), "ReplyIdentify");
        let body = &arr[1];
        assert_eq!(body["apiVersion"].as_i64().unwrap(), 1);
        assert!(!body["arch"].as_str().unwrap().is_empty());
        assert!(!body["version"].as_str().unwrap().is_empty());
    }

    #[test]
    fn handshake_frames_round_trip() {
        // Every handshake payload must survive framing with the RIDE magic.
        for payload in ["SupportedProtocols=2", "UsingProtocol=2"] {
            let f = frame(payload);
            assert_eq!(
                u32::from_be_bytes([f[0], f[1], f[2], f[3]]) as usize,
                f.len()
            );
            assert_eq!(&f[4..8], b"RIDE");
            assert_eq!(&f[8..], payload.as_bytes());
        }
    }
}
