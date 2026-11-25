# S08 Settings and appearance

The Settings tab saves preferences to the profile, the theme picker
recolors the app, and the sidebar and command palette work.

Start: run `$E g $VM reset`, then press new tab `$E press $VM new-tab`
and `$E g $VM wait 'muxy@'`. Tests continue from each other in order.

## T08.1 Settings opens and searches

1. Press settings `$E press $VM settings`. Wait: `sleep 2`.
2. **Shot** `t08-1-settings`. Check: a "Settings" tab in the main window with a search
   box and a list: General, Quick Terminal, Composer, Appearance, Keyboard,
   Commands, Terminal, Browser, Servers, Extensions, AI, Backup & Restore.
3. Press search `$E press $VM find`, `$E type $VM 'copy on select'`.
   Wait: `sleep 1`.
4. **Shot** `t08-1-search`. Check: the page shows "Terminal", a "Copy on select"
   row with a switch on its right, currently off (grey).

## T08.2 A preference is saved

1. Run `$E g $VM config terminal.toml`. Check: a line `copy_on_select = false`
   or no `copy_on_select` line at all.
2. Click the "Copy on select" switch: `$E click-element $VM 'Copy on select' --role toggle`.
3. **Shot** `t08-2-on`. Check: the switch is now on (blue).
4. Run `$E g $VM config terminal.toml`. Check: a line `copy_on_select = true`.
5. Close Settings: `$E press $VM close-window`. **Shot** `t08-2-closed`.
   Check: the Settings tab is gone; the main window remains.

## T08.3 Theme picker

1. Run `$E g $VM config settings.toml` and note the `dark_theme` and `light_theme` lines.
2. **Shot** `t08-3-before`.
3. Press theme picker `$E press $VM theme-picker`, `$E type $VM 'dracula'`.
   Wait: `sleep 1`. **Shot** `t08-3-list`. Check: rows "Dracula" and "Dracula+".
4. `$E key $VM Return`, wait `sleep 2`, then `$E key $VM Escape`.
5. **Shot** `t08-3-after`. Check: the window's colors changed to a dark purple-grey
   theme compared with `t08-3-before`.
6. Run `$E g $VM config settings.toml`. Check: `dark_theme` or `light_theme`
   is now `"Dracula"`.

## T08.4 Sidebar toggle

1. Press sidebar toggle `$E press $VM sidebar`.
2. **Shot** `t08-4-expanded`. Check: the sidebar is wide and shows the word
   `Home` next to its icon, and an "All Projects" filter at its top.
3. Press the same key again. **Shot** `t08-4-collapsed`. Check: the sidebar is
   back to a narrow column of icons.

## T08.5 Command palette runs a command

1. Press palette `$E press $VM palette`, `$E type $VM 'split right'`.
2. **Shot** `t08-5-palette`. Check: "Split Right" is listed with its shortcut.
3. `$E key $VM Return`. Wait: `sleep 2`.
4. **Shot** `t08-5-split`. Check: the tab now has two panes side by side.
