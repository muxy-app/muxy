# Editor

Muxy includes a lightweight built‑in code editor that opens files as tabs. It's designed for quick edits in the same workspace as your terminals — not as a replacement for a full IDE.

## Opening files

- **Quick Open:** `Cmd+P` — fuzzy file search over the active worktree.
- File tree → click or right‑click a file → **Open**.
- File menu → **Open File…**.
- Drag a file from Finder onto a tab bar.

## Editing

- Standard macOS text editing shortcuts.
- **Save:** `Cmd+S`.
- **Undo / Redo:** `Cmd+Z` / `Cmd+Shift+Z`.

Unsaved changes are tracked per tab; quitting with unsaved files prompts to **Save All / Cancel / Discard**.

## Syntax highlighting

The editor highlights 30+ languages including Swift, C/C++/Objective‑C, JavaScript/TypeScript, Python, Ruby, Go, Rust, HTML/CSS, JSON/YAML/TOML, Markdown, and shell scripts. The active syntax theme follows the app theme — change it in **Settings → Appearance**.

## Find / Replace

`Cmd+F` opens find within the editor. `Cmd+Opt+F` opens find and replace.

## Markdown preview

Markdown files (`.md`, `.markdown`) get a live preview pane. Toggle modes from the editor toolbar:

- **Edit only**
- **Preview only**
- **Split** — editor and preview side‑by‑side with synchronised scrolling.

Preview features:

- GitHub‑flavoured Markdown.
- Mermaid diagrams.
- Local and remote images.
- Heading anchors (clickable from the preview).

Zoom the preview with `Cmd+=`, `Cmd+-`, `Cmd+0`.

## External editor

If you'd rather have files open in your editor of choice, **Settings → Editor** lets you set a default external editor command. Quick Open and file‑tree double‑click then route to that command instead of the built‑in editor.
