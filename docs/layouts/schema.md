# Layout Schema

A Muxy layout is a tree of pane surfaces inside one top-level tab. Each leaf contains one `tab:`. Branches arrange their child panes horizontally or vertically and may be nested arbitrarily.

The first tab found in depth-first order is the top-level tab shown in the tab strip and tabs-focused sidebar. Every other tab in the layout is a child pane inside that tab.

## Single pane

```yaml
tab:
  name: editor
  command: nvim
```

## Two-pane horizontal split

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim
  - tab:
      name: shell
```

## Nested splits

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim
  - layout: vertical
    panes:
      - tab:
          name: logs
          command: tail -f /tmp/app.log
      - tab:
          name: btop
          command: btop
```

## Fields

| Field | Description |
| --- | --- |
| `layout` | `horizontal` for side-by-side panes or `vertical` for stacked panes. Defaults to `horizontal`. |
| `panes[]` | Child panes. When present, the node is a branch and `panes` takes precedence over leaf fields. |
| `tab` | The single tab displayed by a leaf pane. |
| `tab.name` | Optional title. Defaults to the first word of `command`, or `Terminal`. |
| `tab.command` | Optional string or list of strings joined with `&&`. |

A tab may be a bare command:

```yaml
tab: htop
```

A list-form command runs its entries in sequence:

```yaml
tab:
  name: setup
  command:
    - cd src
    - npm install
```

## Legacy `tabs:` compatibility

Existing layouts using `tabs:` arrays continue to load. The first entry becomes that leaf's pane in the layout. Additional entries become independent top-level tabs and do not belong to the split:

```yaml
tabs:
  - name: editor
    command: nvim
  - name: shell
```

This opens `editor` as the layout's first pane and preserves `shell` as a separate top-level tab. Use singular `tab:` in new layouts.

## JSON form

Layout files live in `.muxy/layouts/` and may use a `.yaml`, `.yml`, or `.json` extension. The same schema works in either format:

```json
{
  "layout": "horizontal",
  "panes": [
    { "tab": { "name": "editor", "command": "nvim" } },
    {
      "layout": "vertical",
      "panes": [
        { "tab": { "name": "logs", "command": "tail -f log" } },
        { "tab": { "name": "btop", "command": "btop" } }
      ]
    }
  ]
}
```
