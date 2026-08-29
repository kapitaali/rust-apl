//! XML Archive format for workspace persistence.
//!
//! A simple XML-based format for )SAVE/)LOAD.

use crate::cell::Cell;
use crate::parser::Environment;
use crate::value::ValueP;
use std::collections::HashMap;
use std::path::PathBuf;

const XML_MAJOR: u32 = 1;
const XML_MINOR: u32 = 11;

pub fn save_xml(env: &Environment, name: &str) -> Result<String, String> {
    let path = PathBuf::from(format!("{}.xml", name));
    let xml = generate_xml(env)?;
    std::fs::write(&path, xml)
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(path.display().to_string())
}

pub fn load_xml(env: &mut Environment, name: &str) -> Result<String, String> {
    let path = PathBuf::from(format!("{}.xml", name));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    parse_xml(env, &text)?;
    Ok(path.display().to_string())
}

fn generate_xml(env: &Environment) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<Workspace major=\"{}\" minor=\"{}\">\n",
        XML_MAJOR, XML_MINOR
    ));

    // Variables
    let mut names = env.var_names();
    names.sort();
    for n in &names {
        if n.starts_with('⎕') {
            continue;
        }
        if let Some(v) = env.get(n) {
            if let Some(payload) = serialize_var(v) {
                out.push_str(&format!(
                    "  <Variable name=\"{}\" value=\"{}\"/>\n",
                    xml_escape(n),
                    xml_escape(&payload)
                ));
            }
        }
    }

    // Functions
    for fname in env.funcs.names() {
        let callable = env.funcs.get(&fname).expect("name came from names()");
        if let Some(f) = callable.interpreted() {
            if f.name.starts_with(crate::parser::DFNS_PREFIX) {
                continue;
            }
            out.push_str(&format!(
                "  <Function name=\"{}\">\n",
                xml_escape(&fname)
            ));
            out.push_str(&format!(
                "    <Header>{}</Header>\n",
                xml_escape(&reconstruct_header(f))
            ));
            for src in &f.source {
                out.push_str(&format!("    <Source>{}</Source>\n", xml_escape(src)));
            }
            out.push_str("  </Function>\n");
        }
    }

    out.push_str("</Workspace>\n");
    Ok(out)
}

