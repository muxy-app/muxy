# Layouts

Muxy can apply named pane/tab layouts to a worktree on demand. Layouts live in-repo under `{Project.path}/.muxy/layouts/` so they can be checked in alongside the project.

## Behavior

- Each file in `.muxy/layouts/` defines one named layout. The file name (without extension) is the layout's name.
- When at least one layout exists for the active worktree, a layout picker appears in the window's top bar.
- Selecting a layout asks for confirmation; on accept, all current terminals and tabs in that worktree are closed and the layout is applied.
- Layouts are not auto-applied on project open — the user picks one explicitly.

## File location

```
<project-root>/.muxy/layouts/
  dev.yaml
  release.yaml
  scratch.json
```

Supported extensions: `.yaml`, `.yml`, `.json`.

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

The same schema works as JSON at `.muxy/layouts/<name>.json`:

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
