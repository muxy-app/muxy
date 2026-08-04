# demo-docker-sleep

Demo for the `worktree.offline` extension event.

When every terminal pane in a worktree goes offline (Muxy released their idle surfaces), the
background script runs `docker compose stop` in that worktree. When the worktree wakes — a pane
materializes, a new terminal is created there, or its last terminal closes — it runs
`docker compose start` for the stacks it stopped itself.

## Trying it

1. Copy this directory to `~/.config/muxy/extensions/demo-docker-sleep` and reload extensions.
2. Point a Muxy worktree at a repo containing a `docker-compose.yml` and bring the stack up
   (`docker compose up -d`).
3. Settings → Terminal → lower the offline idle threshold so panes sleep quickly.
4. Switch to another worktree and wait for the idle threshold to pass. A "Docker paused"
   notification confirms the stack was stopped.
5. Switch back. A "Docker resumed" notification confirms it was started again.

The first `docker` call prompts for `exec` consent — "Allow & remember" keeps the demo quiet
afterwards.

## What it demonstrates

- `worktree.offline` fires only once **every** terminal pane in the worktree is offline, so a
  worktree with one active terminal never suspends.
- The payload carries `worktreePath`, so the background script can act on the directory without
  `muxy.worktrees.list()` (which background scripts do not have).
- Every `offline: "true"` is followed by an `offline: "false"`, so the stack is never left stopped.
- Only services this extension stopped are restarted, so services that were already stopped stay
  stopped. Work is serialized per worktree so a fast sleep/wake cycle cannot interleave a `stop`
  with a `start`.
