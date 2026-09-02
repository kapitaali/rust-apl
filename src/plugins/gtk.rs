use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gtk4 as gtk;
use gtk::prelude::*;

use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

static GTK_WINDOW_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone)]
pub enum GtkEvent {
    ButtonClicked(String),
    EntryChanged(String),
    WindowClosed,
}

static GTK_EVENT_TX: Mutex<Option<Sender<GtkEvent>>> = Mutex::new(None);
static GTK_EVENT_RX: Mutex<Option<Arc<Mutex<Receiver<GtkEvent>>>>> = Mutex::new(None);

fn init_event_channel() {
    let mut tx_guard = GTK_EVENT_TX.lock().unwrap();
    let mut rx_guard = GTK_EVENT_RX.lock().unwrap();
    if tx_guard.is_none() {
        let (event_tx, event_rx) = mpsc::channel();
        *tx_guard = Some(event_tx);
        *rx_guard = Some(Arc::new(Mutex::new(event_rx)));
    }
}

fn get_event_sender() -> Option<Sender<GtkEvent>> {
    GTK_EVENT_TX.lock().unwrap().clone()
}

fn get_event_receiver() -> Option<Arc<Mutex<Receiver<GtkEvent>>>> {
    GTK_EVENT_RX.lock().unwrap().clone()
}

fn send_event(event: GtkEvent) {
    if let Some(tx) = get_event_sender() {
        let _ = tx.send(event);
    }
}

pub fn gtk_wait_timeout(ms: u64) -> bool {
    thread::sleep(Duration::from_millis(500));
    let start = Instant::now();
    let timeout = Duration::from_millis(ms);
    while GTK_WINDOW_COUNT.load(Ordering::SeqCst) > 0 {
        if start.elapsed() > timeout {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    true
}

#[derive(Debug, Clone)]
pub enum GtkMessage {
    ShowText(String),
    AppendText(String),
    Clear,
    ClearHistory,
    Close,
    Wait(u64),
    CreateCalculator,
    SetEntryText(String),
    GetEntryText(Sender<String>),
}

#[derive(Clone)]
pub struct GtkHandle {
    tx: Sender<(GtkMessage, Sender<()>)>,
}

impl GtkHandle {
    pub fn send(&self, msg: GtkMessage) -> AplResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send((msg, ack_tx))
            .map_err(|_| ErrorCode::DomainError)?;
        ack_rx.recv().map_err(|_| ErrorCode::DomainError)?;
        Ok(())
    }

    /// Get the current text from the GTK entry field
    pub fn get_entry_text(&self) -> AplResult<String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send((GtkMessage::GetEntryText(reply_tx), ack_tx))
            .map_err(|_| ErrorCode::DomainError)?;
        ack_rx.recv().map_err(|_| ErrorCode::DomainError)?;
        reply_rx.recv().map_err(|_| ErrorCode::DomainError)
    }
}

thread_local! {
    static GTK_HANDLE: RefCell<Option<GtkHandle>> = RefCell::new(None);
}

