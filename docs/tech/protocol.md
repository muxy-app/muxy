# Protocol

Clients and the server talk over a reliable, ordered local byte stream. This
contract covers projects, terminal sessions, and server capabilities. Exact
types, kind numbers, and limits live in the protocol crate and its fixtures;
this document says what they mean.

## Versions

Versions describe how peers encode requests and responses, not whether released
clients and servers may communicate. Every released version remains supported
permanently. Peers negotiate a mutually supported contract, so a newer release
can always communicate with an older release.

Until the first official release there is one mutable development schema, V1.
All current messages use it. Do not add versions or pre-release compatibility
adapters. Development app and server builds must use the same schema and beta compatibility identifier. This does
not permit losing saved user data when storage formats change.

After release, versioned contracts are immutable and the envelope and hello
remain stable. New contracts may be added, but released contracts are never
removed or inferred from a highest version number alone.

## Framing

| Field | Size | Meaning |
| --- | --- | --- |
| length | `u32` | Bytes that follow |
| version | `u16` | Version this frame is written in |
| channel | `u32` | `0` is control; each attachment gets its own |
| kind | `u8` | Six bits of kind, two flag bits reserved for compression and continuation |
| payload | the rest | Postcard, except raw terminal input |

Integers are little endian. A frame is at most 16 MiB. The flag bits are
zero in v1.

## Handshake

The client opens with a hello listing its versions and beta compatibility
identifier and waits. The reply identifies the running server build and process
instance. The hello encoding and update metadata stay stable across beta schema
changes; incompatible identifiers are rejected before ordinary requests. The server
replies with its own; both choose a mutually supported contract. A malformed or
unsupported implementation may be rejected and closed. Official releases always
share a supported contract. Any other traffic before hello is fatal.

## Messages

| Message | From | Channel |
| --- | --- | --- |
| Hello, hello reply, version unsupported | client, server | control |
| Attach, detach, resize, and their replies | client, server | control |
| Open-pane references, conditional close, and their replies | client, server | control |
| List, create, and end session, and their replies | client, server | control |
| Project catalog pages, field mutations, deletion, and their replies | client, server | control |
| Catalog, session-list, and watched activity revision invalidations | server | control |
| Read activity, acknowledge events, claim desktop delivery, and their replies | client, server | control |
| Identify client and its reply | client, server | control |
| Read saved terminal content, discard session and saved content, and their replies | client, server | control |
| History page and search, and their replies | client, server | control |
| Set terminal colors and its reply | client, server | control |
| Read and write server settings, stop server, and their replies | client, server | control |
| Run or cancel a project command, and their replies | client, server | control |
| Ping, pong | client, server | control |
| Frame ack | client | control |
| Session ended, server restarting for an update | server | control |
| Error | server | control |
| Input, cell pixel dimensions | client | session |
| Acknowledged input write and its reply | client, server | control |
| Screen frame, metadata event | server | session |

A request carries a client-chosen ID and gets exactly one reply, in any
order. Errors about a request, such as a bad path, size, limit, or cursor,
an unknown session or channel, or a failed spawn, are correlated and leave
the connection usable. Anything malformed or out of place is fatal: the
server reports it and closes, and a client that sees it closes. Only the
server sends errors. Any number of clients may attach to one session. Connected
clients register the sessions used by all their open panes, independently of visible output
subscriptions. A pane close ends and discards a session only when no other
connected pane uses it; the server makes that decision atomically and preserves
it across retries. Disconnecting or quitting a client never ends its sessions.

## Projects and membership

Project descriptors carry stable identity, Home status, Unix directory bytes,
shared metadata, kind, and parent. Icons retain existing emoji values or identify
SF Symbols; optional logos carry bounded square PNG images. Catalog pages are
bounded by count and bytes; coalesced revision invalidations tell clients when to refetch without losing changes during a fetch.
Mutations acknowledge durable storage. Each session creation carries an explicit
project and a durable client operation token; retrying returns the same result,
including an ended result. Live-session lists remain live-only; project-filtered
lists distinguish live sessions from retained ended content. Membership and
lifecycle changes also advance the catalog revision.

Each connection represents a client instance independently of its saved layout
and can identify itself as desktop, TUI, or CLI. Session listings expose the
current owner and whether the requesting client is attached. Attachment changes
invalidate session listings without writing attachment state to storage. Repeated
attachments and reference updates preserve a client's position; disconnecting
removes it from the attachment order.

