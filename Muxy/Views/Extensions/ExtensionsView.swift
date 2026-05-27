import AppKit
import SwiftUI

struct ExtensionsView: View {
    @State private var store = ExtensionStore.shared
    @State private var grantStore = ExtensionGrantStore.shared
    @State private var selectedExtensionID: String?

    var body: some View {
        VStack(spacing: 0) {
            header
            ExtensionsDivider()
            content
        }
        .frame(minWidth: 760, minHeight: 560)
        .background(MuxyTheme.bg)
        .foregroundStyle(MuxyTheme.fg)
        .tint(MuxyTheme.accent)
        .preferredColorScheme(MuxyTheme.colorScheme)
    }

    @ViewBuilder
    private var content: some View {
        if let id = selectedExtensionID, let status = store.statuses.first(where: { $0.id == id }) {
            ExtensionDetailPage(
                status: status,
                store: store,
                grantStore: grantStore
            )
        } else {
            ExtensionsListPage(
                store: store,
                onSelect: { selectedExtensionID = $0 }
            )
        }
    }

    private var header: some View {
        HStack(spacing: 10) {
            if selectedExtensionID != nil {
                Button {
                    selectedExtensionID = nil
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "chevron.left")
                            .font(.system(size: 11, weight: .semibold))
                        Text("Extensions")
                            .font(.system(size: 12, weight: .medium))
                    }
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(MuxyTheme.hover, in: RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain)
                .help("Back to Extensions")
            } else {
                HStack(spacing: 8) {
                    Image(systemName: "puzzlepiece.extension")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text("Extensions")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                    SettingsDevelopmentBadge(text: "DEV")
                }
            }
            Spacer()
            if selectedExtensionID == nil {
                Button {
                    store.reload()
                } label: {
                    Text("Reload")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.accent)
                }
                .buttonStyle(.plain)
                .help("Reload Extensions")
                Button {
                    NSWorkspace.shared.activateFileViewerSelecting([store.rootDirectory])
                } label: {
                    Text("Reveal Folder")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.accent)
                }
                .buttonStyle(.plain)
                .help("Open extensions folder in Finder")
            }
            Button {
                NSApp.keyWindow?.close()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: 28, height: 28)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Close")
        }
        .padding(.horizontal, 16)
        .frame(height: 56)
        .background(MuxyTheme.bg)
    }
}

private struct ExtensionsDivider: View {
    var body: some View {
        Rectangle()
            .fill(MuxyTheme.border)
            .frame(height: 1)
    }
}

private struct ExtensionsListPage: View {
    let store: ExtensionStore
    let onSelect: (String) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                developmentBanner
                if !store.loadFailures.isEmpty {
                    LoadFailuresBlock(failures: store.loadFailures)
                }
                if store.statuses.isEmpty {
                    emptyState
                } else {
                    VStack(spacing: 0) {
                        ForEach(Array(store.statuses.enumerated()), id: \.element.id) { index, status in
                            ExtensionRow(status: status) {
                                onSelect(status.id)
                            }
                            if index < store.statuses.count - 1 {
                                Rectangle()
                                    .fill(MuxyTheme.border)
                                    .frame(height: 1)
                            }
                        }
                    }
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 10))
                    .overlay(
                        RoundedRectangle(cornerRadius: 10)
                            .stroke(MuxyTheme.border, lineWidth: 1)
                    )
                }
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
    }

    private var developmentBanner: some View {
        HStack(alignment: .top, spacing: 8) {
            SettingsDevelopmentBadge(text: "DEV")
            Text("Extensions are under active development. APIs, manifest format, and behavior may change without notice.")
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Image(systemName: "puzzlepiece.extension")
                .font(.system(size: 32))
                .foregroundStyle(MuxyTheme.fgDim)
            Text("No extensions installed")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text("Drop an extension into the extensions folder to get started.")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 60)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 10))
    }
}

private struct LoadFailuresBlock: View {
    let failures: [ExtensionStore.LoadFailure]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                Text("Load Errors")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            ForEach(failures) { failure in
                VStack(alignment: .leading, spacing: 2) {
                    Text(failure.directory.lastPathComponent)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(MuxyTheme.diffRemoveFg)
                    Text(failure.message)
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.diffRemoveBg, in: RoundedRectangle(cornerRadius: 8))
    }
}

