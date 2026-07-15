import AppKit
import MuxyShared
import SwiftTerm
import SwiftUI

struct RemoteTerminalBridge: View {
    let paneID: UUID
    let focused: Bool
    let visible: Bool
    let onFocus: () -> Void

    @Environment(RemoteMacWorkspaceStore.self) private var workspaceStore

    @State private var size: (cols: UInt32, rows: UInt32)?
    @State private var isTakingOver = false
    @State private var didAttemptTakeover = false

    private var connection: RemoteMacConnection? { workspaceStore.activeConnection }

    private var ownsPane: Bool { connection?.isPaneOwnedByThisMac(paneID) == true }

    var body: some View {
        ZStack {
            if let connection, visible {
                RemoteMacTerminalRepresentable(
                    connection: connection,
                    paneID: paneID,
                    focused: focused,
                    onFocus: onFocus,
                    onSize: handleSize
                )
                .opacity(ownsPane ? 1 : 0.01)
                .allowsHitTesting(ownsPane)
            }

            if visible, !ownsPane {
                takeoverOverlay
            }
        }
        .background(themeColor(connection?.deviceTheme?.bg ?? 0x000000))
        .contentShape(Rectangle())
        .onDisappear {
            releasePane()
        }
        .onChange(of: paneID) { previousPaneID, _ in
            if let connection {
                Task { await connection.releasePane(paneID: previousPaneID) }
            }
            didAttemptTakeover = false
            attemptTakeover()
        }
        .onChange(of: visible) { _, isVisible in
            guard isVisible else {
                releasePane()
                return
            }
            didAttemptTakeover = false
            attemptTakeover()
        }
    }

    private var takeoverOverlay: some View {
        VStack(spacing: UIMetrics.spacing4) {
            if isTakingOver {
                ProgressView().controlSize(.small)
                Text("Connecting terminal…")
                    .font(.system(size: UIMetrics.fontFootnote))
            } else {
                Image(systemName: "desktopcomputer")
                    .font(.system(size: UIMetrics.iconXL))
                Text(ownerMessage)
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                Button("Take Over", action: takeOver)
            }
        }
        .foregroundStyle(themeColor(connection?.deviceTheme?.fg ?? 0xFFFFFF))
        .padding(UIMetrics.spacing6)
    }

    private var ownerMessage: String {
        guard let owner = connection?.paneOwners[paneID] else { return "Terminal is available" }
        return "Controlled on \(owner.displayName)"
    }

    private func handleSize(_ cols: UInt32, _ rows: UInt32) {
        size = (cols, rows)
        attemptTakeover()
    }

    private func attemptTakeover() {
        guard visible, !didAttemptTakeover, size != nil else { return }
        didAttemptTakeover = true
        takeOver()
    }

    private func takeOver() {
        guard visible, let size, let connection else { return }
        isTakingOver = true
        let takeoverTask = connection.takeOverPane(paneID: paneID, cols: size.cols, rows: size.rows)
        Task {
            do {
                try await takeoverTask.value
            } catch {
                ToastState.shared.show(error.localizedDescription)
            }
            isTakingOver = false
        }
    }

    private func releasePane() {
        guard let connection else { return }
        Task { await connection.releasePane(paneID: paneID) }
    }

    private func themeColor(_ rgb: UInt32) -> SwiftUI.Color {
        SwiftUI.Color(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }
}

private struct RemoteMacTerminalRepresentable: NSViewRepresentable {
    let connection: RemoteMacConnection
    let paneID: UUID
    let focused: Bool
    let onFocus: () -> Void
    let onSize: (UInt32, UInt32) -> Void

    func makeNSView(context: Context) -> RemoteSwiftTermView {
        let font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        let view = RemoteSwiftTermView(frame: .zero, font: font)
        view.terminalDelegate = context.coordinator
        view.backspaceSendsControlH = false
        view.allowMouseReporting = true
        view.onFocus = onFocus
        context.coordinator.bind(
            view: view,
            connection: connection,
            paneID: paneID,
            onSize: onSize
        )
        view.apply(theme: connection.deviceTheme)
        updateFocus(view)
        return view
    }

    func updateNSView(_ view: RemoteSwiftTermView, context: Context) {
        context.coordinator.update(
            view: view,
            connection: connection,
            paneID: paneID,
            onSize: onSize
        )
        view.onFocus = onFocus
        view.apply(theme: connection.deviceTheme)
        updateFocus(view)
    }

