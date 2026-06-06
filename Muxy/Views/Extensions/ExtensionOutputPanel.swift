import AppKit
import SwiftUI

struct ExtensionOutputPanel: View {
    @Binding var selectedExtensionID: String?

    @State private var store = ExtensionStore.shared
    @State private var lines: [String] = []
    @State private var tailer: ExtensionLogTailer?
    @State private var coalescer: ExtensionLogCoalescer?

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            logBody
        }
        .frame(maxWidth: .infinity)
        .background(MuxyTheme.bg)
        .onAppear { restartTailer() }
        .onDisappear {
            tailer?.stop()
            coalescer?.cancel()
        }
        .onChange(of: effectiveExtensionID) { _, _ in
            restartTailer()
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Menu {
                ForEach(store.statuses) { status in
                    Button(status.muxyExtension.displayName) {
                        selectedExtensionID = status.id
                    }
                }
                if store.statuses.isEmpty {
                    Text("No extensions").foregroundStyle(MuxyTheme.fgMuted)
                }
            } label: {
                HStack(spacing: 4) {
                    Text(activeLabel)
                        .lineLimit(1)
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontCaption))
                }
                .foregroundStyle(MuxyTheme.fg)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 4))
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            Spacer()
            Button("Reveal") {
                if let url = activeLogURL {
                    NSWorkspace.shared.activateFileViewerSelecting([url])
                }
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.accent)
            Button("Clear") {
                tailer?.clear()
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.accent)
        }
        .font(.system(size: UIMetrics.fontFootnote))
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }

    private var logBody: some View {
        ScrollViewReader { proxy in
            ScrollView {
                Group {
                    if lines.isEmpty {
                        Text("No log output yet.")
                            .font(.system(size: UIMetrics.fontFootnote))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .padding(8)
                    } else {
                        logText
                            .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 8)
                            .textSelection(.enabled)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.vertical, 4)

                Color.clear
                    .frame(height: 1)
                    .id(scrollAnchorID)
            }
            .onChange(of: lines.last) { _, _ in
                proxy.scrollTo(scrollAnchorID, anchor: .bottom)
            }
        }
        .frame(maxHeight: .infinity)
    }

    private var logText: Text {
        let lastIndex = lines.count - 1
        return lines.enumerated().reduce(Text("")) { result, entry in
            let (index, line) = entry
            let suffix = index < lastIndex ? "\n" : ""
            return result + Text(line + suffix).foregroundStyle(color(for: line))
        }
    }

    private let scrollAnchorID = "muxy.extension-console.bottom"

    private var effectiveExtensionID: String? {
        if let selectedExtensionID, store.statuses.contains(where: { $0.id == selectedExtensionID }) {
            return selectedExtensionID
        }
        return store.statuses.first?.id
    }

    private var activeLabel: String {
        guard let id = effectiveExtensionID else { return "(none)" }
        return id
    }

    private var activeLogURL: URL? {
        guard let id = effectiveExtensionID,
              let status = store.statuses.first(where: { $0.id == id })
        else { return nil }
        return status.logFileURL
    }

    private func restartTailer() {
        tailer?.stop()
        tailer = nil
        coalescer?.cancel()
        lines = []
        guard let url = activeLogURL else { return }
        let newCoalescer = ExtensionLogCoalescer { update in
            applyUpdate(update)
        }
        let newTailer = ExtensionLogTailer(url: url) { update in
            newCoalescer.ingest(update)
        }
        coalescer = newCoalescer
        tailer = newTailer
        newTailer.start()
    }

    private func applyUpdate(_ update: ExtensionLogUpdate) {
        switch update {
        case let .reset(newLines):
            lines = newLines
        case let .append(newLines):
            lines.append(contentsOf: newLines)
        }
        if lines.count > ExtensionLogTailer.maxBufferedLines {
            lines.removeFirst(lines.count - ExtensionLogTailer.maxBufferedLines)
        }
    }

    private func color(for line: String) -> Color {
        if line.hasPrefix("[err]") { return MuxyTheme.diffRemoveFg }
        if line.hasPrefix("[warn]") { return MuxyTheme.warning }
        return MuxyTheme.fgMuted
    }
}
