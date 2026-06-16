import AppKit
import SwiftUI

struct ExtensionConsentOverlay: View {
    private let service = ExtensionConsentService.shared

    var body: some View {
        ExtensionConsentSheetHost(pending: service.pendingPrompt) { request, choice in
            service.respond(requestID: request.id, choice: choice)
        }
        .frame(width: 0, height: 0)
        .allowsHitTesting(false)
    }
}

private struct ExtensionConsentSheetHost: NSViewRepresentable {
    let pending: ExtensionConsentRequest?
    let onChoice: (ExtensionConsentRequest, ExtensionConsentChoice) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onChoice: onChoice)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.attach(host: view)
        return view
    }

    func updateNSView(_: NSView, context: Context) {
        context.coordinator.update(pending: pending, onChoice: onChoice)
    }

    static func dismantleNSView(_: NSView, coordinator: Coordinator) {
        coordinator.dismiss()
    }

    @MainActor
    final class Coordinator {
        private weak var host: NSView?
        private var presentedRequestID: UUID?
        private var sheet: ExtensionConsentSheetWindow?
        private var onChoice: (ExtensionConsentRequest, ExtensionConsentChoice) -> Void

        init(onChoice: @escaping (ExtensionConsentRequest, ExtensionConsentChoice) -> Void) {
            self.onChoice = onChoice
        }

        func attach(host: NSView) {
            self.host = host
        }

        func update(
            pending: ExtensionConsentRequest?,
            onChoice: @escaping (ExtensionConsentRequest, ExtensionConsentChoice) -> Void
        ) {
            self.onChoice = onChoice

            if let pending {
                if presentedRequestID == pending.id {
                    sheet?.updateRequest(pending)
                    return
                }
                if presentedRequestID != nil {
                    dismiss()
                }
                present(request: pending)
                return
            }
            dismiss()
        }

        func dismiss() {
            guard let sheet else { return }
            let parent = sheet.sheetParent
            parent?.endSheet(sheet)
            self.sheet = nil
            presentedRequestID = nil
        }

        private func present(request: ExtensionConsentRequest) {
            guard let window = parentWindow() else { return }
            let sheet = ExtensionConsentSheetWindow(request: request) { [weak self] choice in
                guard let self else { return }
                let request = self.sheet?.currentRequest ?? request
                self.dismiss()
                self.onChoice(request, choice)
            }
            self.sheet = sheet
            presentedRequestID = request.id
            window.beginSheet(sheet)
        }

        private func parentWindow() -> NSWindow? {
            if let host, let window = host.window {
                return window
            }
            return NSApp.windows.first { $0.identifier == ShortcutContext.mainWindowIdentifier }
        }
    }
}

private final class ExtensionConsentSheetWindow: NSPanel {
    private(set) var currentRequest: ExtensionConsentRequest
    private let onChoice: (ExtensionConsentChoice) -> Void
    private let hostingView: NSHostingView<ExtensionConsentDialog>

    init(
        request: ExtensionConsentRequest,
        onChoice: @escaping (ExtensionConsentChoice) -> Void
    ) {
        currentRequest = request
        self.onChoice = onChoice
        let dialog = ExtensionConsentDialog(request: request, onChoice: onChoice)
        let host = NSHostingView(rootView: dialog)
        host.translatesAutoresizingMaskIntoConstraints = true
        hostingView = host

        let intrinsic = host.fittingSize
        let width: CGFloat = max(520, intrinsic.width)
        let height: CGFloat = intrinsic.height
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: width, height: height),
            styleMask: [.titled, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        titlebarAppearsTransparent = true
        titleVisibility = .hidden
        isMovable = false
        isMovableByWindowBackground = false
        hasShadow = true
        isOpaque = false
        backgroundColor = .clear
        contentView = host
        host.frame = NSRect(x: 0, y: 0, width: width, height: height)
    }

