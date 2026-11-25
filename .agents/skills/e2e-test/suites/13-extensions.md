# S13 Extensions

Extensions load from a folder, start disabled, turn on from Settings or the
CLI, reload with new versions, survive a broken reload, and unload while
keeping their files. Floating extensions draw over the window and their
background scripts restore what they saved.

Start: run `$E g $VM reset`, then:

1. Run `$E g $VM quit`.
2. Run `$E g $VM extension-fixture`. Check: it prints `extension fixtures ready`.
3. Run `$E g $VM floating-fixture`. Check: it prints `floating fixtures ready`.
4. Run `$E g $VM launch`.

Tests continue from each other in order.

`muxy extension list` prints one line per extension:
`<name>	<version>	<enabled|disabled>	<unpacked|marketplace>	<folder>`, then any
`load error:` lines. Commands marked "refused" must print a `muxy:` error and
exit non-zero; that is the expected result, so quote the message and keep going.

## T13.1 A store extension is listed

1. Run `$E g $VM muxy extension list`. Check: a line starting
   `focus-timer	99.0.0	disabled	marketplace`. No `hello-ext` line yet.

## T13.2 Load an unpacked folder from the CLI

1. Run `$E g $VM muxy extension load '~/e2e/hello-ext'`. Check: it prints `hello-ext`.
2. Run `$E g $VM muxy extension list`. Check: a line starting
   `hello-ext	0.1.0	disabled	unpacked` whose folder ends in `/e2e/hello-ext`.
3. Press palette `$E press $VM palette`, `$E type $VM 'hello: toggle'`.
4. **Shot** `t13-2-palette-off`. Check: no "Hello: Toggle Panel" row (a disabled
   extension adds no commands). `$E key $VM Escape`.

## T13.3 Turn it on in Settings

1. Press settings `$E press $VM settings`. Wait: `sleep 2`.
2. Click Extensions: `$E click-element $VM Extensions --role tab`. Wait: `sleep 2`.
3. **Shot** `t13-3-installed`. Check: the installed list has `hello-ext` and
   `focus-timer` rows.
4. Click the row: `$E click-element $VM hello-ext --role button`. Wait: `sleep 1`.
5. **Shot** `t13-3-details`. Check: `hello-ext` details with a "Disabled" badge,
   an "Unpacked" badge, and an "Enable" button.
6. Click Enable: `$E click-element $VM Enable --role button`. Wait: `sleep 3`.
   If a permissions confirmation appears, take **Shot** `t13-3-permissions`
   and click its confirming button by name (quote the name you used).
7. **Shot** `t13-3-enabled`. Check: the badge now says "Enabled" and the button
   "Disable".
8. Run `$E g $VM muxy extension list`. Check: `hello-ext	0.1.0	enabled	unpacked`.
9. Close Settings: `$E press $VM close-window`.

## T13.4 Its command opens its panel

1. Press palette `$E press $VM palette`, `$E type $VM 'hello: toggle'`.
2. **Shot** `t13-4-palette`. Check: a "Hello: Toggle Panel" row.
3. `$E key $VM Return`. Wait: `sleep 3`.
4. **Shot** `t13-4-panel`. Check: a panel titled "Hello" is open, and its page
   shows the word `Hello`.
5. Press palette `$E press $VM palette`, `$E type $VM 'hello: toggle'`,
   `$E key $VM Return`. Wait: `sleep 2`.
6. **Shot** `t13-4-closed`. Check: the Hello panel is gone.

## T13.5 Reload picks up a new version

1. Run `$E g $VM extension-edit 0.2.0`. Check: `hello-ext is now version 0.2.0`.
2. Run `$E g $VM muxy extension reload hello-ext`. Check: exits 0.
3. Run `$E g $VM muxy extension list`. Check: `hello-ext	0.2.0	enabled	unpacked`.

## T13.6 A broken reload keeps the running copy

1. Run `$E g $VM extension-edit broken`. Check: `package.json is now broken`.
2. Run `$E g $VM muxy extension reload hello-ext`. Check: refused; quote the reason.
3. Run `$E g $VM muxy extension list`. Check: still
   `hello-ext	0.2.0	enabled	unpacked`.
4. Press palette `$E press $VM palette`, `$E type $VM 'hello: toggle'`.
   **Shot** `t13-6-still-there`. Check: "Hello: Toggle Panel" is still listed.
   `$E key $VM Escape`.
5. Run `$E g $VM extension-edit 0.3.0`, then
   `$E g $VM muxy extension reload hello-ext`. Check: exits 0.
6. Run `$E g $VM muxy extension list`. Check: `hello-ext	0.3.0	enabled	unpacked`.

## T13.7 Disable and unload

1. Run `$E g $VM muxy extension disable hello-ext`. Check: exits 0.
2. Press palette `$E press $VM palette`, `$E type $VM 'hello: toggle'`.
   **Shot** `t13-7-disabled`. Check: no "Hello: Toggle Panel" row. `$E key $VM Escape`.
3. Run `$E g $VM muxy extension unload hello-ext`. Check: exits 0.
4. Run `$E g $VM muxy extension list`. Check: no `hello-ext` line;
   `focus-timer` is still listed.
5. Run `$E g $VM exists '~/e2e/hello-ext/package.json'`. Check: `yes` (files kept).
6. Run `$E g $VM muxy extension unload focus-timer`. Check: refused (unload is
   only for unpacked folders). Run the list again: `focus-timer` is still there.

## T13.8 A floating note is restored by its background script

1. Run `$E g $VM muxy extension load '~/e2e/sticky-notes'`. Check: prints `sticky-notes`.
2. Run `$E g $VM muxy extension enable sticky-notes`. Wait: `sleep 3`.
3. Press palette `$E press $VM palette`, `$E type $VM 'sticky notes: new note'`,
   `$E key $VM Return`. Wait: `sleep 3`.
4. **Shot** `t13-8-note`. Check: a yellow note card floats over the window, with
   a dot and an `x` at the right of its top bar.
5. Run `$E g $VM quit`, then `$E g $VM launch`. Wait: `sleep 5`.
6. **Shot** `t13-8-restored`. Check: a yellow note card is shown again
   (its background script reopened it from storage).

## T13.9 A detached panel animates over the window

1. Run `$E g $VM muxy extension load '~/e2e/desk-pet'`, then
   `$E g $VM muxy extension enable desk-pet`. Wait: `sleep 3`.
2. Press palette `$E press $VM palette`, `$E type $VM 'desk pet'`.
   **Shot** `t13-9-palette`. Check: a "Desk Pet: Show or Hide" row.
3. `$E key $VM Return`. Wait: `sleep 3`.
4. **Shot** `t13-9-pet-a`. Wait: `sleep 3`. **Shot** `t13-9-pet-b`.
   Check: a cat emoji near the bottom of the window in both shots, at a
   different left-right position in `b` than in `a` (it walks).
5. Run `$E g $VM muxy extension status desk-pet`. Check: the first line shows
   `enabled`, and there is no `load error:` line.

Needs verification before first use:
- T13.3: the names `Extensions` (tab), `hello-ext` (button) and `Enable`
  (button) come from the source; confirm `click-element` finds each once with
  that role on macOS and Linux. Whether enabling asks to approve `notifications:write`.
- T13.4: where a `floating` panel opens (over the right edge is the default).
- T13.8/T13.9: whether detached panels draw on Wayland and X11 the same as on
  macOS. Clicking the note's text box isn't a control; this suite avoids typing
  into it.
