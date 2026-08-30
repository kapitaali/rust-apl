//! GNU APL XML format reader/writer (Phase 8).
//!
//! Reads and writes workspace files in the format used by C++ GNU APL.
//! Reference: src/workspaces/CONTINUE.xml in the GNU APL source.

use crate::cell::Cell;
use crate::parser::Environment;
use crate::value::ValueP;
use std::collections::HashMap;

/// Parse a GNU APL XML workspace file.
pub fn load_gnu_xml(env: &mut Environment, name: &str) -> Result<(), String> {
    let path = format!("{}.xml", name);
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path, e))?;
    parse_gnu_xml(env, &text)
}

/// Save workspace in GNU APL XML format.
pub fn save_gnu_xml(env: &Environment, name: &str) -> Result<String, String> {
    let path = format!("{}.xml", name);
    let xml = generate_gnu_xml(env)?;
    std::fs::write(&path, xml).map_err(|e| format!("cannot write {}: {}", path, e))?;
    Ok(path)
}

fn generate_gnu_xml(env: &Environment) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("<?xml version='1.0' encoding='UTF-8' standalone='yes'?>\n\n");
    out.push_str("<!DOCTYPE Workspace\n[\n");
    out.push_str("    <!ELEMENT Workspace (Function*, Value*, Ravel*, SymbolTable,\n");
    out.push_str("                         Symbol*, Commands?, StateIndicator)>\n");
    out.push_str("    <!ATTLIST Workspace  wsid       CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  year       CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  month      CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  day        CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  hour       CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  minute     CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  second     CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  timezone   CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  saving_SVN CDATA #REQUIRED>\n");
    out.push_str("    <!ATTLIST Workspace  syntax     CDATA #IMPLIED>\n");
    out.push_str("]>\n\n");

    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let year = 1970 + secs / 31_557_600;
    let month = (secs % 31_557_600) / 2_628_000 + 1;
    let day = (secs % 2_628_000) / 86_400 + 1;
    let hour = (secs % 86_400) / 3600;
    let minute = (secs % 3600) / 60;
    let second = secs % 60;

    out.push_str(&format!(
        "<Workspace wsid=\"RUST-APL\" year=\"{}\" month=\"{}\" day=\"{}\" \
         hour=\"{}\" minute=\"{}\" second=\"{}\" timezone=\"3600\" \
         saving_SVN=\"no-svn\" syntax=\"1.11.3\">\n",
        year, month, day, hour, minute, second
    ));

    // Functions
    for name in env.funcs.names() {
        if let Some(func) = env.funcs.get(&name) {
            if let Some(dfu) = func.interpreted() {
                out.push_str(&format!(
                    "  <Function fid=\"0x{:X}\" tag=\"0x43080907\"/>\n",
                    name.as_str().as_ptr() as usize
                ));
            }
        }
    }

    // Values (simplified - just user variables)
    for name in env.var_names() {
        if name.starts_with('\u{2395}') {
            continue;
        }
        let val = env.get(&name).cloned().unwrap();
        let vid = name.as_str().as_ptr() as usize;
        let rk = val.rank();
        out.push_str(&format!(
            "  <Value flg=\"0x400\" vid=\"{}\" parent=\"-1\" rk=\"{}\"",
            vid, rk
        ));
        if rk > 0 {
            for r in 0..rk.min(8) {
                out.push_str(&format!(" sh-{}=\"{}\"", r, val.get_shape_item(r as i16)));
            }
        }
        out.push_str("/>\n");
    }

    // Ravels
    for name in env.var_names() {
        if name.starts_with('\u{2395}') {
            continue;
        }
        let val = env.get(&name).cloned().unwrap();
        let vid = name.as_str().as_ptr() as usize;
        out.push_str(&format!("  <Ravel vid=\"{}\" depth=\"0\" cells=\"", vid));
        emit_gnu_cells(&mut out, val.cells());
        out.push_str("\"/>\n");
    }

    // Symbol table
    out.push_str("  <SymbolTable size=\"0\">\n");
    for name in env.var_names() {
        if name.starts_with('\u{2395}') {
            continue;
        }
        out.push_str(&format!(
            "    <Symbol name=\"{}\" stack-size=\"1\">\n      <Variable vid=\"{}\"/>\n    </Symbol>\n",
            xml_escape(&name),
            name.as_str().as_ptr() as usize
        ));
    }
    out.push_str("  </SymbolTable>\n");

    // State indicator
    out.push_str("  <StateIndicator levels=\"0\">\n");
    for (fname, pc) in &env.call_stack {
        out.push_str(&format!(
            "    <SI-entry level=\"0\" pc=\"{}\" line=\"0\">\n      <Execute>\n        <UCS uni=\"{}\"/>\n      </Execute>\n    </SI-entry>\n",
            pc,
            xml_escape(fname)
        ));
    }
    out.push_str("  </StateIndicator>\n");

    out.push_str("</Workspace>\n");
    Ok(out)
}

