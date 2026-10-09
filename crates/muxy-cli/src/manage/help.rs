pub(crate) const ROOT: &str = "Muxy - terminal client and server management

Usage: muxy [--host DESTINATION] [COMMAND]

  (no command)  Open the terminal UI; Ctrl-B ? shows help
  <folder>      Open a folder as a project in the desktop app, or in the
                terminal UI without it or over SSH
  server        Start, inspect or stop the server
  project       List, add, edit or delete server projects
  session       Create terminals, send input, read output and end sessions
  worktree      List, create, register or remove Git worktree projects
  settings      Read or update server settings
  activity      Read or acknowledge agent activity
  git           Run a server Git action using JSON
  files         Run a server file action using JSON
  exec          Execute a program in a server project
  mobile        Enable access, pair phones and revoke devices
  install-skills
                Install the muxy-cli skill for AI coding agents
  stdio         Connect stdin/stdout to the server; used over SSH
  --help | --version | --build-info

Run muxy <command> --help for usage. Management commands accept --json.
Commands connect directly to the server, starting it if needed, except server
status/stop, which never start it. --host uses the server on another computer
over SSH (user@host, an ssh_config alias or ssh://user@host:port); Muxy must be
installed there, and directories are absolute paths on that computer.
MUXY_DIR selects the local profile, which also keeps the terminal UI layouts
for other computers. No desktop app is required. Tabs, panes and workspaces
belong to UI clients.
Project selectors accept an exact ID, unique name or directory path.
Inside a Muxy terminal, session commands default to that terminal.
Use ./NAME to open a folder named like a command.
Use -- before literal arguments that start with a dash, or before the command
of session create.";

pub(crate) const MOBILE: &str = "Usage: muxy mobile [action]
  (no action)               Mobile access status and paired devices
  enable [--port N]         Let phones connect; --port changes the port (7419)
  disable                   Turn mobile access off
  pair [--address HOST]...  Show a pairing code and wait for the phone
  revoke <device>           Revoke a device by the start of its ID

Pairing codes list the server's own addresses. --address puts a DNS name or
IPv4 address first, for a server that phones reach by another name, such as a
cloud server's public name. Repeat it for more, up to 8 addresses in all.";
pub(crate) const INSTALL_SKILLS: &str = "Usage: muxy install-skills [--dir DIR]...
Installs the muxy-cli skill, which teaches AI coding agents to use this command,
into each agent folder that exists: ~/.claude/skills, ~/.codex/skills and
~/.agents/skills. --dir adds another skills folder; repeat it for more.
An installed copy, including the Muxy 1.x skill, is replaced.";
pub(crate) const SERVER: &str = "Usage: muxy server start|status|stop [--json]
  stop [--force]  Stop only if idle; --force ends all running terminals.";
pub(crate) const PROJECT: &str = "Usage: muxy project <action> [--json]
  list
  add <directory> [--name NAME] [--create] [--reuse]
  rename <project> <name>
  set-color <project> <#RRGGBB>
  set-icon <project> [icon]        Omit icon to clear it
  delete <project> --yes          Ends its terminals and deletes child projects;
                                 files and worktree folders stay on disk

Add creates a distinct project even when its directory is already registered;
--reuse prints the existing one instead. --create makes the directory first.
Text list columns: id, name, directory (tab-separated).";
pub(crate) const SESSION: &str = "Usage: muxy session <action> [--json]
  list [--project PROJECT] [--all]  Live sessions; --all includes saved/ended ones
  create [project] [--directory PATH] [--cols N] [--rows N] [-- COMMAND...]
  send [id] <text>                 Send literal text without pressing Return
  send-keys [id] <key>             Enter, Tab, Escape, Backspace, Ctrl+C/D/Z
  read-screen [id] [--lines N] [--saved]
  history [id] [--before CURSOR] [--limit N] [--saved]
  search [id] <text> [--ignore-case] [--before CURSOR] [--limit N] [--saved]
  wait [id] --text TEXT [--ignore-case] | --exit [--timeout-ms N]
  end [id] --yes                   End the process, keeping saved output
  discard [id] --yes               End the process and delete saved output

Create prints the new session ID and types COMMAND into it, then Return.
Sessions outlive this command; the apps list them under Existing Terminals.
Inside a Muxy terminal, the ID defaults to that terminal and the project to its
project. Wait prints the first visible line containing TEXT, or how the session
ended; it fails after the timeout (default 30000ms, maximum 3600000ms).
Input/output commands briefly attach to a live terminal; --saved reads its last
checkpoint instead and also works after it ends. They never close UI panes.
Ended sessions and their saved output are automatically removed after 7 days.
Read-screen returns the last N visible rows (default 50), not scrollback.
History/search return one page as JSON, with next for --before; 0 starts paging.
Text list columns: id, project-id, status, attached, directory.
JSON session IDs and pagination cursors are strings to preserve all 64 bits.";
pub(crate) const WORKTREE: &str = "Usage: muxy worktree <action> [--json]
  list <project>
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
Text list columns: project-id, branch, directory (tab-separated).";
const SETTINGS: &str = "Usage: muxy settings get [--json]
       muxy settings set <key> <value> [--json]
Keys: default-shell (absolute path or 'default'), history-budget-bytes (integer),
      shell-integration (true/false). Get returns JSON; paths are UTF-8 strings.";
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

pub(crate) fn topic(group: &str) -> Option<&'static str> {
    Some(match group {
        "server" => SERVER,
        "project" => PROJECT,
        "session" => SESSION,
        "worktree" => WORKTREE,
        "settings" => SETTINGS,
        "activity" => ACTIVITY,
        "git" => GIT,
        "files" => FILES,
        "exec" => EXEC,
        _ => return None,
    })
}
