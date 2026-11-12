mod commands;
mod discovery;

pub(crate) use discovery::{alias, children, detail, flags, known, requested, takes_value};

pub(crate) const ROOT: &str = "Muxy - persistent terminals from your shell

Usage: muxy [--host DESTINATION] <command> [options]

Common commands:
  ls              List live sessions (session list)
  new             Create a persistent terminal (session create)
  send            Send text or stdin to a terminal (session send)
  send-key        Press one or more keys (session send-keys)
  capture         Read a terminal's screen (session read-screen)
  wait            Wait for text or a session to exit (session wait)

Resources:
  project         List, inspect, add, edit and delete projects
  session         Inspect, name, control and read terminal sessions
  worktree        Create, register and remove Git worktree projects
  activity        Read or acknowledge agent activity

Server and automation:
  server          Start, inspect or stop the server
  settings        Read or update server settings
  exec            Execute a program in a project, without a shell
  git / files     Run advanced server actions using protocol JSON
  mobile          Enable phone access, pair devices and manage notifications
  status          Report program status to this terminal (OSC 7501)
  stdio           Bridge stdin/stdout to the server; used over SSH

Desktop app:
  browser         Open and automate browser tabs
  extension       Load, inspect, reload and publish extensions
  config          Read or change this computer's app settings

Utilities:
  help [command]  Show help; add a subcommand for its specific usage
  completion      Generate Bash, Zsh or Fish shell completions
  install-skills  Install the CLI guide for AI coding agents
  version         Print the CLI version (also --version)

Options:
  --host DEST     Use a server over SSH; must come before the command
  -h, --help      Show help without connecting to a server
  -V, --version   Print the CLI version
  --build-info    Print build information as JSON

Examples:
  muxy project add ~/code/app --name app
  muxy new app -- 'npm run dev'
  muxy ls --project app
  muxy help session send
  muxy --host devbox project list --json

Output and targeting:
  Terminals get readable tables; pipes keep the original text formats.
  Use --json for structured output, or list --quiet for project/session IDs.
  Project selectors accept an ID, unique name or directory.
  Inside a Muxy terminal, session commands default to that terminal.
  Use -- before literal arguments starting with a dash.

Without a command, muxy opens the terminal UI. muxy <folder> opens a project;
use ./NAME for a folder named like a command. Tabs and layouts belong to apps.
Commands start their server if needed, except server status/stop. --host takes
user@host, an SSH alias or ssh://user@host:port; remote paths must be absolute.
MUXY_DIR selects the local profile. NO_COLOR disables help and table colors.";

pub(crate) const MOBILE: &str = "Manage phone access and push notifications.

Usage: muxy mobile [command]

Commands:
  (no action)               Mobile access status and paired devices
  enable [--port N]         Let phones connect; --port changes the port (7419)
  disable                   Turn mobile access off
  pair [--address HOST]...  Show a pairing code and wait for the phone
  revoke <device>           Revoke a device by the start of its ID
  push on|off               Turn push notifications to paired phones on or off
  push reset <device>       Forget a phone's push registration and key

Pairing codes list the server's own addresses. --address puts a DNS name or
IPv4 address first, for a server that phones reach by another name, such as a
cloud server's public name. Repeat it for more, up to 8 addresses in all.

Push notifications reach a paired phone that turned them on in the Muxy app,
through the muxy.app relay, which sees only encrypted content. Turning them off
pauses every phone: muxy.app forgets them at once, and this computer keeps
their registrations for 90 days, so turning them on again resumes those phones
without reconnecting. Reset gives one phone a new key the next time it
connects, without unpairing it.";
pub(crate) const BROWSER: &str = "Automate browser tabs in the local desktop app.

Usage: muxy browser <command>

