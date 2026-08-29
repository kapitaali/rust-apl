//! XML Archive format for workspace persistence.
//!
//! A pragmatic XML format for )SAVE/)LOAD. We iterate public APIs
//! (var_names(), funcs.names()) and emit cells in a compact text format
//! that survives round-trip.

use crate::cell::Cell;
use crate::parser::Environment;
use crate::value::ValueP;
use std::collections::HashMap;
use std::path::PathBuf;

/// Save workspace to XML file.
pub fn save_xml(env: &Environment, name: &str) -> Result<String, String> {
    let path = PathBuf::from(format!("{}.xml", name));
    let xml = generate_xml(env)?;
    std::fs::write(&path, xml).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(path.display().to_string())
}

/// Load workspace from XML file.
pub fn load_xml(env: &mut Environment, name: &str) -> Result<(), String> {
    let path = PathBuf::from(format!("{}.xml", name));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    parse_xml(env, &text)
}

fn generate_xml(env: &Environment) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<Workspace>\n");

    // Variables
    out.push_str("  <Variables>\n");
    for name in env.var_names() {
        if name.starts_with('⎕') {
            continue;
        }
        let val = env.get_var(&name).unwrap();
        out.push_str(&format!(
            "    <Variable name=\"{}\" kind=\"{}\" rank=\"{}\"",
            xml_escape(&name),
            value_kind(&val),
            val.rank()
        ));
        if val.rank() > 0 {
            out.push_str(" dims=\"");
            for r in 0..val.rank() {
                if r > 0 {
                    out.push(',');
                }
                out.push_str(&val.get_shape_item(r as i16).to_string());
            }
            out.push('"');
        }
        out.push_str(">");
        emit_cells(&mut out, val.cells());
        out.push_str("</Variable>\n");
    }
    out.push_str("  </Variables>\n");

    // Functions
    out.push_str("  <Functions>\n");
    for name in env.funcs.names() {
        if let Some(func) = env.funcs.get(&name) {
            if let Some(dfu) = func.interpreted() {
                out.push_str(&format!(
                    "    <Function name=\"{}\" result=\"{}\" left=\"{}\" right=\"{}\">\n",
                    xml_escape(&name),
                    xml_escape(&dfu.result.as_deref().unwrap_or("")),
                    xml_escape(&dfu.arg_left.as_deref().unwrap_or("")),
                    xml_escape(&dfu.arg_right.as_deref().unwrap_or(""))
                ));
                for src in &dfu.source {
                    out.push_str(&format!("      <Line>{}</Line>\n", xml_escape(src)));
                }
                out.push_str("    </Function>\n");
            }
        }
    }
    out.push_str("  </Functions>\n");

    out.push_str("</Workspace>\n");
    Ok(out)
}

/// Emit ravel cells as compact text: type-tag + values
/// Character mode: ²...⁰ wraps runs of characters
fn emit_cells(out: &mut String, cells: &[Cell]) {
    let mut in_char_run = false;
    for (i, cell) in cells.iter().enumerate() {
        match cell {
            Cell::Char(ch) => {
                if !in_char_run {
                    out.push_str("²");
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
                    out.push_str("⁰");
                    in_char_run = false;
                }
                match cell {
                    Cell::Int(i) => {
                        out.push_str("⁴");
                        out.push_str(&i.to_string());
                    }
                    Cell::Float(f) => {
                        out.push_str("⁵");
                        out.push_str(&format!("{:?}", f));
                    }
                    Cell::Complex(c) => {
                        out.push_str("⁶");
                        out.push_str(&format!("{}J{}", c.re, c.im));
                    }
                    Cell::Pointer(_) => {
                        out.push_str("³0");
                    }
                    Cell::Lval(_) => {
                        out.push_str("³0");
                    }
                    Cell::Char(_) => unreachable!(),
                }
            }
        }
    }
    if in_char_run {
        out.push_str("⁰");
    }
}

/// Value kind tag for XML attribute.
fn value_kind(v: &ValueP) -> &'static str {
    if v.rank() == 0 {
        match v.first_cell() {
            Some(Cell::Int(_)) => "int",
            Some(Cell::Float(_)) => "float",
            Some(Cell::Complex(_)) => "complex",
            Some(Cell::Char(_)) => "char",
            _ => "scalar",
        }
    } else {
        "array"
    }
}

