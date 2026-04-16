import MuxyShared
import SwiftUI
import UIKit

enum TerminalFont {
    private static let nerdFontName = "JetBrainsMonoNFM-Regular"
    private static let defaultSize: CGFloat = 12

    static var current: Font {
        let size = UserDefaults.standard.object(forKey: "terminalFontSize") as? CGFloat ?? defaultSize
        if UserDefaults.standard.bool(forKey: "useNerdFont"),
           UIFont(name: nerdFontName, size: size) != nil
        {
            return .custom(nerdFontName, size: size)
        }
        return .system(size: size, design: .monospaced)
    }

    static var fontSize: CGFloat {
        get { UserDefaults.standard.object(forKey: "terminalFontSize") as? CGFloat ?? defaultSize }
        set { UserDefaults.standard.set(newValue, forKey: "terminalFontSize") }
    }

    static var useNerdFont: Bool {
        get {
            if UserDefaults.standard.object(forKey: "useNerdFont") == nil { return true }
            return UserDefaults.standard.bool(forKey: "useNerdFont")
        }
        set { UserDefaults.standard.set(newValue, forKey: "useNerdFont") }
    }
}

struct TerminalView: View {
    let paneID: UUID
    @Environment(ConnectionManager.self) private var connection
    @State private var content: String = ""
    @State private var cols: UInt32 = 80
    @State private var rows: UInt32 = 24
    @State private var pollTask: Task<Void, Never>?
    @State private var inputCoordinator = TerminalInputCoordinator()

    var body: some View {
        VStack(spacing: 0) {
            terminalOutput
            KeyboardAccessoryBar(paneID: paneID, coordinator: inputCoordinator)
        }
        .background(Color.black)
        .ignoresSafeArea(.keyboard, edges: .bottom)
        .onAppear {
            inputCoordinator.onSend = { text in
                Task { await connection.sendTerminalInput(paneID: paneID, text: text) }
            }
            startPolling()
        }
        .onDisappear {
            stopPolling()
        }
        .onChange(of: paneID) { _, _ in
            content = ""
            stopPolling()
            startPolling()
        }
    }

    private var terminalOutput: some View {
        ZStack(alignment: .bottom) {
            ScrollViewReader { proxy in
                ScrollView {
                    Text(content.isEmpty ? "Connecting..." : content)
                        .font(TerminalFont.current)
                        .foregroundStyle(.white)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                        .padding(8)
                        .id("terminal-bottom")
                }
                .scrollIndicators(.hidden)
                .defaultScrollAnchor(.bottom)
                .onChange(of: content) { _, _ in
                    withAnimation(.easeOut(duration: 0.1)) {
                        proxy.scrollTo("terminal-bottom", anchor: .bottom)
                    }
                }
            }

            TerminalInputField(coordinator: inputCoordinator)
                .frame(height: 1)
                .opacity(0.01)
        }
        .contentShape(Rectangle())
        .onTapGesture {
            inputCoordinator.becomeFirstResponder()
        }
    }

    private func startPolling() {
        pollTask = Task {
            while !Task.isCancelled {
                if let dto = await connection.getTerminalContent(paneID: paneID) {
                    content = dto.content
                    cols = dto.cols
                    rows = dto.rows
                }
                try? await Task.sleep(for: .milliseconds(150))
            }
        }
    }

    private func stopPolling() {
        pollTask?.cancel()
        pollTask = nil
    }
}

@MainActor
final class TerminalInputCoordinator {
    var onSend: ((String) -> Void)?
    weak var textField: TerminalUITextField?

    func send(_ text: String) {
        onSend?(text)
    }

    func becomeFirstResponder() {
        textField?.becomeFirstResponder()
    }
}

struct TerminalInputField: UIViewRepresentable {
    let coordinator: TerminalInputCoordinator

    func makeUIView(context: Context) -> TerminalUITextField {
        let field = TerminalUITextField(frame: .zero)
        field.onInsert = { [weak coordinator] text in
            coordinator?.send(text)
        }
        field.onDelete = { [weak coordinator] in
            coordinator?.send("\u{7F}")
        }
        coordinator.textField = field
        DispatchQueue.main.async {
            field.becomeFirstResponder()
        }
        return field
    }

    func updateUIView(_ uiView: TerminalUITextField, context: Context) {}
}

final class TerminalUITextField: UIView, UIKeyInput, UITextInputTraits {
    var onInsert: ((String) -> Void)?
    var onDelete: (() -> Void)?

    var autocapitalizationType: UITextAutocapitalizationType = .none
    var autocorrectionType: UITextAutocorrectionType = .no
    var spellCheckingType: UITextSpellCheckingType = .no
    var smartDashesType: UITextSmartDashesType = .no
    var smartQuotesType: UITextSmartQuotesType = .no
    var smartInsertDeleteType: UITextSmartInsertDeleteType = .no
    var keyboardType: UIKeyboardType = .asciiCapable
    var returnKeyType: UIReturnKeyType = .default
    var enablesReturnKeyAutomatically: Bool = false

    var hasText: Bool { true }

    override var canBecomeFirstResponder: Bool { true }

    func insertText(_ text: String) {
        onInsert?(text)
    }

    func deleteBackward() {
        onDelete?()
    }
}

struct KeyboardAccessoryBar: View {
    let paneID: UUID
    let coordinator: TerminalInputCoordinator

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                keyButton("esc") { coordinator.send("\u{1B}") }
                keyButton("tab") { coordinator.send("\t") }
                keyButton("ctrl+c") { coordinator.send("\u{03}") }
                keyButton("ctrl+d") { coordinator.send("\u{04}") }
                keyButton("ctrl+l") { coordinator.send("\u{0C}") }
                keyButton("-") { coordinator.send("-") }
                keyButton("/") { coordinator.send("/") }
                keyButton("|") { coordinator.send("|") }
                Spacer(minLength: 8)
                arrowKey("chevron.left") { coordinator.send("\u{1B}[D") }
                arrowKey("chevron.up") { coordinator.send("\u{1B}[A") }
                arrowKey("chevron.down") { coordinator.send("\u{1B}[B") }
                arrowKey("chevron.right") { coordinator.send("\u{1B}[C") }
            }
            .padding(.horizontal, 8)
        }
        .frame(height: 44)
        .background(Color(white: 0.12))
    }

    private func keyButton(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 13, weight: .medium, design: .monospaced))
                .foregroundStyle(.white)
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(Color(white: 0.22), in: RoundedRectangle(cornerRadius: 6))
        }
    }

    private func arrowKey(_ systemName: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 36, height: 30)
                .background(Color(white: 0.22), in: RoundedRectangle(cornerRadius: 6))
        }
    }
}