Commands:
  open [url] [--split] [--project PROJECT] [--worktree WORKTREE]
       [--profile PROFILE]          Open a tab and print its ID
  list [--project PROJECT] [--worktree WORKTREE] [--json]
  navigate <tab-id> <url>
  read <tab-id> [--json]            Title, URL and page text
  close <tab-id>
  reload|back|forward <tab-id>
  screenshot <tab-id> [--out FILE]  Print the page as a base64 PNG, or save it
  eval <tab-id> <javascript>        Print the result as JSON
  click|hover|scroll-into-view|check|uncheck <tab-id> <selector>
  fill <tab-id> <selector> <text>
  type <tab-id> <selector> <text> [--submit]
  press <tab-id> <key> [selector]
  select <tab-id> <selector> <value>
  is <tab-id> visible|hidden|enabled|disabled|checked <selector>
  get-text|get-value|get-count <tab-id> <selector>
  get-attribute <tab-id> <selector> <attribute>
  get-html <tab-id> [selector]
  find <tab-id> role|text|label|placeholder|testid <value>
  snapshot <tab-id> [selector]      Interactive elements, each with a selector
  wait <tab-id> [--selector SELECTOR] [--text TEXT] [--url-contains TEXT]
       [--function JAVASCRIPT] [--timeout-ms N]
  wait-for <tab-id> <selector> [ms]
  wait-for-navigation <tab-id> [ms | --timeout-ms N]
  storage get <tab-id> <key> [local|session]
  storage set <tab-id> <key> <value> [local|session]
  storage clear <tab-id> [local|session]
  cookies get <tab-id> [url]
  cookies set <tab-id> <name> <value> <domain> [--path PATH] [--secure]
              [--http-only] [--expires UNIX-SECONDS]
  cookies delete <tab-id> <name> [domain]
  cookies clear <tab-id>

Browser commands talk to the Muxy desktop app on this computer, not to the
server, so they don't take --host. Open starts the app if it isn't running.
Tabs open in the current project, or the one named by --project and
--worktree; --split opens the tab beside the current pane instead.
Use the tab ID that open or list prints. Page actions print their result as
JSON. Waits give up after --timeout-ms or ms (default 5000, 10000 for
wait-for-navigation; maximum 60000).
MUXY_CLI_TIMEOUT sets how many seconds the app has to answer (default 10).
Piped list columns: tab-id, title, url, profile, active (tab-separated).
Terminals show a table. Run muxy help browser <command> for specific usage.";
pub(crate) const EXTENSION: &str = "Manage extensions in the local desktop app.

Usage: muxy extension <command>

Commands:
  list [--json]                     Extensions, their state and folders
  status <name> [--json]            One extension, its load errors and recent log
  logs <name> [--lines N]           Recent log lines (default 100)
  load <folder>                     Load an unpacked folder; it starts disabled
  enable <name>                     Turn an extension on
  disable <name>                    Turn an extension off
  reload <name>                     Restart one extension from its folder
  unload <name>                     Forget an unpacked folder and keep its files
  validate <name|folder> [--json]   Check what publishing needs
  publish <name> [--json]           Open a pull request to the extension store

Developing and testing:
  dev <folder> [--once]             Load, enable, then build and reload on change
  pages <name> [--json]             The extension's open pages and their IDs
  open <name> panel:<id>|tab:<type>|modal:<entry> [--data JSON]
                                    Open a page, wait for it, print its ID
  run <name> <command>              Run one of its commands
  emit <name> <event> [json]        Send it an event its manifest lists
  page <action> <page-id> [...]     Drive a page with the browser actions:
                                    read, screenshot [--out FILE], eval,
                                    click, type, fill, press, snapshot, wait,
                                    find, get-text and the rest

Extension commands talk to the Muxy desktop app on this computer, not to the
server, so they don't take --host. They start the app if it isn't running.
Publish asks for approval in the app and needs the GitHub CLI, signed in.
Use -- before a name or folder that starts with a dash.
MUXY_CLI_TIMEOUT sets how many seconds the app has to answer (default 10).
Load, reload, enable, disable, unload, validate and open get 30 more, and
publish 15 minutes more.
Piped list columns: name, version, state, source, folder (tab-separated).
Pages columns: page-id, kind, id, instance, loaded, visible, title.
Terminals show a table. Run muxy help extension <command> for specific usage.";
pub(crate) const CONFIG: &str = "Read and change this computer's desktop app settings.

Usage: muxy config <command>

Commands:
  list [--json]                     Every app setting with its current value
  get <key> [--json]                One setting, or a whole table of them
  set <key> <value> [--json]        Change a setting and apply it
  unset <key> [--json]              Return a setting to its default
  reload [--json]                   Have the running app read the files again

