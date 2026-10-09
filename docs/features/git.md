# Git

For Git projects, the status bar shows the branch, the changes, and pull
request state. Commits and pull requests are written by an AI command-line
tool you already have.

## Branch and changes

- **Branch:** switch, create, or delete branches.
- **Changes:** see changed files with line counts. Stage, unstage, or discard
  them, one at a time or all at once.

## Commit and push

**Commit** asks the AI for a commit message, then commits. Before it starts,
pick the AI tool, whether to include unstaged changes or only what's staged,
and whether to push. Muxy remembers these choices for next time. You can also
add one-off instructions for the AI.

## Create a pull request

**Create PR** creates a new branch, commits your changes, pushes, and opens a
pull request with an AI-written title and description. It follows the
repository's pull request template when there is one. It needs uncommitted
changes and a GitHub remote named `origin`.

When the branch has a pull request, the status bar shows its number, state,
and checks. From there you can merge (merge commit, squash, or rebase), update
it from its base branch, close it, or open it on GitHub. In a worktree, you can
also remove the worktree after merging: Muxy asks first, then switches to the
primary checkout.

Pull request features need the [`gh`](https://cli.github.com) CLI, installed and
signed in on the project's server.

## AI tools

Muxy uses the first installed of: Claude Code (`claude`), Codex (`codex`),
OpenCode (`opencode`), GitHub Copilot (`copilot`), Cursor (`cursor-agent`),
Droid (`droid`), Grok (`grok`), Kiro CLI (`kiro-cli`), Pi (`pi`), Xal (`xal`),
and Antigravity (`agy`).

In **Settings → AI**, pick the tool for commits and for pull requests, and edit
their prompts. A project can have its own pull request prompt, set from the
**Create PR** menu.

- Sign in to the tool in a terminal first. Muxy runs it without prompts.
- The tool runs on this Mac, in the project folder. For a
  [remote project](remote-servers.md), the same folder must exist here.
- Muxy itself always does the staging, committing, pushing, and pull request.
  The AI only writes the text.
