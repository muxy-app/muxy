import SwiftUI

enum TabFocusedInternalPaneRowPosition: Equatable {
    case branch
    case last

    init(index: Int, count: Int) {
        self = index == count - 1 ? .last : .branch
    }
}

enum TabFocusedInternalPaneIcon: Equatable {
    case terminal
    case provider(String)

    init(agentIconName: String?) {
        self = agentIconName.map(Self.provider) ?? .terminal
    }
}

struct TabFocusedInternalPaneRow: View {
    let pane: TerminalPaneState
    let position: TabFocusedInternalPaneRowPosition
    let active: Bool
    let onFocus: () -> Void
    let onClose: () -> Void

    @State private var hovered = false
    @State private var progressStore = TerminalProgressStore.shared

    private var paneProgress: TerminalProgress? {
        progressStore.progress(for: pane.id)
    }

    private var agentStatus: AgentStatus? {
        AgentStatusStore.shared.status(forPane: pane.id)
    }

    private var statusDotColor: Color? {
        if paneProgress?.kind == .error {
            return .red
        }
        if agentStatus == .working {
            return .green
        }
        if agentStatus == .waiting {
            return .yellow
        }
        if paneProgress?.kind == .indeterminate || paneProgress?.kind == .paused {
            return .yellow
        }
        return nil
    }

    @ViewBuilder
    private var statusDot: some View {
        if let dotColor = statusDotColor {
            Circle()
                .fill(dotColor)
                .frame(width: 8, height: 8)
        }
    }

    private var rowBackground: AnyShapeStyle {
        if active { return AnyShapeStyle(MuxyTheme.accent.opacity(0.12)) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private var paneIcon: TabFocusedInternalPaneIcon {
        TabFocusedInternalPaneIcon(
            agentIconName: DetectedAgentStore.shared.iconName(forPane: pane.id)
        )
    }

    @ViewBuilder
    private var leadingIcon: some View {
        switch paneIcon {
        case .terminal:
            Image(systemName: "terminal")
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
        case let .provider(iconName):
            ProviderIconView(
                iconName: iconName,
                size: UIMetrics.iconSM,
                monochromeTint: active ? MuxyTheme.accent : MuxyTheme.fgMuted
            )
        }
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            TabFocusedInternalPaneConnector(position: position)
                .stroke(
                    active ? MuxyTheme.accent.opacity(0.75) : MuxyTheme.fgMuted.opacity(0.45),
                    style: StrokeStyle(lineWidth: 1, lineCap: .round, lineJoin: .round)
                )
                .frame(width: UIMetrics.iconSM)
                .frame(maxHeight: .infinity)
                .accessibilityHidden(true)

            leadingIcon
                .foregroundStyle(active ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconSM, height: UIMetrics.iconSM)

            Text(pane.displayTitle)
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer(minLength: 0)

            ZStack {
                if hovered {
                    Button {
                        onClose()
                    } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .frame(width: UIMetrics.iconSM, height: UIMetrics.iconSM)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Close Pane")
                } else {
                    statusDot
                }
            }
            .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
        }
        .padding(.leading, TabFocusedSidebarMetrics.paneTreeLeadingInset)
        .padding(.trailing, TabFocusedSidebarMetrics.rowHorizontalInset)
        .frame(height: TabFocusedSidebarMetrics.paneRowHeight)
        .background {
            RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                .fill(rowBackground)
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .contentShape(RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous))
        .onHover { hovered = $0 }
        .onTapGesture { onFocus() }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Pane: \(pane.title)")
        .accessibilityAddTraits(active ? [.isButton, .isSelected] : .isButton)
    }
}

private struct TabFocusedInternalPaneConnector: Shape {
    let position: TabFocusedInternalPaneRowPosition

    func path(in rect: CGRect) -> Path {
        let centerY = rect.midY
        var path = Path()
        path.move(to: CGPoint(x: rect.minX, y: rect.minY))
        path.addLine(to: CGPoint(x: rect.minX, y: position == .last ? centerY : rect.maxY))
        path.move(to: CGPoint(x: rect.minX, y: centerY))
        path.addLine(to: CGPoint(x: rect.maxX, y: centerY))
        return path
    }
}
