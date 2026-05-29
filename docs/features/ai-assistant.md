# AI Assistant

Muxy can ask an AI CLI to draft commit messages and pull request text from your git diff.

It runs locally installed tools. Muxy does not proxy prompts through its own server.

## Where it appears

| Place | Action |
| --- | --- |
| Source Control commit box | Generate commit message |
| Create PR sheet | Generate title and description |

The generated text is editable before you commit or create a PR.

## Providers

Choose the tool in **Settings -> AI Assistant**:

- Claude
- Codex
- OpenCode
- Custom command

Optional model overrides are available for Claude, Codex, and OpenCode.

## Prompts

Settings includes separate prompts for:

- Commit messages.
- Pull request drafts.

Reset a prompt to return to Muxy's default.