fn ensure_gtk_initialized() -> AplResult<GtkHandle> {
    GTK_HANDLE.with(|handle| {
        if let Some(h) = handle.borrow().as_ref() {
            return Ok(h.clone());
        }

        init_event_channel();

        let (tx, rx): (
            Sender<(GtkMessage, Sender<()>)>,
            Receiver<(GtkMessage, Sender<()>)>,
        ) = mpsc::channel();

        GTK_WINDOW_COUNT.fetch_add(1, Ordering::SeqCst);

        thread::spawn(move || {
            if let Err(e) = gtk::init() {
                eprintln!("GTK init failed: {}", e);
                GTK_WINDOW_COUNT.fetch_sub(1, Ordering::SeqCst);
                return;
            }

            let window = gtk::Window::new();
            window.set_title(Some("APL Calc"));
            window.set_default_size(900, 800);

            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 5);

            let text_view = gtk::TextView::new();
            text_view.set_editable(false);
            text_view.set_monospace(true);
            text_view.set_wrap_mode(gtk::WrapMode::WordChar);

            let scroll = gtk::ScrolledWindow::new();
            scroll.set_child(Some(&text_view));
            scroll.set_vexpand(true);
            vbox.append(&scroll);

            let status_label = gtk::Label::new(Some("⎕GTK ready"));
            vbox.append(&status_label);

            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Enter expression..."));
            entry.set_margin_start(5);
            entry.set_margin_end(5);
            vbox.append(&entry);

            let grid = gtk::Grid::new();
            grid.set_row_spacing(5);
            grid.set_column_spacing(5);
            grid.set_margin_start(5);
            grid.set_margin_end(5);
            grid.set_margin_bottom(5);
            vbox.append(&grid);

            window.set_child(Some(&vbox));

            let main_loop = gtk::glib::MainLoop::new(None, false);

            {
                let ml = main_loop.clone();
                window.connect_close_request(move |w| {
                    GTK_WINDOW_COUNT.fetch_sub(1, Ordering::SeqCst);
                    send_event(GtkEvent::WindowClosed);
                    // Let GTK destroy the window, then quit the main loop
                    w.destroy();
                    ml.quit();
                    gtk::glib::Propagation::Stop
                });
            }

            window.show();

            let buttons: Vec<(&str, i32, i32, i32)> = vec![
                // Row 0: Clear, backspace, special
                ("C", 0, 0, 1), ("CE", 1, 0, 1), ("⌫", 2, 0, 1), ("⋄", 3, 0, 1), ("⍝", 4, 0, 1), ("∇", 5, 0, 1), ("(", 6, 0, 1), (")", 7, 0, 1),
                // Row 1: Numbers + basic ops
                ("7", 0, 1, 1), ("8", 1, 1, 1), ("9", 2, 1, 1), ("÷", 3, 1, 1), ("+", 4, 1, 1), ("−", 5, 1, 1), ("○", 6, 1, 1), ("!", 7, 1, 1),
                // Row 2: Numbers + basic ops
                ("4", 0, 2, 1), ("5", 1, 2, 1), ("6", 2, 2, 1), ("×", 3, 2, 1), ("⋆", 4, 2, 1), ("|", 5, 2, 1), ("⌈", 6, 2, 1), ("⌊", 7, 2, 1),
                // Row 3: Numbers + comparison
                ("1", 0, 3, 1), ("2", 1, 3, 1), ("3", 2, 3, 1), ("=", 3, 3, 1), ("<", 4, 3, 1), ("≤", 5, 3, 1), (">", 6, 3, 1), ("≥", 7, 3, 1),
                // Row 4: Zero + logic
                ("0", 0, 4, 1), (".", 1, 4, 1), ("≠", 2, 4, 1), ("∧", 3, 4, 1), ("∨", 4, 4, 1), ("~", 5, 4, 1), ("⍪", 6, 4, 1), ("⊆", 7, 4, 1),
                // Row 5: Array ops
                ("⍳", 0, 5, 1), ("⍴", 1, 5, 1), ("↑", 2, 5, 1), ("↓", 3, 5, 1), ("⌽", 4, 5, 1), ("⍉", 5, 5, 1), ("⍋", 6, 5, 1), ("⍒", 7, 5, 1),
                // Row 6: Array ops
                ("∈", 0, 6, 1), ("⊂", 1, 6, 1), ("⊃", 2, 6, 1), ("≡", 3, 6, 1), ("≢", 4, 6, 1), ("⊤", 5, 6, 1), ("⊥", 6, 6, 1), ("→", 7, 6, 1),
                // Row 7: Misc
                ("⌹", 0, 7, 1), ("⍕", 1, 7, 1), ("⍎", 2, 7, 1), ("⍸", 3, 7, 1), ("⍬", 4, 7, 1), ("⍺", 5, 7, 1), ("⍵", 6, 7, 1), ("⌷", 7, 7, 1),
                // Row 8: Operators
                ("/", 0, 8, 1), ("\\", 1, 8, 1), ("¨", 2, 8, 1), ("⍨", 3, 8, 1), ("∘.", 4, 8, 1), ("⎕", 5, 8, 1), ("⍤", 6, 8, 1), ("⍣", 7, 8, 1),
                // Row 9: More primitives
                ("⊣", 0, 9, 1), ("⊢", 1, 9, 1), ("⍱", 2, 9, 1), ("⍲", 3, 9, 1), ("⍟", 4, 9, 1), ("⍙", 5, 9, 1), ("⍠", 6, 9, 1), ("⍫", 7, 9, 1),
                // Row 10: Actions
                ("Compute", 0, 10, 3), ("Plot", 3, 10, 3), ("Quit", 6, 10, 2),
                // Row 11: Actions
                ("History", 0, 11, 3), ("Clear", 3, 11, 3), ("⌫", 6, 11, 2),
            ];

            for (label, col, row_idx, width) in &buttons {
                let btn = gtk::Button::with_label(label);
                btn.set_hexpand(true);

                let entry_clone = entry.clone();
                let label_str = label.to_string();

                btn.connect_clicked(move |_| {
                    let current = entry_clone.text().to_string();
                    match label_str.as_str() {
                        "C" => {
                            entry_clone.set_text("");
                        }
                        "CE" => {
                            entry_clone.set_text("");
                        }
                        "⌫" => {
                            if !current.is_empty() {
                                let mut chars: Vec<char> = current.chars().collect();
                                chars.pop();
                                let new_text: String = chars.into_iter().collect();
                                entry_clone.set_text(&new_text);
                            }
                        }
                        "±" => {
                            if current.starts_with('-') {
                                entry_clone.set_text(&current[1..]);
                            } else if current.starts_with('−') {
                                entry_clone.set_text(&current[3..]);
                            } else if !current.is_empty() {
                                entry_clone.set_text(&format!("−{}", current));
                            }
                        }
                        "%" => {
                            entry_clone.set_text(&format!("{}÷100", current));
                            send_event(GtkEvent::ButtonClicked("Compute".to_string()));
                        }
                        "Compute" => {
                            send_event(GtkEvent::ButtonClicked("Compute".to_string()));
                        }
                        "Plot" => {
                            send_event(GtkEvent::ButtonClicked("Plot".to_string()));
                        }
                        "History" => {
                            send_event(GtkEvent::ButtonClicked("History".to_string()));
                        }
                        "Clear" => {
                            entry_clone.set_text("");
                        }
                        "Quit" => {
                            send_event(GtkEvent::ButtonClicked("Quit".to_string()));
                        }
                        _ => {
                            entry_clone.set_text(&format!("{}{}", current, label_str));
                            send_event(GtkEvent::ButtonClicked(label_str.clone()));
                        }
                    }
                });

                grid.attach(&btn, *col, *row_idx, *width, 1);
            }

            let text_view_clone = text_view.clone();
            let status_label_clone = status_label.clone();
            let entry_clone2 = entry.clone();
            let rx = Arc::new(rx);
            // Track display text locally to avoid GTK buffer.text() returning
            // stale/empty results on subsequent reads.
            let display_text = std::rc::Rc::new(std::cell::RefCell::new(String::new()));

            gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
                if let Ok((msg, ack_tx)) = rx.try_recv() {
                    let buffer = text_view_clone.buffer();
                    match msg {
                        GtkMessage::ShowText(text) => {
                            *display_text.borrow_mut() = text.clone();
                            buffer.set_text(&text);
                            status_label_clone.set_text("Ready");
                        }
                        GtkMessage::AppendText(text) => {
                            let mut acc = display_text.borrow_mut();
                            if acc.is_empty() {
                                *acc = text;
                            } else {
                                acc.push('\n');
                                acc.push_str(&text);
                            }
                            buffer.set_text(&acc);
                            status_label_clone.set_text("Updated");
                        }
                        GtkMessage::Clear => {
                            *display_text.borrow_mut() = String::new();
                            buffer.set_text("");
                            status_label_clone.set_text("Cleared");
                        }
                        GtkMessage::ClearHistory => {
                            *display_text.borrow_mut() = String::new();
                            buffer.set_text("");
                            status_label_clone.set_text("History cleared");
                        }
                        GtkMessage::Close => {
                            window.close();
                            GTK_WINDOW_COUNT.fetch_sub(1, Ordering::SeqCst);
                        }
                        GtkMessage::Wait(ms) => {
                            thread::sleep(Duration::from_millis(ms));
                        }
                        GtkMessage::CreateCalculator => {
                            status_label_clone.set_text("Calculator ready");
                        }
                        GtkMessage::SetEntryText(text) => {
                            entry_clone2.set_text(&text);
                        }
                        GtkMessage::GetEntryText(reply_tx) => {
                            let text = entry_clone2.text().to_string();
                            let _ = reply_tx.send(text);
                        }
                    }
                    let _ = ack_tx.send(());
                }
                gtk::glib::ControlFlow::Continue
            });

            main_loop.run();
        });

        let new_handle = GtkHandle { tx };
        *handle.borrow_mut() = Some(new_handle.clone());
        thread::sleep(Duration::from_millis(500));
        Ok(new_handle)
    })
}

