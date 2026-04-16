import CoreText
import MuxyShared
import SwiftUI
import UIKit

enum TerminalFont {
    static let nerdFontName = "JetBrainsMonoNFM-Regular"
    static let nerdFontBoldName = "JetBrainsMonoNFM-Bold"
    static let defaultSize: CGFloat = 12

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

    static func regular(size: CGFloat) -> UIFont {
        if useNerdFont, let font = UIFont(name: nerdFontName, size: size) { return font }
        return UIFont.monospacedSystemFont(ofSize: size, weight: .regular)
    }

    static func bold(size: CGFloat) -> UIFont {
        if useNerdFont, let font = UIFont(name: nerdFontBoldName, size: size) { return font }
        return UIFont.monospacedSystemFont(ofSize: size, weight: .bold)
    }

    static var current: Font {
        let size = fontSize
        if useNerdFont, UIFont(name: nerdFontName, size: size) != nil {
            return .custom(nerdFontName, size: size)
        }
        return .system(size: size, design: .monospaced)
    }
}

struct TerminalView: View {
    let paneID: UUID
    @Environment(ConnectionManager.self) private var connection
    @State private var cells: TerminalCellsDTO?
    @State private var pollTask: Task<Void, Never>?
    @State private var inputCoordinator = TerminalInputCoordinator()

    var body: some View {
        VStack(spacing: 0) {
            terminalGrid
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
            cells = nil
            stopPolling()
            startPolling()
        }
    }

    private var terminalGrid: some View {
        ZStack(alignment: .bottom) {
            TerminalGridRepresentable(cells: cells, paneID: paneID) { cols, rows in
                Task { await connection.resizeTerminal(paneID: paneID, cols: cols, rows: rows) }
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
                if let dto = await connection.getTerminalCells(paneID: paneID) {
                    cells = dto
                }
                try? await Task.sleep(for: .milliseconds(100))
            }
        }
    }

    private func stopPolling() {
        pollTask?.cancel()
        pollTask = nil
    }
}

struct TerminalGridRepresentable: UIViewRepresentable {
    let cells: TerminalCellsDTO?
    let paneID: UUID
    let onResize: (UInt32, UInt32) -> Void

    func makeUIView(context _: Context) -> TerminalGridView {
        let view = TerminalGridView(frame: .zero)
        view.onResize = onResize
        return view
    }

    func updateUIView(_ uiView: TerminalGridView, context _: Context) {
        uiView.onResize = onResize
        uiView.update(cells: cells)
    }
}

final class TerminalGridView: UIView {
    var onResize: ((UInt32, UInt32) -> Void)?

    private var cells: TerminalCellsDTO?
    private let fontSize: CGFloat = 12
    private var advanceWidth: CGFloat = 0
    private var rowHeight: CGFloat = 0
    private var lastReportedCols: UInt32 = 0
    private var lastReportedRows: UInt32 = 0

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        contentMode = .redraw
        isOpaque = true
        recomputeMetrics()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func update(cells: TerminalCellsDTO?) {
        self.cells = cells
        setNeedsDisplay()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        reportGridSize()
        setNeedsDisplay()
    }

    private func recomputeMetrics() {
        let font = TerminalFont.regular(size: fontSize)
        advanceWidth = ceil(("M" as NSString).size(withAttributes: [.font: font]).width)
        rowHeight = ceil(font.ascender - font.descender + font.leading)
    }

    private func reportGridSize() {
        guard advanceWidth > 0, rowHeight > 0 else { return }
        let cols = max(UInt32(floor(bounds.width / advanceWidth)), 20)
        let rows = max(UInt32(floor(bounds.height / rowHeight)), 5)
        guard cols != lastReportedCols || rows != lastReportedRows else { return }
        lastReportedCols = cols
        lastReportedRows = rows
        onResize?(cols, rows)
    }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }

        let defaultBg: UIColor = {
            if let cell = cells?.cells.first {
                return color(rgb: cell.bg)
            }
            return .black
        }()
        defaultBg.setFill()
        UIRectFill(rect)

        guard let cells else { return }

        let cols = Int(cells.cols)
        let rows = Int(cells.rows)
        guard cols > 0, rows > 0 else { return }

        let regular = TerminalFont.regular(size: fontSize)
        let bold = TerminalFont.bold(size: fontSize)

        ctx.textMatrix = .identity
        ctx.translateBy(x: 0, y: bounds.height)
        ctx.scaleBy(x: 1, y: -1)

        let cursorVisible = cells.cursorVisible
        let cursorX = Int(cells.cursorX)
        let cursorY = Int(cells.cursorY)

        for row in 0 ..< rows {
            for col in 0 ..< cols {
                let cell = cells.cells[row * cols + col]
                let flags = cell.flags
                if flags & TerminalCellFlag.spacer != 0 { continue }

                let width = advanceWidth * ((flags & TerminalCellFlag.wide) != 0 ? 2 : 1)
                let cellRect = CGRect(
                    x: CGFloat(col) * advanceWidth,
                    y: bounds.height - CGFloat(row + 1) * rowHeight,
                    width: width,
                    height: rowHeight
                )

                var bgColor = color(rgb: cell.bg)
                var fgColor = color(rgb: cell.fg)

                let onCursor = cursorVisible && row == cursorY && col == cursorX
                if onCursor {
                    let tmp = bgColor
                    bgColor = fgColor
                    fgColor = tmp
                }

                ctx.setFillColor(bgColor.cgColor)
                ctx.fill(cellRect)

                if flags & TerminalCellFlag.invisible != 0 { continue }
                if cell.codepoint == 0 || cell.codepoint == 0x20 { continue }

                guard let scalar = Unicode.Scalar(cell.codepoint) else { continue }
                let glyphString = String(Character(scalar))

                let baseFont: UIFont = (flags & TerminalCellFlag.bold != 0) ? bold : regular
                var drawColor = fgColor
                if flags & TerminalCellFlag.faint != 0 {
                    drawColor = drawColor.withAlphaComponent(0.65)
                }

                var attrs: [NSAttributedString.Key: Any] = [
                    .font: baseFont,
                    .foregroundColor: drawColor,
                ]
                if flags & TerminalCellFlag.italic != 0,
                   let descriptor = baseFont.fontDescriptor.withSymbolicTraits(.traitItalic)
                {
                    attrs[.font] = UIFont(descriptor: descriptor, size: baseFont.pointSize)
                }
                if flags & TerminalCellFlag.underline != 0 {
                    attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue
                    attrs[.underlineColor] = drawColor
                }
                if flags & TerminalCellFlag.strike != 0 {
                    attrs[.strikethroughStyle] = NSUnderlineStyle.single.rawValue
                    attrs[.strikethroughColor] = drawColor
                }

                let attributed = NSAttributedString(string: glyphString, attributes: attrs)
                let line = CTLineCreateWithAttributedString(attributed)
                ctx.textPosition = CGPoint(
                    x: cellRect.minX,
                    y: cellRect.minY - baseFont.descender - baseFont.leading / 2
                )
                CTLineDraw(line, ctx)
            }
        }
    }

    private func color(rgb: UInt32) -> UIColor {
        let r = CGFloat((rgb >> 16) & 0xFF) / 255.0
        let g = CGFloat((rgb >> 8) & 0xFF) / 255.0
        let b = CGFloat(rgb & 0xFF) / 255.0
        return UIColor(red: r, green: g, blue: b, alpha: 1.0)
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

    func makeUIView(context _: Context) -> TerminalUITextField {
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

    func updateUIView(_: TerminalUITextField, context _: Context) {}
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