fn parse_xml(env: &mut Environment, text: &str) -> Result<(), String> {
    let mut pos = 0;
    let bytes = text.as_bytes();

    while pos < bytes.len() {
        // Find next '<'
        if let Some(start) = text[pos..].find('<') {
            let start = pos + start;
            if let Some(end) = text[start..].find('>') {
                let end = start + end + 1;
                let tag = &text[start..end];

                if tag.starts_with("<Variable ") {
                    let attrs = parse_attributes(tag);
                    if let (Some(name), Some(value)) = (attrs.get("name"), attrs.get("value")) {
                        let v = deserialize_var(value)?;
                        env.set(name, v);
                    }
                } else if tag.starts_with("<Function ") {
                    let attrs = parse_attributes(tag);
                    if let Some(_name) = attrs.get("name") {
                        let mut header = String::new();
                        let mut source_lines = Vec::new();
                        // Parse until </Function>
                        pos = end;
                        while let Some(next_start) = text[pos..].find('<') {
                            let next_start = pos + next_start;
                            if let Some(next_end) = text[next_start..].find('>') {
                                let next_end = next_start + next_end + 1;
                                let next_tag = &text[next_start..next_end];
                                if next_tag == "</Function>" {
                                    pos = next_end;
                                    break;
                                } else if next_tag.starts_with("<Header>") {
                                    if let Some(header_end) = text[next_start..].find("</Header>") {
                                        let header_start = next_start + next_tag.len();
                                        let header_end = next_start + header_end;
                                        header = text[header_start..header_end].to_string();
                                        pos = header_end + "</Header>".len();
                                        continue;
                                    }
                                } else if next_tag.starts_with("<Source>") {
                                    if let Some(source_end) = text[next_start..].find("</Source>") {
                                        let source_start = next_start + next_tag.len();
                                        let source_end = next_start + source_end;
                                        source_lines.push(text[source_start..source_end].to_string());
                                        pos = source_end + "</Source>".len();
                                        continue;
                                    }
                                }
                                pos = next_end;
                            } else {
                                break;
                            }
                        }
                        crate::functions_def::define_function(&mut env.funcs, &header, &source_lines)
                            .map_err(|e| format!("error loading function {}: {}", header, e))?;
                        continue;
                    }
                }
                pos = end;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    Ok(())
}

/// Serialize a variable to a string payload
fn serialize_var(v: &ValueP) -> Option<String> {
    let cells = v.cells();
    if cells.iter().any(|c| c.is_pointer_cell()) {
        return None;
    }

    let kind = match cells.first()? {
        Cell::Int(_) => "AI",
        Cell::Float(_) => "AF",
        Cell::Char(_) => "AC",
        Cell::Complex(_) => "AX",
        _ => return None,
    };
    let kind = if kind == "AI" && cells.iter().any(|c| matches!(c, Cell::Float(_))) {
        "AF"
    } else {
        kind
    };

    let dims: Vec<String> = (0..v.rank() as usize)
        .map(|k| v.get_shape_item(k as i16).to_string())
        .collect();
    let vals: Vec<String> = cells
        .iter()
        .map(|c| match (kind, c) {
            ("AI", Cell::Int(i)) => Ok(i.to_string()),
            ("AF", Cell::Int(i)) => Ok((*i as f64).to_string()),
            ("AF", Cell::Float(f)) => Ok(f.to_string()),
            ("AC", Cell::Char(cp)) => Ok(cp.to_string()),
            ("AX", Cell::Complex(c)) => Ok(format!("{}J{}", c.re, c.im)),
            _ => Err(()),
        })
        .collect::<Result<_, _>>()
        .ok()?;
    Some(format!("{};{};{}", kind, dims.join(","), vals.join(",")))
}

fn deserialize_var(payload: &str) -> Result<ValueP, String> {
    let mut parts = payload.splitn(3, ';');
    let kind = parts.next().ok_or("corrupt var payload")?;
    let dim_str = parts.next().ok_or("corrupt var payload")?;
    let val_str = parts.next().ok_or("corrupt var payload")?;
    if matches!(kind, "AI" | "AF" | "AC" | "AX") {
        let dims = parse_dims(dim_str)?;
        let shape =
            crate::shape::Shape::from_dims(&dims).map_err(|e| format!("shape error: {:?}", e))?;
        let cells: Vec<Cell> = val_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| match kind {
                "AI" => s
                    .parse::<i64>()
                    .map(Cell::Int)
                    .map_err(|_| "bad int".to_string()),
                "AF" => s
                    .parse::<f64>()
                    .map(Cell::Float)
                    .map_err(|_| "bad float".to_string()),
                "AC" => s
                    .parse::<u32>()
                    .map(Cell::Char)
                    .map_err(|_| "bad char".to_string()),
                "AX" => {
                    let parts: Vec<&str> = s.split('J').collect();
                    if parts.len() != 2 {
                        return Err("bad complex".to_string());
                    }
                    let re = parts[0].parse::<f64>().map_err(|_| "bad complex re")?;
                    let im = parts[1].parse::<f64>().map_err(|_| "bad complex im")?;
                    Ok(Cell::Complex(crate::types::APLComplex::new(re, im)))
                }
                _ => unreachable!(),
            })
            .collect::<Result<_, _>>()?;
        return Ok(ValueP::from_parts(shape, cells).map_err(|e| format!("shape error: {:?}", e))?);
    }
    match kind {
        "I" => Ok(ValueP::scalar_from(Cell::Int(
            dim_str.parse().map_err(|_| "bad int")?,
        ))),
        "F" => Ok(ValueP::scalar_from(Cell::Float(
            dim_str.parse().map_err(|_| "bad float")?,
        ))),
        "C" => Ok(ValueP::scalar_from(Cell::Char(
            dim_str.parse().map_err(|_| "bad char")?,
        ))),
        _ => Err(format!("unknown var kind {}", kind)),
    }
}

fn parse_dims(s: &str) -> Result<Vec<i64>, String> {
    if s.is_empty() {
        return Ok(vec![]);
    }
    s.split(',')
        .map(|d| d.parse().map_err(|_| "bad dim".to_string()))
        .collect()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn parse_attributes(tag: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    if let Some(start) = tag.find('<') {
        if let Some(end) = tag.find('>') {
            let tag_content = &tag[start + 1..end];
            let tag_content = tag_content.trim_end_matches('/');
            let parts: Vec<&str> = tag_content.split_whitespace().collect();
            for part in &parts[1..] {
                if let Some(eq_pos) = part.find('=') {
                    let key = &part[..eq_pos];
                    let val = part[eq_pos + 1..].trim_matches('"');
                    attrs.insert(key.to_string(), val.to_string());
                }
            }
        }
    }
    attrs
}

fn reconstruct_header(f: &crate::functions_def::DefinedFunction) -> String {
    let mut header = String::new();
    if let Some(r) = &f.result {
        header.push_str(r);
        header.push('←');
    }
    header.push_str(&f.name);
    if let Some(l) = &f.arg_left {
        header.push(' ');
        header.push_str(l);
    }
    if let Some(r) = &f.arg_right {
        header.push(' ');
        header.push_str(r);
    }
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_roundtrip_simple() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        env.eval_line("X←42").unwrap();
        env.eval_line("Y←3.5").unwrap();
        env.eval_line("S←'HELLO'").unwrap();

        let path = save_xml(&env, "test_xml_roundtrip").unwrap();
        assert!(path.ends_with(".xml"));

        let mut env2 = Environment::new();
        crate::sysvars::init_sysvars(&mut env2);
        load_xml(&mut env2, "test_xml_roundtrip").unwrap();

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
    fn test_xml_escape() {
        assert_eq!(xml_escape("A&B"), "A&amp;B");
        assert_eq!(xml_escape("A<B"), "A&lt;B");
        assert_eq!(xml_escape("A>B"), "A&gt;B");
        assert_eq!(xml_escape("A\"B"), "A&quot;B");
    }

    #[test]
    fn test_parse_attributes() {
        let tag = r#"<Variable name="X" value="AI;;42"/>"#;
        let attrs = parse_attributes(tag);
        assert_eq!(attrs.get("name"), Some(&"X".to_string()));
        assert_eq!(attrs.get("value"), Some(&"AI;;42".to_string()));
    }

    #[test]
    fn test_xml_roundtrip_functions() {
        let mut env = Environment::new();
        crate::sysvars::init_sysvars(&mut env);
        crate::functions_def::define_function(&mut env.funcs, "INCR", &["⍵+1".to_string()])
            .unwrap();

        let path = save_xml(&env, "test_xml_fns").unwrap();
        let mut env2 = Environment::new();
        crate::sysvars::init_sysvars(&mut env2);
        load_xml(&mut env2, "test_xml_fns").unwrap();

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

        let path = save_xml(&env, "test_xml_complex").unwrap();
        let mut env2 = Environment::new();
        crate::sysvars::init_sysvars(&mut env2);
        load_xml(&mut env2, "test_xml_complex").unwrap();

        let c = env2.eval_line("C").unwrap().unwrap();
        assert_eq!(c.element_count(), 3);
        assert_eq!(c.cells()[0], Cell::complex(1.0, 2.0));
        assert_eq!(c.cells()[1], Cell::complex(2.0, 3.0));
        let _ = std::fs::remove_file(path);
    }
}
