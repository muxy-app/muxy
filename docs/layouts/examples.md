# Layout Examples

These examples can be placed in a project's `.muxy/layouts/` directory. Each diagram shows the resulting top-level tab with its pane surfaces drawn as boxes.

## `single.yaml` — one pane

```yaml
tab:
  name: shell
```

```
Tab strip: [ shell ]
┌─────────────────────────────────────┐
│ shell                               │
└─────────────────────────────────────┘
```

## `side-by-side.yaml` — editor next to a shell

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim .
  - tab:
      name: shell
```

```
Tab strip: [ editor ]
┌─────────────────────┬───────────────────┐
│ editor              │ shell             │
│ nvim .              │                   │
└─────────────────────┴───────────────────┘
```

## `stacked.yaml` — two panes stacked vertically

```yaml
layout: vertical
panes:
  - tab:
      name: top
  - tab:
      name: bottom
```

```
Tab strip: [ top ]
┌─────────────────────────────────────┐
│ top                                 │
├─────────────────────────────────────┤
│ bottom                              │
└─────────────────────────────────────┘
```

## `tri-row.yaml` — three columns

```yaml
layout: horizontal
panes:
  - tab:
      name: left
  - tab:
      name: mid
  - tab:
      name: right
```

```
Tab strip: [ left ]
┌───────────────┬──────────────┬───────────────┐
│ left          │ mid          │ right         │
└───────────────┴──────────────┴───────────────┘
```

## `quad.yaml` — 2×2 grid via nested splits

```yaml
layout: horizontal
panes:
  - layout: vertical
    panes:
      - tab:
          name: tl
      - tab:
          name: bl
  - layout: vertical
    panes:
      - tab:
          name: tr
      - tab:
          name: br
```

```
Tab strip: [ tl ]
┌─────────────────────┬───────────────────┐
│ tl                  │ tr                │
├─────────────────────┼───────────────────┤
│ bl                  │ br                │
└─────────────────────┴───────────────────┘
```

## `dev.yaml` — editor on the left, top and shell on the right

```yaml
layout: horizontal
panes:
  - tab:
      name: editor
      command: nvim .
  - layout: vertical
    panes:
      - tab:
          name: top
          command: top
      - tab:
          name: shell
```

```
Tab strip: [ editor ]
┌─────────────────────┬───────────────────┐
│ editor              │ top               │
│ nvim .              ├───────────────────┤
│                     │ shell             │
└─────────────────────┴───────────────────┘
```
