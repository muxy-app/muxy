# Sandboxed terminals

Sandboxed terminals are experimental and currently available on local macOS.
They use the separately installed [nono 0.79.0](https://github.com/nolabs-ai/nono/releases/tag/v0.79.0)
executable. Nono's security guarantees are still under development.

In **Settings → Server**, set the nono executable and approve any additional
tool installations and runtime paths. These paths are read-only. The project
folder is writable, and each session has its own temporary home and cache.
The real home, credentials and shell startup files are not shared automatically.
Approved environment names explicitly forward values from the server's environment.

Choose **New Sandboxed Terminal** from a project's menu. Select blocked networking
or the configured list of approved domains. Unrestricted networking is unavailable
with this backend version because it also exposes host control sockets.
Hover over the terminal's sandbox label to inspect its permissions.

Splits inherit the policy. Reattaching preserves it. Settings changes affect new
terminals; an ordinary running terminal cannot be converted into a sandbox.
If sandbox setup fails, the terminal does not start outside the sandbox.

The CLI uses the same settings:

```sh
muxy settings set sandbox-executable /absolute/path/to/nono
muxy settings set sandbox-domains api.example.com
muxy settings set sandbox-network domains
muxy session create MyProject --sandbox
```

Tool paths are separated by semicolons in `sandbox-tools`; domain and environment
name lists use commas. Runtime dependencies must also be approved. For example,
a Rust build needs its toolchain and the selected Apple command-line tools/SDK.

Files inside the workspace, including credentials stored there, are accessible.
Linked Git worktrees can be edited, but Git operations needing shared metadata
outside the workspace are denied. Session homes are removed when sessions end;
authenticate inside each session or explicitly forward the required credentials.
