# Getting Started

## Requirements

- macOS 14 or newer (Apple Silicon or Intel)

## Install

1. Download the latest build from the releases page.
2. Drag `Muxy.app` to `/Applications` and launch it.
3. Optional: **Muxy -> Install CLI** writes a `muxy` wrapper to `/usr/local/bin/muxy`. If that location needs admin access and installation fails, Muxy falls back to `~/bin/muxy` or `~/.local/bin/muxy`.

## Add your first project

A project is just a directory you've added to Muxy.

1. Open the sidebar with `⌘B` (or **View → Toggle Sidebar**).
2. Click **+** at the bottom of the sidebar — or **File → Open Project…** (`⌘O`).
3. Type the folder name, choose the matching path, and press Return. Add parent names with a space or `/` to narrow results, such as `muxy Projects`. You can also type an explicit path such as `~/Projects/muxy`.
4. Right-click the project to rename, recolor, change its icon, or copy its absolute path.

Removing a project keeps its files on disk. To add it back with its name, icon, and settings intact, open **Add Project** and choose it under **Recently Removed**. You can assign **Recently Removed Projects** in **Settings → Keyboard Shortcuts** to open the chooser; it is unassigned by default.
