//! A flat, human-oriented view of every binding in a config (`detentctl bindings`,
//! IPC `bindings.list`). Pure: no device, no daemon.

use serde::{Deserialize, Serialize};

use crate::schema::{Bind, Config, Do, Layer};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingRow {
    /// `base` or the profile name.
    pub profile: String,
    /// The layer the binding sits in (`[layer.<name>]`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// Full trigger as written (`gesture + left`; layer entries as `hold + target`).
    pub trigger: String,
    /// `keys` | `exec` | `action` | `button` | `device`.
    pub kind: String,
    /// The action's value (`Ctrl+W`, `kde.overview`, ...).
    pub action: String,
    /// Extra parameters (`menu = "main"`), TOML inline syntax; empty if none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub params: String,
}

fn describe(d: &Do) -> (String, String) {
    let pairs = [
        ("keys", &d.keys),
        ("exec", &d.exec),
        ("action", &d.action),
        ("button", &d.button),
        ("device", &d.device),
    ];
    pairs
        .iter()
        .find_map(|(k, v)| v.as_ref().map(|v| (k.to_string(), v.clone())))
        .unwrap_or_default()
}

fn params(d: &Do) -> String {
    d.params
        .iter()
        .map(|(k, v)| format!("{k} = {v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn push_all(
    out: &mut Vec<BindingRow>,
    profile: &str,
    binds: &[Bind],
    layers: &std::collections::BTreeMap<String, Layer>,
) {
    for b in binds {
        let (kind, action) = describe(&b.action);
        out.push(BindingRow {
            profile: profile.to_string(),
            layer: None,
            trigger: b.when.clone(),
            kind,
            action,
            params: params(&b.action),
        });
    }
    for (name, l) in layers {
        for (target, d) in &l.bindings {
            let (kind, action) = describe(d);
            out.push(BindingRow {
                profile: profile.to_string(),
                layer: Some(name.clone()),
                trigger: format!("{} + {target}", l.hold),
                kind,
                action,
                params: params(d),
            });
        }
    }
}

/// Every binding: base first (in file order, then layers by name), then each profile.
pub fn bindings(cfg: &Config) -> Vec<BindingRow> {
    let mut out = Vec::new();
    push_all(&mut out, "base", &cfg.bind, &cfg.layer);
    for (name, p) in &cfg.profile {
        push_all(&mut out, name, &p.bind, &p.layer);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn example_config_bindings() {
        let text = include_str!("../../../config/detent.example.toml");
        let cfg = crate::parse(text).unwrap();
        let rows = super::bindings(&cfg);
        assert!(!rows.is_empty());
        assert!(
            rows.iter()
                .all(|r| !r.kind.is_empty() && !r.action.is_empty())
        );
        assert!(rows.iter().any(|r| r.layer.as_deref() == Some("gesture")));
        assert!(rows.iter().any(|r| r.profile == "firefox"));
        // base rows come first
        let first_profile = rows.iter().position(|r| r.profile != "base").unwrap();
        assert!(rows[first_profile..].iter().all(|r| r.profile != "base"));
    }
}
