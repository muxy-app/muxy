# AI Usage

Muxy can read usage / quota data from common AI coding providers and surface it in a sidebar popover. Toggle the popover with `Cmd+L`, or from the **View → AI Usage** menu.

## Supported providers

- Claude Code
- GitHub Copilot
- OpenAI Codex CLI
- Cursor CLI
- Amp
- Z.ai
- MiniMax
- Kimi
- Factory

Enable / disable each one in **Settings → AI Usage**.

## What's shown

Per provider, you see the metrics that provider exposes — typically some combination of:

- Session / 5h / hourly windows
- Premium request count
- Daily / weekly / monthly limits
- Billing period summary

Toggle **Show Secondary Limits** in settings to keep the popover compact.

## Where the data comes from

Muxy reads tokens / credentials from each provider's standard locations: environment variables, local credential JSON files in your home directory (e.g. `~/.claude`, `~/.cursor`), and the macOS Keychain. For providers that need OAuth refresh (Claude Code, Factory, Kimi), Muxy refreshes tokens silently before fetching usage.

Nothing is sent to Muxy's servers — requests go directly from your Mac to each provider.

## Auto‑refresh

Choose an interval in **Settings → AI Usage**: Off / 5m / 15m / 30m / 1h. Manual refresh is always available from the popover.

## Hook integrations

For Claude Code, OpenCode, Codex, and Cursor, Muxy can also receive real‑time usage and notification events through hook scripts that ship with the app. See [Notifications](notifications.md) for the shared hook setup.