private struct ExtensionRow: View {
    let status: ExtensionStore.ExtensionStatus
    let onOpen: () -> Void
    @State private var hovered = false

    private var ext: MuxyExtension { status.muxyExtension }

    var body: some View {
        Button(action: onOpen) {
            HStack(spacing: 12) {
                Image(systemName: "puzzlepiece.extension.fill")
                    .font(.system(size: 16))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: 22)
                VStack(alignment: .leading, spacing: 3) {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(ext.displayName)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fg)
                            .lineLimit(1)
                        Text("v\(ext.manifest.version)")
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.fgMuted)
                        ExtensionStatusBadge(status: status)
                    }
                    if let description = ext.manifest.description, !description.isEmpty {
                        Text(description)
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }
                }
                Spacer(minLength: 12)
                ExtensionPermissionSummary(permissions: ext.manifest.permissions)
                Image(systemName: "chevron.right")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(hovered ? MuxyTheme.hover : Color.clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

private struct ExtensionPermissionCounts {
    var read: Int = 0
    var write: Int = 0
    var action: Int = 0

    init(_ permissions: [ExtensionPermission]) {
        for permission in permissions {
            switch permission.kind {
            case .read: read += 1
            case .write: write += 1
            case .action: action += 1
            }
        }
    }
}

private struct ExtensionPermissionSummary: View {
    let permissions: [ExtensionPermission]

    var body: some View {
        if permissions.isEmpty {
            Text("no permissions")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(MuxyTheme.fgDim)
        } else {
            let counts = ExtensionPermissionCounts(permissions)
            HStack(spacing: 6) {
                if counts.read > 0 {
                    summaryChip(color: MuxyTheme.warning, label: "R", count: counts.read)
                }
                if counts.write > 0 {
                    summaryChip(color: MuxyTheme.diffRemoveFg, label: "W", count: counts.write)
                }
                if counts.action > 0 {
                    summaryChip(color: MuxyTheme.accent, label: "A", count: counts.action)
                }
            }
        }
    }

    private func summaryChip(color: Color, label: String, count: Int) -> some View {
        HStack(spacing: 3) {
            Text(label)
                .font(.system(size: 10, weight: .bold))
            Text("\(count)")
                .font(.system(size: 10, weight: .medium))
        }
        .foregroundStyle(color)
        .padding(.horizontal, 6)
        .padding(.vertical, 2)
        .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
    }
}

private struct ExtensionStatusBadge: View {
    let status: ExtensionStore.ExtensionStatus

    var body: some View {
        let (label, color) = info
        Text(label)
            .font(.system(size: 10, weight: .medium))
            .foregroundStyle(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
    }

    @MainActor
    private var info: (String, Color) {
        if status.isRunning { return ("running", MuxyTheme.diffAddFg) }
        if status.isEnabled { return ("stopped", MuxyTheme.fgMuted) }
        return ("disabled", MuxyTheme.fgDim)
    }
}

private struct ExtensionPermissionTagsRow: View {
    let permissions: [ExtensionPermission]

    var body: some View {
        if permissions.isEmpty {
            Text("no permissions")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(MuxyTheme.fgDim)
        } else {
            ExtensionPermissionFlowLayout(spacing: 6, lineSpacing: 6) {
                ForEach(permissions, id: \.rawValue) { permission in
                    ExtensionPermissionTag(permission: permission)
                }
            }
        }
    }
}

private struct ExtensionPermissionTag: View {
    let permission: ExtensionPermission

    var body: some View {
        let (color, kindLabel) = style
        HStack(spacing: 4) {
            Circle()
                .fill(color)
                .frame(width: 5, height: 5)
            Text(permission.rawValue)
                .font(.system(size: 10, weight: .medium, design: .monospaced))
                .foregroundStyle(color)
            if let kindLabel {
                Text(kindLabel)
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(color)
                    .padding(.horizontal, 3)
                    .padding(.vertical, 1)
                    .background(color.opacity(0.18), in: RoundedRectangle(cornerRadius: 3))
            }
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(color.opacity(0.1), in: RoundedRectangle(cornerRadius: 5))
        .overlay(
            RoundedRectangle(cornerRadius: 5)
                .stroke(color.opacity(0.35), lineWidth: 1)
        )
        .help(helpText)
    }

    @MainActor
    private var style: (Color, String?) {
        switch permission.kind {
        case .read: (MuxyTheme.warning, "R")
        case .write: (MuxyTheme.diffRemoveFg, "W")
        case .action: (MuxyTheme.accent, nil)
        }
    }

    private var helpText: String {
        switch permission.kind {
        case .read: "Read access: \(permission.rawValue)"
        case .write: "Write access: \(permission.rawValue)"
        case .action: "Action: \(permission.rawValue)"
        }
    }
}

private struct ExtensionPermissionFlowLayout: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache _: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        let layout = arrange(subviews: subviews, in: maxWidth)
        return CGSize(width: maxWidth.isFinite ? maxWidth : layout.width, height: layout.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache _: inout ()) {
        let layout = arrange(subviews: subviews, in: bounds.width)
        for placement in layout.placements {
            subviews[placement.index].place(
                at: CGPoint(x: bounds.minX + placement.point.x, y: bounds.minY + placement.point.y),
                proposal: ProposedViewSize(placement.size)
            )
        }
    }

    private struct Placement {
        let index: Int
        let point: CGPoint
        let size: CGSize
    }

    private struct ArrangedLayout {
        let width: CGFloat
        let height: CGFloat
        let placements: [Placement]
    }

    private func arrange(subviews: Subviews, in maxWidth: CGFloat) -> ArrangedLayout {
        var placements: [Placement] = []
        var currentX: CGFloat = 0
        var currentY: CGFloat = 0
        var lineHeight: CGFloat = 0
        var widest: CGFloat = 0
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            if currentX > 0, currentX + size.width > maxWidth {
                currentY += lineHeight + lineSpacing
                currentX = 0
                lineHeight = 0
            }
            placements.append(Placement(index: index, point: CGPoint(x: currentX, y: currentY), size: size))
            currentX += size.width + spacing
            lineHeight = max(lineHeight, size.height)
            widest = max(widest, currentX - spacing)
        }
        return ArrangedLayout(width: widest, height: currentY + lineHeight, placements: placements)
    }
}

private struct ExtensionDetailPage: View {
    let status: ExtensionStore.ExtensionStatus
    let store: ExtensionStore
    let grantStore: ExtensionGrantStore
    @State private var showLogs = false

    private var ext: MuxyExtension { status.muxyExtension }

    private var grantRules: [ExtensionGrantRule] {
        grantStore.rules.filter { $0.extensionID == status.id }
            .sorted { $0.createdAt < $1.createdAt }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                heroBlock
                if let error = status.lastError {
                    errorBlock(error)
                }
                permissionsBlock
                if !ext.manifest.commands.isEmpty { commandsBlock }
                if !ext.manifest.tabTypes.isEmpty { tabTypesBlock }
                grantsBlock
                logsBlock
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
    }

    private var heroBlock: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(ext.displayName)
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Text("v\(ext.manifest.version)")
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgMuted)
                ExtensionStatusBadge(status: status)
                Spacer()
                Toggle("", isOn: enabledBinding)
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .controlSize(.small)
            }
            if let description = ext.manifest.description, !description.isEmpty {
                Text(description)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack(spacing: 12) {
                Button {
                    NSWorkspace.shared.activateFileViewerSelecting([ext.directory])
                } label: {
                    Text("Reveal in Finder")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.accent)
                }
                .buttonStyle(.plain)
                Text(ext.directory.path)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 10))
    }

    private func errorBlock(_ error: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                Text("Runtime Error")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
            }
            Text(error)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(MuxyTheme.diffRemoveFg)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.diffRemoveBg, in: RoundedRectangle(cornerRadius: 8))
    }

    private var permissionsBlock: some View {
        DetailSection(title: "Permissions") {
            VStack(alignment: .leading, spacing: 10) {
                ExtensionPermissionTagsRow(permissions: ext.manifest.permissions)
                permissionLegend
            }
        }
    }

    private var permissionLegend: some View {
        HStack(spacing: 14) {
            legendItem(color: MuxyTheme.warning, label: "Read")
            legendItem(color: MuxyTheme.diffRemoveFg, label: "Write")
            legendItem(color: MuxyTheme.accent, label: "Action")
        }
        .padding(.top, 2)
    }

    private func legendItem(color: Color, label: String) -> some View {
        HStack(spacing: 4) {
            Circle().fill(color).frame(width: 6, height: 6)
            Text(label)
                .font(.system(size: 10))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }

    private var commandsBlock: some View {
        DetailSection(title: "Commands") {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(ext.manifest.commands) { command in
                    HStack(spacing: 8) {
                        Text(command.id)
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundStyle(MuxyTheme.fg)
                            .frame(minWidth: 140, alignment: .leading)
                        Text(command.title)
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.fgMuted)
                        Spacer()
                        Text(actionLabel(command.action))
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(MuxyTheme.fgDim)
                    }
                    .padding(.vertical, 3)
                }
            }
        }
    }

    private func actionLabel(_ action: ExtensionCommandAction) -> String {
        switch action {
        case .event: "event"
        case let .openTab(tabType, _): "opens \(tabType)"
        case let .runScript(script): "runs \(script)"
        }
    }

    private var tabTypesBlock: some View {
        DetailSection(title: "Tab Types") {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(ext.manifest.tabTypes) { tabType in
                    HStack(spacing: 8) {
                        Text(tabType.id)
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundStyle(MuxyTheme.fg)
                            .frame(minWidth: 140, alignment: .leading)
                        Text(tabType.title)
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    .padding(.vertical, 3)
                }
            }
        }
    }

    private var grantsBlock: some View {
        DetailSection(title: "Permission Rules") {
            VStack(alignment: .leading, spacing: 6) {
                if grantRules.isEmpty {
                    Text("No saved rules. The extension will prompt the first time it requests exec, send-keys, or read-screen.")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    HStack {
                        Spacer()
                        Button("Clear All") {
                            grantStore.removeAll(for: status.id)
                        }
                        .buttonStyle(.plain)
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.diffRemoveFg)
                    }
                    ForEach(grantRules) { rule in
                        ExtensionGrantRuleRow(rule: rule, grantStore: grantStore)
                    }
                }
            }
        }
    }

    private var logsBlock: some View {
        DetailSection(
            title: "Logs",
            trailing: AnyView(
                HStack(spacing: 10) {
                    Button {
                        NSWorkspace.shared.activateFileViewerSelecting([status.logFileURL])
                    } label: {
                        Text("Reveal Log")
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.accent)
                    }
                    .buttonStyle(.plain)
                    Button {
                        showLogs.toggle()
                    } label: {
                        Text(showLogs ? "Hide" : "Show")
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.accent)
                    }
                    .buttonStyle(.plain)
                }
            )
        ) {
            if showLogs {
                logView
            } else {
                Text(status.logFileURL.path)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
        }
    }

    private var logView: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 1) {
                let lines = ExtensionLogTail.read(url: status.logFileURL, maxLines: 200)
                if lines.isEmpty {
                    Text("No log output.")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)
                } else {
                    ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                        Text(line)
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            .padding(10)
        }
        .frame(maxHeight: 200)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }

    private var enabledBinding: Binding<Bool> {
        Binding(
            get: { status.isEnabled },
            set: { store.setEnabled($0, for: status.id) }
        )
    }
}

private struct DetailSection<Content: View>: View {
    let title: String
    var trailing: AnyView?
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(title)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .textCase(.uppercase)
                    .tracking(0.6)
                Spacer()
                if let trailing { trailing }
            }
            content
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(MuxyTheme.surface.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
    }
}

private struct ExtensionGrantRuleRow: View {
    let rule: ExtensionGrantRule
    let grantStore: ExtensionGrantStore

    var body: some View {
        HStack(spacing: 8) {
            decisionBadge
            Text(rule.verb.rawValue)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
                .frame(width: 130, alignment: .leading)
            Text(rule.match.displayString)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
            Button {
                grantStore.remove(ruleID: rule.id)
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background(MuxyTheme.bg.opacity(0.5), in: RoundedRectangle(cornerRadius: 5))
    }

    private var decisionBadge: some View {
        let isAllow = rule.decision == .allow
        let label = isAllow ? "allow" : "deny"
        let color = isAllow ? MuxyTheme.diffAddFg : MuxyTheme.diffRemoveFg
        return Text(label)
            .font(.system(size: 10, weight: .semibold))
            .foregroundStyle(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
    }
}
