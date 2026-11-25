use super::*;

fn invoke(i: &Item) -> (String, String) {
    match i.action.steps().first() {
        Some(Step::Invoke(a, p)) => (a.clone(), p.clone()),
        other => panic!("not an invoke: {other:?}"),
    }
}

#[test]
fn parses_example_settings() {
    let json = r##"{"enable": true, "mode": "click", "size": "large", "haptic_ticks": true,
        "labels": "always", "tooltip_ms": 250, "theme": "dark", "colors": {"accent": "#ff6600"},
        "menus": {"main": {"items": [
            {"label": "Overview", "action": "kde.overview", "icon": "/nonexistent/x.svg"},
            {"label": "Shot", "exec": "spectacle -r", "text": "S"},
            {"label": "Copy", "keys": "Ctrl+C", "color": "#c43e1c"},
            {"label": "Vol", "action": "kde.volume", "invert": true},
            {"label": "Site", "open": "https://example.org/a'b"},
            {"label": "Term", "app": "org.kde.konsole.desktop"},
            {"label": "Sel all + copy", "steps": [{"keys": "Ctrl+A"}, {"delay_ms": 40}, {"keys": "Ctrl+C"}]},
            {"label": "AI", "items": [{"label": "A", "keys": "A"}, {"label": "B", "keys": "B"}]}
        ], "apps": {"firefox": {"items": [{"label": "Close tab", "keys": "Ctrl+W"}]}}}}}"##;
    let s = Settings::parse(json).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.labels, Labels::Always);
    assert_eq!(s.tooltip_ms, 250);
    // defaults: tooltips after 500 ms of hover
    let d = Settings::default();
    assert_eq!((d.labels, d.tooltip_ms), (Labels::Hover, 500));
    assert_eq!(s.layout.ring, 104.0);
    // theme = "dark" wins over the desktop; overrides apply to both palettes
    assert_eq!(s.colors(false).accent, 0xff6600ff);
    assert_eq!(s.colors(false).bubble, Colors::DARK.bubble);
    assert_eq!(s.light.accent, 0xff6600ff);
    let menu = s.menu("main").unwrap_or_else(|| panic!("no main"));
    let m = &menu.items;
    assert_eq!(m.len(), 8);
    assert_eq!(invoke(&m[0]), ("kde.overview".into(), "{}".into()));
    assert_eq!(m[1].icon, Icon::Text("S".into()));
    assert_eq!(m[2].color, Some(0xc43e1cff));
    assert_eq!(invoke(&m[3]).1, r#"{"invert":true}"#);
    assert_eq!(
        invoke(&m[4]).1,
        r#"{"cmd":"xdg-open 'https://example.org/a'\\''b'"}"#
    );
    assert_eq!(invoke(&m[5]).1, r#"{"cmd":"gtk-launch 'org.kde.konsole'"}"#);
    assert_eq!(
        m[6].action.steps(),
        vec![
            Step::Invoke("keys".into(), r#"{"keys":"Ctrl+A"}"#.into()),
            Step::DelayMs(40),
            Step::Invoke("keys".into(), r#"{"keys":"Ctrl+C"}"#.into()),
        ]
    );
    assert!(matches!(m[7].action, ItemAction::Folder(_)) && m[7].action.steps().is_empty());
    // per-app ring
    assert_eq!(menu.items_for(Some("firefox"))[0].label, "Close tab");
    assert_eq!(
        menu.items_for(Some("org.mozilla.firefox"))[0].label,
        "Close tab"
    );
    assert_eq!(menu.items_for(Some("konsole"))[0].label, "Overview");
    assert_eq!(menu.items_for(None)[0].label, "Overview");
}

#[test]
fn theme_follows_the_system_by_default() {
    let s = Settings::parse("{}").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.theme, Theme::System);
    assert_eq!(s.colors(true), Colors::DARK);
    assert_eq!(s.colors(false), Colors::LIGHT);
    let s = Settings::parse(r#"{"theme":"light"}"#).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.colors(true), Colors::LIGHT);
    let s =
        Settings::parse(r##"{"colors":{"edge":"#00000000"}}"##).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((s.light.edge, s.dark.edge), (0, 0));
    assert!(Settings::parse(r#"{"theme":"auto"}"#).is_err());
}

#[test]
fn adjustable_items() {
    let json = r#"{"menus":{"m":{"items":[
        {"label":"Volume","adjust":"kde.volume"},
        {"label":"Fine","adjust":"kde.brightness","step":5,"fine_step":2,"max":150,"invert_fine":true}
    ]}}}"#;
    // two in one ring: refused
    assert!(Settings::parse(json).is_err());
    let json = r#"{"menus":{"m":{"items":[
        {"label":"Volume","adjust":"kde.volume"},
        {"label":"More","items":[{"label":"B","adjust":"kde.brightness","step":5,"fine_step":2,"max":150,"invert_fine":true}]}
    ]}}}"#;
    let s = Settings::parse(json).unwrap_or_else(|e| panic!("{e}"));
    let items = &s.menu("m").unwrap_or_else(|| panic!("m")).items;
    let ItemAction::Adjust(a) = &items[0].action else {
        panic!("not adjustable")
    };
    assert_eq!(
        (a.source.as_str(), a.step, a.fine_step, a.max),
        ("kde.volume", 2, 1, 100)
    );
    assert!(!a.invert_fine);
    assert!(items[0].action.steps().is_empty());
    let ItemAction::Folder(f) = &items[1].action else {
        panic!("not a folder")
    };
    let ItemAction::Adjust(b) = &f[0].action else {
        panic!("not adjustable")
    };
    assert_eq!(
        (b.step, b.fine_step, b.max, b.invert_fine),
        (5, 2, 150, true)
    );
    for bad in [
        r#"{"label":"V","adjust":"volume"}"#,
        r#"{"label":"V","adjust":"kde.volume","step":0}"#,
        r#"{"label":"V","adjust":"kde.volume","max":151}"#,
        r#"{"label":"V","steps":[{"adjust":"kde.volume"}]}"#,
    ] {
        let j = format!(r#"{{"menus":{{"m":{{"items":[{bad}]}}}}}}"#);
        assert!(Settings::parse(&j).is_err(), "{bad}");
    }
}

