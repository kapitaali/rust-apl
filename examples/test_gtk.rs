fn main() {
    eprintln!("DEBUG: main started");
    if let Err(e) = gtk::init() {
        eprintln!("GTK init failed: {}", e);
        return;
    }
    eprintln!("GTK init succeeded");
    let w = gtk::Window::new();
    w.set_title(Some("test"));
    w.show();
    eprintln!("Window shown, running main loop");
    gtk::main();
    eprintln!("Main loop finished");
}
