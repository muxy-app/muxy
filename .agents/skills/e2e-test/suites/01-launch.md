# S01 Launch

The app, CLI and server are one build, the app starts on a clean profile, and
it starts its own server.

## T01.1 Build identity

1. Run `$E g $VM info`.
2. Check: the `muxy-app:`, `muxy:` and `muxy-server:` lines show the same
   JSON text (same version, compatibility and protocol).
3. Check: `session:` is `macos` on macOS, `wayland` on Wayland, `x11` on X11.

PASS when both checks hold. Evidence: the command output.

## T01.2 First launch

1. Run `$E g $VM reset`. It ends with `app running: pid ...; server: {...}`.
2. **Shot** `t01-2-launch`. Check all of these:
   - A Muxy window is open.
   - The sidebar's top icon is the Home (house) icon.
   - The middle of the window says "No tabs in Home" with a blue "New Tab"
     button.
   - The status bar's right end says "Server · Connected".

PASS when every point holds.

## T01.3 Server started by the app

1. Run `$E g $VM muxy server status`.
2. Check: it prints JSON with `"build"` and `"instance"`, and no `muxy:` error.
3. Run `$E g $VM muxy project list`.
4. Check: exactly one line, whose second column is `Home`.

PASS when both checks hold.

## T01.4 No errors at startup

1. Run `$E g $VM logs`.
2. Check: no line contains `panic`, and no line contains `ERROR` (upper case).
   Lines with `WARN` are fine.

PASS when the check holds. On FAIL, copy the matching lines into the report.
