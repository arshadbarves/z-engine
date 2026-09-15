use std::collections::BTreeMap;

use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Debug)]
pub(super) struct Element {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub text: String,
    pub depth: usize,
}

/// XML structure with built-in escapes only; no DTD or custom entity expansion.
pub(super) fn elements(text: &str) -> Result<Vec<Element>, String> {
    let mut reader = Reader::from_str(text);
    let mut elements = Vec::<Element>::new();
    let mut stack = Vec::new();
    let mut roots = 0;
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                if stack.is_empty() {
                    roots += 1;
                }
                let name = String::from_utf8(start.local_name().as_ref().to_vec())
                    .map_err(|e| e.to_string())?;
                let mut attributes = BTreeMap::new();
                for attr in start.attributes() {
                    let attr = attr.map_err(|e| e.to_string())?;
                    let key =
                        String::from_utf8(attr.key.as_ref().to_vec()).map_err(|e| e.to_string())?;
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|e| e.to_string())?;
                    attributes.insert(key, value.into_owned());
                }
                elements.push(Element {
                    name,
                    attributes,
                    text: String::new(),
                    depth: stack.len(),
                });
                if !empty {
                    stack.push(elements.len() - 1);
                }
            }
            Event::End(_) => {
                stack.pop().ok_or("Unexpected XML closing tag")?;
            }
            Event::Text(value) => {
                if let Some(index) = stack.last() {
                    elements[*index]
                        .text
                        .push_str(&value.decode().map_err(|e| e.to_string())?);
                } else if !value.iter().all(u8::is_ascii_whitespace) {
                    return Err("Text outside XML root element".into());
                }
            }
            Event::GeneralRef(value) => {
                let index = stack.last().ok_or("Reference outside XML root element")?;
                let reference = format!("&{};", value.decode().map_err(|e| e.to_string())?);
                let decoded = quick_xml::escape::unescape(&reference).map_err(|e| e.to_string())?;
                elements[*index].text.push_str(&decoded);
            }
            Event::CData(value) => {
                let index = stack.last().ok_or("CDATA outside XML root element")?;
                elements[*index]
                    .text
                    .push_str(&value.decode().map_err(|e| e.to_string())?);
            }
            Event::DocType(_) => {
                return Err("DTDs are not supported in project manifests".into());
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if roots != 1 || !stack.is_empty() {
        return Err("Expected exactly one closed XML root element".into());
    }
    Ok(elements)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predefined_numeric_escapes_and_cdata_preserve_text() {
        let parsed = elements(
            "<project><name>A &amp; B &#38; C &#x26; D &lt;&gt;&quot;&apos;</name>\
             <value><![CDATA[true]]></value></project>",
        )
        .unwrap();
        assert_eq!(parsed[1].text, "A & B & C & D <>\"'");
        assert_eq!(parsed[2].text, "true");
    }

    #[test]
    fn custom_entities_and_dtds_remain_rejected() {
        for input in [
            "<project>&custom;</project>",
            "<!DOCTYPE project [<!ENTITY custom 'value'>]><project>&custom;</project>",
            "&amp;<project/>",
        ] {
            assert!(elements(input).is_err(), "{input}");
        }
    }
}
