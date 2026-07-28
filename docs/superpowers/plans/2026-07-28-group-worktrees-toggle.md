# Group Worktrees Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a persisted Group worktrees setting that switches both Tab Focused and Agents Focused between top-level and project-nested worktree rows.

**Architecture:** Store one Boolean in `WorktreeListPreferences`, expose it through the existing settings catalog and `@AppStorage`, and pass it into the shared focused-sidebar views. Keep row selection in one pure resolver; reuse the existing grouped tree and make its leaf content switch between tab and agent behavior.

**Tech Stack:** Swift 6, SwiftUI, Foundation `UserDefaults`, Swift Testing, Swift Package Manager

## Global Constraints

- Requires macOS 14+ and Swift 6.0+.
- Use only existing SPM dependencies.
- Do not add code comments.
- Default Group worktrees to off.
- Show the setting only for Tab Focused and Agents Focused.
- Do not run the app.
- Run `scripts/checks.sh --fix` after implementation.
- Run `scripts/setup.sh` before tests because this checkout currently lacks ignored Ghostty resources and `Muxy/Resources/terminfo`.

---

### Task 1: Preference and Settings Registration

**Files:**
- Modify: `Muxy/Models/Layout/AppLayout.swift`
- Modify: `Muxy/Models/Preferences/WorktreeListPreferences.swift`
- Modify: `Muxy/Views/Settings/Shared/SettingsCatalog.swift`
- Test: `Tests/MuxyTests/Models/Layout/AppLayoutTests.swift`
- Test: `Tests/MuxyTests/Views/Settings/Shared/SettingsCatalogTests.swift`

**Interfaces:**
- Consumes: existing `AppLayout`, `WorktreeListPreferences`, and `SettingsCatalogItem`
- Produces: `AppLayout.supportsGroupedWorktrees`, `WorktreeListPreferences.groupWorktreesKey`, and `WorktreeListPreferences.defaultGroupWorktrees`

- [ ] **Step 1: Restore ignored build resources**

Run:

```bash
scripts/setup.sh
```

Expected: `GhosttyKit.xcframework`, Ghostty resources, and `Muxy/Resources/terminfo` exist.

- [ ] **Step 2: Write failing layout and catalog tests**

Add to `AppLayoutTests`:

```swift
@Test("group worktrees is available in focused layouts")
func groupedWorktreeAvailability() {
    #expect(!AppLayout.projectFocused.supportsGroupedWorktrees)
    #expect(AppLayout.tabFocused.supportsGroupedWorktrees)
    #expect(AppLayout.agentsFocused.supportsGroupedWorktrees)
    #expect(!WorktreeListPreferences.defaultGroupWorktrees)
}
```

Extend `worktreeListSettingsAreRegisteredAndSearchable()`:

```swift
#expect(SettingsCatalog.items.contains {
    $0.key == WorktreeListPreferences.groupWorktreesKey && $0.category == .appearance
})
#expect(SettingsCatalog.matchingItems(query: "group worktrees").contains {
    $0.key == WorktreeListPreferences.groupWorktreesKey
})
#expect(SettingsCatalog.jsonEditableItems.contains {
    $0.key == WorktreeListPreferences.groupWorktreesKey
})
```

- [ ] **Step 3: Run tests and verify the new symbols fail**

Run:

```bash
swift test --filter 'AppLayoutTests|SettingsCatalogTests'
```

Expected: compilation fails because `supportsGroupedWorktrees` and `groupWorktreesKey` do not exist.

- [ ] **Step 4: Add the minimal preference model**

Add to `AppLayout`:

```swift
var supportsGroupedWorktrees: Bool {
    self != .projectFocused
}
```

Add to `WorktreeListPreferences`:

```swift
static let groupWorktreesKey = "muxy.worktrees.groupWorktrees"
static let defaultGroupWorktrees = false
```

Register the setting beside the other worktree appearance items:

```swift
SettingsCatalogItem(
    key: WorktreeListPreferences.groupWorktreesKey,
    title: "Group Worktrees",
    description: "Groups worktrees under their project in Tab Focused and Agents Focused layouts.",
    category: .appearance,
    section: "Layout",
    defaultValue: WorktreeListPreferences.defaultGroupWorktrees,
    aliases: ["nested", "folders", "tab focused", "agents focused"]
),
```

- [ ] **Step 5: Run the targeted tests**

Run:

```bash
swift test --filter 'AppLayoutTests|SettingsCatalogTests'
```

Expected: all selected tests pass.

- [ ] **Step 6: Commit the preference model**

```bash
git add Muxy/Models/Layout/AppLayout.swift Muxy/Models/Preferences/WorktreeListPreferences.swift Muxy/Views/Settings/Shared/SettingsCatalog.swift Tests/MuxyTests/Models/Layout/AppLayoutTests.swift Tests/MuxyTests/Views/Settings/Shared/SettingsCatalogTests.swift
git commit -m "feat: add grouped worktree preference"
```

