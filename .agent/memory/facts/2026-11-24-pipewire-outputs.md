---
title: PipeWire output devices on the owner's laptop (listing, switching, moving streams)
date: 2026-11-24
source_task: radial volume dial + output fan (kde.volume.targets / .target)
tags: [pipewire, wireplumber, audio, kde, verified]
---

Verified on Fedora 44, PipeWire 1.6.11, WirePlumber 0.5 (owner's laptop, 2026-11-24):

- `pw-dump` lists 5 `Audio/Sink` nodes: HDMI 1/2/3, Speaker (one PCI card, device 54) and
  the AB13X USB DAC (device 80, the default). HDMI 2 and 3 have no monitor but still exist
  as sinks; the device's `EnumRoute` says `available: "no"` for them (HDMI 1 "yes", Speaker
  and the USB DAC "unknown"). Match a sink to its route with the node's
  `card.profile.device` against the route's `devices`; the active `Route` list wins.
- `node.nick` is the short name ("HDMI 1", "Speaker", "AB13X USB Audio"); the PCI
  descriptions all start "Core Ultra 200H/200V Series Processors HD Audio …".
- `device.icon-name` is `audio-card-analog` everywhere, `device.form-factor` unset.
- `Props.channelVolumes` are cubic: 0.357921 ↔ `wpctl` 0.71.
- Default sink: metadata `default`, key `default.audio.sink`, value `{ "name": <node.name> }`.
- `wpctl set-default <id>`: streams that follow the default (Brave) move at once.
- A stream pinned at creation (`pw-cat --target …`) does not follow; `pw-metadata -- <id>
  target.object -1` makes it follow the default (moved within ~0.7 s); `pw-metadata -d <id>
  target.object` sends it back to its pinned output. `-1` needs the `--`.
- `pw-cat` needs `--raw` to play `/dev/zero` (otherwise sndfile rejects it): handy for a
  silent test stream.

Not verified: Bluetooth sinks (A2DP vs headset profiles), whether Plasma shows an OSD when
the default output changes.
