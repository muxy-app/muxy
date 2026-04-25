import SwiftUI

struct AIUsagePreviewButton: View {
    let display: (percent: Int, iconName: String)?
    let percentLabel: String?
    let expanded: Bool
    let onTap: () -> Void

    @State private var hovered = false

    private var foreground: Color {
        hovered ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    var body: some View {
        Button(action: onTap) {
            Group {
                if expanded {
                    expandedLabel
                } else {
                    compactLabel
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("AI Usage")
    }

    private var expandedLabel: some View {
        HStack(spacing: 4) {
            iconGlyph
            if let percentLabel {
                Text(percentLabel)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(foreground)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
        }
        .frame(height: 24)
    }

    private var compactLabel: some View {
        iconGlyph
            .frame(width: 24, height: 24)
    }

    @ViewBuilder
    private var iconGlyph: some View {
        if let display {
            ProviderIconView(iconName: display.iconName, size: 14, style: .monochrome(foreground))
        } else {
            Image(systemName: "sparkles")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(foreground)
        }
    }
}

struct AIUsagePanel: View {
    let snapshots: [AIProviderUsageSnapshot]
    let isRefreshing: Bool
    let lastRefreshDate: Date?
    let onRefresh: () -> Void

    private static let relativeFormatter: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                Image(systemName: "sparkles")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Text("AI Usage")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Spacer()
                Button(action: onRefresh) {
                    Group {
                        if isRefreshing {
                            ProgressView()
                                .controlSize(.small)
                        } else {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 11, weight: .semibold))
                        }
                    }
                    .frame(width: 14, height: 14)
                }
                .buttonStyle(.plain)
                .foregroundStyle(MuxyTheme.fgMuted)
                .disabled(isRefreshing)
                .help("Refresh usage")
                if let lastRefreshDate {
                    Text(Self.relativeFormatter.localizedString(for: lastRefreshDate, relativeTo: Date()))
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }

            if snapshots.isEmpty {
                Text(isRefreshing ? "Refreshing usage data..." : "No usage data yet.")
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgDim)
            }

            if !snapshots.isEmpty {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(snapshots) { snapshot in
                        AIProviderUsageView(snapshot: snapshot)
                    }
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 8))
    }
}

struct AIProviderUsageView: View {
    let snapshot: AIProviderUsageSnapshot

    @AppStorage(AIUsageSettingsStore.sidebarPreviewProviderIDKey) private var pinnedRawValue: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                ProviderIconView(iconName: snapshot.providerIconName, size: 14, style: .monochrome(MuxyTheme.fg))
                Text(snapshot.providerName)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer(minLength: 4)
            }

            switch snapshot.state {
            case .available:
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(snapshot.rows) { row in
                        AIUsageMetricRowView(
                            row: row,
                            fetchedAt: snapshot.fetchedAt,
                            providerID: snapshot.providerID,
                            isPinned: AIUsageSettingsStore.isSidebarPinned(
                                providerID: snapshot.providerID,
                                rowLabel: row.label,
                                pinnedRawValue: pinnedRawValue
                            )
                        )
                    }
                }
            case let .unavailable(message),
                 let .error(message):
                Text(message)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
        }
    }
}

struct AIUsageMetricRowView: View {
    let row: AIUsageMetricRow
    let fetchedAt: Date
    let providerID: String
    let isPinned: Bool

    @AppStorage(AIUsageSettingsStore.usageDisplayModeKey) private var usageDisplayModeRaw = AIUsageSettingsStore.defaultUsageDisplayMode
        .rawValue
    @State private var pinHovered = false

    private var canPin: Bool { row.percent != nil }

    private func togglePin() {
        if isPinned {
            AIUsageSettingsStore.setSidebarPreviewPin(nil)
        } else {
            AIUsageSettingsStore.setSidebarPreviewPin(
                AISidebarPreviewPin(providerID: providerID, rowLabel: row.label)
            )
        }
    }

    private var usageDisplayMode: AIUsageDisplayMode {
        AIUsageDisplayMode(rawValue: usageDisplayModeRaw) ?? AIUsageSettingsStore.defaultUsageDisplayMode
    }

    private var paceResult: AIUsagePaceResult? {
        guard let percentUsed = row.percent,
              let resetsAt = row.resetDate,
              let duration = row.periodDuration
        else { return nil }

        return AIUsagePaceCalculator.compute(
            usedPercent: percentUsed,
            resetsAt: resetsAt,
            periodDuration: duration,
            now: fetchedAt
        )
    }

    private var paceIndicatorColor: Color {
        guard let paceResult else { return .clear }
        switch paceResult.status {
        case .ahead:
            return .green
        case .onTrack:
            return .yellow
        case .behind:
            return .red
        }
    }

    private var paceDetailText: String? {
        guard let paceResult else { return nil }

        if let eta = paceResult.runsOutIn {
            return "Runs out in \(AIUsagePaceCalculator.formatDuration(eta))"
        }

        if let deficit = paceResult.deficitPercent, deficit > 0 {
            return "\(Int(deficit))% in deficit"
        }

        switch usageDisplayMode {
        case .used:
            return "\(Int(paceResult.projectedUsedPercentAtReset))% used at reset"
        case .remaining:
            return "\(Int(paceResult.projectedLeftPercentAtReset))% left at reset"
        }
    }