### Task 2: Focused Sidebar Grouping

**Files:**
- Modify: `Muxy/Views/Settings/InterfaceSettingsView.swift`
- Modify: `Muxy/Views/Layouts/TabFocused/TabFocusedSidebar.swift`
- Modify: `Muxy/Views/Layouts/TabFocused/TabFocusedProjectRow.swift`
- Modify: `Muxy/Views/Layouts/TabFocused/TabFocusedWorktreeTree.swift`
- Modify: `Muxy/Views/Layouts/TabFocused/WorktreeLeafRow.swift`
- Test: `Tests/MuxyTests/Views/Layouts/TabFocused/TabFocusedWorktreeTreeTests.swift`
- Modify: `docs/user-guide/settings.md`

**Interfaces:**
- Consumes: `AppLayout.supportsGroupedWorktrees`, `WorktreeListPreferences.groupWorktreesKey`, `TabFocusedSidebarContent`, `TabFocusedSidebarRowItem`, `TabFocusedSidebarRowTap`, `TabFocusedTabsList`, and `AgentsFocusedTabsList`
- Produces: `TabFocusedSidebarRows.resolve(projects:content:groupWorktrees:worktreesForProject:hasTabs:)` and content-aware grouped worktree rows

- [ ] **Step 1: Write failing sidebar row tests**

Add this suite to `TabFocusedWorktreeTreeTests.swift`:

```swift
@Suite("Focused sidebar rows")
struct TabFocusedSidebarRowsTests {
    @Test("grouped focused layouts contain project rows only")
    func groupedRows() {
        var project = Project(name: "Repo", path: "/repo")
        project.worktreesEnabled = true
        let secondary = Worktree(name: "feature", path: "/feature", isPrimary: false)

        for content in [TabFocusedSidebarContent.tabs, .agents] {
            let rows = TabFocusedSidebarRows.resolve(
                projects: [project],
                content: content,
                groupWorktrees: true,
                worktreesForProject: { _ in [secondary] },
                hasTabs: { _ in true }
            )

            #expect(rows.map(\.id) == [project.id])
        }
    }

    @Test("ungrouped tab focused includes only worktrees with tabs")
    func ungroupedTabRows() {
        var project = Project(name: "Repo", path: "/repo")
        project.worktreesEnabled = true
        let primary = Worktree(name: "Repo", path: "/repo", isPrimary: true)
        let open = Worktree(name: "open", path: "/open", isPrimary: false)
        let closed = Worktree(name: "closed", path: "/closed", isPrimary: false)

        let rows = TabFocusedSidebarRows.resolve(
            projects: [project],
            content: .tabs,
            groupWorktrees: false,
            worktreesForProject: { _ in [primary, open, closed] },
            hasTabs: { $0.worktreeID == open.id }
        )

        #expect(rows.map(\.id) == [project.id, open.id])
    }

    @Test("ungrouped agents focused includes every secondary worktree")
    func ungroupedAgentRows() {
        var project = Project(name: "Repo", path: "/repo")
        project.worktreesEnabled = true
        let primary = Worktree(name: "Repo", path: "/repo", isPrimary: true)
        let first = Worktree(name: "first", path: "/first", isPrimary: false)
        let second = Worktree(name: "second", path: "/second", isPrimary: false)

        let rows = TabFocusedSidebarRows.resolve(
            projects: [project],
            content: .agents,
            groupWorktrees: false,
            worktreesForProject: { _ in [primary, first, second] },
            hasTabs: { _ in false }
        )

        #expect(rows.map(\.id) == [project.id, first.id, second.id])
    }
}
```

- [ ] **Step 2: Run the sidebar tests and verify RED**

Run:

```bash
swift test --filter TabFocusedSidebarRowsTests
```

Expected: compilation fails because `TabFocusedSidebarRows` does not exist.

- [ ] **Step 3: Add the pure row resolver**

Add beside `TabFocusedSidebarProjectSelection`:

```swift
enum TabFocusedSidebarRows {
    static func resolve(
        projects: [Project],
        content: TabFocusedSidebarContent,
        groupWorktrees: Bool,
        worktreesForProject: (UUID) -> [Worktree],
        hasTabs: (WorktreeKey) -> Bool
    ) -> [TabFocusedSidebarRowItem] {
        projects.flatMap { project in
            var rows: [TabFocusedSidebarRowItem] = [.project(project)]
            guard !groupWorktrees, project.worktreesEnabled, !project.isHome else { return rows }
            for worktree in worktreesForProject(project.id) where !worktree.isPrimary {
                let key = WorktreeKey(projectID: project.id, worktreeID: worktree.id)
                guard content == .agents || hasTabs(key) else { continue }
                rows.append(.worktree(project, worktree))
            }
            return rows
        }
    }
}
```

Bind the preference in `TabFocusedSidebar`:

```swift
@AppStorage(WorktreeListPreferences.groupWorktreesKey)
private var groupWorktrees = WorktreeListPreferences.defaultGroupWorktrees
```

Replace its current `rows` implementation:

```swift
private var rows: [TabFocusedSidebarRowItem] {
    TabFocusedSidebarRows.resolve(
        projects: projects,
        content: content,
        groupWorktrees: groupWorktrees,
        worktreesForProject: { worktreeStore.list(for: $0) },
        hasTabs: { appState.hasTabs(for: $0) }
    )
}
```

Pass `groupWorktrees` to every `TabFocusedProjectRow`.

- [ ] **Step 4: Make the grouped tree content-aware**

Add `let groupWorktrees: Bool` to `TabFocusedProjectRow`. Render the grouped tree for either focused layout:

```swift
if groupWorktrees, !isWorktreeRow, project.worktreesEnabled {
    TabFocusedWorktreeTree(
        project: project,
        worktrees: worktreeStore.list(for: project.id),
        shortcutNumbers: shortcutNumbers,
        content: content
    )
}
```

Hide the project-level create action when grouped worktree leaves own that action:

```swift
case .agents:
    if isWorktreeRow || !groupWorktrees || !project.worktreesEnabled {
        AgentsFocusedTabActions(
            project: project,
            worktree: listWorktree,
            showingProviders: $showAgentProviderMenu
        )
    }
```

Add `let content: TabFocusedSidebarContent` to `TabFocusedWorktreeTree` and pass it into every `WorktreeLeafRow`.

Add `let content: TabFocusedSidebarContent` and `@State private var showAgentProviderMenu = false` to `WorktreeLeafRow`. Switch the expanded list:

```swift
if isExpanded {
    switch content {
    case .tabs:
        TabFocusedTabsList(
            project: project,
            worktree: worktree,
            shortcutNumbers: shortcutNumbers
        )
    case .agents:
        AgentsFocusedTabsList(project: project, worktree: worktree)
    }
}
```

Switch the leaf create action:

```swift
@ViewBuilder
private var leafAction: some View {
    switch content {
    case .tabs:
        SidebarActionButton(symbol: "plus", label: "New Terminal Tab") {
            activate()
            appState.createTab(projectID: project.id)
        }
    case .agents:
        AgentsFocusedTabActions(
            project: project,
            worktree: worktree,
            showingProviders: $showAgentProviderMenu
        )
    }
}
```

Use the existing focused-row tap policy:

```swift
private func handleTap() {
    switch TabFocusedSidebarRowTap.resolve(content: content, isActive: isActive) {
    case .toggleExpansion:
        toggle()
    case .activateRow:
        activate()
    }
}

private func activate() {
    TabFocusedSidebarTarget.activate(
        project: project,
        worktree: worktree,
        appState: appState,
        worktreeStore: worktreeStore
    )
}
```

Call `handleTap()` from the leaf header and keep the existing expansion animation.

- [ ] **Step 5: Add the conditional Settings row**

Bind the preference in `InterfaceSettingsView`:

```swift
@AppStorage(WorktreeListPreferences.groupWorktreesKey)
private var groupWorktrees = WorktreeListPreferences.defaultGroupWorktrees
```

Add after the App Layout row:

```swift
if layoutStore.layout.supportsGroupedWorktrees {
    SettingsToggleRow(label: "Group worktrees", isOn: $groupWorktrees)
}
```

- [ ] **Step 6: Document the focused-layout setting**

Add to `docs/user-guide/settings.md`:

```markdown
## Focused-layout worktree grouping

In **Appearance → Layout**, select **Tab Focused** or **Agents Focused** to show **Group worktrees**. It is off by default. Turn it on to nest all worktrees under their project; turn it off to keep worktrees as top-level rows. Tab Focused shows top-level worktrees only when they have open tabs, while Agents Focused shows every secondary worktree.
```

- [ ] **Step 7: Run targeted tests**

Run:

```bash
swift test --filter 'TabFocusedSidebarRowsTests|TabFocusedSidebarRowTapTests|AppLayoutTests|SettingsCatalogTests'
```

Expected: all selected tests pass.

- [ ] **Step 8: Run required repository validation**

Run:

```bash
scripts/checks.sh --fix
```

Expected: formatting, linting, build, and tests all pass.

- [ ] **Step 9: Commit the focused sidebar behavior**

```bash
git add Muxy/Views/Settings/InterfaceSettingsView.swift Muxy/Views/Layouts/TabFocused/TabFocusedSidebar.swift Muxy/Views/Layouts/TabFocused/TabFocusedProjectRow.swift Muxy/Views/Layouts/TabFocused/TabFocusedWorktreeTree.swift Muxy/Views/Layouts/TabFocused/WorktreeLeafRow.swift Tests/MuxyTests/Views/Layouts/TabFocused/TabFocusedWorktreeTreeTests.swift docs/user-guide/settings.md
git commit -m "feat: group worktrees in focused layouts"
```