    func updateRequest(_ request: ExtensionConsentRequest) {
        currentRequest = request
        hostingView.rootView = ExtensionConsentDialog(request: request, onChoice: onChoice)
        let fitting = hostingView.fittingSize
        let newSize = NSSize(width: max(520, fitting.width), height: fitting.height)
        setContentSize(newSize)
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    override func cancelOperation(_: Any?) {
        onChoice(.denyOnce)
    }
}

struct ExtensionConsentDialog: View {
    let request: ExtensionConsentRequest
    let onChoice: (ExtensionConsentChoice) -> Void

    @State private var blockKind = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            header
            payloadBox
            blockToggle
            buttons
        }
        .padding(20)
        .frame(maxWidth: 520, alignment: .leading)
        .background(MuxyTheme.bg)
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    private var header: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: iconName)
                .font(.system(size: 28, weight: .light))
                .foregroundStyle(MuxyTheme.accent)
            VStack(alignment: .leading, spacing: 4) {
                Text("Allow \(request.extensionDisplayName)?")
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Text(verbDescription)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            Spacer(minLength: 0)
        }
    }

    private var payloadBox: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(request.payloadDetails.enumerated()), id: \.offset) { _, detail in
                Text(detail)
                    .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fg)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(12)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
    }

    private var blockToggle: some View {
        VStack(alignment: .leading, spacing: 6) {
            Toggle(isOn: $blockKind) {
                Text("Block all \(request.verb.kindDisplayName) from this extension")
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fg)
            }
            .toggleStyle(.checkbox)
            HStack(spacing: 6) {
                Image(systemName: "info.circle")
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Text("\"Remember\" saves rule: ")
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    + Text(rememberRuleDescription)
                    .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer()
            }
        }
    }

    private var rememberRuleDescription: String {
        blockKind ? "all \(request.verb.kindDisplayName)" : request.suggestedMatch.displayString
    }

    private var buttons: some View {
        HStack(spacing: 8) {
            Button("Deny & remember") { onChoice(blockKind ? .blockKind : .denyAndRemember) }
                .buttonStyle(.bordered)
                .tint(blockKind ? MuxyTheme.diffRemoveFg : nil)
            Spacer()
            Button("Cancel") { onChoice(.denyOnce) }
                .keyboardShortcut(.escape, modifiers: [])
                .buttonStyle(.bordered)
            Button("Allow") { onChoice(.allowOnce) }
                .buttonStyle(.bordered)
                .disabled(blockKind)
            Button("Allow & remember") { onChoice(.allowAndRemember) }
                .keyboardShortcut(.return, modifiers: [])
                .buttonStyle(.borderedProminent)
                .disabled(blockKind)
        }
    }

    private var iconName: String {
        switch request.verb {
        case .exec: "terminal.fill"
        case .panesSend,
             .panesSendKeys: "keyboard.fill"
        case .panesReadScreen: "eye.fill"
        case .tabsOpenForeign: "rectangle.stack.fill"
        case .tabsRunCommand: "terminal.fill"
        case .remoteInvoke: "antenna.radiowaves.left.and.right"
        case .gitWrite: "arrow.triangle.branch"
        case .filesWrite: "doc.fill"
        case .httpFetch: "globe"
        case .projectsDelete: "trash.fill"
        }
    }

    private var verbDescription: String {
        switch request.verb {
        case .exec: "wants to run a shell command"
        case .panesSend: "wants to type into a terminal"
        case .panesSendKeys: "wants to press keys in a terminal"
        case .panesReadScreen: "wants to read terminal output"
        case .tabsOpenForeign: "wants to open another extension's tab"
        case .tabsRunCommand: "wants to open a terminal that runs a command"
        case .remoteInvoke: "wants to serve a mobile request"
        case .gitWrite: "wants to modify the git repository"
        case .filesWrite: "wants to modify workspace files"
        case .httpFetch: "wants to make a network request"
        case .projectsDelete: "wants to delete a project"
        }
    }
}
