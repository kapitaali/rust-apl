use gtk::prelude::*;
use gtk4 as gtk;

fn main() {
    gtk::init().expect("Failed to initialize GTK");

    let window = gtk::Window::new();
    window.set_title(Some("APL Calculator"));
    window.set_default_size(400, 500);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 5);
    vbox.set_margin_top(10);
    vbox.set_margin_bottom(10);
    vbox.set_margin_start(10);
    vbox.set_margin_end(10);

    let display = gtk::Entry::new();
    display.set_placeholder_text(Some("Enter APL expression..."));
    vbox.append(&display);

    let result_label = gtk::Label::new(Some("Result: "));
    result_label.set_selectable(true);
    result_label.set_xalign(0.0);
    vbox.append(&result_label);

    let grid = gtk::Grid::new();
    grid.set_row_spacing(5);
    grid.set_column_spacing(5);
    grid.set_hexpand(true);
    grid.set_vexpand(true);

    let buttons = [
        ("⍳", 0, 0, 1),
        ("+", 0, 1, 1),
        ("-", 0, 2, 1),
        ("×", 0, 3, 1),
        ("7", 1, 0, 1),
        ("8", 1, 1, 1),
        ("9", 1, 2, 1),
        ("÷", 1, 3, 1),
        ("4", 2, 0, 1),
        ("5", 2, 1, 1),
        ("6", 2, 2, 1),
        ("*", 2, 3, 1),
        ("1", 3, 0, 1),
        ("2", 3, 1, 1),
        ("3", 3, 2, 1),
        ("⍟", 3, 3, 1),
        ("0", 4, 0, 1),
        ("(", 4, 1, 1),
        (")", 4, 2, 1),
        ("=", 4, 3, 1),
        ("←", 5, 0, 1),
        ("C", 5, 1, 2),
        ("⍴", 5, 3, 1),
    ];

    for &(label, row, col, colspan) in &buttons {
        let btn = gtk::Button::with_label(label);
        btn.set_hexpand(true);
        btn.set_vexpand(true);
        grid.attach(&btn, col, row, colspan, 1);

        let display_clone = display.clone();
        let result_clone = result_label.clone();

        btn.connect_clicked(move |_| match label {
            "=" => evaluate(&display_clone, &result_clone),
            "C" => {
                display_clone.set_text("");
                result_clone.set_text("Result: ");
            }
            "←" => {
                let text = display_clone.text();
                if !text.is_empty() {
                    display_clone.set_text(&text[..text.len() - 1]);
                }
            }
            _ => {
                display_clone.set_text(&format!("{}{}", display_clone.text(), label));
            }
        });
    }

    vbox.append(&grid);

    let info = gtk::Label::new(Some("APL Calculator — uses apl Rust library"));
    info.set_margin_top(10);
    vbox.append(&info);

    window.set_child(Some(&vbox));
    window.show();

    let main_loop = gtk::glib::MainLoop::new(None, false);
    main_loop.run();
}

fn evaluate(display: &gtk::Entry, result_label: &gtk::Label) {
    let expr = display.text().to_string();
    if expr.is_empty() {
        return;
    }

    match apl::tokenizer::tokenize(&expr) {
        Ok(tokens) => match apl::parser::parse(&tokens) {
            Ok((ast, _)) => {
                let mut env = apl::parser::Environment::new();
                apl::sysvars::init_sysvars(&mut env);
                match env.eval(&ast) {
                    Ok(value) => {
                        result_label.set_text(&format!("Result: {:?}", value));
                    }
                    Err(e) => {
                        result_label.set_text(&format!("Error: {:?}", e));
                    }
                }
            }
            Err(e) => {
                result_label.set_text(&format!("Parse error: {:?}", e));
            }
        },
        Err(e) => {
            result_label.set_text(&format!("Tokenize error: {:?}", e));
        }
    }
}
