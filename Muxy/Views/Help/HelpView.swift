import SwiftUI

enum HelpSection: String, CaseIterable, Identifiable {
    case welcome
    case shortcuts
    case features
    case links

    var id: String { rawValue }

    var title: String {
        switch self {
        case .welcome: "Welcome"
        case .shortcuts: "Keyboard Shortcuts"
        case .features: "Features"
        case .links: "Links"
        }
    }

    var systemImage: String {
        switch self {
        case .welcome: "sparkles"
        case .shortcuts: "keyboard"
        case .features: "square.grid.2x2"
        case .links: "link"
        }
    }
}

struct HelpView: View {
    @State private var selection: HelpSection = .welcome

    var body: some View {
        NavigationSplitView {
            List(HelpSection.allCases, selection: $selection) { section in
                Label(section.title, systemImage: section.systemImage)
                    .tag(section)
            }
            .navigationSplitViewColumnWidth(min: 180, ideal: 200)
        } detail: {
            Group {
                switch selection {
                case .welcome: HelpWelcomeView()
                case .shortcuts: HelpShortcutsView()
                case .features: HelpFeaturesView()
                case .links: HelpLinksView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(MuxyTheme.bg)
        }
        .preferredColorScheme(MuxyTheme.colorScheme)
    }
}

private struct HelpWelcomeView: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Welcome to Muxy")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundStyle(MuxyTheme.fg)
                Text(
                    "Muxy is a native macOS terminal multiplexer organised around projects, "
                        + "worktrees, tabs, and split panes — with a built‑in editor, source "
                        + "control view, and a remote API for mobile companion apps."
                )
                .foregroundStyle(MuxyTheme.fgMuted)

                HelpQuickStart()

                Text("Where to next")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                    .padding(.top, 8)
                Text(
                    "Use the sidebar to browse keyboard shortcuts, an overview of features, "
                        + "and links to the docs, repository, mobile app, and Discord."
                )
                .foregroundStyle(MuxyTheme.fgMuted)
            }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct HelpQuickStart: View {
    private struct Step: Identifiable {
        let id = UUID()
        let title: String
        let detail: String
        let shortcut: String?
    }

    private let steps: [Step] = [
        Step(title: "Add a project", detail: "Click + in the sidebar, or use File → Open Project…", shortcut: "⌘O"),
        Step(title: "Open a new tab", detail: "Each tab is a terminal by default.", shortcut: "⌘T"),
        Step(title: "Split a pane", detail: "Split right or down to multiplex within one tab.", shortcut: "⌘D"),
        Step(title: "Open Source Control", detail: "Stage, commit, push, and review diffs.", shortcut: "⌘K"),
        Step(title: "Quick‑open a file", detail: "Fuzzy‑search files in the active worktree.", shortcut: "⌘P"),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Quick start")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            ForEach(Array(steps.enumerated()), id: \.element.id) { index, step in
                HStack(alignment: .top, spacing: 12) {
                    Text("\(index + 1)")
                        .font(.system(size: 12, weight: .semibold, design: .rounded))
                        .frame(width: 22, height: 22)
                        .background(MuxyTheme.fgMuted.opacity(0.15), in: Circle())
                        .foregroundStyle(MuxyTheme.fg)
                    VStack(alignment: .leading, spacing: 2) {
                        HStack(spacing: 8) {
                            Text(step.title)
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(MuxyTheme.fg)
                            if let shortcut = step.shortcut {
                                Text(shortcut)
                                    .font(.system(size: 11, weight: .medium, design: .rounded))
                                    .foregroundStyle(MuxyTheme.fgMuted)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(MuxyTheme.fgMuted.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
                            }
                        }
                        Text(step.detail)
                            .font(.system(size: 12))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                }
            }
        }
    }
}

private struct HelpShortcutsView: View {
    private var groups: [(category: String, actions: [ShortcutAction])] {
        let bound = Set(KeyBindingStore.shared.bindings.map(\.action))
        let actions = ShortcutAction.allCases.filter { bound.contains($0) }
        return ShortcutAction.categories.compactMap { category in
            let inCategory = actions.filter { $0.category == category }
            return inCategory.isEmpty ? nil : (category, inCategory)
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text("Keyboard Shortcuts")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundStyle(MuxyTheme.fg)
                Text("Defaults shown. All shortcuts can be remapped in Settings → Shortcuts.")
                    .foregroundStyle(MuxyTheme.fgMuted)

                ForEach(groups, id: \.category) { group in
                    VStack(alignment: .leading, spacing: 6) {
                        Text(group.category)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fg)
                            .padding(.bottom, 2)
                        ForEach(group.actions) { action in
                            HStack {
                                Text(action.displayName)
                                    .font(.system(size: 12))
                                    .foregroundStyle(MuxyTheme.fg)
                                Spacer()
                                Text(KeyBindingStore.shared.combo(for: action).displayString)
                                    .font(.system(size: 11, weight: .medium, design: .rounded))
                                    .foregroundStyle(MuxyTheme.fgMuted)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(MuxyTheme.fgMuted.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
                            }
                            .padding(.vertical, 3)
                        }
                    }
                }
            }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct HelpFeaturesView: View {
    private struct Feature: Identifiable {
        let id = UUID()
        let title: String
        let summary: String
        let docPath: String
    }

    private let features: [Feature] = [
        Feature(
            title: "Projects",
            summary: "Add directories as projects, switch between them, and open them via CLI or URL scheme.",
            docPath: "docs/features/projects.md"
        ),
        Feature(
            title: "Worktrees",
            summary: "Per‑worktree tabs and splits, with declarative setup commands for new worktrees.",
            docPath: "docs/features/worktrees.md"
        ),
        Feature(
            title: "Tabs & Splits",
            summary: "Terminal, editor, source control, and diff tabs. Nested binary splits, drag and drop.",
            docPath: "docs/features/tabs-and-splits.md"
        ),
        Feature(
            title: "Terminal",
            summary: "libghostty‑powered terminal, find in pane, custom command shortcuts.",
            docPath: "docs/features/terminal.md"
        ),
        Feature(
            title: "Editor",
            summary: "Built‑in syntax‑highlighting editor with quick open and live markdown preview.",
            docPath: "docs/features/editor.md"
        ),
        Feature(
            title: "Source Control",
            summary: "Status, staging, commits, push/pull, branches, diff viewer, and GitHub PRs.",
            docPath: "docs/features/source-control.md"
        ),
        Feature(
            title: "File Tree",
            summary: "Gitignore‑aware file tree with create / rename / delete / drag‑and‑drop.",
            docPath: "docs/features/file-tree.md"
        ),
        Feature(
            title: "Layouts",
            summary: "Declarative .muxy/layouts/*.yaml workspaces and one‑time .muxy/startup.yaml.",
            docPath: "docs/features/layouts.md"
        ),
        Feature(
            title: "Notifications",
            summary: "OSC sequences, hook scripts, and a Unix socket API for external tools.",
            docPath: "docs/features/notifications.md"
        ),
        Feature(
            title: "AI Usage",
            summary: "Quota tracking for Claude Code, Copilot, Codex, Cursor, and more.",
            docPath: "docs/features/ai-usage.md"
        ),
        Feature(
            title: "Themes",
            summary: "Paired light / dark themes synced with macOS appearance and Ghostty colors.",
            docPath: "docs/features/themes.md"
        ),
        Feature(
            title: "Remote Server",
            summary: "WebSocket API for mobile companion clients with trust‑on‑first‑use pairing.",
            docPath: "docs/features/remote-server.md"
        ),
    ]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Features")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundStyle(MuxyTheme.fg)
                Text("Every feature has a short overview here and a full page in the documentation.")
                    .foregroundStyle(MuxyTheme.fgMuted)

                VStack(spacing: 8) {
                    ForEach(features) { feature in
                        FeatureRow(feature: feature)
                    }
                }
            }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private struct FeatureRow: View {
        let feature: Feature

        var body: some View {
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(feature.title)
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                    Text(feature.summary)
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                Spacer()
                Button("Read more") {
                    HelpLinks.openDoc(feature.docPath)
                }
                .buttonStyle(.link)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(MuxyTheme.fgMuted.opacity(0.06), in: RoundedRectangle(cornerRadius: 8))
        }
    }
}

private struct HelpLinksView: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("Links")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundStyle(MuxyTheme.fg)
                Text("Find more information, file an issue, or join the community.")
                    .foregroundStyle(MuxyTheme.fgMuted)