pub fn quad_gtk(b: &ValueP) -> AplResult<ValueP> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let handle = ensure_gtk_initialized()?;

    let mut cmd = String::new();
    for c in cells {
        match c {
            crate::cell::Cell::Char(ch) => {
                if let Some(ch) = char::from_u32(*ch) {
                    cmd.push(ch);
                }
            }
            _ => return Err(ErrorCode::DomainError),
        }
    }

    let cmd = cmd.trim();
    if cmd == "close" {
        handle.send(GtkMessage::Close)?;
    } else if cmd == "clear" {
        handle.send(GtkMessage::Clear)?;
    } else if cmd == "clearhistory" {
        handle.send(GtkMessage::ClearHistory)?;
    } else if cmd == "calc" || cmd == "calculator" {
        handle.send(GtkMessage::CreateCalculator)?;
    } else if let Some(text) = cmd.strip_prefix("text ") {
        handle.send(GtkMessage::ShowText(text.to_string()))?;
    } else if let Some(text) = cmd.strip_prefix("append ") {
        handle.send(GtkMessage::AppendText(text.to_string()))?;
    } else if let Some(text) = cmd.strip_prefix("entry ") {
        handle.send(GtkMessage::SetEntryText(text.to_string()))?;
    } else if cmd == "getentry" {
        // Return the current entry field content
        let text = handle.get_entry_text()?;
        return Ok(ValueP::char_vector(
            &text.chars().map(|c| c as u32).collect::<Vec<_>>(),
        ));
    } else if let Some(ms_str) = cmd.strip_prefix("wait ") {
        let ms: u64 = ms_str.parse().map_err(|_| ErrorCode::DomainError)?;
        handle.send(GtkMessage::Wait(ms))?;
    } else {
        handle.send(GtkMessage::AppendText(cmd.to_string()))?;
    }

    Ok(ValueP::char_vector(
        &"gtk window".chars().map(|c| c as u32).collect::<Vec<_>>(),
    ))
}

