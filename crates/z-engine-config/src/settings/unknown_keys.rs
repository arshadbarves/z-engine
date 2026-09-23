//! Detects misspelled keys. The known keys come from serializing the
//! default [`Settings`], so the check follows the schema without a list.

use serde_json::Value as Json;
use toml::Table;

use super::Settings;

/// Sections whose keys are user-chosen names rather than schema fields.
const NAMED_SECTIONS: [&str; 2] = ["hooks", "pricing"];

/// Top-level and section keys of `table` that the schema does not know.
pub(crate) fn unknown_keys(table: &Table) -> Vec<String> {
    let Ok(Json::Object(schema)) = serde_json::to_value(Settings::default()) else {
        return Vec::new();
    };
    let mut unknown = Vec::new();
    for (key, value) in table {
        if key == "schema" || NAMED_SECTIONS.contains(&key.as_str()) {
            continue;
        }
        match (schema.get(key), value) {
            (None, _) => unknown.push(key.clone()),
            (Some(Json::Object(fields)), toml::Value::Table(section)) => unknown.extend(
                section
                    .keys()
                    .filter(|field| !fields.contains_key(*field))
                    .map(|field| format!("{key}.{field}")),
            ),
            _ => {}
        }
    }
    unknown.sort();
    unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_typos_but_not_named_entries() {
        let table: Table = toml::from_str(
            r#"
schema = 2
modle = "x"
[model]
main = "m"
effrot = "high"
[hooks.MyEvent]
[pricing."a/b"]
input = 1.0
output = 2.0
[mcp.servers.fs]
command = "npx"
"#,
        )
        .unwrap();
        assert_eq!(unknown_keys(&table), vec!["model.effrot", "modle"]);
    }
}
