# Layout Schema

A Muxy workspace is a tree of panes inside a single window. Each leaf pane is a stack of tabs (one tab visible at a time). Panes can be nested with horizontal or vertical splits.

A node is either a **leaf** (`tabs:`) or a **branch** (`layout:` + `panes:`). Branches may be nested arbitrarily.

## Single pane with tabs

```yaml
tabs:
  - name: editor
    command: nvim
  - name: shell
```

## Two-pane horizontal split

```yaml
layout: horizontal
panes:
  - tabs:
      - name: editor
        command: nvim
  - tabs:
      - name: shell
```

## Nested splits

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

## Fields

| Field | Description |
| --- | --- |
| `layout` | `horizontal` (panes side-by-side) or `vertical` (panes stacked). Defaults to `horizontal`. |
| `panes[]` | Child panes. Required when `layout` is set; mutually exclusive with `tabs`. |
| `tabs[]` | Tabs in this pane. Required for leaves. |
| `tabs[].name` | Optional. Tab title. Defaults to the first word of `command`, or `Terminal`. |
| `tabs[].command` | Optional. String, or a list of strings joined with `&&`. |

A tab may also be written inline as a bare string command:

```yaml
tabs:
  - htop
```

A list-form command:

```yaml
tabs:
  - name: setup
    command:
      - cd src
      - npm install
```

## JSON form

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
