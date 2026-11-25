---
title: "ADR 0001: Userspace daemon over HID++, not a kernel driver"
status: accepted
version: 1.0.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [adr]
---

# ADR 0001: Userspace daemon over HID++, not a kernel driver

**Context.** The kernel's `hid-logitech-hidpp` already drives the MX Master 4. What's missing is configurability: diversion, remapping, settings, plugins.

**Decision.** Build a userspace daemon that talks HID++ 2.0 over `/dev/hidraw*` and emits through `uinput`. Coexist with the kernel driver (distinct swid, ignore its traffic).

**Consequences.** Safe and fast to iterate on (crashes don't panic the kernel), no root at runtime (udev `uaccess`), portable across kernels. We must re-apply volatile state after reconnects and coexist with kernel-managed features. Only one HID++ configuration tool should run at a time (we warn about Solaar/logiops).
