---
title: notify-rust hides why a notification closed unless you use the async wait
date: 2026-11-25
source_task: tray sponsor nudge (bins/detent-tray/src/sponsor.rs)
tags: [fact, tray, notifications, zbus]
---

notify-rust 4.17 (zbus backend, feature `z`):

- `NotificationHandle::wait_for_action(|action: &str|)` maps every `NotificationClosed`
  signal to the string `"__closed"`, dropping the reason (1 expired, 2 dismissed by the
  user, 3 closed by `CloseNotification`).
- `on_close(|reason: CloseReason|)` has the reason but never sees actions.
- `wait_for_action_async(|r: &ActionResponse|)` gets both: `ActionResponse::Custom(id)` or
  `ActionResponse::Closed(CloseReason)`. Drive it from a plain thread with
  `zbus::block_on` (public in zbus 5; notify-rust itself uses it). It returns after the
  first `ActionInvoked` or `NotificationClosed` for that id.

The wait blocks its thread until the server answers, and some servers never send
`NotificationClosed` on expiry, so always wait on a thread that has nothing else to do.