#[test]
fn rejects_bad_settings() {
    // retired keys are accepted (older configs keep loading)
    assert!(Settings::parse(r#"{"mode":"click","dead_zone":24}"#).is_ok());
    assert!(Settings::parse(r#"{"size":"huge"}"#).is_err());
    assert!(Settings::parse(r##"{"colors":{"bubble":"#12"}}"##).is_err());
    assert!(Settings::parse(r##"{"colors":{"glow":"#123"}}"##).is_err());
    // 9 items in a ring
    let nine: Vec<String> = (0..9)
        .map(|i| format!(r#"{{"label":"{i}","keys":"A"}}"#))
        .collect();
    let j = format!(r#"{{"menus":{{"m":{{"items":[{}]}}}}}}"#, nine.join(","));
    assert!(Settings::parse(&j).is_err());
    // nested folder
    let j = r#"{"menus":{"m":{"items":[{"label":"F","items":[{"label":"G","items":[{"label":"x","keys":"A"}]}]}]}}}"#;
    assert!(Settings::parse(j).is_err());
    // item without any action
    assert!(Settings::parse(r#"{"menus":{"m":{"items":[{"label":"x"}]}}}"#).is_err());
}

/// The example config is the user-facing documentation: it must parse.
#[test]
fn example_config_parses() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../config/detent.example.toml"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let doc: toml::Value = toml::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
    let radial = &doc["plugins"]["radial"];
    let json = serde_json::to_string(radial).unwrap_or_else(|e| panic!("{e}"));
    let s = Settings::parse(&json).unwrap_or_else(|e| panic!("{e}"));
    let main = s.menu("main").unwrap_or_else(|| panic!("no main menu"));
    assert_eq!(main.items.len(), 8);
    assert!(matches!(main.items[7].action, ItemAction::Folder(_)));
    assert_eq!(main.items_for(Some("firefox")).len(), 4);
}

#[test]
fn colors() {
    assert_eq!(parse_color("#fff"), Ok(0xffffffff));
    assert_eq!(parse_color("814efa"), Ok(0x814efaff));
    assert_eq!(parse_color("#00000080"), Ok(0x00000080));
    assert!(parse_color("#ggg").is_err());
}
