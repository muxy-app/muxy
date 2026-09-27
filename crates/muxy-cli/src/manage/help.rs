pub(crate) const ROOT: &str = "Muxy - terminal client and server management

Usage: muxy [COMMAND]

  (no command)  Open the terminal UI; Ctrl-B ? shows help
  server        Start, inspect or stop the local server
  project       List, add, edit or delete server projects
  session       Create terminals, send input, read output and end sessions
  worktree      List, create, register or remove Git worktree projects
  settings      Read or update server settings
  activity      Read or acknowledge agent activity
  git           Run a server Git action using JSON
  files         Run a server file action using JSON
  exec          Execute a program in a server project
  mobile        Enable access, pair phones and revoke devices
  --help | --version | --build-info

Run muxy <command> --help for usage. Management commands accept --json.
MUXY_DIR selects the local server profile. Commands connect directly to the
server, starting it if needed, except server status/stop, which never start it.
No desktop app is required. Tabs, panes and workspaces belong to UI clients.
Project selectors accept an exact ID, unique name or directory path.
Use -- before literal arguments that start with a dash.";

pub(crate) const SERVER: &str = "Usage: muxy server start|status|stop [--json]
  stop [--force]  Stop only if idle; --force ends all running terminals.";
pub(crate) const PROJECT: &str = "Usage: muxy project <action> [--json]
  list
  add <directory> [--name NAME]
  rename <project> <name>
  set-color <project> <#RRGGBB>
  set-icon <project> [icon]        Omit icon to clear it
  delete <project> --yes          Ends its terminals and deletes child projects;
                                 files and worktree folders stay on disk

Add creates a distinct project even when its directory is already registered.
Text list columns: id, name, directory (tab-separated).";
pub(crate) const SESSION: &str = "Usage: muxy session <action> [--json]
  list [--project PROJECT]         Includes saved/ended sessions
  create <project> [--directory PATH] [--cols N] [--rows N]
  send <id> <text>                 Send literal text without pressing Return
  send-keys <id> <key>             Enter, Tab, Escape, Backspace, Ctrl+C/D/Z
  read-screen <id> [--lines N] [--saved]
  history <id> [--before CURSOR] [--limit N] [--saved]
  search <id> <text> [--ignore-case] [--before CURSOR] [--limit N] [--saved]
  end <id> --yes                   End the process, keeping saved output
  discard <id> --yes               End the process and delete saved output

Create prints the new session ID. Sessions outlive this command.
Input/output commands briefly attach to a live terminal; --saved reads its last
checkpoint instead and also works after it ends. They never close UI panes.
Read-screen returns the last N visible rows (default 50), not scrollback.
History/search return one page as JSON, with next for --before; 0 starts paging.
Text list columns: id, project-id, status, attached, directory.
JSON session IDs and pagination cursors are strings to preserve all 64 bits.";
pub(crate) const WORKTREE: &str = "Usage: muxy worktree <action> [--json]
  list <project>
  create <project> <directory> --branch NAME [--base REF | --existing]
  register <project> <directory>
  remove <worktree-project> --yes

Create defaults to a new branch from HEAD. Register adds an existing Git worktree.
Remove deletes the worktree directory and its project, ending its terminals.
The server checks the removal snapshot before deleting. List returns JSON.";
const SETTINGS: &str = "Usage: muxy settings get [--json]
       muxy settings set <key> <value> [--json]
Keys: default-shell (absolute path or 'default'), history-budget-bytes (integer),
      shell-integration (true/false). Get returns JSON; paths are UTF-8 strings.";
const ACTIVITY: &str = "Usage: muxy activity list [--json]
       muxy activity ack <event-id>... [--json]
List returns the server's activity snapshot as JSON, with session/event IDs as strings.";
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