Keys are dotted paths in settings.toml, such as appearance.dark_theme, or in
terminal.toml after terminal., such as terminal.font_size. Quote a part that
holds a dot: keymap.\"command.build\". Values are TOML (true, 14, 0.5,
[\"a\", \"b\"]); anything else is text, so names need no quotes.
A change is saved only if the app can start with it; set and unset then have
a running desktop app apply it, and never start one. reload applies hand edits.
Use -- before a key or value that starts with a dash.
Optional settings appear in list once they are set. Remote servers are managed
in Settings, and server settings with muxy settings.
Piped list lines: key = value, with the value in TOML. Terminals show a table.";
pub(crate) const INSTALL_SKILLS: &str = "Usage: muxy install-skills [--dir DIR]...
Installs the muxy-cli skill, which teaches AI coding agents to use this command,
into each agent folder that exists: ~/.claude/skills, ~/.codex/skills and
~/.agents/skills. --dir adds another skills folder; repeat it for more.
An installed copy, including the Muxy 1.x skill, is replaced.";
pub(crate) const STATUS: &str = "Usage: muxy status <state> [--app APP] [--id ID] [--progress N]
                   [--kind KIND] [--title TEXT] [--msg TEXT]
  state             idle, working, done, blocked, error, or clear
  --app APP         Program name for machines, such as build or claude-code
  --id ID           Report on one part of the work, such as deploy/us-east
  --progress N      Percent done, 0 to 100, while working or blocked
  --kind KIND       What a blocked program needs: permission, question or auth
  --title TEXT      Short label
  --msg TEXT        One line saying what it is doing or waiting for

Tells the terminal what a script is doing (OSC 7501). It needs no server, so it
also works over SSH. Muxy clears working and blocked when the program that
reported them exits, and shows only reports without --id; clear removes a
record and the parts beneath it.";
pub(crate) const SERVER: &str = "Manage the background terminal server.

Usage: muxy server <command> [--json]

Commands:
  start           Start if needed, then show build information
  status          Show build information; never starts the server
  stop [--force]  Stop only if idle; --force ends all running terminals

Examples:
  muxy server status
  muxy --host devbox server status --json";
pub(crate) const PROJECT: &str = "Manage projects on the server.

Usage: muxy project <command> [--json]

Commands:
  list [--quiet]                 List projects; --quiet (-q) prints IDs only
  inspect <project>              Show project details
  add <directory> [--name NAME] [--create] [--reuse]
  rename <project> <name>
  set-color <project> <#RRGGBB>
  set-icon <project> [icon]        Omit icon to clear it
  delete <project> --yes          Ends its terminals and deletes child projects;
                                 files and worktree folders stay on disk

Add creates a distinct project even when its directory is already registered;
--reuse prints the existing one instead. --create makes the directory first.
Piped list columns: id, name, directory (tab-separated). Terminals show a table.

Examples:
  muxy project add ~/code/app --name app
  muxy project inspect app
  muxy project list --quiet";
pub(crate) const SESSION: &str = "Manage persistent terminal sessions.

Usage: muxy session <command> [--json]

Commands:
  list [--project PROJECT] [--all] [--quiet]
                                   Live sessions; --all includes saved/ended ones
  inspect [id]                     Show session details, including its name
  rename [id] <name>               Name a session for every connected app
  create [project] [--directory PATH] [--cols N] [--rows N] [-- COMMAND...]
  send [id] <text> | --stdin       Send text or piped bytes without adding Return
  send-keys [id] <key>...          Enter, Tab, Escape, Backspace, Ctrl+C/D/Z
  read-screen [id] [--lines N] [--saved]
  history [id] [--before CURSOR] [--limit N] [--saved]
  search [id] <text> [--ignore-case] [--before CURSOR] [--limit N] [--saved]
  wait [id] --text TEXT [--ignore-case] | --exit [--timeout-ms N]
  end [id] --yes                   End the process, keeping saved output
  discard [id] --yes               End the process and delete saved output
  show [id] [--background]         Show it in a desktop app tab without raising the app

Create prints the new session ID and types COMMAND into it, then Return.
COMMAND words are joined with spaces without shell quoting; quote the whole
command to preserve its quotes and shell syntax.
Sessions outlive this command; the apps list them under Existing Terminals.
Show opens the session in a new tab of its project in the desktop app on this
computer, or selects the tab that already shows it, without bringing a running
app to the front; it starts the app if needed and doesn't take --host.
--background adds the tab without switching the window to it.
Inside a Muxy terminal, the ID defaults to that terminal and the project to its
project. Wait prints the first visible line containing TEXT, or how the session
ended; it fails after the timeout (default 30000ms, maximum 3600000ms).
Use exec in COMMAND when --exit should wait for that program to finish.
Input/output commands briefly attach to a live terminal; --saved reads its last
checkpoint instead and also works after it ends. They never close UI panes.
Ended sessions and their saved output are automatically removed after 7 days.
Read-screen returns the last N visible rows (default 50), not scrollback.
History/search return one page as JSON, with next for --before; 0 starts paging.
Piped list columns: id, project-id, status, attached, directory.
Terminals also show names. --quiet (-q) prints only IDs and cannot use --json.
--stdin sends at most 1 MiB, verbatim; oversized input sends nothing.
JSON session IDs and pagination cursors are strings to preserve all 64 bits.

Examples:
  muxy session create app -- 'npm run dev'
  muxy session rename 42 dev-server
  printf 'npm test\\n' | muxy session send 42 --stdin
  muxy session send-keys 42 Ctrl+C Enter
  muxy help session wait";
pub(crate) const WORKTREE: &str = "Manage Git worktree projects.

Usage: muxy worktree <command> [--json]

Commands:
  list <project> [--quiet]        --quiet (-q) prints registered project IDs only
  create <project> <name> [--branch NAME] [--base REF | --existing]
         [--directory PATH] [--hooks]
  checkout-pr <project> <number> [--name NAME] [--directory PATH] [--hooks]
  register <project> <directory>
  remove <worktree-project> --yes [--force] [--hooks]

Create makes a new branch named after the worktree from HEAD; --existing checks
out a branch that exists. The directory defaults to the worktree location in
Settings. --hooks runs the setup or teardown commands from .muxy/worktree.json
and the per-machine worktree.json, printing each one; without it none run.
Remove deletes the worktree directory and its project, ending its terminals,
and refuses uncommitted changes unless --force is given.
Piped list columns: project-id, branch, directory (tab-separated).
Terminals show a table. --quiet omits unregistered worktrees.";
const SETTINGS: &str = "Usage: muxy settings get [--json]
       muxy settings set <key> <value> [--json]
Keys: default-shell (absolute path or 'default'), history-budget-bytes (integer),
      shell-integration (true/false). Paths are UTF-8 strings.
Get shows labeled fields in a terminal, JSON when piped or with --json.";
const ACTIVITY: &str = "Usage: muxy activity list [--json]
       muxy activity ack <event-id>... [--json]
Text list columns: session-id, project-id, provider, state, event-id, event
(tab-separated), one row per session. JSON returns the server's activity
snapshot, with session/event IDs as strings.";
const GIT: &str = "Usage: muxy git <project> <action-json> [--json]
Examples: muxy git Home '\"Summary\"'
          muxy git MyProject '{\"Log\":{\"max_count\":20,\"skip\":0}}'
Uses the protocol GitAction JSON shape and returns GitReply JSON.
Actions can modify the repository. Watch is unavailable on a one-shot client.
Protocol ServerPath values are byte arrays, not strings.";
const FILES: &str = "Usage: muxy files <project> <action-json> [--json]
Examples: muxy files MyProject '{\"List\":[]}'
          muxy files MyProject '{\"Read\":[82,69,65,68,77,69,46,109,100]}'
Uses the protocol FilesAction JSON shape and returns FilesReply JSON.
Paths are project-relative byte arrays. Actions can write or delete files.
Watch/unwatch are unavailable on a one-shot client.";
const EXEC: &str = "Usage: muxy exec <project> [--json] [--timeout-ms N] -- <program> [args...]
Runs through the server in the project's directory, without a shell.
For shell syntax, explicitly use /bin/sh -c '...'. Timeout defaults to 30000ms
(maximum 300000ms). Text mode forwards stdout/stderr. JSON mode returns the
complete result. A failed, timed-out, cancelled or truncated run exits nonzero.";

pub(crate) const COMPLETION: &str = "Usage: muxy completion <bash|zsh|fish>

Generate command and option completions without connecting to a server.

Examples:
  eval \"$(muxy completion bash)\"
  source <(muxy completion zsh)     # run compinit first
  muxy completion fish > ~/.config/fish/completions/muxy.fish";
const STDIO: &str = "Usage: muxy stdio [--no-start]

Bridge stdin/stdout to the server for remote clients.
--no-start refuses to start a missing server.";
const VERSION: &str = "Usage: muxy version

Print this CLI's version without connecting to a server.";

pub(crate) fn topic(group: &str) -> Option<&'static str> {
    Some(match group {
        "completion" => COMPLETION,
        "stdio" => STDIO,
        "version" => VERSION,
        "session" | "ls" | "new" | "send" | "send-key" | "capture" | "wait" => SESSION,
        "server" => SERVER,
        "project" => PROJECT,
        "worktree" => WORKTREE,
        "settings" => SETTINGS,
        "config" => CONFIG,
        "activity" => ACTIVITY,
        "git" => GIT,
        "files" => FILES,
        "exec" => EXEC,
        "mobile" => MOBILE,
        "install-skills" => INSTALL_SKILLS,
        "browser" => BROWSER,
        "extension" => EXTENSION,
        "status" => STATUS,
        _ => return None,
    })
}
