import SwiftUI

struct TabFocusedAttentionItem: Identifiable {
    enum State: Equatable {
        case waiting
        case finished

        var systemImage: String {
            switch self {
            case .waiting: "questionmark.circle.fill"
            case .finished: "checkmark.circle.fill"
            }
        }
    }

    let paneID: UUID
    let projectID: UUID
    let worktreeID: UUID
    let worktreePath: String
    let areaID: UUID
    let tabID: UUID
    let tabTitle: String
    let scopeTitle: String
    let state: State

    var id: UUID { paneID }
}

struct TabFocusedAttentionMenu: View {
    let items: [TabFocusedAttentionItem]
    let onSelect: (TabFocusedAttentionItem) -> Void

    private var waitingItems: [TabFocusedAttentionItem] {
        items.filter { $0.state == .waiting }
    }

    private var finishedItems: [TabFocusedAttentionItem] {
        items.filter { $0.state == .finished }
    }

    var body: some View {
        Menu {
            if !waitingItems.isEmpty {
                Section(L10n.string("Waiting for attention")) {
                    attentionItems(waitingItems)
                }
            }
            if !finishedItems.isEmpty {
                Section(L10n.string("Finished")) {
                    attentionItems(finishedItems)
                }
            }
        } label: {
            label
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
        .accessibilityLabel(accessibilityLabel)
    }

    private var label: some View {
        HStack(spacing: UIMetrics.spacing2) {
            if !waitingItems.isEmpty {
                Image(systemName: TabFocusedAttentionItem.State.waiting.systemImage)
                    .foregroundStyle(color(for: .waiting))
                Text(L10n.resource("Waiting for attention"))
                countBadge(waitingItems.count, color: color(for: .waiting))
            } else {
                Image(systemName: TabFocusedAttentionItem.State.finished.systemImage)
                    .foregroundStyle(color(for: .finished))
                Text(L10n.resource("Finished"))
                countBadge(finishedItems.count, color: color(for: .finished))
            }

            if !waitingItems.isEmpty, !finishedItems.isEmpty {
                Image(systemName: TabFocusedAttentionItem.State.finished.systemImage)
                    .foregroundStyle(color(for: .finished))
                Text("\(finishedItems.count)")
                    .foregroundStyle(MuxyTheme.fgMuted)
            }

            Spacer(minLength: 0)
        }
        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
        .foregroundStyle(MuxyTheme.fg)
        .padding(.horizontal, UIMetrics.spacing3)
        .padding(.vertical, UIMetrics.spacing2)
        .background {
            RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                .fill(MuxyTheme.surface)
        }
    }

    private func countBadge(_ count: Int, color: Color) -> some View {
        Text("\(count)")
            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            .foregroundStyle(MuxyTheme.bg)
            .padding(.horizontal, UIMetrics.spacing2)
            .padding(.vertical, 1)
            .background(Capsule().fill(color))
    }

    private func color(for state: TabFocusedAttentionItem.State) -> Color {
        switch state {
        case .waiting: MuxyTheme.warning
        case .finished: MuxyTheme.accent
        }
    }

    @ViewBuilder
    private func attentionItems(_ items: [TabFocusedAttentionItem]) -> some View {
        ForEach(items) { item in
            Button {
                onSelect(item)
            } label: {
                Label {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(item.tabTitle)
                        Text(item.scopeTitle)
                            .font(.caption)
                    }
                } icon: {
                    Image(systemName: item.state.systemImage)
                        .foregroundStyle(color(for: item.state))
                }
            }
        }
    }

    private var accessibilityLabel: String {
        if !waitingItems.isEmpty {
            return L10n.string("Waiting for attention")
        }
        return L10n.string("Finished")
    }
}
