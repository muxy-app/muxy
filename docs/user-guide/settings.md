# Settings

Open settings with `Cmd+,` (**Muxy -> Settings...**). Use search at the top to find settings by name.

## Remote Macs

On the Mac that owns the projects, enable **Allow desktop connections** under **Remote Access**. On the other Mac, open **Remote Devices**, add a **Muxy Mac**, and select a discovered Mac or enter its host and port. Approve the first connection on the host.

The connected Mac then appears in the workspace switcher. Selecting it loads the host's projects into the same project sidebar, tab strip, split layout, title bar, and terminal interface used for local workspaces. Actions supported by the remote-server API operate on the host, while Mac A keeps its local workspace state separate. Terminal panes request control only while visible and release it when hidden. Browser, extension, and source-control tabs use the normal tab UI, but their content is shown only on the host until those content types can be streamed.

Remote access uses unencrypted WebSockets. Use it only on a trusted local network or through a private VPN such as Tailscale. Pairing tokens remain in Keychain and are never included in backups.
