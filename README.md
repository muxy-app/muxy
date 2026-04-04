<p align="center">
  <img src="Muxy/Resources/logo.png" alt="Muxy" width="128" height="128">
</p>

<h1 align="center">Muxy</h1>

<p align="center">A macOS terminal multiplexer built with SwiftUI and <a href="https://github.com/ghostty-org/ghostty">libghostty</a>.</p>

<div align="center">
  <img src="https://img.shields.io/github/downloads/muxy-app/muxy/total" />
</div>

## Screenshots

<img width="3098" height="1684" alt="image" src="https://github.com/user-attachments/assets/4dfd29c0-874a-4470-857c-b030949a8007" />
<img width="3098" height="1684" alt="image" src="https://github.com/user-attachments/assets/888659a7-ca2c-4ab1-b9bc-c0a73141d4ca" />
<img width="3098" height="1684" alt="image" src="https://github.com/user-attachments/assets/e99c7714-5865-438a-a3a1-14344919aaf1" />
<img width="3098" height="1684" alt="image" src="https://github.com/user-attachments/assets/7afb10fc-36a9-4cfe-b76a-0af5a91a186e" />

## Features

- **Project-based workflow** — Organize terminals by project with persistent workspace state
- **Vertical tabs** — Sidebar tab strip with drag-and-drop reordering, pinning, renaming, and middle-click close
- **Split panes** — Horizontal and vertical splits with keyboard navigation and resizable dividers
- **Built-in Git diff** — Unified and side-by-side diff views with syntax highlighting, PR detection, and file watching
- **200+ themes** — Browse and search Ghostty themes with a built-in theme picker
- **Customizable shortcuts** — 40+ configurable keyboard shortcuts with conflict detection
- **Workspace persistence** — Tabs, splits, and focus state are saved and restored per project
- **In-terminal search** — Find text in terminal output with match navigation
- **Drag and drop** — Reorder tabs and projects, drag tabs between panes to create splits
- **Auto-updates** — Built-in update checking via Sparkle

## Requirements

- macOS 14+
- Swift 6.0+

## Download

You can download the latest release from the [releases page](https://github.com/muxy-app/muxy/releases)

## Local Development

```bash
scripts/setup.sh          # downloads GhosttyKit.xcframework
swift build               # debug build
swift run Muxy             # run
```

## License

[MIT](LICENSE)
