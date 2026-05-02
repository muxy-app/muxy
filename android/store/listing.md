# Muxy Android — store listing copy

These are working copy stubs for any future Play Store / F-Droid submission. v1
ships via GitHub Releases (sideload only); update before any store submission.

## Title

Muxy

## Short description (80 chars max)

Remote-control Muxy, the macOS terminal multiplexer, from your Android device.

## Full description

Muxy for Android is a remote-control client for the Muxy desktop app on macOS.
Pair once over Tailscale or any trusted network, and you can:

- Browse projects, worktrees, and tabs from your phone or tablet
- Take over a terminal pane and see live output rendered with the Termux
  terminal core
- Type into the pane with a custom accessory bar tuned for terminal use:
  Esc, Tab, Ctrl/Shift/Alt/Cmd modifiers, paste, and an analog D-pad
- Run a full git workflow against the active project: stage, commit, push,
  pull, branches, worktrees, and pull-request creation
- Get a notification feed pulled from Muxy's notification store

You still run all your terminals on the Mac. Muxy for Android is a thin
remote, not a local terminal — your zsh/tmux/nvim stays on macOS where it
belongs.

## Privacy

Muxy for Android stores its pairing token encrypted by an Android Keystore
key on this device only. Nothing is uploaded to a Muxy server because
there is no Muxy server — every byte of terminal output flows directly
from your Mac over the network you control.

The desktop server speaks plain WebSocket (no TLS). Use only on a trusted
network: a VPN, Tailscale, or a private LAN you control.

## Categories

- Productivity / Developer Tools

## Tags

terminal, ssh, tmux, remote, developer, android, mac, macos, ghostty, termux

## Screenshots (TODO)

- 1080×2400 phone: connect screen, project list, terminal with accessory
  bar visible, VCS sheet, branches sheet, error report sheet
- 2560×1600 tablet: workspace with terminal, settings sheet

## Promo video (TODO)

90-second walkthrough: pair → open project → take over pane → type a git
command → push → switch branch.

## License notes (Play Store / F-Droid)

The Android binary is GPL-3.0 because it vendors Termux's terminal-emulator
and terminal-view. Source code is at github.com/muxy-app/muxy under the
`android/` directory. Releases attach the source tag (or commit) used to
build the APK alongside the artifact.
