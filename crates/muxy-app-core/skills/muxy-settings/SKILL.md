---
name: muxy-settings
description: Read and change the Muxy desktop app's settings with the `muxy config` commands, which save them and apply them to the running app. Use this whenever the user wants Muxy configured, such as its theme, fonts, terminal, shortcuts, sidebar, Quick Terminal, or browser.
---

# Muxy settings

You were started in Muxy's profile folder, next to its settings files. Change
settings only with `muxy config`. It checks every change and saves one only if
Muxy can still start with it, then has the running app apply it right away.

## The loop

1. Ask what the user wants changed, unless they already said.
2. Find the setting. `muxy config list` prints every setting with its current
   value, defaults included. Narrow it down with `grep`, or read one table with
   `muxy config get <table>`. `references/settings.md` explains what the
   settings do.
3. Say what you will change, from what to what, and wait for a yes before
   changing more than the user asked for.
4. Change it, then read it back:

   ```bash
   muxy config set appearance.dark_theme "Catppuccin Mocha"
   muxy config get appearance.dark_theme
   ```

5. Tell the user what changed, and how to undo it with `muxy config unset`.

## Commands

```bash
muxy config list
muxy config list --json
muxy config get terminal.font_size
muxy config get terminal
muxy config set terminal.font_size 14
muxy config set panes.new_pane_directory current
muxy config set keymap.new_tab cmd-t
muxy config unset terminal.font_size
muxy config reload
```

- Keys are dotted paths into `settings.toml`, or into `terminal.toml` after
  `terminal.`. A part holding a dot goes in double quotes, as some shortcut
  names do: `muxy config set 'keymap."settings.focus_navbar"' cmd-l`.
- Values are TOML: `true`, `14`, `0.5`, `["Menlo", "Monaco"]`. Anything that
  isn't TOML is taken as text, so names need no quotes of their own.
- `unset` returns a setting to its default.
- A rejected change prints why and leaves the files as they were. Read the
  message, fix the value, and try again instead of editing the file.
- `reload` is only needed after a file was edited by hand.

## What lives where

- Theme names: `appearance.dark_theme` and `appearance.light_theme`. Your own
  themes are files in `themes/` here; run `muxy config reload` after adding one.
- Shortcuts: `keymap.<action>`, written like `cmd-shift-t` or `ctrl-alt-left`.
  `muxy config get keymap` lists every action. `references/keyboard-shortcuts.md`
  lists the defaults.
- Terminal fonts, colors, cursor, padding and scrolling: `terminal.*`.
- Server settings, such as the default shell and history size, belong to the
  server: use `muxy settings get` and `muxy settings set <key> <value>`.
- Remote servers are read-only here. Point the user at **Settings → Devices**.

## Don't

- Don't edit, move or delete any file in this folder by hand. It also holds
  Muxy's state, keys, paired devices, backups and logs.
- Don't print or share the contents of files other than the settings.
- Don't restart or quit Muxy; changes apply without it.
