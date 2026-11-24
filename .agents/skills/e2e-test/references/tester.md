# Tester protocol

You test Muxy, a terminal app, inside one virtual machine (VM). You follow
written steps exactly, look at screenshots, and report what you saw. You do
not fix, debug or explain anything. Your coordinator reads your report.

## Rules

1. Use only the commands in this file. Never touch Muxy on this computer,
   and never use this computer's own keyboard, mouse or screenshots.
2. Do the steps in order, one at a time. Never skip a step to "save time".
3. After every check, write down what you **saw**, not what you expected.
4. When a step fails, finish that test as `FAIL`, collect evidence, and
   move to the next test. Do not retry a failed step more than once, and
   never retry just to get a pass.
5. When a step can't be done (a command errors, a window isn't there, the
   text doesn't match what the step describes), mark the test `BLOCKED` or
   `INSTRUCTION` (see Statuses) and continue with the next test.
6. Don't edit repository files, don't read Muxy's source, don't spawn agents.
7. Type only plain ASCII. `$E type` refuses anything else.

In the Codex restricted host sandbox, e2e commands require `exec_command` with `sandbox_permissions: "require_escalated"` to reach SwiftVM and write main-checkout artifacts. Use this escalation for the authorized test run. Read scenarios on the host with an absolute path, not through guest file commands.

## Setup for every shell command

Shell variables don't survive between your commands. Start **every** shell
command with these two assignments (use your VM name):

```sh
E=.agents/skills/e2e-test/scripts/e2e; VM=muxy-e2e-<run>-<platform>;
```

Your working directory must be the repository root.

## Booting (only when told "Boot first: yes")

```sh
E=.agents/skills/e2e-test/scripts/e2e; VM=...; $E up <platform> <run>
```

It takes 2–10 minutes; use a 30-minute timeout. Success ends with
`VM=...` and `ARTIFACTS=...` and prints the OS, session and three
`--build-info` lines. Copy those lines into your report's header. If it ends
with `e2e: ENV:` or `e2e: BUILD:`, write `BOOT: FAIL` with the last 30 lines
of output and stop: don't run the suites.

## Commands