    private var displayDetail: String? {
        guard let detail = row.detail?.trimmingCharacters(in: .whitespacesAndNewlines), !detail.isEmpty else {
            return nil
        }

        switch usageDisplayMode {
        case .used:
            if let converted = convertRemainingFractionToUsed(detail) {
                return converted
            }
            if let converted = convertRemainingPercentToUsed(detail) {
                return converted
            }
            return detail
        case .remaining:
            if let converted = convertUsedFractionToRemaining(detail) {
                return converted
            }
            if let converted = convertUsedPercentToRemaining(detail) {
                return converted
            }
            return detail
        }
    }

    private func convertUsedFractionToRemaining(_ detail: String) -> String? {
        guard let match = fractionMatch(from: detail), !match.isRemainingLabel else { return nil }
        let remaining = max(0, match.total - match.left)
        return "\(AIUsageParserSupport.formatNumber(remaining))/\(AIUsageParserSupport.formatNumber(match.total))"
    }

    private func convertRemainingFractionToUsed(_ detail: String) -> String? {
        guard let match = fractionMatch(from: detail), match.isRemainingLabel else { return nil }
        let used = max(0, match.total - match.left)
        return "\(AIUsageParserSupport.formatNumber(used))/\(AIUsageParserSupport.formatNumber(match.total))"
    }

    private func convertUsedPercentToRemaining(_ detail: String) -> String? {
        guard let used = percentMatch(from: detail, modeToken: "used") else { return nil }
        let remaining = max(0, min(100, 100 - used))
        return "\(AIUsageParserSupport.formatNumber(remaining))% left"
    }

    private func convertRemainingPercentToUsed(_ detail: String) -> String? {
        guard let remaining = percentMatch(from: detail, modeToken: "left|remaining") else { return nil }
        let used = max(0, min(100, 100 - remaining))
        return "\(AIUsageParserSupport.formatNumber(used))% used"
    }

    private struct FractionMatch {
        let left: Double
        let total: Double
        let isRemainingLabel: Bool
    }

    private func fractionMatch(from detail: String) -> FractionMatch? {
        let pattern = #"^\s*([0-9]+(?:\.[0-9]+)?)\s*/\s*([0-9]+(?:\.[0-9]+)?)(?:\s*(left|remaining))?\s*$"#
        guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else { return nil }
        let range = NSRange(detail.startIndex ..< detail.endIndex, in: detail)
        guard let match = regex.firstMatch(in: detail, options: [], range: range),
              match.numberOfRanges >= 4,
              let leftRange = Range(match.range(at: 1), in: detail),
              let totalRange = Range(match.range(at: 2), in: detail),
              let left = Double(detail[leftRange]),
              let total = Double(detail[totalRange]),
              total > 0
        else {
            return nil
        }

        let remainingRange = match.range(at: 3)
        let isRemainingLabel = remainingRange.location != NSNotFound
        return FractionMatch(left: left, total: total, isRemainingLabel: isRemainingLabel)
    }

    private func percentMatch(from detail: String, modeToken: String) -> Double? {
        let pattern = "^\\s*([0-9]+(?:\\.[0-9]+)?)%\\s*(?:" + modeToken + ")\\s*$"
        guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else { return nil }
        let range = NSRange(detail.startIndex ..< detail.endIndex, in: detail)
        guard let match = regex.firstMatch(in: detail, options: [], range: range),
              match.numberOfRanges >= 2,
              let valueRange = Range(match.range(at: 1), in: detail),
              let value = Double(detail[valueRange])
        else {
            return nil
        }
        return value
    }

    private var displayPercent: Double? {
        guard let percent = row.percent else { return nil }
        let clamped = max(0, min(100, percent))
        switch usageDisplayMode {
        case .used:
            return clamped
        case .remaining:
            return max(0, min(100, 100 - clamped))
        }
    }

    private static let resetFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 4) {
                Text(row.label)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgMuted)

                if paceDetailText != nil {
                    Circle()
                        .fill(paceIndicatorColor)
                        .frame(width: 6, height: 6)
                }

                if canPin {
                    Button(action: togglePin) {
                        Image(systemName: isPinned ? "pin.fill" : "pin")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(isPinned ? MuxyTheme.accent : (pinHovered ? MuxyTheme.fg : MuxyTheme.fgMuted))
                            .rotationEffect(.degrees(45))
                            .frame(width: 14, height: 14)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .onHover { pinHovered = $0 }
                    .help(isPinned ? "Unpin from sidebar" : "Show this usage in the sidebar")
                }

                Spacer()
                if let percent = displayPercent {
                    Text("\(Int(percent.rounded()))%")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(MuxyTheme.fg)
                }
                if let detail = displayDetail {
                    Text(detail)
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }

            if let percent = displayPercent {
                ProgressView(value: percent, total: 100)
                    .tint(MuxyTheme.accent)
                    .controlSize(.small)
            }

            if let resetDate = row.resetDate {
                HStack(spacing: 6) {
                    Text("Resets \(Self.resetFormatter.string(from: resetDate))")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgDim)

                    Spacer(minLength: 0)

                    if let paceDetailText {
                        Text(paceDetailText)
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.fgDim)
                            .lineLimit(1)
                    }
                }
            }
        }
    }
}
