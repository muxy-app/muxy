# Decisions

The key technical choices, why they were made, and what was turned down.

Most numbers come from early prototypes, measured on an Apple M3 with release
builds. Every candidate replayed the same recorded terminal output: an idle
shell, large log dumps, a vim session, a full-screen monitor, heavy color
changes, Unicode, and long lines with resizes, plus 100 sessions at once. The
raw reports are in git history.

| | Decision | In short |
| --- | --- | --- |
| D1 | Ghostty is the terminal engine | Least memory, fastest parsing |
| D2 | The server owns the screen; apps only draw | Small apps, instant attach |
| D3 | History is a byte budget | Predictable memory |
| D4 | Rows travel as style runs | 4 to 8 times smaller than cells |
| D5 | CBOR messages, postcard rows | Messages can grow, rows stay compact |
| D6 | zstd streaming compression, planned | Halves interactive traffic |
| D7 | Any byte stream is a transport | Unix socket locally, SSH to other computers, TLS for phones |
| D8 | One merged frame in flight | Slow apps never pile up |
| D9 | The app draws rows directly | Redraws only on change |
| D10 | portable-pty for PTYs | Same speed, ready for Windows |
| D11 | Phones pin a certificate, or use SSH | No cloud; pairing needs no SSH setup |
| D12 | Protocol changes are additive | Old and new builds keep talking |

## D1. Ghostty is the terminal engine

The server keeps every terminal in libghostty-vt. With compressed history it
uses about 1.7 MB per 10,000 rows, against 51 MB for Alacritty, and it parsed a
colored build log faster than every alternative. 100 sessions, 30 of them busy,
fit in 135 MB.

Turned down: Alacritty for memory, including a 1.3 GB peak on resize; WezTerm's
core for speed; vt100 for memory and wrong combining marks; a custom grid,
because Ghostty already beats it.

## D2. The server owns the screen; apps only draw

Four designs were built and compared. With the screen on the server and rows
sent to apps, 100 sessions used 9 MB in the app, sent under 2 percent of the raw
output, and attached in 1 ms. Streaming raw bytes to apps used 2.3 GB, sent
everything, and attached more slowly the more history there was.

Turned down: raw byte streaming and a hybrid. Every app would parse everything
again and keep its own history.

## D3. History is a byte budget

Ghostty limits history by bytes, and with compression a byte limit gives
predictable memory where a row limit doesn't. The setting is in bytes, and the
server reports how many rows it holds.

## D4. Rows travel as style runs

A row is sent as runs of text that share a style. That is 4 to 8 times smaller
than one record per cell, smaller than re-sending escape codes, quick to decode,
and needs no terminal emulator in the app.

## D5. Messages are CBOR; screen rows are postcard

Screen rows use postcard, the most compact format tested, and the same bytes the
server saves to disk. Everything else is CBOR with numbered fields, so builds can
add fields without breaking each other (D12). It costs a few bytes per frame.

## D6. Compression will be zstd with a streaming context

Not built yet. zstd level 1 with one context per connection halved interactive
traffic for about 15 µs per frame.

Turned down: lz4 for its ratio, higher zstd levels for speed, and trained
dictionaries, which fit their training data and little else.

## D7. Any byte stream is a transport

Unix sockets, TCP, and stdio pipes were all ten times faster than any real
program writes to a terminal. Apps on the same computer use a Unix socket and
phones use TLS (D11). Other computers are reached over SSH, which runs
`muxy stdio` there to join its stdin and stdout to that computer's server. ssh
logs in with the user's keys, agent, or certificates. For a password login, the
desktop asks for the password, keeps it only in memory, and answers ssh's
prompt for it.

Turned down: a stream multiplexing library, because D8 fits terminals better.

## D8. One merged frame in flight

Each session has at most one waiting frame per app. Newer output merges into it,
and it is sent once the app acknowledges the previous one. For an app that takes
3 ms per frame, this cut server memory from 128 MB to 3 MB and the time to reach
the latest screen from 94 s to 10 s. Nothing is lost, because a newer row
replaces an older one.

## D9. The app draws rows directly

The app draws one text line per row and one rectangle per color run, and redraws
only when a frame arrives. Sixteen panes of vim held 60 fps on about a third of
one core. Caching shaped text didn't help, since painting is the cost. A full
terminal emulator in the app could be added later without changing the server.

## D10. PTYs use portable-pty

Every candidate was equally fast, because the kernel sets the pace.
portable-pty also supports Windows behind the same API.

## D11. Phones pin a certificate and get their own token

Phones connect over TLS 1.3 to a self-signed certificate that they pin when
scanning the pairing code. Each phone gets its own token, which the server keeps
only as a hash. The phone apps embed the same Rust client as the desktop app.

Pairing stays the default because it needs no SSH setup. Phones can also
connect over SSH, since the apps already embed an SSH client: it runs
`muxy stdio` on the computer (D7).

Turned down: a cloud relay, which is another service to trust; mutual TLS, for
handling certificates on phones; and a native Swift, Kotlin, or JSON protocol,
which would drift from the Rust one.

## D12. Protocol changes are additive

During the beta, one compatibility number changed 20 times in 15 days, each
time forcing a server restart and locking out phones. Now fields have permanent
numbers, builds skip what they don't know, and only breaking changes bump the
version. See the [protocol](./protocol.md#versions).

Turned down: a version per change, protobuf, JSON, and a custom postcard
extension.
