use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use gtk::glib::value::ToValue;
use gtk::prelude::*;
use gtk4 as gtk;

use crate::cell::Cell;
use crate::plugin_system::{AplPlugin, PluginInfo, PluginRegistrar};
use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

#[derive(Debug, Clone)]
pub enum GtkMessage {
    ShowText(String),
    ShowPlot(String),
    ShowTable(Vec<Vec<String>>),
    Close,
}

pub struct GtkHandle {
    tx: Sender<GtkMessage>,
}

impl GtkHandle {
    pub fn send(&self, msg: GtkMessage) -> AplResult<()> {
        self.tx.send(msg).map_err(|_| ErrorCode::DomainError)
    }
}

thread_local! {
    static GTK_HANDLE: RefCell<Option<Rc<GtkHandle>>> = RefCell::new(None);
}

fn ensure_gtk_initialized() -> AplResult<Rc<GtkHandle>> {
    GTK_HANDLE.with(|handle| {
        if let Some(h) = handle.borrow().as_ref() {
            return Ok(h.clone());
        }

        let (tx, rx): (Sender<GtkMessage>, Receiver<GtkMessage>) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));

        thread::spawn(move || {
            gtk::init().expect("Failed to initialize GTK");

            let window = gtk::Window::new();
            window.set_title(Some("⎕GTK — APL Window"));
            window.set_default_size(800, 600);

            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 5);

            let notebook = gtk::Notebook::new();
            vbox.append(&notebook);

            let status_label = gtk::Label::new(Some("⎕GTK ready"));
            vbox.append(&status_label);

            window.set_child(Some(&vbox));
            window.show();

            let window = Rc::new(window);
            let notebook = Rc::new(notebook);
            let status_label = Rc::new(status_label);

            gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                let guard = rx.lock().expect("GTK channel lock poisoned");
                match guard.try_recv() {
                    Ok(GtkMessage::ShowText(text)) => {
                        show_text(&notebook, &status_label, &text);
                        gtk::glib::ControlFlow::Continue
                    }
                    Ok(GtkMessage::ShowPlot(path)) => {
                        show_plot(&notebook, &status_label, &path);
                        gtk::glib::ControlFlow::Continue
                    }
                    Ok(GtkMessage::ShowTable(table)) => {
                        show_table(&notebook, &status_label, &table);
                        gtk::glib::ControlFlow::Continue
                    }
                    Ok(GtkMessage::Close) => {
                        window.close();
                        gtk::glib::ControlFlow::Break
                    }
                    Err(_) => gtk::glib::ControlFlow::Continue,
                }
            });

            let main_loop = gtk::glib::MainLoop::new(None, false);
            main_loop.run();
        });

        let rc_handle = Rc::new(GtkHandle { tx });
        *handle.borrow_mut() = Some(rc_handle.clone());
        Ok(rc_handle)
    })
}

fn show_text(notebook: &gtk::Notebook, status: &gtk::Label, text: &str) {
    let label = gtk::Label::new(Some(text));
    label.set_margin_top(10);
    label.set_margin_bottom(10);
    label.set_margin_start(10);
    label.set_margin_end(10);
    label.set_selectable(true);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&label));

    let tab_label = gtk::Label::new(Some("Text"));
    let _page = notebook.append_page(&scroll, Some(&tab_label));
    status.set_text("Text displayed");
}

fn show_plot(notebook: &gtk::Notebook, status: &gtk::Label, path: &str) {
    let picture = gtk::Picture::for_filename(path);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&picture));

    let tab_label = gtk::Label::new(Some("Plot"));
    let _page = notebook.append_page(&scroll, Some(&tab_label));
    status.set_text(&format!("Plot: {}", path));
}

fn show_table(notebook: &gtk::Notebook, status: &gtk::Label, table: &[Vec<String>]) {
    if table.is_empty() || table[0].is_empty() {
        return;
    }

    let cols = table[0].len();
    let rows = table.len();

    // Build column types vector (dynamic)
    let column_types: Vec<gtk::glib::types::Type> =
        (0..cols).map(|_| gtk::glib::types::Type::STRING).collect();

    let list_store = gtk::ListStore::new(&column_types[..]);

    for row in table {
        let iter = list_store.append();
        for (col_idx, cell) in row.iter().enumerate().take(cols) {
            list_store.set_value(&iter, col_idx as u32, &cell.to_value());
        }
    }

    let tree_view = gtk::TreeView::new();
    tree_view.set_model(Some(&list_store));

    for (i, cell) in table[0].iter().enumerate() {
        let column = gtk::TreeViewColumn::new();
        column.set_title(cell);

        let renderer = gtk::CellRendererText::new();
        column.pack_start(&renderer, true);
        column.add_attribute(&renderer, "text", i as i32);

        tree_view.append_column(&column);
    }

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&tree_view));

    let tab_label = gtk::Label::new(Some("Table"));
    let _page = notebook.append_page(&scroll, Some(&tab_label));
    status.set_text(&format!("Table: {}×{}", rows, cols));
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
    } else if let Some(text) = cmd.strip_prefix("text ") {
        handle.send(GtkMessage::ShowText(text.to_string()))?;
    } else if let Some(path) = cmd.strip_prefix("plot ") {
        handle.send(GtkMessage::ShowPlot(path.to_string()))?;
    } else {
        handle.send(GtkMessage::ShowText(cmd.to_string()))?;
    }

    Ok(ValueP::char_vector(
        &"gtk window".chars().map(|c| c as u32).collect::<Vec<_>>(),
    ))
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