Startup leaves an incompatible running server and its sessions intact; see
[beta compatibility](#beta-update-compatibility).

## Screen

Rows are style runs with server-supplied cell boundaries, so the client
needs no width table. A row in a message replaces that row entirely. Frames
carry only visible rows, are numbered from one per attachment, and are
acked cumulatively. A resize carries the whole screen as one reset.
Synchronized output defers screen publication until the application finishes its
update, with a one-second recovery timeout. Frames and attach snapshots also
carry bounded Kitty image pixels and placements. Image replacements travel
atomically with their screen; unchanged image state is omitted from deltas.
Client cell pixel dimensions let the server resolve image sizes and positions.

OSC 8 hyperlinks are bounded URI spans sent as whole-screen metadata
replacements, including an empty replacement when cleared. Their attachment
frame sequence prevents activation ahead of the matching screen; zero refers
to the attach snapshot. History carries no OSC 8 links. Detecting plain links
and choosing browser, editor, or Finder openers are app policy.

Composer uses bounded acknowledged input writes. Success confirms that the server
wrote the bytes to the PTY, not that the terminal application processed them. A
failed or unconfirmed write is never automatically retried. Ordinary keystrokes
continue to use streamed input without acknowledgements.

## Terminal colors

A client can send RGB defaults for foreground, background, cursor,
and all 256 palette colors, plus default cursor shape and blinking. These
defaults apply to sessions created or attached through that connection and
updates are queued to its existing attachments. New sessions receive them before processing terminal output.
The emulator uses them to answer terminal color queries; theme selection
stays in the app. Terminal programs retain their runtime overrides.
Like size, defaults are session-wide: the latest update
or colored attach wins, and detaching leaves them unchanged. Clients resend
colors on reconnect and theme changes.

## Server settings

Clients can read and write the default shell, per-session history budget, and
shell integration preference. A successful write confirms validation and durable
storage before new sessions use the values. Existing sessions are unchanged; the
saved-history budget takes effect at the next server start. Stop acknowledges the
request, then gracefully ends sessions and closes connections, preserving saved
output. Conditional stop atomically reserves shutdown only if no session is live
or being created; otherwise it replies busy and leaves the server running. An accepted idle
update stop notifies all clients that the server is restarting, so they can
reconnect with bounded retries. Ordinary stop never requests reconnection. Restart is app policy: wait for shutdown before starting and reconnecting.

## Attach and metadata

Attach returns an atomic snapshot: size, screen, cursor, recent history
with a cursor to older rows, title, and working directory. The initial
hyperlink replacement follows the snapshot on the attachment channel.
Title, directory, process, and bell events follow on that channel; bell is
transient. Live prompt starts accompany attach snapshots and history pages,
indexed within their history rows followed by their screen rows. Screen prompt
updates are full replacements tied to an attachment frame sequence, so marks
never get ahead of the screen. Saved records do not yet retain prompt marks.
The app derives the pane title as program title, then process
name, then working directory, and the foreground process's shell flag
drives close confirmation.

## History

The server never pushes history; the app pages for it. A page or search
request freezes a view of the retained rows and walks it with an opaque
cursor until exhausted. Reflow or eviction makes a cursor stale. Search is
literal, optionally case-insensitive, within a row, and bounded in
results and work, so an empty reply may still continue.

## Paths

Server paths are lossless Unix bytes, preserved as sent. NUL is rejected
only when a path is handed to the OS.

## Lifecycle

Session ended carries the exit reason and reaches every connection once,
after final terminal content is saved and the session leaves live listings.
Saved content is read by session ID without creating an input channel.
Discard ends a live process and removes its saved content; repeated discard
is harmless. Its reply confirms completion, including pending saves.
Detach and session end retire the channel and drop its pending output;
late traffic on it is ignored.
Ordering across channels is guaranteed only at handshake, attach, resize,
metadata watermark, detach, and session end.

## Flow control and deferred compression

The runtime implements per-channel frame merging, acknowledgement credits, and
control-first writing as described in the [architecture](./architecture.md#client-connection).
Streaming wire compression and chunking remain deferred. D6 remains the
compression target; any wire changes follow the version policy above.

## Beta update compatibility

The compatibility identifier is separate from the build and V1 wire version.
Bump it when encoding, required behavior, or shared storage and resources make
mixed builds unsafe. Wire fixtures and the beta compatibility declaration are
checked in CI; behavior and storage compatibility also require release review.
Regenerate the declaration with `python3 scripts/beta_compatibility.py --write`
after reviewing and changing the identifier. No beta schema adapters are kept.

Signed update candidates report build metadata without starting a server.
Matching identifiers permit an app update while the older server continues;
otherwise installation waits for an atomic idle stop or explicit destructive
confirmation. Scheduling stays in the app. Pre-metadata beta updaters retain
their existing restart behavior for the transition release.

## Extension commands

Command requests identify a server project and a connection-scoped job. The app
checks extension permissions and consent before sending them. The server bounds
input, output, concurrency, and execution time; cancellation and disconnect stop
the command’s process group. Completion reports exit status and whether output
was truncated, timed out, or cancelled.

## AI activity

Session title updates follow open-pane references independently of screen attachments.

Reading activity subscribes to coalesced revision invalidations. Snapshots include
all detected sessions and up to 200 pending events, at most one per session,
independently of terminal attachments. Acknowledgements name observed event IDs
and idempotently remove them from shared memory. Ending a session removes its
activity; server restart starts with no activity history. Delivery claims are
limited to the elected desktop
client and prevent duplicate alerts within a server lifetime. Clients establish a
fresh notification baseline on reconnect.
