//! Trigger grammar (docs/specs/config.md#triggers):
//!
//! ```text
//! trigger  := (modifier " + ")* target
//! target   := control ("." edge)?
//! edge     := press | release | tap | double_tap | hold "(" N "ms)" | up | down | left | right
//! ```
//! Examples: `thumb.press`, `gesture + left`, `gesture + thumb + wheel.up`,
//! `thumb.hold(300ms)`, `swipe.left`. Sequences (`a, b`) are planned (phase 6).

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeSpec {
    Press,
    Release,
    Tap,
    DoubleTap,
    Hold(Duration),
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub modifiers: Vec<String>,
    pub control: String,
    /// `None` means the control's default edge (press for buttons, rotate for wheels).
    pub edge: Option<EdgeSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TriggerError {
    #[error("empty trigger")]
    Empty,
    #[error("empty control name in `{0}`")]
    EmptyControl(String),
    #[error("unknown edge `{0}`")]
    UnknownEdge(String),
    #[error("sequences (`a, b`) are not supported yet")]
    SequenceUnsupported,
    #[error("modifier `{0}` cannot have an edge")]
    ModifierWithEdge(String),
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn parse_edge(s: &str) -> Result<EdgeSpec, TriggerError> {
    Ok(match s {
        "press" => EdgeSpec::Press,
        "release" => EdgeSpec::Release,
        "tap" => EdgeSpec::Tap,
        "double_tap" => EdgeSpec::DoubleTap,
        "hold" => EdgeSpec::Hold(Duration::from_millis(300)),
        "up" => EdgeSpec::Up,
        "down" => EdgeSpec::Down,
        "left" => EdgeSpec::Left,
        "right" => EdgeSpec::Right,
        other => {
            let ms = other
                .strip_prefix("hold(")
                .and_then(|r| r.strip_suffix("ms)"))
                .and_then(|n| n.trim().parse::<u64>().ok())
                .ok_or_else(|| TriggerError::UnknownEdge(other.to_string()))?;
            EdgeSpec::Hold(Duration::from_millis(ms))
        }
    })
}

impl Trigger {
    pub fn parse(s: &str) -> Result<Self, TriggerError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(TriggerError::Empty);
        }
        if s.contains(',') {
            return Err(TriggerError::SequenceUnsupported);
        }
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let target = parts.pop().ok_or(TriggerError::Empty)?;
        let mut modifiers = Vec::with_capacity(parts.len());
        for m in parts {
            if m.contains('.') {
                return Err(TriggerError::ModifierWithEdge(m.to_string()));
            }
            if !valid_name(m) {
                return Err(TriggerError::EmptyControl(s.to_string()));
            }
            modifiers.push(m.to_string());
        }
        let (control, edge) = match target.split_once('.') {
            Some((c, e)) => (c, Some(parse_edge(e)?)),
            None => (target, None),
        };
        if !valid_name(control) {
            return Err(TriggerError::EmptyControl(s.to_string()));
        }
        Ok(Self {
            modifiers,
            control: control.to_string(),
            edge,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_examples() {
        let t = Trigger::parse("gesture + left").unwrap();
        assert_eq!(t.modifiers, vec!["gesture"]);
        assert_eq!(t.control, "left");
        assert_eq!(t.edge, None);

        let t = Trigger::parse("thumb.hold(450ms)").unwrap();
        assert_eq!(t.edge, Some(EdgeSpec::Hold(Duration::from_millis(450))));

        let t = Trigger::parse("gesture + thumb + wheel.up").unwrap();
        assert_eq!(t.modifiers, vec!["gesture", "thumb"]);
        assert_eq!(t.edge, Some(EdgeSpec::Up));
    }

    #[test]
    fn rejects_bad() {
        assert_eq!(Trigger::parse(""), Err(TriggerError::Empty));
        assert_eq!(
            Trigger::parse("a, b"),
            Err(TriggerError::SequenceUnsupported)
        );
        assert!(matches!(
            Trigger::parse("thumb.wiggle"),
            Err(TriggerError::UnknownEdge(_))
        ));
        assert!(matches!(
            Trigger::parse("gesture.tap + left"),
            Err(TriggerError::ModifierWithEdge(_))
        ));
        assert!(matches!(
            Trigger::parse("gesture + "),
            Err(TriggerError::EmptyControl(_))
        ));
    }
}