    static func dismantleNSView(_: RemoteSwiftTermView, coordinator: Coordinator) {
        coordinator.unbind()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    private func updateFocus(_ view: RemoteSwiftTermView) {
        guard focused else { return }
        DispatchQueue.main.async { [weak view] in
            view?.window?.makeFirstResponder(view)
        }
    }

    @MainActor
    final class Coordinator: NSObject, TerminalViewDelegate {
        weak var view: RemoteSwiftTermView?
        weak var connection: RemoteMacConnection?
        private var paneID: UUID?
        private var observerID: UUID?
        private var onSize: ((UInt32, UInt32) -> Void)?
        private var lastSize: (Int, Int)?
        private var resizeTask: Task<Void, Never>?

        func bind(
            view: RemoteSwiftTermView,
            connection: RemoteMacConnection,
            paneID: UUID,
            onSize: @escaping (UInt32, UInt32) -> Void
        ) {
            self.view = view
            self.connection = connection
            self.paneID = paneID
            self.onSize = onSize
            subscribe()
        }

        func update(
            view: RemoteSwiftTermView,
            connection: RemoteMacConnection,
            paneID: UUID,
            onSize: @escaping (UInt32, UInt32) -> Void
        ) {
            self.onSize = onSize
            guard self.connection !== connection || self.paneID != paneID else { return }
            unbind()
            view.getTerminal().resetToInitialState()
            bind(view: view, connection: connection, paneID: paneID, onSize: onSize)
        }

        func unbind() {
            resizeTask?.cancel()
            resizeTask = nil
            if let observerID {
                connection?.removeEventObserver(observerID)
            }
            observerID = nil
            view = nil
            connection = nil
            paneID = nil
            onSize = nil
            lastSize = nil
        }

        private func subscribe() {
            guard let connection else { return }
            observerID = connection.addEventObserver { [weak self] event in
                guard let self, let view, let paneID else { return }
                let output: TerminalOutputEventDTO? = switch event.data {
                case let .terminalOutput(value):
                    value
                case let .terminalSnapshot(value):
                    value
                default:
                    nil
                }
                guard let output, output.paneID == paneID else { return }
                view.feed(byteArray: [UInt8](output.bytes)[...])
            }
        }

        nonisolated func send(source _: SwiftTerm.TerminalView, data: ArraySlice<UInt8>) {
            MainActor.assumeIsolated {
                guard let connection, let paneID else { return }
                connection.sendTerminalInput(paneID: paneID, bytes: Data(data))
            }
        }

        nonisolated func sizeChanged(source _: SwiftTerm.TerminalView, newCols: Int, newRows: Int) {
            MainActor.assumeIsolated {
                guard newCols > 0, newRows > 0 else { return }
                if let lastSize, lastSize.0 == newCols, lastSize.1 == newRows { return }
                lastSize = (newCols, newRows)
                let cols = UInt32(newCols)
                let rows = UInt32(newRows)
                onSize?(cols, rows)
                resizeTask?.cancel()
                resizeTask = Task { @MainActor [weak self] in
                    try? await Task.sleep(for: .milliseconds(100))
                    guard !Task.isCancelled, let self, let connection, let paneID else { return }
                    try? await connection.resizeTerminal(paneID: paneID, cols: cols, rows: rows)
                }
            }
        }

        nonisolated func setTerminalTitle(source _: SwiftTerm.TerminalView, title _: String) {}
        nonisolated func hostCurrentDirectoryUpdate(source _: SwiftTerm.TerminalView, directory _: String?) {}
        nonisolated func scrolled(source _: SwiftTerm.TerminalView, position _: Double) {}
        nonisolated func requestOpenLink(source _: SwiftTerm.TerminalView, link: String, params _: [String: String]) {
            guard let url = URL(string: link),
                  let scheme = url.scheme?.lowercased(),
                  scheme == "https" || scheme == "http"
            else { return }
            Task { @MainActor in NSWorkspace.shared.open(url) }
        }

        nonisolated func bell(source _: SwiftTerm.TerminalView) {}
        nonisolated func clipboardCopy(source _: SwiftTerm.TerminalView, content _: Data) {}

        nonisolated func clipboardRead(source _: SwiftTerm.TerminalView) -> Data? {
            nil
        }

        nonisolated func iTermContent(source _: SwiftTerm.TerminalView, content _: ArraySlice<UInt8>) {}
        nonisolated func rangeChanged(source _: SwiftTerm.TerminalView, startY _: Int, endY _: Int) {}
    }
}

private final class RemoteSwiftTermView: SwiftTerm.TerminalView {
    var onFocus: (() -> Void)?
    private var appliedTheme: DeviceThemeEventDTO?
    private var hasAppliedTheme = false

    override func mouseDown(with event: NSEvent) {
        onFocus?()
        super.mouseDown(with: event)
    }

    func apply(theme: DeviceThemeEventDTO?) {
        if hasAppliedTheme,
           appliedTheme?.fg == theme?.fg,
           appliedTheme?.bg == theme?.bg,
           appliedTheme?.palette == theme?.palette
        {
            return
        }
        let foreground = theme?.fg ?? 0xFFFFFF
        let background = theme?.bg ?? 0x000000
        let terminal = getTerminal()
        setForegroundColor(source: terminal, color: Self.swiftTermColor(foreground))
        setBackgroundColor(source: terminal, color: Self.swiftTermColor(background))
        if let palette = theme?.palette, palette.count == 16 {
            installColors(palette.map(Self.swiftTermColor))
        }
        caretColor = Self.nsColor(foreground)
        appliedTheme = theme
        hasAppliedTheme = true
    }

    private static func swiftTermColor(_ rgb: UInt32) -> SwiftTerm.Color {
        SwiftTerm.Color(
            red: UInt16((rgb >> 16) & 0xFF) * 0x0101,
            green: UInt16((rgb >> 8) & 0xFF) * 0x0101,
            blue: UInt16(rgb & 0xFF) * 0x0101
        )
    }

    private static func nsColor(_ rgb: UInt32) -> NSColor {
        NSColor(
            red: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}
