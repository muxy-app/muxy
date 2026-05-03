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

## Examples

The examples below live in `.muxy/layouts/` in this repo and double as a reference for the schema. Each diagram shows the resulting window with panes drawn as boxes; tabs are listed at the top of their pane.

### `single.yaml` — one pane, multiple tabs

```yaml
tabs:
  - name: shell
  - name: pwd
    command: pwd
  - htop
```

```
┌─[ shell | pwd | htop ]──────────────┐
│                                     │
│                                     │
│                                     │
└─────────────────────────────────────┘
```

### `side-by-side.yaml` — editor next to a shell

```yaml
layout: horizontal
panes:
  - tabs:
      - name: editor
        command: nvim .
  - tabs:
      - name: shell
```

```
┌─[ editor ]──────────┬─[ shell ]─────────┐
│                     │                   │
│  nvim .             │                   │
│                     │                   │
└─────────────────────┴───────────────────┘
```

### `stacked.yaml` — two panes stacked vertically

```yaml
layout: vertical
panes:
  - tabs:
      - name: top
  - tabs:
      - name: bottom
```

```
┌─[ top ]─────────────────────────────┐
│                                     │
├─[ bottom ]──────────────────────────┤
│                                     │
└─────────────────────────────────────┘
```

### `tri-row.yaml` — three columns

```yaml
layout: horizontal
panes:
  - tabs:
      - name: left
  - tabs:
      - name: mid
  - tabs:
      - name: right
```

```
┌─[ left ]──────┬─[ mid ]──────┬─[ right ]─────┐
│               │              │               │
│               │              │               │
└───────────────┴──────────────┴───────────────┘
```

### `quad.yaml` — 2×2 grid via nested splits

```yaml
layout: horizontal
panes:
  - layout: vertical
    panes:
      - tabs:
          - name: tl
      - tabs:
          - name: bl
  - layout: vertical
    panes:
      - tabs:
          - name: tr
      - tabs:
          - name: br
```

```
┌─[ tl ]──────────────┬─[ tr ]────────────┐
│                     │                   │
├─[ bl ]──────────────┼─[ br ]────────────┤
│                     │                   │
└─────────────────────┴───────────────────┘
```

### `dev.yaml` — editor on the left, top + shell on the right

```yaml
layout: horizontal
panes:
  - tabs:
      - name: editor
        command: nvim .
      - name: shell
  - layout: vertical
    panes:
      - tabs:
          - name: top
            command: top
      - tabs:
          - name: shell
```

```
┌─[ editor | shell ]──┬─[ top ]───────────┐
│                     │                   │
│  nvim .             │  top              │
│                     ├─[ shell ]─────────┤
│                     │                   │
└─────────────────────┴───────────────────┘
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