fn parse_xml(env: &mut Environment, text: &str) -> Result<(), String> {
    // Parse Variables
    if let Some(section) = extract_element(text, "Variables") {
        for elem in extract_elements_with_tags(section, "Variable") {
            let attrs = parse_attributes(&elem);
            if let Some(name) = attrs.get("name") {
                let inner = extract_inner_content(&elem, "Variable").unwrap_or_default();
                let cells = parse_cells(inner)?;
                let dims: Vec<i64> = attrs
                    .get("dims")
                    .map(|d| d.split(',').filter_map(|s| s.parse().ok()).collect())
                    .unwrap_or_default();
                let val = if dims.is_empty() {
                    if cells.len() == 1 {
                        ValueP::scalar_from(cells[0].clone())
                    } else {
                        return Err(format!(
                            "scalar variable {} has {} cells",
                            name,
                            cells.len()
                        ));
                    }
                } else {
                    let shape = crate::shape::Shape::from_dims(&dims)
                        .map_err(|e| format!("shape error: {:?}", e))?;
                    ValueP::from_parts(shape, cells)
                        .map_err(|e| format!("from_parts error: {:?}", e))?
                };
                env.insert_var(xml_unescape(name), val);
            }
        }
    }

    // Parse Functions
    if let Some(section) = extract_element(text, "Functions") {
        for elem in extract_elements_with_tags(section, "Function") {
            let attrs = parse_attributes(&elem);
            if let Some(name) = attrs.get("name") {
                let result = attrs.get("result").cloned().unwrap_or_default();
                let left = attrs.get("left").cloned().unwrap_or_default();
                let right = attrs.get("right").cloned().unwrap_or_default();

                let mut header = String::new();
                if !result.is_empty() {
                    header.push_str(&result);
                    header.push_str("←");
                }
                header.push_str(name);
                if !right.is_empty() {
                    header.push(' ');
                    header.push_str(&right);
                }
                if !left.is_empty() {
                    header.push(' ');
                    header.push_str(&left);
                }

                let source_lines: Vec<String> = extract_element_children(&elem, "Line")
                    .iter()
                    .map(|s| xml_unescape(s.trim()))
                    .collect();

                if !source_lines.is_empty() {
                    crate::functions_def::define_function(&mut env.funcs, &header, &source_lines)
                        .map_err(|e| format!("error loading function {}: {}", header, e))?;
                }
            }
        }
    }

    Ok(())
}

/// Parse cells from content inside <Variable>...</Variable>
fn parse_cells(input: &str) -> Result<Vec<Cell>, String> {
    let mut cells = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '²' {
            // Start char mode
            chars.next();
            let mut char_str = String::new();
            while let Some(c) = chars.next() {
                if c == '⁰' {
                    break;
                }
                char_str.push(c);
            }
            // Parse XML entities
            let mut i = 0;
            let bytes = char_str.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'&' && i + 5 < bytes.len() {
                    if bytes[i + 1..i + 5] == *b"amp;" {
                        cells.push(Cell::Char('&' as u32));
                        i += 5;
                        continue;
                    } else if bytes[i + 1..i + 5] == *b"lt;" {
                        cells.push(Cell::Char('<' as u32));
                        i += 4;
                        continue;
                    } else if bytes[i + 1..i + 5] == *b"gt;" {
                        cells.push(Cell::Char('>' as u32));
                        i += 4;
                        continue;
                    }
                }
                let c = char_str[i..].chars().next().unwrap();
                cells.push(Cell::Char(c as u32));
                i += c.len_utf8();
            }
        } else if ch == '⁴' {
            // Integer
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
            // Float
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
            // Complex: reJimag
            chars.next();
            let mut re_str = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' {
                    re_str.push(c);
                    chars.next();
                } else if c == 'J' {
                    chars.next();
                    break;
                } else {
                    break;
                }
            }
            let mut im_str = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E' {
                    im_str.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            cells.push(Cell::Complex(crate::types::APLComplex {
                re: re_str.parse().map_err(|_| format!("bad re: {}", re_str))?,
                im: im_str.parse().map_err(|_| format!("bad im: {}", im_str))?,
            }));
        } else if ch == '³' {
            // Pointer / nested placeholder
            chars.next();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    chars.next();
                } else {
                    break;
                }
            }
        } else if ch == '⁰' {
            // End char mode (already handled)
            chars.next();
        } else {
            chars.next();
        }
    }

    Ok(cells)
}

