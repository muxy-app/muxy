# Protocol

Clients and the server talk over a reliable, ordered byte stream: a local
socket, or TLS for [paired devices](#paired-devices). This contract covers
projects, terminal sessions, and server capabilities. Exact
types, kind numbers, and limits live in the protocol crate and its fixtures;
this document says what they mean.

## Versions

A version changes only for a breaking change. Additive changes keep it, so any
two builds that share a version can talk. During the beta each build speaks one
version, currently V2. After the first release, older versions stay supported.

Fields and enum variants carry permanent numbers. A peer skips fields it doesn't
know, reads missing optional fields as absent, and keeps unknown values of
enums the server sends as unrecognized. Numbers are never reused. See the
[compatibility rules](#compatibility-rules). None of this permits losing saved
user data when storage formats change.

## Framing

| Field | Size | Meaning |
| --- | --- | --- |
| length | `u32` | Bytes that follow |
| version | `u16` | Version this frame is written in |
| channel | `u32` | `0` is control; each attachment gets its own |
| kind | `u8` | Six bits of kind, two flag bits reserved for compression and continuation |
| payload | the rest | A CBOR array of the message's fields, except raw terminal input |

Integers are little endian. A frame is at most 16 MiB. The flag bits are
zero. Screen rows inside a payload keep the postcard layout saved records use.
A kind a build doesn't know is ignored.

## Handshake

The client opens with a hello listing the versions it speaks and waits. The
server replies with its versions, its build and process instance, and the
features it offers; both use the highest shared version. With no shared version
the server replies version unsupported and closes. Hello, hello reply, and
version unsupported keep their framing across versions, and a peer from before
V2 gets version unsupported in its own framing. Any other known traffic before
hello is fatal.

## Messages

| Message | From | Channel |
| --- | --- | --- |
| Hello, hello reply, version unsupported | client, server | control |
| Attach, detach, resize, and their replies | client, server | control |
| Open-pane references, conditional close, and their replies | client, server | control |
| List, create, and end session, and their replies | client, server | control |
| Project catalog pages, field mutations, deletion, and their replies | client, server | control |
| Changed: catalog, session-list, activity, and mobile access revision invalidations | server | control |
| Read activity, acknowledge events, claim desktop delivery, and their replies | client, server | control |
| Identify client and its reply | client, server | control |
| Read saved terminal content, discard session and saved content, and their replies | client, server | control |
| History page and search, and their replies | client, server | control |
| Set terminal colors and its reply | client, server | control |
| Read and write server settings, stop server, and their replies | client, server | control |
| Run or cancel a project command, and their replies | client, server | control |
| Authenticate or pair, and their replies | device, server | control |
| Read and write mobile access, start and cancel pairing, revoke a device, and their replies | client, server | control |
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
the connection usable. A request the server can't read, such as a method from a
newer client, gets a correlated unsupported error, and a client treats a reply
it can't read the same way. Anything malformed or out of place is fatal: the
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
and can identify itself as desktop, TUI, CLI, or mobile. Network connections
are always mobile, and local connections cannot claim to be. Session listings expose the
current owner and whether the requesting client is attached. Attachment changes
invalidate session listings without writing attachment state to storage. Repeated
attachments and reference updates preserve a client's position; disconnecting
removes it from the attachment order.

Startup leaves an incompatible running server and its sessions intact; see
[compatibility rules](#compatibility-rules).

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

## Paired devices

A network connection sends Hello, then exactly one Authenticate or Pair
request. Until that succeeds, frames are small, one deadline covers TLS, Hello,
and authentication, and only a few connections wait at once. Every
authentication failure (unknown device, wrong token, revoked device, or access
turned off) gets the same correlated Unauthorized reply before the connection
closes, so a phone can tell that it must pair again. Pairing links carry the
server's addresses, port, certificate fingerprint, and a one-time secret:
`muxy://pair?v=1&h=HOST&p=PORT&f=FINGERPRINT&s=SECRET`. The server's identity
and name arrive in the Paired reply.

Paired devices cannot manage mobile access, write server settings, stop the
server, or run extension commands; everything else behaves as for local
clients. Revoking a device or turning mobile access off closes its
connections.

## Compatibility rules

| Change | How | Version bump |
| --- | --- | --- |
| Add a field | Next unused number; optional or defaulted | No |
| Add a variant | Next number. Clients show a neutral fallback for enums the server sends; otherwise an older peer fails just that request or skips that event | No |
| Add a request | New method; older servers reply unsupported. Add a feature only when a client must hide it for older servers | No |
| A new field the server must act on | A new method or feature, since older servers ignore unknown fields | No |
| Remove anything | Stop using it and keep its number reserved | No |
| Change a type or meaning, make an optional field required, change screen rows or the header | Breaking | Yes |

Encoded samples of each version live in
`crates/muxy-protocol/tests/fixtures/v<N>` and are append-only within it. CI
checks that the current build reads every fixture and that the last release
reads the current ones.

App updates keep the running server when both builds share a protocol version;
otherwise installation waits for an atomic idle stop or explicit destructive
confirmation. Build metadata lists the protocol versions. Updaters from before
V2 read a frozen compatibility identifier instead. Scheduling stays in the app.

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