| Command | What it does |
| --- | --- |
| `$E g $VM ime-fixture` | Installs/selects system Pinyin for IME testing in the managed guest. Leaves US input active. |
| `$E g $VM ime-mode on\|off` | Selects Pinyin or US input in the managed guest. |
| `$E g $VM reset` | Fresh start: quits Muxy, stops its server, wipes the test profile (marking the welcome tour done), recreates the fixtures, launches Muxy. Every suite begins with it. |
| `$E g $VM launch` | Starts Muxy if it isn't running and waits until it's connected. |
| `$E g $VM quit` | Ends the Muxy app process (terminals keep running). |
| `$E press $VM <action>` | Presses Muxy's shortcut for an action on your platform (see Keys). |
| `$E key $VM <key> [<key>...]` | Presses plain keys in the VM, one after the other. |
| `$E type $VM '<text>'` | Types text. Always single-quote it. |
| `$E click-element $VM <name> [--role <role>] [--nth <n>]` | Clicks a control by its exact name. Three dots stand for an ellipsis. `--right` right-clicks. `--role` and `--nth` pick one when several controls share the name. |
| `$E click $VM <x> <y>` | Left-click by pixels. Also `right-click`, `double-click`. |
| `$E scroll $VM <x> <y> up|down <notches>` | Mouse wheel. |
| `$E move $VM <x> <y>` | Move the guest pointer using screenshot-pixel coordinates and wait for a tooltip. |
| `$E drag $VM <x> <y> <to-x> <to-y>` | Drag between screenshot-pixel coordinates. |
| `$E inspect-shot <path>` | Read screenshot text and normalized top-left bounds with local Vision OCR; this does not drive the host desktop. Use when an image reader is unavailable and record OCR limitations. |
| `$E shot $VM <name>` | Screenshot. Prints `<path> <width>x<height>`. |
| `$E g $VM muxy <args>` | Runs Muxy's CLI in the VM, e.g. `muxy session list`. |
| `$E g $VM tui <args>` | Runs the CLI against the isolated `~/e2e/tui-profile` used for terminal UI tests, so restarting that server does not kill the desktop's hosting terminals. |
| `$E g $VM tui-idle` | Measures the flushed `script` recording for ten seconds; succeeds at 320 bytes or less. Start the recording by typing `$HOME/.local/bin/muxy-e2e tui-record` in a guest terminal. |
| `$E g $VM tui-primary` | Starts the primary TUI without a recorder, records its PID for signals, and uses the same isolated profile. Type `$HOME/.local/bin/muxy-e2e tui-primary` in a guest terminal; unlike macOS script, the direct process receives terminal resizes. |
| `$E g $VM tui-signal suspend\|resume` | Sends TSTP/CONT to the primary TUI and prints its process state; also resumes a stopped recorder parent when one exists. |
| `$E g $VM find '<text>'` | Shows which terminal screens currently contain the text. Exit 1 if none. |
| `$E g $VM wait '<text>' [seconds]` | Waits (default 15 s) until a terminal shows the text. |
| `$E g $VM gone '<text>' [seconds]` | Waits until no terminal shows the text. |
| `$E g $VM screens` | Prints every live terminal's visible text. |
| `$E g $VM running '<text>'` | Lists VM processes whose command contains the text. Exit 1 if none. |
| `$E g $VM clip-set '<text>'` / `clip-get` | Sets / prints the VM's clipboard. |
| `$E g $VM config <file>` | Prints `settings.toml`, `terminal.toml` or `server.toml` from the test profile. |
| `$E g $VM lines <path>` | Counts lines in a VM file. |
| `$E g $VM git <folder> <args>` | Runs Git in a VM folder, outside Muxy. Single-quote `~` paths. |
| `$E g $VM file <path>` | Prints a VM file, or says it doesn't exist. |
| `$E g $VM exists <path>` | Prints `yes` or `no`. |
| `$E g $VM rm-tree <path>` | Deletes a folder inside `~/e2e` or `~/.muxy/worktrees`, outside Muxy. |
| `$E g $VM http-start` | Serves the fixture web page at `http://127.0.0.1:8765/`. |
| `$E g $VM dialog-fixture` | Installs inert AI-provider executables and an unstaged alpha change, only to open/cancel the commit dialog without an account or AI call. Restart the app afterward. |
| `$E g $VM symbol-language-fixture` | With the app quit, installs and selects a resource-only French pack in the managed guest profile: Folder becomes Dossier and Folder Fill becomes Dossier rempli. Launch afterward. |
| `$E g $VM agent-fixture` | Installs stub agents (`claude`, `codex`, `kiro-cli`) that print `AGENT-STUB`, `AGENT-ARGS`, `AGENT-CWD`, `AGENT-MUXY`, and `AGENT-CLI` lines, then wait. |
| `$E g $VM agent-fixture-remove` | Removes those stub agents. |
| `$E g $VM remote-fixture` | With the app quit: starts a second server profile in this VM with project `beta`, and lists it in settings as the remote server `devbox`, reached through a custom command. Launch afterward. |
| `$E g $VM extension-fixture` | With the app quit: creates `~/e2e/hello-ext`, `~/e2e/hello-ready`, and `focus-timer` as if installed from the store. Launch afterward. |
| `$E g $VM extension-art-fixture` | With the app quit: installs `art-demo` 1.2.3 as if from the store, with an orange icon, a teal and a purple screenshot, author "Art Studio", and a "Greeting" setting. Launch afterward. |
| `$E g $VM extension-edit <version\|broken>` | Sets `~/e2e/hello-ext` to a version, or breaks its `package.json`. |
| `$E g $VM security-fixture` | Writes `~/e2e/sec-ext`: palette command "Sec Test: Open Tab" opens a "Sec View" tab with the heading `SEC-PAGE`, a green bar image, `script:`/`fetch:` status lines, a text field and a `typed:` line; its top-bar icon is two squares. Load it with `$E g $VM muxy extension load '~/e2e/sec-ext'`. |
| `$E g $VM image-fixture` | Writes `~/e2e/images`: a test PNG, a Sixel image, and `show`, which prints them between text markers. Run `~/e2e/images/show <kitty\|kitty-file\|kitty-temp\|iterm\|sixel\|fifo\|da1\|iterm-gif\|kitty-gif\|kitty-anim>` in a guest terminal. The last three show an animation of red, green, blue and yellow frames, two seconds each. |
| `$E g $VM pixel-fixture` | Writes `~/e2e/pixels/probe`, a kitty-graphics test program (macOS and Linux). `probe draw` fills the pane with a picture of 1-pixel stripes and checks, turns on pixel mouse reports, draws a red cross where a click is reported and prints `CLICK-<n> x=.. y=..`, and prints pointer motion as `MOVE x=.. y=..` on the last line. `probe cells` prints `CELL-CLICK col=.. row=..`. `probe stream <seconds>` tries to send 60 frames a second, then prints `STREAM-DONE sent=<n> rate=<sends per second>`: that counts frames sent, not frames Muxy drew. The first status line reads `PROBE image=<w>x<h> bytes=<n>MB`. A pane under 7 rows or 32 pixels wide prints `PROBE-TOO-SMALL`. Press `q` to quit. |
| `$E g $VM floating-fixture` | Writes the `desk-pet` and `sticky-notes` sample extensions to `~/e2e`. Load each with `$E g $VM muxy extension load '~/e2e/<name>'`. |
| `$E g $VM exec-fixture` | Writes `~/e2e/exec-ext`: palette commands "Exec Test: Short", "Long" (a ~3000-character command), "Other" (appends a line to `~/e2e/exec-other.txt`) and "Long Alert". Load it with `$E g $VM muxy extension load '~/e2e/exec-ext'`. |
| `$E g $VM bg-fixture` | Writes `~/e2e/bg-ext`: palette commands "BG Test: Plain Panel", "Slow Panel", "Glass Panel", "Plain Tab", "Slow Tab" and "Modal", and the sidebar "BG Sidebar". Slow pages show nothing for 15 seconds, then a red page. Load it with `$E g $VM muxy extension load '~/e2e/bg-ext'`. |
| `$E g $VM launch-without-muxy` | With the app quit: starts it with no `muxy` on its `PATH` and no installed link. Linux only. |
| `$E g $VM logs` | App and server log tails and processes. |
| `$E g $VM app-pid` | Prints the Muxy app's process id, or nothing when it isn't running. |
| `$E g $VM link-launch` | Restarts Muxy registered for `muxy-beta://` links, as an installed app would be: through a generated app bundle on macOS, a desktop entry on Linux, and the app's own registration on Windows. Use it instead of `launch` in link tests. |
| `$E g $VM open-link '<url>'` | Opens a URL the way a browser or another app would (`open`, `xdg-open`, `start`), then waits 3 seconds. Always single-quote the URL. |
| `$E g $VM open-session <id\|'text'> [right\|down]` | Finds the terminal by id, or the one live terminal whose screen shows the text, prints `muxy session link` for it (with `--split right` or `--split down` when given) as `link: ...`, then opens that link like `open-link`. |
| `$E desktop $VM <action> [args]` | Runs `swiftvm control` on the VM (see Driving the desktop). |

