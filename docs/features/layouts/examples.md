# Layout Examples

These examples mirror the files in `.muxy/layouts/` in this repo. Each diagram shows the resulting window with panes drawn as boxes; tabs are listed at the top of their pane.

## `single.yaml` — one pane, multiple tabs

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
└─────────────────────────────────────┘
```

## `side-by-side.yaml` — editor next to a shell

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
│  nvim .             │                   │
└─────────────────────┴───────────────────┘
```

## `stacked.yaml` — two panes stacked vertically

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

## `tri-row.yaml` — three columns

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
└───────────────┴──────────────┴───────────────┘
```

## `quad.yaml` — 2×2 grid via nested splits

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

## `dev.yaml` — editor on the left, top + shell on the right

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
│                     │  top              │
│  nvim .             ├─[ shell ]─────────┤
│                     │                   │
└─────────────────────┴───────────────────┘
```
