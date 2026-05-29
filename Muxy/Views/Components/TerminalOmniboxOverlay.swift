import SwiftUI

struct TerminalOmniboxOverlay: View {
    let projects: [TerminalOmniboxProjectItem]
    let worktrees: [TerminalOmniboxWorktreeItem]
    let openTabs: [OpenTerminalTabItem]
    let closedTabs: [ClosedTerminalTabSnapshot]
    let commandShortcuts: [CommandShortcut]
    let extensionCommands: [ExtensionPaletteItem]
    let activeProjectID: UUID?
    let activeWorktreeID: UUID?
    let commandProjectIDs: Set<UUID>
    let launchScope: TerminalOmniboxLaunchScope
    let onSelect: (TerminalOmniboxItem, UUID?, UUID?) -> Void
    let onDismiss: () -> Void

    @State private var query = ""
    @State private var highlightedIndex: Int? = 0

    private var trimmedQuery: String {
        query.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var displayList: [TerminalOmniboxItem] {
        let items = baseItems
        guard !trimmedQuery.isEmpty else { return items }
        return items.filter { $0.searchKey.localizedCaseInsensitiveContains(trimmedQuery) }
    }

    private var baseItems: [TerminalOmniboxItem] {
        TerminalOmniboxItemResolver.items(
            in: TerminalOmniboxItemContext(
                projects: projects,
                worktrees: worktrees,
                openTabs: openTabs,
                closedTabs: closedTabs,
                commandShortcuts: commandShortcuts,
                extensionCommands: extensionCommands,
                activeProjectID: activeProjectID,
                activeWorktreeID: activeWorktreeID,
                commandProjectIDs: commandProjectIDs
            ),
            launchScope: launchScope
        )
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture { onDismiss() }

            OverlayPanel(width: UIMetrics.scaled(720), height: UIMetrics.scaled(460)) {
                VStack(spacing: 0) {
                    searchField
                    Divider().overlay(MuxyTheme.border)
                    resultsList
                    Divider().overlay(MuxyTheme.border)
                    footer
                }
            }
        }
        .onAppear {
            applyLaunchScope()
        }
        .onChange(of: query) {
            highlightedIndex = displayList.isEmpty ? nil : 0
        }
        .onChange(of: launchScope) {
            applyLaunchScope()
        }
        .onChange(of: openTabs.count) {
            highlightedIndex = displayList.isEmpty ? nil : min(highlightedIndex ?? 0, displayList.count - 1)
        }
        .onChange(of: closedTabs.count) {
            highlightedIndex = displayList.isEmpty ? nil : min(highlightedIndex ?? 0, displayList.count - 1)
        }
    }

