<p align="center">
  <img src="Muxy/Resources/Assets.xcassets/AppIcon.appiconset/icon_128@2x.png" alt="Muxy" width="128" height="128">
</p>

<h1 align="center">Muxy</h1>

<p align="center">Lightweight and Memory efficient terminal for Mac built with SwiftUI and <a href="https://github.com/ghostty-org/ghostty">libghostty</a>.</p>
<p align="center"><p align="center"><a href="#install">Mac</a> | <a href="https://apps.apple.com/de/app/muxy/id6762464046?l=en-GB">iOS</a> | <a href="https://play.google.com/store/apps/details?id=com.muxy.app">Android</a> | <a href="https://discord.gg/4eMXAmJQ2n">Discord</a></p>

<div align="center">
  <img src="https://img.shields.io/github/downloads/muxy-app/muxy/total" />
  <img src="https://img.shields.io/github/v/release/muxy-app/muxy" />
  <img src="https://img.shields.io/github/license/muxy-app/muxy" />
  <img src="https://img.shields.io/github/commit-activity/m/muxy-app/muxy" />
</div>

## Vision

Lightweight terminal that has a rich API for extensions

## Screenshots

<img width="3004" alt="image" src="https://github.com/user-attachments/assets/f1fa0f64-e483-4277-b22a-7fb29eaf985d" />

## Features

- Project groups
- Vertical tabs
- Split panes
- Git worktrees
- Quick open & command palette
- Text editor with syntax highlighting
- Markdown & HTML preview (with Mermaid diagrams)
- Image viewer
- Extensions
- Mobile companion apps (iOS & Android)
- Rich input panel with image attachments
- Voice input
- Notifications (in-app & native macOS)
- 490+ themes
- 60+ customizable shortcuts
- Workspace & session persistence

## Agent Skills

```bash
# Drive the workspace from a shell (open projects, splits, send keys, read panes)
npx skills add github.com/muxy-app/muxy/tree/main/Muxy/Resources/skills/muxy-cli

# Author Muxy extensions (manifest, window.muxy API, theming)
npx skills add github.com/muxy-app/muxy/tree/main/Muxy/Resources/skills/muxy-extension

# Or install both into every detected AI harness at once (requires the Muxy CLI)
muxy install-skills
```

## Requirements

- macOS 14+
- Swift 6.0+

## Install

### Homebrew

```bash
brew tap muxy-app/tap
brew install --cask muxy
```

### Manual

Download the latest release from the [releases page](https://github.com/muxy-app/muxy/releases)

### iOS

[Instructions](https://github.com/muxy-app/mobile)

### Android

[Instructions](https://github.com/muxy-app/mobile)

## Local Development

```bash
scripts/setup.sh          # downloads GhosttyKit.xcframework
swift build               # debug build
swift run Muxy             # run
```

## License

[MIT](LICENSE)
