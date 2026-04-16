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

    private var themeBg: Color {
        connection.terminalTheme?.bgColor ?? .black
    }

    var body: some View {
        terminalGrid
            .background(themeBg)
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
            TerminalGridRepresentable(
                cells: cells,
                paneID: paneID,
                onResize: { cols, rows in
                    Task { await connection.resizeTerminal(paneID: paneID, cols: cols, rows: rows) }
                },
                onScroll: { dx, dy in
                    Task { await connection.scrollTerminal(paneID: paneID, deltaX: dx, deltaY: dy, precise: true) }
                }
            )

            TerminalInputField(coordinator: inputCoordinator, theme: connection.terminalTheme)
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
    let onScroll: (Double, Double) -> Void

    func makeUIView(context _: Context) -> TerminalGridView {
        let view = TerminalGridView(frame: .zero)
        view.onResize = onResize
        view.onScroll = onScroll
        return view
    }

    func updateUIView(_ uiView: TerminalGridView, context _: Context) {
        uiView.onResize = onResize
        uiView.onScroll = onScroll
        uiView.update(cells: cells)
    }
}

final class TerminalGridView: UIView {
    var onResize: ((UInt32, UInt32) -> Void)?
    var onScroll: ((Double, Double) -> Void)?

    private var cells: TerminalCellsDTO?
    private let fontSize: CGFloat = 12
    private var advanceWidth: CGFloat = 0
    private var rowHeight: CGFloat = 0
    private var lastReportedCols: UInt32 = 0
    private var lastReportedRows: UInt32 = 0
    private var lastPanTranslation: CGPoint = .zero

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        contentMode = .redraw
        isOpaque = true
        recomputeMetrics()
        let pan = UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:)))
        pan.minimumNumberOfTouches = 1
        pan.maximumNumberOfTouches = 2
        addGestureRecognizer(pan)
    }

    @objc
    private func handlePan(_ gesture: UIPanGestureRecognizer) {
        switch gesture.state {
        case .began:
            lastPanTranslation = .zero
        case .changed:
            let translation = gesture.translation(in: self)
            let dx = translation.x - lastPanTranslation.x
            let dy = translation.y - lastPanTranslation.y
            lastPanTranslation = translation
            guard abs(dx) > 0.5 || abs(dy) > 0.5 else { return }
            onScroll?(Double(dx), Double(dy))
        case .ended,
             .cancelled,
             .failed:
            lastPanTranslation = .zero
        default:
            break
        }
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
    let theme: ConnectionManager.TerminalTheme?

    func makeUIView(context _: Context) -> TerminalUITextField {
        let field = TerminalUITextField(frame: .zero)
        field.onInsert = { [weak coordinator] text in
            coordinator?.send(text)
        }
        field.onDelete = { [weak coordinator] in
            coordinator?.send("\u{7F}")
        }
        field.onAccessoryKey = { [weak coordinator] text in
            coordinator?.send(text)
        }
        field.applyTheme(theme)
        coordinator.textField = field
        DispatchQueue.main.async {
            field.becomeFirstResponder()
        }
        return field
    }

    func updateUIView(_ uiView: TerminalUITextField, context _: Context) {
        uiView.applyTheme(theme)
    }
}

final class TerminalUITextField: UIView, UIKeyInput, UITextInputTraits {
    var onInsert: ((String) -> Void)?
    var onDelete: (() -> Void)?
    var onAccessoryKey: ((String) -> Void)?

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

    private lazy var accessoryBar: TerminalAccessoryBar = {
        let bar = TerminalAccessoryBar()
        bar.onKey = { [weak self] text in self?.onAccessoryKey?(text) }
        return bar
    }()

    override var inputAccessoryView: UIView? { accessoryBar }

    func applyTheme(_ theme: ConnectionManager.TerminalTheme?) {
        accessoryBar.applyTheme(theme)
    }

    func insertText(_ text: String) {
        onInsert?(text)
    }

    func deleteBackward() {
        onDelete?()
    }
}

final class TerminalAccessoryBar: UIInputView {
    var onKey: ((String) -> Void)?

    private let scrollView = UIScrollView()
    private let stack = UIStackView()
    private var themeButtons: [UIButton] = []
    private var currentTheme: ConnectionManager.TerminalTheme?