// --- XML utilities ---

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
}

fn extract_element<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = text.find(&open)?;
    let end = text.find(&close)?;
    Some(&text[start + open.len()..end])
}

fn extract_element_children<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let mut results = Vec::new();
    let open = format!("<{} ", tag);
    let open_simple = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut pos = 0;
    while pos < text.len() {
        let remaining = &text[pos..];
        let start = if let Some(i) = remaining.find(&open) {
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
        let inner_start = if text.as_bytes().get(tag_end.wrapping_sub(1)) == Some(&b'/') {
            results.push("");
            pos = tag_end + 1;
            continue;
        } else {
            tag_end + 1
        };
        if let Some(end) = text[inner_start..].find(&close) {
            let end = inner_start + end;
            results.push(&text[inner_start..end]);
            pos = end + close.len();
        } else {
            break;
        }
    }
    results
}

/// Extract full elements (including opening/closing tags) for a given tag name.
fn extract_elements_with_tags<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
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

/// Extract inner content from a full element string.
fn extract_inner_content<'a>(elem: &'a str, tag: &str) -> Option<&'a str> {
    let start = elem.find('>')? + 1;
    let close = format!("</{}>", tag);
    let end = elem.rfind(&close)?;
    Some(&elem[start..end])
}

fn parse_attributes(elem: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    if let Some(start) = elem.find(' ') {
        let rest = &elem[start + 1..];
        let end = rest
            .find("/>")
            .or_else(|| rest.find('>'))
            .unwrap_or(rest.len());
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
    fn test_xml_escape() {
        assert_eq!(xml_escape("A&B"), "A&amp;B");
        assert_eq!(xml_escape("A<B"), "A&lt;B");
    }

    #[test]
    fn test_parse_attributes() {
        let tag = r#"<Variable name="X" kind="int" rank="0"/>"#;
        let attrs = parse_attributes(tag);
        assert_eq!(attrs.get("name"), Some(&"X".to_string()));
        assert_eq!(attrs.get("kind"), Some(&"int".to_string()));
    }

    #[test]
    fn test_xml_roundtrip_simple() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("X←42").unwrap();
        env.eval_line("Y←3.5").unwrap();

        let path = save_xml(&env, "test_rt").unwrap();
        let mut env2 = Environment::new();
        crate::sysvars::init_sysvars(&mut env2);
        load_xml(&mut env2, "test_rt").unwrap();

        assert_eq!(
            env2.eval_line("X+0").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(42))
        );
        assert_eq!(
            env2.eval_line("Y+0").unwrap().unwrap().first_cell(),
            Some(&Cell::Float(3.5))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_xml_roundtrip_function() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        crate::functions_def::define_function(&mut env.funcs, "INCR", &["⍵+1".to_string()])
            .unwrap();

        let path = save_xml(&env, "test_fn").unwrap();
        let mut env2 = Environment::new();
        crate::sysvars::init_sysvars(&mut env2);
        load_xml(&mut env2, "test_fn").unwrap();

        assert_eq!(
            env2.eval_line("INCR 5").unwrap().unwrap().first_cell(),
            Some(&Cell::Int(6))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_xml_roundtrip_complex() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("C←1J2 2J3 3J4").unwrap();

        let path = save_xml(&env, "test_cx").unwrap();
        let mut env2 = Environment::new();
        crate::sysvars::init_sysvars(&mut env2);
        load_xml(&mut env2, "test_cx").unwrap();

        let c = env2.eval_line("C").unwrap().unwrap();
        assert_eq!(c.element_count(), 3);
        assert_eq!(c.cells()[0], Cell::complex(1.0, 2.0));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_xml_roundtrip_string() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("S←'HELLO'").unwrap();

        let path = save_xml(&env, "test_s").unwrap();
        let mut env2 = Environment::new();
        crate::sysvars::init_sysvars(&mut env2);
        load_xml(&mut env2, "test_s").unwrap();

        let s = env2.eval_line("S").unwrap().unwrap();
        assert_eq!(s.cells()[0], Cell::Char('H' as u32));
        assert_eq!(s.cells()[4], Cell::Char('O' as u32));
        let _ = std::fs::remove_file(path);
    }
}
