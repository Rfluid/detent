---
title: Check prior art before choosing protocol constants
date: 2026-11-23
source_task: phase-r
tags: [lesson, hidpp]
---

The scaffold picked swid 0xD for Detent; prior art showed G HUB uses 0xD (and Solaar 0xB, kernel 1). Changed to 0xE. Any constant that other software also picks (swid, uinput vendor IDs, D-Bus names, socket paths) should be checked against the prior-art map in docs/research/ first.