    private var searchField: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .accessibilityHidden(true)
            PaletteSearchField(
                text: $query,
                placeholder: searchPlaceholder,
                onSubmit: { confirmSelection() },
                onEscape: { onDismiss() },
                onArrowUp: { moveHighlight(-1) },
                onArrowDown: { moveHighlight(1) },
                onTab: { handleTab() },
                onBackTab: { handleBackTab() }
            )
            .frame(height: UIMetrics.scaled(28))
        }
        .frame(height: UIMetrics.scaled(28))
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing5)
    }

    private var searchPlaceholder: String {
        switch launchScope {
        case .projects:
            "Search project..."
        case .worktrees:
            "Search worktree..."
        case .openTabs:
            "Search open tabs..."
        case .commandShortcuts:
            "Search custom commands..."
        case .history:
            "Search history..."
        }
    }

    private var resultsList: some View {
        Group {
            if displayList.isEmpty {
                VStack {
                    Spacer()
                    Text(emptyStateText)
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Spacer()
                }
            } else {
                ScrollViewReader { proxy in
                    ScrollView(.vertical, showsIndicators: true) {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(displayList.enumerated()), id: \.element.id) { index, item in
                                if shouldShowSectionHeader(at: index) {
                                    TerminalOmniboxSectionHeader(title: item.sectionTitle)
                                }
                                TerminalOmniboxRow(item: item, isHighlighted: index == highlightedIndex)
                                    .contentShape(Rectangle())
                                    .onTapGesture { handleTap(item) }
                                    .id(item.id)
                            }
                        }
                    }
                    .onChange(of: highlightedIndex) { _, newIndex in
                        guard let newIndex, newIndex < displayList.count else { return }
                        proxy.scrollTo(displayList[newIndex].id, anchor: nil)
                    }
                }
            }
        }
        .frame(maxHeight: .infinity)
    }

    private var footer: some View {
        HStack(spacing: UIMetrics.scaled(18)) {
            TerminalOmniboxHint(symbol: "return", label: returnHintLabel)
            HStack(spacing: UIMetrics.scaled(4)) {
                TerminalOmniboxHint(text: tabHintText)
                TerminalOmniboxHint(symbol: "arrow.up.arrow.down", label: navigateHintLabel)
            }
            TerminalOmniboxHint(text: "Esc", label: "Close")
        }
        .frame(maxWidth: .infinity, alignment: .center)
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private var emptyStateText: String {
        switch launchScope {
        case .projects:
            "No projects found"
        case .worktrees:
            "No worktrees found"
        case .openTabs:
            "No open tabs found"
        case .commandShortcuts:
            "No custom commands found"
        case .history:
            "No history found"
        }
    }

    private var returnHintLabel: String {
        switch launchScope {
        case .projects,
             .worktrees:
            "Switch"
        default:
            "Open"
        }
    }

    private var tabHintText: String {
        "Tab/⇧Tab"
    }

    private var navigateHintLabel: String {
        "Navigate"
    }

    private func handleTab() {
        moveHighlight(1)
    }

    private func handleBackTab() {
        moveHighlight(-1)
    }

    private func applyLaunchScope() {
        query = ""
        highlightedIndex = displayList.isEmpty ? nil : 0
    }

    private func shouldShowSectionHeader(at index: Int) -> Bool {
        guard index < displayList.count else { return false }
        if index == 0 { return true }
        return displayList[index].sectionTitle != displayList[index - 1].sectionTitle
    }

    private func moveHighlight(_ delta: Int) {
        guard !displayList.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : displayList.count - 1
            return
        }
        highlightedIndex = max(0, min(displayList.count - 1, current + delta))
    }

    private func confirmSelection() {
        guard let index = highlightedIndex, index < displayList.count else { return }
        dispatchSelection(displayList[index])
    }

    private func handleTap(_ item: TerminalOmniboxItem) {
        dispatchSelection(item)
    }

    private func dispatchSelection(_ item: TerminalOmniboxItem) {
        switch item {
        case .project,
             .worktree:
            onSelect(item, nil, nil)
        case .commandShortcut:
            onSelect(item, activeProjectID, activeWorktreeID)
        case .openTab,
             .closedTab,
             .extensionCommand:
            onSelect(item, nil, nil)
        }
    }
}

private struct TerminalOmniboxSectionHeader: View {
    let title: String

    var body: some View {
        HStack {
            Text(title.uppercased())
                .font(.system(size: UIMetrics.fontXS, weight: .bold))
                .tracking(0.6)
                .foregroundStyle(MuxyTheme.fgDim)
            Spacer()
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.top, UIMetrics.spacing4)
        .padding(.bottom, UIMetrics.scaled(3))
    }
}

private struct TerminalOmniboxRow: View {
    let item: TerminalOmniboxItem
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        HStack(spacing: UIMetrics.spacing5) {
            Image(systemName: item.symbol)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconLG, alignment: .center)
            VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
                Text(item.title)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if let subtitle = item.subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            Spacer(minLength: UIMetrics.spacing2)
        }
        .frame(height: UIMetrics.scaled(40))
        .padding(.horizontal, UIMetrics.spacing6)
        .background(isHighlighted ? MuxyTheme.surface : hovered ? MuxyTheme.hover : .clear)
        .onHover { hovered = $0 }
    }
}

private struct TerminalOmniboxHint: View {
    var symbol: String?
    var text: String?
    var label: String?

    var body: some View {
        HStack(spacing: UIMetrics.scaled(4)) {
            HStack(spacing: UIMetrics.scaled(3)) {
                if let symbol {
                    Image(systemName: symbol)
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                if let text {
                    Text(text)
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }
            .lineLimit(1)
            .fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, UIMetrics.scaled(4))
            .padding(.vertical, UIMetrics.scaled(2))
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            if let label {
                Text(label)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
            }
        }
        .fixedSize(horizontal: true, vertical: false)
    }
}