fn emit_gnu_cells(out: &mut String, cells: &[Cell]) {
    let mut in_char_run = false;
    for cell in cells {
        match cell {
            Cell::Char(ch) => {
                if !in_char_run {
                    out.push('²');
                    in_char_run = true;
                }
                let c = char::from_u32(*ch).unwrap_or(' ');
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '⁰' => out.push_str("&#x2070;"),
                    _ => out.push(c),
                }
            }
            _ => {
                if in_char_run {
                    out.push('⁰');
                    in_char_run = false;
                }
                match cell {
                    Cell::Int(i) => {
                        out.push('⁴');
                        out.push_str(&i.to_string());
                    }
                    Cell::Float(f) => {
                        out.push('⁵');
                        out.push_str(&format!("{:?}", f));
                    }
                    Cell::Complex(c) => {
                        out.push('⁶');
                        out.push_str(&format!("{}J{}", c.re, c.im));
                    }
                    Cell::Pointer(_) => out.push_str("³0"),
                    Cell::Lval(_) => out.push_str("³0"),
                    Cell::Char(_) => unreachable!(),
                }
            }
        }
    }
    if in_char_run {
        out.push('⁰');
    }
}

fn parse_gnu_xml(env: &mut Environment, text: &str) -> Result<(), String> {
    // Parse Function elements
    for func_elem in extract_elements(text, "Function") {
        let attrs = parse_attrs(&func_elem);
        let _fid = attrs.get("fid").cloned().unwrap_or_default();
        // Functions are referenced by fid; we'd need a mapping to restore them
        // For now, skip function restoration from GNU XML
        let _ = func_elem;
    }

    // Parse Value elements to get shape info
    let mut vid_to_shape: HashMap<String, (usize, Vec<i64>)> = HashMap::new();
    for val_elem in extract_elements(text, "Value") {
        let attrs = parse_attrs(&val_elem);
        if let (Some(vid), Some(rk)) = (attrs.get("vid"), attrs.get("rk")) {
            let rk: usize = rk.parse().map_err(|_| "bad rk")?;
            let mut shape = Vec::new();
            for i in 0..8 {
                if let Some(s) = attrs.get(&format!("sh-{}", i)) {
                    shape.push(s.parse().map_err(|_| "bad sh")?);
                } else {
                    break;
                }
            }
            vid_to_shape.insert(vid.clone(), (rk, shape));
        }
    }

    // Parse Ravel elements
    let mut vid_to_cells: HashMap<String, Vec<Cell>> = HashMap::new();
    for ravel_elem in extract_elements(text, "Ravel") {
        let attrs = parse_attrs(&ravel_elem);
        if let Some(vid) = attrs.get("vid") {
            if let Some(cells_str) = attrs.get("cells") {
                let cells = parse_gnu_cells(cells_str)?;
                vid_to_cells.insert(vid.clone(), cells);
            }
        }
    }

    // Parse SymbolTable to map names to vids
    if let Some(st) = extract_element(text, "SymbolTable") {
        for sym_elem in extract_elements(st, "Symbol") {
            let attrs = parse_attrs(&sym_elem);
            if let Some(name) = attrs.get("name") {
                // Find the Variable child
                for var_elem in extract_elements(sym_elem, "Variable") {
                    let var_attrs = parse_attrs(&var_elem);
                    if let Some(vid) = var_attrs.get("vid") {
                        if let Some(cells) = vid_to_cells.get(vid) {
                            if let Some((rk, shape)) = vid_to_shape.get(vid) {
                                let val = if *rk == 0 && cells.len() == 1 {
                                    ValueP::scalar_from(cells[0].clone())
                                } else if !shape.is_empty() {
                                    let sh = crate::shape::Shape::from_dims(shape)
                                        .map_err(|e| format!("shape error: {:?}", e))?;
                                    ValueP::from_parts(sh, cells.clone())
                                        .map_err(|e| format!("from_parts: {:?}", e))?
                                } else {
                                    ValueP::int_vector(&[])
                                };
                                env.insert_var(name.clone(), val);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn parse_gnu_cells(input: &str) -> Result<Vec<Cell>, String> {
    let mut cells = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '²' {
            chars.next();
            let mut s = String::new();
            while let Some(c) = chars.next() {
                if c == '⁰' {
                    break;
                }
                s.push(c);
            }
            // Parse XML entities
            let mut i = 0;
            while i < s.len() {
                if s[i..].starts_with("&amp;") {
                    cells.push(Cell::Char('&' as u32));
                    i += 5;
                } else if s[i..].starts_with("&lt;") {
                    cells.push(Cell::Char('<' as u32));
                    i += 4;
                } else if s[i..].starts_with("&gt;") {
                    cells.push(Cell::Char('>' as u32));
                    i += 4;
                } else if s[i..].starts_with("&#x2070;") {
                    cells.push(Cell::Char('⁰' as u32));
                    i += 8;
                } else {
                    let c = s[i..].chars().next().unwrap();
                    cells.push(Cell::Char(c as u32));
                    i += c.len_utf8();
                }
            }
        } else if ch == '⁴' {
            chars.next();
            let mut num = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '-' || c == '+' {
                    num.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            cells.push(Cell::Int(
                num.parse().map_err(|_| format!("bad int: {}", num))?,
            ));
        } else if ch == '⁵' {
            chars.next();
            let mut num = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' {
                    num.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            cells.push(Cell::Float(
                num.parse().map_err(|_| format!("bad float: {}", num))?,
            ));
        } else if ch == '⁶' {
            chars.next();
            let mut re = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' {
                    re.push(c);
                    chars.next();
                } else if c == 'J' {
                    chars.next();
                    break;
                } else {
                    break;
                }
            }
            let mut im = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' {
                    im.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            cells.push(Cell::Complex(crate::types::APLComplex {
                re: re.parse().map_err(|_| format!("bad re: {}", re))?,
                im: im.parse().map_err(|_| format!("bad im: {}", im))?,
            }));
        } else if ch == '³' {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    chars.next();
                } else {
                    break;
                }
            }
        } else if ch == '⁰' {
            chars.next();
        } else if ch == '¹' {
            // Hex digit mode (for ⎕AV)
            chars.next();
            let mut hex = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_hexdigit() {
                    hex.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Ok(val) = u32::from_str_radix(&hex, 16) {
                if let Some(c) = char::from_u32(val) {
                    cells.push(Cell::Char(c as u32));
                }
            }
        } else {
            chars.next();
        }
    }
    Ok(cells)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn extract_element<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = text.find(&open)?;
    let end = text.find(&close)?;
    Some(&text[start + open.len()..end])
}

fn extract_elements<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let mut results = Vec::new();
    let open_start = format!("<{} ", tag);
    let open_simple = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut pos = 0;
    while pos < text.len() {
        let remaining = &text[pos..];
        let start = if let Some(i) = remaining.find(&open_start) {
            pos + i
        } else if let Some(i) = remaining.find(&open_simple) {
            pos + i
        } else {
            break;
        };
        let tag_end = match text[start..].find('>') {
            Some(i) => start + i,
            None => break,
        };
        if text.as_bytes().get(tag_end.wrapping_sub(1)) == Some(&b'/') {
            results.push(&text[start..tag_end + 1]);
            pos = tag_end + 1;
            continue;
        }
        let inner_start = tag_end + 1;
        if let Some(end) = text[inner_start..].find(&close) {
            let end = inner_start + end + close.len();
            results.push(&text[start..end]);
            pos = end;
        } else {
            break;
        }
    }
    results
}

fn parse_attrs(elem: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    if let Some(start) = elem.find(' ') {
        let rest = &elem[start + 1..];
        let end = rest.find('>').unwrap_or(rest.len());
        let attr_str = &rest[..end];
        let mut chars = attr_str.chars().peekable();
        while chars.peek().is_some() {
            while chars.peek() == Some(&' ') {
                chars.next();
            }
            let mut key = String::new();
            while let Some(&c) = chars.peek() {
                if c == '=' {
                    chars.next();
                    break;
                }
                if c.is_whitespace() {
                    break;
                }
                key.push(c);
                chars.next();
            }
            if key.is_empty() {
                break;
            }
            while chars.peek() == Some(&' ') {
                chars.next();
            }
            if chars.peek() != Some(&'"') {
                continue;
            }
            chars.next();
            let mut value = String::new();
            while let Some(c) = chars.next() {
                if c == '"' {
                    break;
                }
                value.push(c);
            }
            attrs.insert(key, value);
        }
    }
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gnu_cells_int() {
        let cells = parse_gnu_cells("⁴42").unwrap();
        assert_eq!(cells, vec![Cell::Int(42)]);
    }

    #[test]
    fn test_parse_gnu_cells_float() {
        let cells = parse_gnu_cells("⁵3.5").unwrap();
        assert_eq!(cells, vec![Cell::Float(3.5)]);
    }

    #[test]
    fn test_parse_gnu_cells_complex() {
        let cells = parse_gnu_cells("⁶1J2").unwrap();
        assert_eq!(cells, vec![Cell::complex(1.0, 2.0)]);
    }

    #[test]
    fn test_parse_gnu_cells_string() {
        let cells = parse_gnu_cells("²HELLO⁰").unwrap();
        assert_eq!(cells.len(), 5);
        assert_eq!(cells[0], Cell::Char('H' as u32));
        assert_eq!(cells[4], Cell::Char('O' as u32));
    }

    #[test]
    fn test_parse_gnu_cells_mixed() {
        let cells = parse_gnu_cells("⁴1⁴2⁴3").unwrap();
        assert_eq!(cells, vec![Cell::Int(1), Cell::Int(2), Cell::Int(3)]);
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("A&B"), "A&amp;B");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_extract_elements() {
        let text = "<Root><Item a=\"1\"/><Item b=\"2\">content</Item></Root>";
        let items = extract_elements(text, "Item");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_parse_attrs() {
        let elem = r#"<Variable vid="42" name="X"/>"#;
        let attrs = parse_attrs(elem);
        assert_eq!(attrs.get("vid"), Some(&"42".to_string()));
        assert_eq!(attrs.get("name"), Some(&"X".to_string()));
    }
}
