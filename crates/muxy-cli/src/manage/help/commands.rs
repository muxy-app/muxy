pub(super) const COMMANDS: &[(&str, &str, &str)] = &[
    (
        "server start",
        "[--json]",
        "Start the server if needed and show its build information.",
    ),
    (
        "server status",
        "[--json]",
        "Show the running server's build information without starting it.",
    ),
    (
        "server stop",
        "[--force] [--json]",
        "Stop an idle server. --force also ends every running terminal.",
    ),
    (
        "project list",
        "[--json | --quiet]",
        "List projects. --quiet (-q) prints only IDs, one per line.",
    ),
    (
        "project inspect",
        "<project> [--json]",
        "Show a project's name, directory, parent, color and icon. Select by ID, unique name or directory.",
    ),
    (
        "project add",
        "<directory> [--name NAME] [--create] [--reuse] [--json]",
        "Register a project. --create makes the directory; --reuse returns an existing project in that directory.",
    ),
    (
        "project rename",
        "<project> <name> [--json]",
        "Change the project's name without changing its directory.",
    ),
    (
        "project set-color",
        "<project> <#RRGGBB> [--json]",
        "Change the project color. Quote the color so the shell does not treat # as a comment.",
    ),
    (
        "project set-icon",
        "<project> [icon] [--json]",
        "Set a project icon, or omit icon to clear it.",
    ),
    (
        "project delete",
        "<project> --yes [--json]",
        "Delete the project and child projects, ending their terminals. Files and worktree folders stay on disk.",
    ),
    (
        "session list",
        "[--project PROJECT] [--all] [--json | --quiet]",
        "List live sessions. --all includes saved and ended sessions. --quiet (-q) prints only IDs.",
    ),
    (
        "session inspect",
        "[id] [--json]",
        "Show a live or saved session's name, project, directory, status and owner. Defaults to this terminal's session.",
    ),
    (
        "session rename",
        "[id] <name> [--json]",
        "Name a session for every connected app. Names must be nonempty, trimmed, at most 256 bytes and contain no control characters.",
    ),
    (
        "session create",
        "[project] [--directory PATH] [--cols N] [--rows N] [--json] [-- COMMAND...]",
        "Create a persistent terminal and print its ID. Defaults: 80 columns, 24 rows, and this terminal's project. COMMAND is typed followed by Return; words are joined with spaces, so quote a whole shell expression to preserve its syntax.",
    ),
    (
        "session send",
        "[id] <text> [--json]\n       muxy session send [id] --stdin [--json]",
        "Send literal text without pressing Return. --stdin reads piped or redirected bytes verbatim, including newlines, up to 1 MiB. Oversized input sends nothing. Defaults to this terminal's session.",
    ),
    (
        "session send-keys",
        "[id] <key>... [--json]",
        "Send one or more keys in order: Enter, Tab, Escape, Backspace, Ctrl+C, Ctrl+D or Ctrl+Z. Defaults to this terminal's session.",
    ),
    (
        "session read-screen",
        "[id] [--lines N] [--saved] [--json]",
        "Print the last N visible rows (default 50), not scrollback. --saved reads the last checkpoint and works after the session ends.",
    ),
    (
        "session history",
        "[id] [--before CURSOR] [--limit N] [--saved] [--json]",
        "Read one scrollback page as JSON. --limit defaults to 200 (maximum 500). Pass next as --before to continue; 0 starts paging.",
    ),
    (
        "session search",
        "[id] <text> [--ignore-case] [--before CURSOR] [--limit N] [--saved] [--json]",
        "Search scrollback and return a page of matches as JSON. --limit defaults to 100 (maximum 500). Pass next as --before to continue.",
    ),
    (
        "session wait",
        "[id] (--text TEXT [--ignore-case] | --exit) [--timeout-ms N] [--json]",
        "Wait for visible text or for the shell to exit. Timeout: 30000ms by default, at most 3600000ms. Use exec in the created command to wait for that program's exit. Prints the matching line or exit reason.",
    ),
    (
        "session end",
        "[id] --yes [--json]",
        "End the process, keeping saved output. This also closes any UI panes showing it.",
    ),
    (
        "session discard",
        "[id] --yes [--json]",
        "End the process and delete saved output. This also closes any UI panes showing it.",
    ),
    (
        "session show",
        "[id] [--background] [--json]",
        "Show a terminal in the local desktop app without raising it. --background adds the tab without selecting it. Starts the app if needed; does not accept --host.",
    ),
    (
        "worktree list",
        "<project> [--json | --quiet]",
        "List Git worktrees. --quiet (-q) prints registered project IDs only, omitting unregistered worktrees.",
    ),
    (
        "worktree create",
        "<project> <name> [--branch NAME] [--base REF | --existing] [--directory PATH] [--hooks] [--json]",
        "Create a worktree and project. Defaults to a new branch named after the worktree, based on HEAD. --existing uses an existing branch. --hooks runs configured setup commands.",
    ),
    (
        "worktree checkout-pr",
        "<project> <number> [--name NAME] [--directory PATH] [--hooks] [--json]",
        "Check out a pull request in a new worktree project. --hooks runs configured setup commands.",
    ),
    (
        "worktree register",
        "<project> <directory> [--json]",
        "Register an existing Git worktree as a project.",
    ),
    (
        "worktree remove",
        "<worktree-project> --yes [--force] [--hooks] [--json]",
        "Delete a worktree's directory and project, ending its terminals. Refuses uncommitted changes unless --force is given. --hooks runs teardown commands.",
    ),
    (
        "settings get",
        "[--json]",
        "Read server settings: default_shell, history_budget_bytes and shell_integration.",
    ),
    (
        "settings set",
        "<key> <value> [--json]",
        "Set default-shell (absolute path or default), history-budget-bytes (integer), or shell-integration (true/false).",
    ),
    (
        "activity list",
        "[--json]",
        "List agent activity and the latest unread event per session.",
    ),
    (
        "activity ack",
        "<event-id>... [--json]",
        "Mark the given activity events as read.",
    ),
    (
        "config list",
        "[--json]",
        "List desktop app settings, including defaults. Does not start the app or server.",
    ),
    (
        "config get",
        "<key> [--json]",
        "Read one dotted setting key or a table, such as terminal or terminal.font_size.",
    ),
    (
        "config set",
        "<key> <value> [--json]",
        "Save a setting and apply it to the running app. Values are TOML, or text when not valid TOML. Use -- before a literal value starting with a dash.",
    ),
    (
        "config unset",
        "<key> [--json]",
        "Return a setting to its default and apply it to the running app.",
    ),
    (
        "config reload",
        "[--json]",
        "Have the running desktop app read its settings files again. Does not start the app.",
    ),
    (
        "mobile enable",
        "[--port N]",
        "Enable phone access. --port changes the listening port (default 7419).",
    ),
    ("mobile disable", "", "Disable phone access."),
    (
        "mobile pair",
        "[--address HOST]...",
        "Print a pairing code and wait for a phone. Repeat --address for DNS names or IPv4 addresses, without ports, up to 8 addresses in all.",
    ),
    (
        "mobile revoke",
        "<device>",
        "Revoke a phone by the unique start of its device ID.",
    ),
    (
        "mobile push on",
        "",
        "Resume encrypted push notifications to paired phones.",
    ),
    (
        "mobile push off",
        "",
        "Pause push notifications to every paired phone.",
    ),
    (
        "mobile push reset",
        "<device>",
        "Forget one phone's push registration and key without unpairing it.",
    ),
    (
        "extension list",
        "[--json]",
        "List extensions, their state and folders, then any load errors.",
    ),
    (
        "extension status",
        "<name> [--json]",
        "Show one extension, load errors and recent logs.",
    ),
    (
        "extension logs",
        "<name> [--lines N]",
        "Print recent log lines (default 100, maximum 10000).",
    ),
    (
        "extension load",
        "<folder>",
        "Load an unpacked extension folder, initially disabled.",
    ),
    ("extension enable", "<name>", "Enable an extension."),
    ("extension disable", "<name>", "Disable an extension."),
    (
        "extension reload",
        "<name>",
        "Restart an extension from its folder.",
    ),
    (
        "extension unload",
        "<name>",
        "Forget an unpacked extension without deleting its files.",
    ),
    (
        "extension validate",
        "<name|folder> [--json]",
        "Check what publishing requires, including the GitHub CLI.",
    ),
    (
        "extension publish",
        "<name> [--json]",
        "Open a pull request to the extension store after approval in the desktop app.",
    ),
    (
        "browser open",
        "[url] [--split] [--project PROJECT] [--worktree WORKTREE] [--profile PROFILE]",
        "Open a browser tab and print its ID. Starts the desktop app if needed.",
    ),
    (
        "browser list",
        "[--project PROJECT] [--worktree WORKTREE] [--json]",
        "List browser tabs with their title, URL, profile and active state.",
    ),
    (
        "browser navigate",
        "<tab-id> <url>",
        "Navigate a browser tab to a URL.",
    ),
    (
        "browser read",
        "<tab-id> [--json]",
        "Read a page's title, URL and text.",
    ),
    ("browser close", "<tab-id>", "Close a browser tab."),
    ("browser reload", "<tab-id>", "Reload a browser tab."),
    (
        "browser back",
        "<tab-id>",
        "Go back in a tab's navigation history.",
    ),
    (
        "browser forward",
        "<tab-id>",
        "Go forward in a tab's navigation history.",
    ),
    (
        "browser screenshot",
        "<tab-id>",
        "Print a screenshot as a base64 PNG, ready to decode.",
    ),
    (
        "browser eval",
        "<tab-id> <javascript>",
        "Evaluate JavaScript and print the result as JSON.",
    ),
    (
        "browser click",
        "<tab-id> <selector>",
        "Click a matching element.",
    ),
    (
        "browser hover",
        "<tab-id> <selector>",
        "Hover over a matching element.",
    ),
    (
        "browser scroll-into-view",
        "<tab-id> <selector>",
        "Scroll a matching element into view.",
    ),
    (
        "browser check",
        "<tab-id> <selector>",
        "Check a matching checkbox.",
    ),
    (
        "browser uncheck",
        "<tab-id> <selector>",
        "Uncheck a matching checkbox.",
    ),
    (
        "browser fill",
        "<tab-id> <selector> <text>",
        "Replace a matching input's value.",
    ),
    (
        "browser type",
        "<tab-id> <selector> <text> [--submit]",
        "Type into an element, optionally submitting it.",
    ),
    (
        "browser press",
        "<tab-id> <key> [selector]",
        "Press a key, optionally targeting an element.",
    ),
    (
        "browser select",
        "<tab-id> <selector> <value>",
        "Select an option by its value.",
    ),
    (
        "browser is",
        "<tab-id> <visible|hidden|enabled|disabled|checked> <selector>",
        "Test the state of a matching element.",
    ),
    (
        "browser get-text",
        "<tab-id> <selector>",
        "Read a matching element's text.",
    ),
    (
        "browser get-value",
        "<tab-id> <selector>",
        "Read a matching input's value.",
    ),
    (
        "browser get-count",
        "<tab-id> <selector>",
        "Count matching elements.",
    ),
    (
        "browser get-attribute",
        "<tab-id> <selector> <attribute>",
        "Read an element attribute.",
    ),
    (
        "browser get-html",
        "<tab-id> [selector]",
        "Read page or element HTML.",
    ),
    (
        "browser find",
        "<tab-id> <role|text|label|placeholder|testid> <value>",
        "Find elements by a semantic selector.",
    ),
    (
        "browser snapshot",
        "<tab-id> [selector]",
        "List interactive elements and selectors that can target them.",
    ),
    (
        "browser wait",
        "<tab-id> [--selector SELECTOR] [--text TEXT] [--url-contains TEXT] [--function JAVASCRIPT] [--timeout-ms N]",
        "Wait for a page condition. Timeout defaults to 5000ms, with a maximum of 60000ms.",
    ),
    (
        "browser wait-for",
        "<tab-id> <selector> [ms]",
        "Wait for an element (default 5000ms, maximum 60000ms).",
    ),
    (
        "browser wait-for-navigation",
        "<tab-id> [ms | --timeout-ms N]",
        "Wait for navigation (default 10000ms, maximum 60000ms).",
    ),
    (
        "browser storage get",
        "<tab-id> <key> [local|session]",
        "Read a browser storage value.",
    ),
    (
        "browser storage set",
        "<tab-id> <key> <value> [local|session]",
        "Set a browser storage value.",
    ),
    (
        "browser storage clear",
        "<tab-id> [local|session]",
        "Clear browser storage.",
    ),
    (
        "browser cookies get",
        "<tab-id> [url]",
        "Read browser cookies.",
    ),
    (
        "browser cookies set",
        "<tab-id> <name> <value> <domain> [--path PATH] [--secure] [--http-only] [--expires UNIX-SECONDS]",
        "Set a browser cookie.",
    ),
    (
        "browser cookies delete",
        "<tab-id> <name> [domain]",
        "Delete a browser cookie.",
    ),
    (
        "browser cookies clear",
        "<tab-id>",
        "Clear browser cookies.",
    ),
    (
        "git",
        "<project> <action-json> [--json]",
        "Run a raw protocol GitAction and print GitReply JSON. Actions can modify repositories; watch is unavailable.",
    ),
    (
        "files",
        "<project> <action-json> [--json]",
        "Run a raw protocol FilesAction and print FilesReply JSON. Paths are project-relative byte arrays; actions can write or delete files.",
    ),
    (
        "exec",
        "<project> [--json] [--timeout-ms N] -- <program> [args...]",
        "Execute a program in the project directory without a shell. Timeout defaults to 30000ms, maximum 300000ms. For shell syntax use /bin/sh -c explicitly.",
    ),
    (
        "status",
        "<state> [--app APP] [--id ID] [--progress N] [--kind KIND] [--title TEXT] [--msg TEXT]",
        "Report idle, working, done, blocked, error or clear to this terminal (OSC 7501). Needs no server.",
    ),
    (
        "install-skills",
        "[--dir DIR]...",
        "Install the Muxy CLI skill into existing agent directories and any extra --dir folders.",
    ),
    (
        "stdio",
        "[--no-start]",
        "Bridge stdin/stdout to the server. Used by remote clients; --no-start refuses to start a missing server.",
    ),
    (
        "completion",
        "<bash|zsh|fish>",
        "Print shell completions for commands and options without contacting a server.",
    ),
    (
        "version",
        "",
        "Print the version of this CLI without contacting a server.",
    ),
];