pub fn quad_gtk_wait() {
    gtk_wait_timeout(u64::MAX);
}

pub fn quad_gtk_event() -> Option<GtkEvent> {
    if let Some(rx) = get_event_receiver() {
        if let Ok(guard) = rx.lock() {
            // Block with timeout to avoid tight loop
            guard.recv_timeout(Duration::from_millis(100)).ok()
        } else {
            None
        }
    } else {
        None
    }
}

pub struct GtkPlugin;

impl GtkPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AplPlugin for GtkPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "gtk".into(),
            version: "0.1.0".into(),
            description: "GTK4 GUI (⎕GTK)".into(),
        }
    }

    fn register(&self, reg: &mut PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕GTK".into(),
            ValueP::char_vector(
                &"gtk v0.1.0 (gtk4)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        reg.sysvars
            .insert("⎕GTK.WIDTH".into(), ValueP::scalar_from(Cell::Int(800)));
        reg.sysvars
            .insert("⎕GTK.HEIGHT".into(), ValueP::scalar_from(Cell::Int(600)));
        Ok(())
    }
}

#[cfg(all(test, feature = "plugin-gtk"))]
mod tests {
    use super::*;

    #[test]
    fn test_gtk_plugin_info() {
        let plugin = GtkPlugin;
        let info = plugin.info();
        assert_eq!(info.name, "gtk");
        assert!(info.description.contains("⎕GTK"));
    }

    #[test]
    fn test_gtk_plugin_register() {
        use crate::functions_def::FunctionTable;
        use std::collections::HashMap;

        let plugin = GtkPlugin;
        let mut func_table = FunctionTable::new();
        let mut sysvars = HashMap::new();
        let mut reg = PluginRegistrar {
            func_table: &mut func_table,
            sysvars: &mut sysvars,
        };

        plugin.register(&mut reg).unwrap();

        assert!(sysvars.contains_key("⎕GTK"));
        assert!(sysvars.contains_key("⎕GTK.WIDTH"));
        assert!(sysvars.contains_key("⎕GTK.HEIGHT"));
    }
}
