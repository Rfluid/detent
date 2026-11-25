---
title: Plasma 6.8 SNI tray icons (verified)
date: 2026-11-24
source_task: detent-tray system tray icon
tags: [kde, plasma, tray, sni, icons]
---

Verified on Fedora 44, Plasma 6.8.3, qt6-qtsvg 6.11 (nested `kwin_wayland --virtual` +
plasmashell with a throwaway `XDG_CONFIG_HOME`, Breeze light and dark):

- A named SNI icon whose shapes carry `class="ColorScheme-Text" style="fill:currentColor"`
  next to `<style id="current-color-scheme">` is recoloured to the panel text colour.
- `IconThemePath` = a dir containing `hicolor/scalable/status/<name>.svg` resolves the icon
  without installing it or refreshing any icon cache.
- QtSvg 6.11 renders `<mask>` (the badge cut-outs) and group `opacity`.
- XML comments must not contain `--` (an icon comment with `--write-icons` failed to parse).
- dbusmenu labels use `_` for mnemonics (`__` = literal underscore); Plasma maps them to `&`.
- The notification server can't see `IconThemePath`: notification icons only resolve once
  the icons are installed in the hicolor theme.
- The owner's panels are auto-hide (`panelVisibility=1`): a full-screen spectacle shot of the
  live session doesn't show the tray. Use the nested session or D-Bus (`busctl --user call
  <item> /MenuBar com.canonical.dbusmenu GetLayout iias -- 0 -1 0`; `Event isvu -- <id>
  clicked s "" 0` triggers a row).
