# Layout Schema

A layout is a tree. Each leaf is one terminal (`tab`). Each branch splits its
`panes` side by side or stacked. The whole tree opens as one tab, titled after
its first terminal.

## A single terminal

```yaml
tab:
  name: editor
  command: nvim .
```

A bare string is a command:

```yaml
tab: htop
```

## Splits

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim .
  - layout: vertical
    panes:
      - tab: npm run dev
      - tab:
          name: shell
```

Splits can nest up to 32 levels. Panes in a split share its space evenly.

## Fields

| Field | Description |
| --- | --- |
| `layout` | `horizontal` (side by side) or `vertical` (stacked). Defaults to `horizontal`. |
| `panes` | A non-empty list of child nodes. Makes the node a split. |
| `tab` | The terminal of a leaf: a command string, or an object with `name` and `command`. |
| `tab.name` | Optional title. Defaults to the command's first word, or `Terminal`. |
| `tab.command` | Optional command, or a list of commands joined with `&&`. Runs in the project folder. |

```yaml
tab:
  name: setup
  command:
    - npm install
    - npm run dev
```

## Legacy `tabs`

Older layouts with a `tabs` list still load. The first entry fills the leaf;
the others open as separate tabs. Use `tab` in new layouts.

```yaml
tabs:
  - name: editor
    command: nvim .
  - name: shell
```

## JSON

The same schema works in `.json` files:

```json
{
  "layout": "horizontal",
  "panes": [
    { "tab": { "name": "editor", "command": "nvim ." } },
    { "tab": "npm run dev" }
  ]
}
```

## Limits

A layout file can be up to 256 KiB, nest up to 32 levels, and hold up to 64
terminals.
