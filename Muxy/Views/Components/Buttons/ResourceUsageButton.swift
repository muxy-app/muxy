import SwiftUI

struct ResourceUsageButton: View {
    @State private var monitor = ProcessResourceMonitor.shared
    @State private var showingPopover = false
    @State private var hovered = false

    var body: some View {
        Button {
            showingPopover.toggle()
        } label: {
            Image(systemName: "cpu")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .padding(.horizontal, 4)
                .frame(maxHeight: .infinity)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help("App & subprocess resource usage")
        .accessibilityLabel("Resource usage")
        .onAppear { monitor.beginObserving() }
        .onDisappear { monitor.endObserving() }
        .popover(isPresented: $showingPopover, arrowEdge: .bottom) {
            ResourceUsagePopover(onClose: { showingPopover = false })
                .onAppear { monitor.refreshNow() }
        }
    }
}

private struct ResourceUsagePopover: View {
    let onClose: () -> Void

    @State private var monitor = ProcessResourceMonitor.shared
    @AppStorage(ResourceUsagePreferences.visibleKey) private var showResourceUsage = ResourceUsagePreferences.defaultVisible

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            header
            ScrollView {
                VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
                    ForEach(ProcessGroup.allCases, id: \.self) { group in
                        groupSection(group)
                    }
                }
            }
            .frame(maxHeight: UIMetrics.scaled(360))
            Divider()
            hideButton
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(260))
        .background(MuxyTheme.bg)
    }

    private var hideButton: some View {
        Button {
            showResourceUsage = false
            onClose()
        } label: {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "eye.slash")
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                Text("Hide from status bar")
                    .font(.system(size: UIMetrics.fontFootnote))
                Spacer(minLength: 0)
            }
            .foregroundStyle(MuxyTheme.fgMuted)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("Hide resource usage. Re-enable it in Settings → Interface.")
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "cpu")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
            Text("Resources")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Spacer(minLength: UIMetrics.spacing6)
            Text(totalLabel)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .monospaced))
                .monospacedDigit()
                .foregroundStyle(MuxyTheme.fg)
        }
    }

    private var totalLabel: String {
        let cpu = ProcessUsageFormat.compactCPU(monitor.snapshot.totalCPUPercent)
        let memory = ProcessUsageFormat.compactMemory(monitor.snapshot.totalMemoryBytes)
        return "\(cpu) · \(memory)"
    }

    @ViewBuilder
    private func groupSection(_ group: ProcessGroup) -> some View {
        let rows = monitor.snapshot.rows(in: group)
        if !rows.isEmpty {
            VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
                Text(group.title.uppercased())
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                    .foregroundStyle(MuxyTheme.fgDim)
                ForEach(rows) { row in
                    processRow(row)
                }
            }
        }
    }

    private func processRow(_ row: ProcessUsageRow) -> some View {
        HStack(spacing: UIMetrics.spacing3) {
            Text(row.name)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
            Text("\(row.pid)")
                .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
            Spacer(minLength: UIMetrics.spacing4)
            Text(ProcessUsageFormat.compactCPU(row.cpuPercent))
                .font(.system(size: UIMetrics.fontCaption, weight: .medium, design: .monospaced))
                .monospacedDigit()
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(34), alignment: .trailing)
            Text(ProcessUsageFormat.compactMemory(row.memoryBytes))
                .font(.system(size: UIMetrics.fontCaption, weight: .medium, design: .monospaced))
                .monospacedDigit()
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(42), alignment: .trailing)
        }
    }
}
