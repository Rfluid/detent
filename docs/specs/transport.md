---
title: HID++ transport and discovery per OS
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/hidpp-transport/", "crates/device/src/requester.rs", "crates/device/src/receiver.rs", "bins/detentctl/src/direct.rs"]
owner: "@rfluid"
tags: [spec, transport, hidpp, portability]
---

# HID++ transport and discovery per OS

How Detent finds a Logitech device and moves HID++ frames to and from it on each OS.
Decision and scope: [ADR 0010](../adr/0010-portability-seams.md). Frame layout: [hidpp.md](hidpp.md).

## API (`detent-hidpp-transport`)

| Item | Linux | macOS / Windows |
|---|---|---|
| `Transport` trait | `send(&Frame)`, `recv(timeout) -> Option<Frame>` | same |
| `OsTransport` | `hidraw::HidrawTransport` | `hid::HidapiTransport` |
| `discover::logitech() -> Vec<DeviceInfo>` | sysfs scan of `/sys/class/hidraw`, every Logitech node | hidapi enumeration, HID++ long-report collections only |
| `DeviceInfo::node` | `/dev/hidrawN` | hidapi path (IOService path / device interface path) |
| `DeviceInfo::uniq` | `HID_UNIQ` (BT address) | HID serial number string, if any |
| `DeviceInfo::receiver` | Bolt PID + 0xFF00/0x0002 in the descriptor, not owned by `hid-logitech-dj`; the receiver's other interfaces are not listed | Bolt PID |

Callers are generic over `Transport` (`DirectRequester<OsTransport>`): static dispatch, no
`dyn`, a stack buffer per read. The daemon's data plane is Linux-only and keeps using
`HidrawTransport` directly (it polls the fd with its eventfd and evdev node).

## Which collection carries HID++

HID++ lives in vendor-defined top-level collections. Linux hidraw gives one node per HID
device with all report IDs on it, so nothing needs choosing there. Windows creates one
device per top-level collection, and hidapi on macOS lists one entry per collection, so
the backend must pick:

| Usage page | Usage | Reports | Seen on | Status |
|---|---|---|---|---|
| 0xFF00 | 0x0001 | 0x10 short (7 B) | USB receivers (Unifying, Bolt), USB / BT-classic devices | not opened |
| 0xFF00 | 0x0002 | 0x11 long (20 B) | same | **opened** |
| 0xFF43 | 0x0202 | 0x11 long (20 B) | Bluetooth LE devices; the only HID++ collection there | **opened** |
| 0xFF43 | 0x0602 | 0x11 long | wired G-series keyboards | opened (prior art only) |

`discover::is_hidpp_long_collection` encodes the "opened" rows. Requests always go out as
long reports (the requester never sends short ones), which every HID++ 2.0 device accepts.
Notifications a device sends as short reports arrive on the 0x0001 collection and are not
read; the portable commands (`devices`, `dump-*`, `get`, `set`, `reset`) don't need them.

**Verified (2026-11-25, owner's MX Master 4, `046D:B042`, Bluetooth, Linux):** the report
descriptor has a mouse collection (report 0x02), `06 43 FF 0A 02 02 A1 01 85 11 95 13`
(usage page 0xFF43, usage 0x0202, report 0x11, 19 data bytes in and out) and a haptics
collection on usage page 0x0E (reports 0x3C feature, 0x3D output). No report 0x10: short
HID++ reports do not exist on this device over Bluetooth. Fact file:
`.agent/memory/facts/2026-11-25-mx4-bt-report-descriptor.md`.

## Backends

**Linux: hidraw.** `O_RDWR | O_NONBLOCK | O_CLOEXEC`, one `read(2)` per report, `poll(2)`
for timeouts. Access through udev `uaccess` rules (`packaging/udev/`). Unchanged.

**macOS: hidapi, IOHIDManager backend.** `hidapi` 2.6 with `macos-shared-device`, which
calls `hid_darwin_set_open_exclusive(0)` so the device is not seized and macOS keeps
delivering pointer input. Reads carry the report ID as the first byte.
⚠ To verify on a Mac: enumeration shows the 0xFF43/0x0202 entry for the MX Master 4 over
Bluetooth; opening it needs no Input Monitoring (TCC) permission, or which prompt appears;
reads and writes work while the OS uses the mouse; what `serial_number` holds.

**Windows: hidapi, `windows-native` backend** (pure Rust over `windows-sys`, no C
toolchain). Each collection is its own device path (`…&Col02#…`); hidapi writes the
report as given and reads return the report ID first. Some stacks pad input reports to
the collection's longest input report, so `HidapiTransport` cuts each report back to the
length its report ID implies before parsing.
⚠ To verify on Windows: the MX Master 4 over Bluetooth LE shows up as 0xFF43/0x0202 and
over a Bolt receiver as 0xFF00/0x0002; shared open works while Logi Options+ runs (or
doesn't, and what the error says); write length requirements for the 20-byte long report.

## Prior art

| Source | License | Finding | Verdict |
|---|---|---|---|
| [Solaar `lib/hidapi/hidapi_impl.py`](https://github.com/pwr-Solaar/Solaar/blob/master/lib/hidapi/hidapi_impl.py) | GPL-2.0 (facts only) | macOS/Windows via the hidapi C library; calls `hid_darwin_set_open_exclusive(0)` on macOS because it opens a device more than once; does not filter by usage page, probes each path with report 0x10/0x11 instead; dedups by path | adopt shared open; adapt: filter by usage page instead of probing (cheaper, no writes to non-HID++ devices) |
| [openlogi-hid `transport.rs`](https://docs.rs/crate/openlogi-hid/0.6.6/source/src/transport.rs) | see crate | long-report collections 0xFF00/0x0002, 0xFF43/0x0202 (BLE, long only), 0xFF43/0x0602 (wired G keyboards); on Windows opens short and long collections and routes by report ID; up-converts short requests to long on long-only devices | adopt the collection table; reject the paired short handle for now (no portable command needs short notifications) |
| [hidapi-rs](https://github.com/ruabmbua/hidapi-rs) | MIT | `windows-native` backend in pure Rust; `macos-shared-device`; `DeviceInfo::usage_page/usage/bus_type` on macOS and Windows; several `HidApi` instances allowed | adopt |
| [libusb/hidapi discussion #403](https://github.com/libusb/hidapi/discussions/403) | n/a | Windows creates a logical HID device per top-level collection | confirms the table above |
| libratbag, logiops, kernel `hid-logitech-hidpp` | MIT / GPL | Linux only (hidraw / in-kernel); nothing on macOS or Windows | n/a |

## Not covered yet

- Hotplug off Linux (the daemon isn't there); `devices` lists what is present now.
- Receivers off Linux: `DeviceInfo::receiver` is set from the PID and `detentctl` expands
  Bolt pairings into slots as on Linux ([hidpp.md](hidpp.md#receivers-logi-bolt)), but
  that is only verified on Linux.
