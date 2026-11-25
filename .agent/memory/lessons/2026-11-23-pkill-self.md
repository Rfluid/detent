---
title: pkill -f matches the test script itself
date: 2026-11-23
source_task: phase-4
tags: [lesson, testing]
---

A test command run as `zsh -lc "... pkill -9 -f target/debug/detent-kde ..."` killed its own shell,
because the shell's command line contains the pattern. Use `pkill -x detent-kde` (process name)
or `pgrep` with a stricter pattern.
