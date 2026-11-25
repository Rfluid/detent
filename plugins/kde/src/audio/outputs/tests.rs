use serde_json::json;

use super::*;

/// Trimmed from `pw-dump` on the author's laptop (2026-11-24): one PCI card with three HDMI
/// outputs (only HDMI 1 has a monitor) and a speaker, one USB DAC (the default), a
/// browser stream, a loopback's inner stream.
fn dump() -> Value {
    let route = |name: &str, dev: u64, available: &str| json!({ "name": name, "direction": "Output", "available": available, "devices": [dev] });
    let sink = |id: u64, dev: u64, pdev: u64, name: &str, nick: &str, vol: f64, mute: bool| {
        json!({
            "id": id, "type": "PipeWire:Interface:Node",
            "info": {
                "props": {
                    "media.class": "Audio/Sink", "node.name": name, "node.nick": nick,
                    "node.description": format!("Long description of {nick}"),
                    "device.id": dev, "card.profile.device": pdev,
                    "device.icon-name": "audio-card-analog",
                },
                "params": { "Props": [{ "channelVolumes": [vol, vol], "mute": mute }] }
            }
        })
    };
    json!([
        {
            "id": 54, "type": "PipeWire:Interface:Device",
            "info": { "props": { "media.class": "Audio/Device" }, "params": {
                "Route": [route("[Out] HDMI1", 2, "yes"), route("[Out] Speaker", 3, "unknown")],
                "EnumRoute": [
                    route("[Out] HDMI3", 0, "no"), route("[Out] HDMI2", 1, "no"),
                    route("[Out] HDMI1", 2, "yes"), route("[Out] Speaker", 3, "unknown"),
                ]
            }}
        },
        {
            "id": 80, "type": "PipeWire:Interface:Device",
            "info": { "props": { "media.class": "Audio/Device" }, "params": {
                "Route": [route("analog-output", 4, "unknown")],
                "EnumRoute": [route("analog-output", 4, "unknown")]
            }}
        },
        sink(47, 54, 0, "alsa_output.pci.HiFi__HDMI3__sink", "HDMI 3", 1.0, false),
        sink(70, 54, 1, "alsa_output.pci.HiFi__HDMI2__sink", "HDMI 2", 1.0, false),
        sink(71, 54, 2, "alsa_output.pci.HiFi__HDMI1__sink", "HDMI 1", 0.064, false),
        sink(72, 54, 3, "alsa_output.pci.HiFi__Speaker__sink", "Speaker", 0.091123, true),
        sink(76, 80, 4, "alsa_output.usb-Generic_AB13X.analog-stereo", "AB13X USB Audio", 0.357921, false),
        {
            "id": 133, "type": "PipeWire:Interface:Node",
            "info": { "props": { "media.class": "Stream/Output/Audio", "application.name": "Brave" } }
        },
        {
            "id": 140, "type": "PipeWire:Interface:Node",
            "info": { "props": { "media.class": "Stream/Output/Audio", "node.link-group": "loopback-1" } }
        },
        {
            "id": 141, "type": "PipeWire:Interface:Node",
            "info": { "props": { "media.class": "Stream/Input/Audio" } }
        },
        {
            "id": 30, "type": "PipeWire:Interface:Metadata",
            "props": { "metadata.name": "default" },
            "metadata": [
                { "subject": 0, "key": "default.audio.sink",
                  "value": { "name": "alsa_output.usb-Generic_AB13X.analog-stereo" } }
            ]
        }
    ])
}

#[test]
fn lists_plugged_in_sinks_with_short_names() {
    let outs = parse_outputs(&dump());
    let names: Vec<&str> = outs.iter().map(|o| o.name.as_str()).collect();
    // HDMI 2 and 3 have no monitor: hidden, like Plasma does.
    assert_eq!(names, ["HDMI 1", "Speaker", "AB13X USB Audio"]);
    let usb = &outs[2];
    assert_eq!((usb.id, usb.current, usb.percent), (76, true, Some(71)));
    assert_eq!(usb.icon, "audio-card");
    assert!(!outs[0].current);
    assert_eq!(outs[0].icon, "video-display");
    assert_eq!(
        (outs[1].icon, outs[1].muted, outs[1].percent),
        ("audio-speakers", true, Some(45))
    );
    assert_eq!(outs[0].percent, Some(40));
}

#[test]
fn json_for_the_radial_menu() {
    let out = &parse_outputs(&dump())[2];
    assert_eq!(
        out.to_json(),
        json!({ "id": "76", "name": "AB13X USB Audio",
                "description": "Long description of AB13X USB Audio", "icon": "audio-card",
                "percent": 71, "muted": false, "current": true })
    );
}

#[test]
fn names_and_icons() {
    assert_eq!(
        display_name(&json!({ "node.description": "D", "node.name": "n" })),
        "D"
    );
    assert_eq!(
        display_name(&json!({ "node.nick": "N", "node.description": "D" })),
        "N"
    );
    assert_eq!(display_name(&json!({})), "Output");
    let icon = |p: Value| icon_for(&p);
    assert_eq!(
        icon(json!({ "device.form-factor": "headset" })),
        "audio-headset"
    );
    assert_eq!(
        icon(json!({ "device.api": "bluez5", "node.name": "bluez_output.x" })),
        "audio-headphones"
    );
    assert_eq!(
        icon(json!({ "node.name": "alsa_output.x.HiFi__Headphones__sink" })),
        "audio-headphones"
    );
}

#[test]
fn moves_app_streams_only() {
    assert_eq!(streams_to_move(&dump()), [133]);
}

#[test]
fn inspect_id() {
    assert_eq!(
        parse_inspect_id("id 76, type PipeWire:Interface:Node\n  * node.name = \"x\""),
        Some(76)
    );
    assert_eq!(parse_inspect_id("nope"), None);
}

#[test]
fn no_device_info_keeps_the_sink() {
    let d = json!([{ "id": 5, "type": "PipeWire:Interface:Node",
        "info": { "props": { "media.class": "Audio/Sink", "node.name": "virtual" } } }]);
    let outs = parse_outputs(&d);
    assert_eq!(outs.len(), 1);
    assert_eq!((outs[0].percent, outs[0].current), (None, false));
    // no description: the short name stands in
    assert_eq!(outs[0].description, "virtual");
}
