//! `detentctl set ... --persist`: write one setting into the config file, preserving
//! formatting and comments (`toml_edit`). The watcher then reloads; desired already
//! matches, so the reload applies nothing new (docs/architecture/device-state.md).

use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, value};

/// `path` is a settable path (`dpi`, `smartshift.mode`, ...); `raw` the CLI value.
pub fn write(config: &Path, device_name: &str, path: &str, raw: &str) -> Result<(), String> {
    let text = match std::fs::read_to_string(config) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", config.display())),
    };
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| format!("{}: {e}", config.display()))?;
    edit(&mut doc, device_name, path, raw)?;
    let out = doc.to_string();
    // The result must still be a valid config, or we don't write it.
    detent_config::load(&out).map_err(|e| format!("edit would make the config invalid: {e}"))?;
    if let Some(dir) = config.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = config.with_extension("toml.tmp");
    std::fs::write(&tmp, out).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, config).map_err(|e| e.to_string())
}

fn edit(doc: &mut DocumentMut, device_name: &str, path: &str, raw: &str) -> Result<(), String> {
    let v = typed(raw);
    let devices = doc
        .entry("device")
        .or_insert_with(|| {
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        })
        .as_table_like_mut()
        .ok_or("`device` is not a table")?;
    // Reuse an existing `[device."<key>"]` that matches this device, else use its name.
    let key = devices
        .iter()
        .map(|(k, _)| k.to_string())
        .find(|k| {
            let k = k.to_lowercase();
            let n = device_name.to_lowercase();
            k == n || n.contains(&k) || k == "*"
        })
        .unwrap_or_else(|| device_name.trim_start_matches("Logitech ").to_string());
    let dev = devices
        .entry(&key)
        .or_insert(Item::Table(Table::new()))
        .as_table_like_mut()
        .ok_or("device entry is not a table")?;
    let mut parts = path.split('.');
    let first = parts.next().ok_or("empty path")?;
    match parts.next() {
        None => {
            dev.insert(first, value(v));
        }
        Some(field) => {
            let sub = dev.entry(first).or_insert_with(|| {
                Item::Value(toml_edit::Value::InlineTable(toml_edit::InlineTable::new()))
            });
            match sub {
                Item::Value(toml_edit::Value::InlineTable(t)) => {
                    t.insert(field, v);
                }
                Item::Table(t) => {
                    t.insert(field, value(v));
                }
                _ => return Err(format!("`{first}` is not a table in the config")),
            }
        }
    }
    Ok(())
}

fn typed(raw: &str) -> toml_edit::Value {
    match raw {
        "true" | "on" | "yes" => true.into(),
        "false" | "off" | "no" => false.into(),
        r => match r.parse::<i64>() {
            Ok(n) => n.into(),
            Err(_) => r.into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_preserve_comments_and_reuse_keys() {
        let text = "# mine\n[device.\"MX Master 4\"]\ndpi = 1600 # fast\nsmartshift = { mode = \"ratchet\" }\n";
        let mut doc: DocumentMut = text.parse().unwrap_or_default();
        edit(&mut doc, "MX Master 4", "dpi", "2000").unwrap_or_default();
        edit(&mut doc, "MX Master 4", "smartshift.mode", "free-spin").unwrap_or_default();
        edit(&mut doc, "MX Master 4", "haptics.enabled", "on").unwrap_or_default();
        let out = doc.to_string();
        assert!(out.contains("# mine"), "{out}");
        assert!(out.contains("dpi = 2000"), "{out}");
        assert!(out.contains("mode = \"free-spin\""), "{out}");
        assert!(out.contains("enabled = true"), "{out}");
        assert!(detent_config::load(&out).is_ok(), "{out}");
    }

    #[test]
    fn creates_section_for_new_file() {
        let mut doc = DocumentMut::new();
        edit(&mut doc, "MX Master 4", "dpi", "1200").unwrap_or_default();
        let out = doc.to_string();
        assert!(out.contains("[device.\"MX Master 4\"]"), "{out}");
        assert!(detent_config::load(&out).is_ok(), "{out}");
    }
}
