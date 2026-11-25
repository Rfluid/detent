# Security policy

Detent talks to your mouse over HID++, injects input through the OS, runs commands from your
config, and hosts plugins on a local socket. Bugs in those paths can matter, so please report
them privately.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting:
**[Report a vulnerability](https://github.com/Rfluid/detent/security/advisories/new)**
(the "Security" tab of the repository, then "Report a vulnerability").

Please don't open a public issue, pull request or discussion for a security problem.

Include what you can:

- the Detent version (`detentctl --version`), OS, desktop and device;
- what an attacker can do and what they need first (local user, another plugin, a crafted
  config file, a malicious HID device...);
- steps or a proof of concept.

Detent is maintained by one person in their spare time. You can expect an acknowledgement
within 7 days and a fix or a plan within 30 days for confirmed issues. You'll be credited in
the advisory unless you ask not to be.

## Supported versions

| Version | Supported |
|---|---|
| latest `0.x` release | yes |
| older releases | no: update first |

## What is in scope

- `detentd`: the HID++ parser and data plane, the IPC socket (`$XDG_RUNTIME_DIR/detent/`),
  the plugin bus (capabilities, leases, the escape chord), command execution from the config.
- `detentctl`, `detent-tray` and the native plugins (`detent-kde`, `detent-radial`).
- The installers (`install.sh`, `scripts/install.ps1`, `scripts/install.sh`) and the udev rule
  in `packaging/udev/`, which grants the logged-in user access to the mouse, `/dev/uinput`
  and the mouse's evdev nodes.

Out of scope: bugs in the mouse firmware or in the OS input stack itself (report those
upstream), and anything that needs root or physical access the attacker already has.

Release binaries for macOS and Windows are not code-signed in 0.1.0; that is known and
tracked, not a vulnerability.
