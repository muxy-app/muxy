# UI Scaling

Muxy supports a centralized UI scale that resizes the main app chrome — sidebar, tabs, toolbars, file tree, source-control surfaces, panels, badges — without using `.scaleEffect()` (which rasterizes at the original size and produces blurry text). Instead, every metric is a real CGFloat that is multiplied by a user-chosen multiplier at the source.

The Settings window is intentionally excluded so it stays at the system-native preferences size regardless of the active scale.

## Components

- **`Muxy/Theme/UIScale.swift`** — `@Observable @MainActor` singleton that stores the user's scale preset and persists to `~/Library/Application Support/Muxy/ui-scale.json`. Exposes `multiplier: CGFloat` derived from the preset.
- **`Muxy/Theme/UIMetrics.swift`** — semantic design tokens (font sizes, spacings, icon sizes, control heights, radii, sidebar widths). Each token reads `UIScale.shared.multiplier` so SwiftUI's `@Observable` tracking re-renders dependent views automatically when the multiplier changes.
- **Settings UI** — `AppearanceSettingsView` exposes a segmented picker under "Interface" → "Size" with three presets: `regular (1.00×)`, `large (1.12×)`, `extraLarge (1.24×)`.

## Usage convention

All chrome views must read sizes from `UIMetrics` instead of hardcoding numeric literals:

```swift
// good
.font(.system(size: UIMetrics.fontBody))
.padding(.horizontal, UIMetrics.spacing4)
.frame(width: UIMetrics.iconLG, height: UIMetrics.iconLG)
.background(.quaternary, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))

// fallback for one-off values (still scales, no semantic match)
.frame(width: UIMetrics.scaled(120))
```

Existing centralized layout enums (`SidebarLayout`, `SettingsMetrics`) delegate to `UIMetrics`, so views that already use them inherit scaling automatically.

## Scope

Scaling applies to **app chrome only** — the user-content areas and the Settings window keep their own sizing:

| Surface | Source of truth |
| --- | --- |
| Terminal contents | libghostty config (`~/.config/ghostty/config`) |
| Editor body text | `EditorSettings.fontSize` |
| Markdown preview body | `EditorSettings.markdownPreviewFontScale` |
| Settings window (`SettingsMetrics`) | static literals — fixed at the macOS-native preferences size |

This avoids double-scaling. The terminal and editor each have their own font controls in Settings; the UI scale only governs the surrounding shell.

## Adding new chrome

When writing a new SwiftUI view:

1. Pick a semantic token from `UIMetrics` (e.g. `.fontBody`, `.spacing4`, `.iconLG`).
2. If no semantic token fits, use `UIMetrics.scaled(value)` to apply the multiplier to a literal — but prefer adding a new token when the value is reused.
3. Never write raw `CGFloat` literals for sizes / spacings / fonts. Reviewers reject those.

1px borders, dividers, and strokes stay at literal `1` since they are intentional pixel-perfect lines.
