//! The StatusNotifierItem (ksni): icon name, status, tooltip and the dbusmenu, all derived
//! from the [`Snapshot`]. Menu callbacks only queue an [`Action`]; work happens elsewhere.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use ksni::menu::{MenuItem, StandardItem};

use crate::menu::{self, Action, Entry};
use crate::model::Snapshot;

pub type Handle = ksni::blocking::Handle<DetentTray>;

/// The last value pushed to the tray, so identical polls and events cost no D-Bus traffic
/// (aura's `apply_pending_status` pattern).
#[derive(Debug)]
pub struct Applied<T>(Option<T>);

impl<T> Default for Applied<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T: PartialEq + Clone> Applied<T> {
    /// `true` (and remembers `next`) when `next` differs from what was applied last.
    pub fn changed(&mut self, next: &T) -> bool {
        if self.0.as_ref() == Some(next) {
            return false;
        }
        self.0 = Some(next.clone());
        true
    }
}

pub struct DetentTray {
    /// SNI title: `Detent` (previews add the state, so panel order is predictable).
    pub title: String,
    pub snap: Snapshot,
    pub actions: Sender<Action>,
    /// XDG default config path, for "Open config…" while the daemon is down.
    pub default_config: PathBuf,
    /// Extra icon theme root (`$DETENT_TRAY_ICON_PATH`) for running uninstalled.
    pub icon_theme_path: String,
}

impl DetentTray {
    fn send(&self, a: Action) {
        let _ = self.actions.send(a);
    }
}

impl ksni::Tray for DetentTray {
    /// Left click opens the menu (Plasma's convention for menu-only items).
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "detent".into()
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }

    fn status(&self) -> ksni::Status {
        if self.snap.needs_attention() {
            ksni::Status::NeedsAttention
        } else {
            ksni::Status::Active
        }
    }

    fn icon_theme_path(&self) -> String {
        self.icon_theme_path.clone()
    }

    fn icon_name(&self) -> String {
        self.snap.icon_name().into()
    }

    fn attention_icon_name(&self) -> String {
        self.snap.icon_name().into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let (title, description) = self.snap.tooltip();
        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title,
            description,
        }
    }

    /// No StatusNotifier host (the tray started before Plasma at login, or plasmashell
    /// restarted). Stay alive: ksni registers the icon as soon as a host appears.
    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        eprintln!(
            "detent-tray: StatusNotifier host unavailable ({reason:?}); \
             the icon will appear when one registers"
        );
        true
    }

    fn watcher_online(&self) {
        eprintln!("detent-tray: StatusNotifier host is back; icon registered");
    }

    /// Middle click: pause / resume.
    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        if let Some(enable) = self.snap.middle_click() {
            self.send(Action::SetRemap(enable));
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        menu::build(&self.snap, &self.default_config)
            .into_iter()
            .map(|e| match e {
                Entry::Separator => MenuItem::Separator,
                Entry::Header(h) => StandardItem {
                    label: menu::plain_dbusmenu(&h),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                Entry::Item {
                    label,
                    enabled,
                    action,
                } => StandardItem {
                    label: menu::to_dbusmenu(&label),
                    enabled,
                    activate: Box::new(move |t: &mut Self| t.send(action.clone())),
                    ..Default::default()
                }
                .into(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applied_skips_identical_states() {
        let mut a = Applied::default();
        assert!(a.changed(&Snapshot::default()));
        assert!(
            !a.changed(&Snapshot::default()),
            "same state: no D-Bus push"
        );
        let s = Snapshot {
            daemon: crate::model::Daemon::Down { failed: true },
            update: None,
        };
        assert!(a.changed(&s));
    }
}