                LinkRow(title: "Documentation", subtitle: "Full docs on GitHub", systemImage: "book", action: HelpLinks.openDocs)
                LinkRow(
                    title: "GitHub Repository",
                    subtitle: "muxy-app/muxy",
                    systemImage: "chevron.left.forwardslash.chevron.right",
                    action: HelpLinks.openRepo
                )
                LinkRow(
                    title: "Mobile App Repository",
                    subtitle: "muxy-app/mobile",
                    systemImage: "iphone",
                    action: HelpLinks.openMobileRepo
                )
                LinkRow(
                    title: "Discord",
                    subtitle: "Join the community",
                    systemImage: "bubble.left.and.bubble.right",
                    action: HelpLinks.openDiscord
                )
                LinkRow(
                    title: "Report an Issue",
                    subtitle: "Bug reports and feature requests",
                    systemImage: "exclamationmark.bubble",
                    action: HelpLinks.openIssues
                )
            }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private struct LinkRow: View {
        let title: String
        let subtitle: String
        let systemImage: String
        let action: () -> Void

        var body: some View {
            Button(action: action) {
                HStack(spacing: 12) {
                    Image(systemName: systemImage)
                        .font(.system(size: 14))
                        .foregroundStyle(MuxyTheme.fg)
                        .frame(width: 24, height: 24)
                        .background(MuxyTheme.fgMuted.opacity(0.12), in: RoundedRectangle(cornerRadius: 6))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(title)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fg)
                        Text(subtitle)
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    Spacer()
                    Image(systemName: "arrow.up.right.square")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(MuxyTheme.fgMuted.opacity(0.06), in: RoundedRectangle(cornerRadius: 8))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }
}

enum HelpLinks {
    static let repoURL = url("https://github.com/muxy-app/muxy")
    static let docsURL = url("https://github.com/muxy-app/muxy/tree/main/docs")
    static let mobileRepoURL = url("https://github.com/muxy-app/mobile")
    static let discordURL = url("https://discord.gg/4eMXAmJQ2n")
    static let issuesURL = url("https://github.com/muxy-app/muxy/issues")

    private static func url(_ string: String) -> URL {
        URL(string: string) ?? URL(fileURLWithPath: "/")
    }

    static func openRepo() {
        NSWorkspace.shared.open(repoURL)
    }

    static func openDocs() {
        NSWorkspace.shared.open(docsURL)
    }

    static func openMobileRepo() {
        NSWorkspace.shared.open(mobileRepoURL)
    }

    static func openDiscord() {
        NSWorkspace.shared.open(discordURL)
    }

    static func openIssues() {
        NSWorkspace.shared.open(issuesURL)
    }

    static func openDoc(_ relativePath: String) {
        guard let url = URL(string: "https://github.com/muxy-app/muxy/blob/main/\(relativePath)") else { return }
        NSWorkspace.shared.open(url)
    }
}