A command that prints `muxy: ...` or `muxy-e2e: ...` and exits non-zero has
failed; quote its output in your report.

## Keys

Muxy shortcuts differ between macOS and the other platforms. Never pick them yourself:
steps say `$E press $VM <action>` (for example `new-tab`, `split-right`,
`palette`), and the script presses the right keys for your VM's platform and
prints them. `$E actions $VM` lists every action.

Plain keys that are the same everywhere use `$E key $VM <key>`: `Return`,
`Escape`, `Tab`, `Up`, `Down`, `Left`, `Right`, `PageUp`, `PageDown`,
`Backspace`, and terminal keys such as `ctrl+c`. Several
keys in one command are pressed one after the other: `$E key $VM Down Down Return`.

## Driving the desktop

Every `$E` key, type, click, scroll and shot command runs through `swiftvm control`
from outside the VM. Use those commands for everything a step asks; never drive
the VM another way.

`$E desktop $VM <action>` runs `swiftvm control` directly, for example
`$E desktop $VM look --app muxy-app`, which lists Muxy's controls by name. Use it
only where a step asks for it.

Click controls with `$E click-element` (see Clicking). Terminal text and how things
look are only in screenshots: use `$E shot` to check them.

## Waiting

The VM is slower than a real computer. After any key that opens or closes
something, the commands already pause briefly. Then:

