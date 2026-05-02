# Declarative Startup Layout

Muxy can reproduce a fixed pane/tab layout every time a project is opened. The layout lives in-repo at `{Project.path}/.muxy/startup.yaml` (or `.yml`/`.json`), so it can be checked in alongside the project.

## Behavior

When `startup.yaml` exists, it overrides any persisted workspace state for that worktree on every project open — the layout is the source of truth.

If the file is absent, Muxy restores the previous workspace from disk as before.

## Model

A Muxy workspace is a tree of panes inside a single window. Each leaf pane is a stack of tabs (one tab visible at a time). Panes can be nested with horizontal or vertical splits.

The config mirrors that:

- A node is either a **leaf** (`tabs:`) or a **branch** (`layout:` + `panes:`).
- Branches may be nested arbitrarily.

## Schema

### Single pane with tabs

```yaml
tabs:
  - name: editor
    command: nvim
  - name: shell
```

### Two-pane horizontal split

```yaml
layout: horizontal
panes:
  - tabs:
      - name: editor
        command: nvim
  - tabs:
      - name: shell
```

### Nested splits

```yaml
layout: horizontal
panes:
  - tabs:
      - name: editor
        command: nvim
  - layout: vertical
    panes:
      - tabs:
          - name: logs
            command: tail -f /tmp/app.log
      - tabs:
          - name: btop
            command: btop
```

### Fields

- `layout` — `horizontal` (panes side-by-side) or `vertical` (panes stacked). Defaults to `horizontal`.
- `panes[]` — child panes. Required when `layout` is set; mutually exclusive with `tabs`.
- `tabs[]` — tabs in this pane. Required for leaves.
  - `name` — optional. Tab title. Defaults to the first word of `command`, or `Terminal`.
  - `command` — optional. String, or a list of strings joined with `&&`:
    ```yaml
    tabs:
      - name: setup
        command:
          - cd src
          - npm install
    ```
  - A tab may also be written inline as a bare string command:
    ```yaml
    tabs:
      - htop
    ```

## JSON

The same schema works as JSON at `.muxy/startup.json`:

```json
{
  "layout": "horizontal",
  "panes": [
    { "tabs": [{ "name": "editor", "command": "nvim" }] },
    {
      "layout": "vertical",
      "panes": [
        { "tabs": [{ "name": "logs", "command": "tail -f log" }] },
        { "tabs": [{ "name": "btop", "command": "btop" }] }
      ]
    }
  ]
}
```
