---
title: The shared terminal is zsh; unquoted variables don't word-split
date: 2026-11-24
source_task: radial output fan screenshots
tags: [shell, tooling]
---

A helper `shot(){ detent-radial --preview … $1 …; }` called with `shot "--pin --fan"` passed
`--pin --fan` as one argument ("unknown option --pin --fan"), because the terminal runs zsh,
which doesn't split unquoted parameters. The screenshot then showed no overlay at all and
looked like a crash. Use `${=1}` in zsh, separate arguments, or `bash -c`.

Also: screenshots of the real session capture whatever is on screen. Use `--preview --at
X,Y,OUTPUT --backdrop "#…"` (paints the whole output) and crop to the ring, or the nested
session (patterns/2026-11-23-nested-kwin-screenshots.md); check every crop before saving it.