- For terminal output, use `$E g $VM wait '<text>'`, never a fixed sleep.
- For the first terminal, wait for its prompt: `$E g $VM wait 'muxy@'`.
  When other terminals already show a prompt, steps use `sleep 3` instead,
  and the next marker check proves the new shell works.
- For anything else, take a screenshot. If what the step expects isn't there
  yet, wait 3 seconds (`sleep 3`) and take one more. Two tries, then report.

## Markers

To prove output came from the program and not from your typing, steps type
arithmetic like `echo done-$((40+2))` and check for the result `done-42`,
which never appears in the typed text. Check exactly the text the step names.

## Screenshots

On macOS, system notification banners can appear at the top right and cover
the title bar's icons. If one covers what a step needs, wait and take the shot
again. Never click a banner: it opens System Settings.

A macOS consent dialog ("Allow "swiftvm-agent" to access ...") can appear over
the app and swallow your typing. It isn't Muxy's: click "Don't Allow", say so in
your report, and redo the step from its start.

1. Take one with `$E shot $VM <short-name>` whenever a step says **Shot**.
2. Open the printed path with your file/image reading tool and look at it.
3. Answer the step's question about it in one plain sentence.

Where things are in a Muxy window: the **tab strip** is the row of tabs at
the top of the window; the **sidebar** is the narrow column of project icons
on the left (Home is the house icon); the **status bar** is the bottom row,
with "Server · Connected" at its right end; terminal text fills the middle.
A **pane** is one terminal area with a header row on top (an icon and a
title); split panes are separated by thin lines. The focused pane's header
shows action icons on its right, and the other panes look dimmed.

## Clicking

Only click when a step says so. Steps name the control:

- `$E click-element $VM <name>` clicks a control by its exact name (like `Cancel`), without
  pixels. It fails with a clear message when the control isn't there, or when several
  controls have that name: take a shot, then report it with the message. Type the
  `--role` and `--nth` options exactly as the step gives them; never add them yourself.
- `$E desktop $VM look --app muxy-app` lists the controls, when a step asks you to read one.

A few steps still say `$E click $VM <x> <y>`, because what they click isn't a control.
Coordinates are pixels of the **original** screenshot, whose size `shot` printed (for
example `2560x1600`). If your image tool shows the picture at a different size, convert:
original x = shown x × (original width ÷ shown width), and the same for y. Take a fresh
screenshot before every such click: windows move between launches.

## Statuses

| Status | Use when |
| --- | --- |
| `PASS` | Every check in the test matched, and you have the evidence. |
| `FAIL` | Muxy did something different from the expected result. |
| `BLOCKED` | A tool, the VM or the boot failed, so the test couldn't run properly. |
| `INSTRUCTION` | The steps don't match what is on screen (a menu item or window the step names doesn't exist), so you couldn't follow them. |
| `SKIP` | The test says it doesn't apply to your platform. |

## Report

After the last suite, run `$E logs $VM` (prints a folder of log copies).
Return the report below as your final message; the coordinator saves it as
`target/e2e/<run>/<platform>/report-<batch>.md`.

```markdown
# E2E report: <platform>, batch <batch>, run <run>
VM: <vm>  Model: <your model name>
Boot: <PASS|FAIL|not run>  Build info: <the three --build-info lines, or "same as batch A">
Totals: <n> PASS, <n> FAIL, <n> BLOCKED, <n> INSTRUCTION, <n> SKIP

| Test | Status | What you saw (one line) | Evidence |
| --- | --- | --- | --- |
| T02.1 | PASS | done-42 shown under the command in the only pane | shots/012-t02-1-output.png |

## Problems
### T03.8 FAIL
- Step: 4 (`$E key $VM Escape` after the "Close Pane?" dialog)
- Expected: the dialog closes and `sleep 600` keeps running
- Actual: <exact observation; quote command output>
- Evidence: shots/031-t03-8-after-escape.png, logs/guest-logs-201502.txt
- Log lines: <up to 10 lines from `$E g $VM logs` that mention error, panic, or failed>
```

Paths are relative to `target/e2e/<run>/<platform>/`. Every row needs at
least one evidence path: a screenshot for visual checks, or the command and
its output for CLI checks.
