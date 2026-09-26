# Muxy technical design

These documents define how the [product model](../product/README.md) is
implemented. They distinguish the current runtime from deferred design choices.

1. [Architecture](./architecture.md) — processes, components, and boundaries.
2. [Protocol](./protocol.md) — wire contract and compatibility policy.
3. [Constraints](./constraints.md) — platform and implementation constraints.
4. [Decisions](./decisions.md) — choices, rationale, and rejected alternatives.
5. [Benchmarks](./benchmarks.md) — historical measurements and methodology.
6. [Mobile SDK](./mobile-sdk.md) — building the phone apps on `muxy-mobile`.

The separate `muxy` CLI/TUI and `muxy-app` desktop use one client library and
protocol to connect to `muxy-server`; paired phones embed the same library
through the `muxy-mobile` SDK. The server owns Ghostty terminals and retained
history; clients render screen updates without parsing terminal output.
Connections carry CBOR with numbered fields, and postcard screen rows, over local
Unix sockets or TLS for paired phones, with merged pending screen frames and
per-channel acknowledgements. Builds that share a protocol version can talk.
Streaming wire compression remains deferred.
