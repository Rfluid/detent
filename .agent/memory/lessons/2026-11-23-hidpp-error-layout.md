---
title: HID++ error report layout
date: 2026-11-23
source_task: scaffold
tags: [lesson, hidpp]
---

Error reports are `[rid, dev, 0xFF, orig_feature_index, orig_fn<<4|swid, error_code, ...]`. The original feature index sits in **byte 3**, where a normal frame's fn/swid byte is, so reading it from `Frame::params()` (byte 4+) is off by one. `classify()` reads raw bytes for this reason; a test covers it.