    init() {
        super.init(
            frame: CGRect(x: 0, y: 0, width: UIScreen.main.bounds.width, height: 44),
            inputViewStyle: .keyboard
        )
        autoresizingMask = [.flexibleWidth]
        allowsSelfSizing = true
        setupSubviews()
        populateKeys()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupSubviews() {
        scrollView.showsHorizontalScrollIndicator = false
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        scrollView.alwaysBounceHorizontal = true
        addSubview(scrollView)

        stack.axis = .horizontal
        stack.spacing = 6
        stack.alignment = .center
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.layoutMargins = UIEdgeInsets(top: 0, left: 10, bottom: 0, right: 10)
        stack.isLayoutMarginsRelativeArrangement = true
        scrollView.addSubview(stack)

        NSLayoutConstraint.activate([
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrollView.topAnchor.constraint(equalTo: topAnchor),
            scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),
            heightAnchor.constraint(equalToConstant: 44),

            stack.leadingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scrollView.contentLayoutGuide.topAnchor),
            stack.bottomAnchor.constraint(equalTo: scrollView.contentLayoutGuide.bottomAnchor),
            stack.heightAnchor.constraint(equalTo: scrollView.frameLayoutGuide.heightAnchor),
        ])
    }

    private func populateKeys() {
        addKey(title: "esc", payload: "\u{1B}")
        addKey(title: "tab", payload: "\t")
        addKey(title: "ctrl+c", payload: "\u{03}")
        addKey(title: "ctrl+d", payload: "\u{04}")
        addKey(title: "ctrl+l", payload: "\u{0C}")
        addKey(title: "-", payload: "-")
        addKey(title: "/", payload: "/")
        addKey(title: "|", payload: "|")
        addKey(systemImage: "chevron.left", payload: "\u{1B}[D")
        addKey(systemImage: "chevron.up", payload: "\u{1B}[A")
        addKey(systemImage: "chevron.down", payload: "\u{1B}[B")
        addKey(systemImage: "chevron.right", payload: "\u{1B}[C")
    }

    private func addKey(title: String? = nil, systemImage: String? = nil, payload: String) {
        let button = UIButton(type: .system)
        var config = UIButton.Configuration.gray()
        config.cornerStyle = .medium
        config.baseBackgroundColor = keyBackgroundColor
        config.baseForegroundColor = keyForegroundColor
        config.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 10, bottom: 6, trailing: 10)
        if let title {
            var attr = AttributedString(title)
            attr.font = UIFont.monospacedSystemFont(ofSize: 13, weight: .medium)
            config.attributedTitle = attr
        } else if let systemImage {
            config.image = UIImage(systemName: systemImage)?
                .withConfiguration(UIImage.SymbolConfiguration(pointSize: 14, weight: .semibold))
        }
        button.configuration = config
        button.accessibilityLabel = title ?? systemImage
        button.addAction(
            UIAction { [weak self] _ in self?.onKey?(payload) },
            for: .touchUpInside
        )
        themeButtons.append(button)
        stack.addArrangedSubview(button)
    }

    func applyTheme(_ theme: ConnectionManager.TerminalTheme?) {
        currentTheme = theme
        let isDark = theme?.isDark ?? true
        overrideUserInterfaceStyle = isDark ? .dark : .light
        for button in themeButtons {
            var config = button.configuration
            config?.baseBackgroundColor = keyBackgroundColor
            config?.baseForegroundColor = keyForegroundColor
            button.configuration = config
        }
    }

    private var keyBackgroundColor: UIColor {
        guard let theme = currentTheme else {
            return UIColor(white: 0.3, alpha: 1.0)
        }
        let base = uiColor(rgb: theme.fg)
        return base.withAlphaComponent(theme.isDark ? 0.14 : 0.1)
    }

    private var keyForegroundColor: UIColor {
        guard let theme = currentTheme else { return .white }
        return uiColor(rgb: theme.fg)
    }

    private func uiColor(rgb: UInt32) -> UIColor {
        UIColor(
            red: CGFloat((rgb >> 16) & 0xFF) / 255.0,
            green: CGFloat((rgb >> 8) & 0xFF) / 255.0,
            blue: CGFloat(rgb & 0xFF) / 255.0,
            alpha: 1.0
        )
    }
}
