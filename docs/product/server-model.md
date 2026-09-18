# Server model

The server runs independently of its clients and provides terminal sessions and
other capabilities. [Ownership](./product-model.md#relationships-and-ownership)
and [client presentation](./app-model.md) are defined separately.

## Server lifetime

Clients start `muxy-server` if needed. It keeps running until stopped or replaced
for a pending update after all sessions end. Stopping or restarting it ends
its processes while preserving settings and saved terminal content. Graceful
shutdown saves final output; an abrupt stop may lose output since the last
completed checkpoint. Recovery never restarts a process.

## Sessions and attachment

Each session has a server-generated ID and belongs to exactly one project.
Creation specifies its project and starting directory; the directory never
determines membership. Quick Terminal and ad-hoc sessions belong to Home.

A session runs until its process exits, a client ends it, or the server stops.
It has no automatic expiry. Quitting, detaching, a client crash, or device sleep
does not end it. Any number of clients may attach simultaneously to receive
output and send input; clients may display it in different layouts. Session
size is shared; policy for conflicting client sizes is deferred.

Each session keeps its attached clients in attachment order. The creating client
attaches first. The earliest remaining client is the session owner; when it
detaches, the next client becomes owner. A session with no attached clients has
no owner. Open panes in inactive tabs remain attached. Ownership identifies a
client and does not change project membership or session permissions.

Attach supplies the current screen and a recent history window. Clients fetch
older history in pages up to the retention limit, configured as a byte budget.
The server reports rows retained and saves the last screen and retained history
even without attached clients. Ending a process does not shrink its retained
history. Saved content remains addressable by session ID until discarded.

Live-session lists contain running processes. Project session lists also expose
retained ended content. An unreachable server means its sessions are
unreachable, not necessarily ended.

## Closing panes

By default, closing a terminal pane removes it from the client's layout immediately.
Its session and saved content remain while another open pane in any connected
client uses them, including inactive tabs. Closing the final connected pane
ends the process and discards its saved content. If the server is unreachable,
the close remains pending; on reconnection the server checks remaining
references before deciding whether to end it.

Desktop users can choose to detach when closing tabs or panes. Detached sessions
keep running and can be reopened from Existing Terminals.

Before ending a foreground program other than the shell, the user confirms
once. Background jobs alone do not trigger confirmation. Ordinary shell jobs
follow normal terminal exit behavior; independently detached work is not
targeted. When a process exits, clients close its panes automatically using these same
reference and cleanup rules.

Explicitly ending or discarding a session and
[deleting its project](./product-model.md#failed-projects-and-deletion) affect
all clients, unlike detaching one pane.

## Other capabilities

Git provides worktree listing, creation from a branch and directory, and
removal. File operations and further server capabilities may be defined later.

## AI activity

The server detects supported foreground AI agents from their processes and live
terminal screens, without provider hooks or plugins. Detection continues without
attached clients. Screen recognition is best effort; unfamiliar interfaces may
not expose every state.

Agent status and the latest 200 attention/completion events belong to the server.
History and read acknowledgements survive restart and are shared across clients.
Reading an event does not change a blocked agent's live status.
