//! Display names (thin wrappers so commands share one source).

pub fn cid_name(cid: u16) -> &'static str {
    detent_hidpp_features::cid::name(cid)
}

/// Config name if the control has one (`gesture`), else the catalogue name.
pub fn control_label(cid: u16) -> String {
    match detent_hidpp_features::cid::config_name(cid) {
        Some(n) => n.to_string(),
        None => format!("0x{cid:04X}"),
    }
}
