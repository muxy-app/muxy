import CoreText
import MuxyShared
import SwiftUI
import UIKit

enum TerminalCursorStyle: String, CaseIterable, Identifiable {
    case block
    case bar
    case underline

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .block: "Block"
        case .bar: "Bar"
        case .underline: "Underline"
        }
    }

    static var current: TerminalCursorStyle {
        get {
            guard let raw = UserDefaults.standard.string(forKey: "terminalCursorStyle"),
                  let style = TerminalCursorStyle(rawValue: raw)
            else { return .block }
            return style
        }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: "terminalCursorStyle") }
    }
}

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
    @State private var pendingGridSize: (cols: UInt32, rows: UInt32)?

    private var themeBg: Color {
        connection.deviceTheme?.bgColor ?? .black
    }

    private var isOwnedBySelf: Bool {
        connection.paneIsOwnedBySelf(paneID)
    }

    var body: some View {
        ZStack {
            terminalGrid
                .opacity(isOwnedBySelf ? 1 : 0)
                .allowsHitTesting(isOwnedBySelf)

            if !isOwnedBySelf {
                MobileTakeOverOverlay(
                    ownerName: ownerDisplayName,
                    theme: connection.deviceTheme,
                    takeOver: takeOverCurrentPane
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(themeBg)
            }
        }
        .background(themeBg)
        .onAppear {
            bindInput()
            startPolling()
            if isOwnedBySelf {
                Task { @MainActor in
                    inputCoordinator.becomeFirstResponder()
                }
            }
        }
        .onDisappear {
            stopPolling()
            Task { await connection.releasePane(paneID: paneID) }
        }
        .onChange(of: paneID) { _, _ in
            cells = nil
            stopPolling()
            bindInput()
            startPolling()
        }
        .onChange(of: isOwnedBySelf) { _, newValue in
            if newValue {
                inputCoordinator.becomeFirstResponder()
            }
        }
        .onChange(of: connection.activeProjectID) { _, newValue in
            if newValue == nil {
                stopPolling()
            }
        }
    }

    private var ownerDisplayName: String {
        if case let .mac(name) = connection.paneOwner(for: paneID) { return name }
        if case let .remote(_, name) = connection.paneOwner(for: paneID) { return name }
        return "Mac"
    }

    private func takeOverCurrentPane() {
        let size = pendingGridSize ?? (cols: 80, rows: 24)
        Task {
            await connection.takeOverPane(paneID: paneID, cols: size.cols, rows: size.rows)
            if let dto = await connection.getTerminalCells(paneID: paneID) {
                cells = dto
            }
        }
    }

    private var terminalGrid: some View {
        ZStack(alignment: .bottom) {
            TerminalGridRepresentable(
                cells: cells,
                paneID: paneID,
                onResize: { cols, rows in
                    pendingGridSize = (cols, rows)
                    guard isOwnedBySelf else { return }
                    Task { await connection.resizeTerminal(paneID: paneID, cols: cols, rows: rows) }
                },
                onScroll: { lines in
                    guard isOwnedBySelf else { return }
                    Task {
                        await connection.scrollTerminal(paneID: paneID, deltaX: 0, deltaY: lines, precise: false)
                        if let dto = await connection.getTerminalCells(paneID: paneID) {
                            cells = dto
                        }
                    }
                }
            )

            TerminalInputField(coordinator: inputCoordinator, theme: connection.deviceTheme)
                .frame(height: 1)
                .opacity(0.01)
        }
        .contentShape(Rectangle())
        .onTapGesture {
            guard isOwnedBySelf else { return }
            inputCoordinator.becomeFirstResponder()
        }
    }

    private func bindInput() {
        inputCoordinator.onSend = { text in
            Task { await connection.sendTerminalInput(paneID: paneID, text: text) }
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

struct MobileTakeOverOverlay: View {
    let ownerName: String
    let theme: ConnectionManager.DeviceTheme?
    let takeOver: () -> Void

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "desktopcomputer")
                .font(.system(size: 28))
                .foregroundStyle(accentColor)
            Text("Controlled on \(ownerName)")
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(primaryColor)
            Text("This terminal is currently being used on \(ownerName). Take over to control it from here.")
                .font(.system(size: 13))
                .foregroundStyle(secondaryColor)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 280)
            Button(action: takeOver) {
                Text("Take Over")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(buttonForeground)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 10)
                    .background(
                        RoundedRectangle(cornerRadius: 12)
                            .fill(accentColor)
                    )
            }
            .buttonStyle(.plain)
        }
        .padding(24)
        .frame(maxWidth: 340)
        .background(
            RoundedRectangle(cornerRadius: 20)
                .fill(panelBackground)
                .overlay(
                    RoundedRectangle(cornerRadius: 20)
                        .strokeBorder(accentColor.opacity(0.2), lineWidth: 1)
                )
        )
        .padding(.horizontal, 24)
    }

    private var accentColor: Color {
        theme?.fgColor ?? .white
    }

    private var primaryColor: Color {
        theme?.fgColor ?? .white
    }

    private var secondaryColor: Color {
        (theme?.fgColor ?? .white).opacity(0.7)
    }

    private var buttonForeground: Color {
        theme?.bgColor ?? .black
    }

    private var panelBackground: Color {
        (theme?.fgColor ?? .white).opacity(0.08)
    }
}

struct TerminalGridRepresentable: UIViewRepresentable {
    let cells: TerminalCellsDTO?
    let paneID: UUID
    let onResize: (UInt32, UInt32) -> Void
    let onScroll: (Double) -> Void

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
    var onScroll: ((Double) -> Void)?

    private var cells: TerminalCellsDTO?
    private let fontSize: CGFloat = 12
    private var advanceWidth: CGFloat = 0
    private var rowHeight: CGFloat = 0
    private var lastReportedCols: UInt32 = 0
    private var lastReportedRows: UInt32 = 0
    private var lastPanTranslation: CGPoint = .zero
    private var scrollAccumulator: CGFloat = 0
    private var cursorBlinkOn: Bool = true
    private var cursorBlinkTimer: Timer?

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
        startCursorBlink()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil {
            cursorBlinkTimer?.invalidate()
            cursorBlinkTimer = nil
        } else if cursorBlinkTimer == nil {
            startCursorBlink()
        }
    }

    private func startCursorBlink() {
        cursorBlinkTimer?.invalidate()
        cursorBlinkTimer = Timer.scheduledTimer(withTimeInterval: 0.55, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.cursorBlinkOn.toggle()
                self.setNeedsDisplay()
            }
        }
    }

    private func resetCursorBlink() {
        cursorBlinkOn = true
        startCursorBlink()
    }

    @objc
    private func handlePan(_ gesture: UIPanGestureRecognizer) {
        switch gesture.state {
        case .began:
            lastPanTranslation = .zero
            scrollAccumulator = 0
        case .changed:
            let translation = gesture.translation(in: self)
            let dy = translation.y - lastPanTranslation.y
            lastPanTranslation = translation
            guard rowHeight > 0 else { return }
            scrollAccumulator += dy
            let lines = (scrollAccumulator / rowHeight).rounded(.towardZero)
            guard lines != 0 else { return }
            scrollAccumulator -= lines * rowHeight
            onScroll?(Double(lines))
        case .ended,
             .cancelled,
             .failed:
            lastPanTranslation = .zero
            scrollAccumulator = 0
        default:
            break
        }
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func update(cells: TerminalCellsDTO?) {
        let previousCursor = self.cells.map { ($0.cursorX, $0.cursorY) }
        self.cells = cells
        if let newCells = cells {
            let newCursor = (newCells.cursorX, newCells.cursorY)
            if previousCursor == nil || previousCursor! != newCursor {
                resetCursorBlink()
            }
        }
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
        let cursorStyle = TerminalCursorStyle.current
        let cursorActive = cursorVisible && cursorBlinkOn

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

                let onCursor = cursorActive && cursorStyle == .block && row == cursorY && col == cursorX
                if onCursor {
                    let tmp = bgColor
                    bgColor = fgColor
                    fgColor = tmp
                }

                ctx.setFillColor(bgColor.cgColor)
                ctx.fill(cellRect)

                if flags & TerminalCellFlag.invisible != 0 { continue }
                if cell.codepoint == 0 || cell.codepoint == 0x20 { continue }

                if drawBlockGlyph(
                    codepoint: cell.codepoint,
                    in: cellRect,
                    color: fgColor,
                    ctx: ctx
                ) {
                    continue
                }

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

        if cursorActive,
           cursorStyle != .block,
           cursorY < rows,
           cursorX < cols
        {
            let cursorCell = cells.cells[cursorY * cols + cursorX]
            let cursorWidth = advanceWidth * ((cursorCell.flags & TerminalCellFlag.wide) != 0 ? 2 : 1)
            let cursorCellRect = CGRect(
                x: CGFloat(cursorX) * advanceWidth,
                y: bounds.height - CGFloat(cursorY + 1) * rowHeight,
                width: cursorWidth,
                height: rowHeight
            )
            let cursorColor = color(rgb: cells.defaultFg)
            ctx.setFillColor(cursorColor.cgColor)
            switch cursorStyle {
            case .bar:
                ctx.fill(CGRect(
                    x: cursorCellRect.minX,
                    y: cursorCellRect.minY,
                    width: max(1.5, advanceWidth * 0.12),
                    height: cursorCellRect.height
                ))
            case .underline:
                let thickness = max(1.5, rowHeight * 0.1)
                ctx.fill(CGRect(
                    x: cursorCellRect.minX,
                    y: cursorCellRect.minY,
                    width: cursorCellRect.width,
                    height: thickness
                ))
            case .block:
                break
            }
        }
    }

    private func color(rgb: UInt32) -> UIColor {
        let r = CGFloat((rgb >> 16) & 0xFF) / 255.0
        let g = CGFloat((rgb >> 8) & 0xFF) / 255.0
        let b = CGFloat(rgb & 0xFF) / 255.0
        return UIColor(red: r, green: g, blue: b, alpha: 1.0)
    }

    private func drawBlockGlyph(
        codepoint: UInt32,
        in rect: CGRect,
        color: UIColor,
        ctx: CGContext
    ) -> Bool {
        guard (0x2580 ... 0x259F).contains(codepoint) else { return false }

        ctx.setFillColor(color.cgColor)
        let x = rect.minX, y = rect.minY, w = rect.width, h = rect.height
        let half = h / 2, halfW = w / 2, quarter = h / 4, threeQuarter = h * 3 / 4
        let oneEighth = h / 8

        switch codepoint {
        case 0x2580: ctx.fill(CGRect(x: x, y: y + half, width: w, height: h - half))
        case 0x2581: ctx.fill(CGRect(x: x, y: y, width: w, height: oneEighth))
        case 0x2582: ctx.fill(CGRect(x: x, y: y, width: w, height: h / 4))
        case 0x2583: ctx.fill(CGRect(x: x, y: y, width: w, height: h * 3 / 8))
        case 0x2584: ctx.fill(CGRect(x: x, y: y, width: w, height: half))
        case 0x2585: ctx.fill(CGRect(x: x, y: y, width: w, height: h * 5 / 8))
        case 0x2586: ctx.fill(CGRect(x: x, y: y, width: w, height: threeQuarter))
        case 0x2587: ctx.fill(CGRect(x: x, y: y, width: w, height: h * 7 / 8))
        case 0x2588: ctx.fill(rect)
        case 0x2589: ctx.fill(CGRect(x: x, y: y, width: w * 7 / 8, height: h))
        case 0x258A: ctx.fill(CGRect(x: x, y: y, width: threeQuarter, height: h))
        case 0x258B: ctx.fill(CGRect(x: x, y: y, width: w * 5 / 8, height: h))
        case 0x258C: ctx.fill(CGRect(x: x, y: y, width: halfW, height: h))
        case 0x258D: ctx.fill(CGRect(x: x, y: y, width: w * 3 / 8, height: h))
        case 0x258E: ctx.fill(CGRect(x: x, y: y, width: quarter, height: h))
        case 0x258F: ctx.fill(CGRect(x: x, y: y, width: w / 8, height: h))
        case 0x2590: ctx.fill(CGRect(x: x + halfW, y: y, width: w - halfW, height: h))
        case 0x2591:
            ctx.setAlpha(0.25)
            ctx.fill(rect)
            ctx.setAlpha(1.0)
        case 0x2592:
            ctx.setAlpha(0.5)
            ctx.fill(rect)
            ctx.setAlpha(1.0)
        case 0x2593:
            ctx.setAlpha(0.75)
            ctx.fill(rect)
            ctx.setAlpha(1.0)
        case 0x2594: ctx.fill(CGRect(x: x, y: y + h - oneEighth, width: w, height: oneEighth))
        case 0x2595: ctx.fill(CGRect(x: x + w - w / 8, y: y, width: w / 8, height: h))
        case 0x2596: ctx.fill(CGRect(x: x, y: y, width: halfW, height: half))
        case 0x2597: ctx.fill(CGRect(x: x + halfW, y: y, width: w - halfW, height: half))
        case 0x2598: ctx.fill(CGRect(x: x, y: y + half, width: halfW, height: h - half))
        case 0x2599:
            ctx.fill(CGRect(x: x, y: y, width: w, height: half))
            ctx.fill(CGRect(x: x, y: y + half, width: halfW, height: h - half))
        case 0x259A:
            ctx.fill(CGRect(x: x, y: y + half, width: halfW, height: h - half))
            ctx.fill(CGRect(x: x + halfW, y: y, width: w - halfW, height: half))
        case 0x259B:
            ctx.fill(CGRect(x: x, y: y + half, width: w, height: h - half))
            ctx.fill(CGRect(x: x, y: y, width: halfW, height: half))
        case 0x259C:
            ctx.fill(CGRect(x: x, y: y + half, width: w, height: h - half))
            ctx.fill(CGRect(x: x + halfW, y: y, width: w - halfW, height: half))
        case 0x259D: ctx.fill(CGRect(x: x + halfW, y: y + half, width: w - halfW, height: h - half))
        case 0x259E:
            ctx.fill(CGRect(x: x + halfW, y: y + half, width: w - halfW, height: h - half))
            ctx.fill(CGRect(x: x, y: y, width: halfW, height: half))
        case 0x259F:
            ctx.fill(CGRect(x: x + halfW, y: y + half, width: w - halfW, height: h - half))
            ctx.fill(CGRect(x: x, y: y, width: w, height: half))
        default: return false
        }
        return true
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
    let theme: ConnectionManager.DeviceTheme?

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
        return field
    }

    func updateUIView(_ uiView: TerminalUITextField, context _: Context) {
        uiView.applyTheme(theme)
    }
}

enum TerminalModifier: String, CaseIterable, Identifiable {
    case ctrl
    case shift
    case alt
    case cmd

    var id: String { rawValue }

    var title: String { rawValue }

    var displayName: String {
        switch self {
        case .ctrl: "Control"
        case .shift: "Shift"
        case .alt: "Option"
        case .cmd: "Command"
        }
    }

    var glyph: String {
        switch self {
        case .ctrl: "⌃"
        case .shift: "⇧"
        case .alt: "⌥"
        case .cmd: "⌘"
        }
    }
}

final class TerminalUITextField: UIView, UIKeyInput, UITextInputTraits {
    var onInsert: ((String) -> Void)?
    var onDelete: (() -> Void)?
    var onAccessoryKey: ((String) -> Void)?

    private var modifierArmed = false
    private var activeModifier: TerminalModifier = .ctrl

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
        bar.onModifierToggle = { [weak self] armed in self?.modifierArmed = armed }
        bar.onModifierChange = { [weak self] modifier in self?.activeModifier = modifier }
        bar.onKeyboardToggle = { [weak self] in self?.toggleKeyboard() }
        return bar
    }()

    private let hiddenKeyboardPlaceholder: UIView = {
        let v = UIView()
        v.frame = CGRect(x: 0, y: 0, width: 0, height: 0)
        return v
    }()

    private var keyboardHidden = false

    override var inputView: UIView? {
        keyboardHidden ? hiddenKeyboardPlaceholder : nil
    }

    private func toggleKeyboard() {
        keyboardHidden.toggle()
        accessoryBar.setKeyboardVisible(!keyboardHidden)
        reloadInputViews()
        if !isFirstResponder { becomeFirstResponder() }
    }

    override var inputAccessoryView: UIView? { accessoryBar }

    func applyTheme(_ theme: ConnectionManager.DeviceTheme?) {
        accessoryBar.applyTheme(theme)
    }

    func insertText(_ text: String) {
        if modifierArmed, let mapped = Self.transform(text, with: activeModifier) {
            modifierArmed = false
            accessoryBar.setModifierArmed(false)
            if mapped.isEmpty {
                onInsert?(text)
            } else {
                onAccessoryKey?(mapped)
            }
            return
        }
        onInsert?(text)
    }

    func deleteBackward() {
        onDelete?()
    }

    private static func transform(_ text: String, with modifier: TerminalModifier) -> String? {
        switch modifier {
        case .ctrl:
            ctrlTransform(text)
        case .shift:
            text.uppercased()
        case .alt:
            "\u{1B}" + text
        case .cmd:
            text
        }
    }

    private static func ctrlTransform(_ text: String) -> String? {
        guard text.count == 1, let scalar = text.unicodeScalars.first else { return nil }
        let value = scalar.value
        switch value {
        case 0x40 ... 0x5F:
            return String(UnicodeScalar(value - 0x40)!)
        case 0x61 ... 0x7A:
            return String(UnicodeScalar(value - 0x60)!)
        case 0x20:
            return "\u{00}"
        default:
            return nil
        }
    }
}

@MainActor
final class TerminalAccessoryModel: ObservableObject {
    @Published var theme: ConnectionManager.DeviceTheme?
    @Published var modifierArmed: Bool = false
    @Published var activeModifier: TerminalModifier = .ctrl
    @Published var keyboardVisible: Bool = true

    var onKey: ((String) -> Void)?
    var onModifierToggle: ((Bool) -> Void)?
    var onModifierChange: ((TerminalModifier) -> Void)?
    var onKeyboardToggle: (() -> Void)?

    func setModifierArmed(_ armed: Bool) {
        guard modifierArmed != armed else { return }
        modifierArmed = armed
        onModifierToggle?(armed)
    }

    func toggleModifier() {
        setModifierArmed(!modifierArmed)
    }

    func selectModifier(_ modifier: TerminalModifier) {
        guard activeModifier != modifier else { return }
        activeModifier = modifier
        onModifierChange?(modifier)
        if modifierArmed {
            setModifierArmed(false)
        }
    }
}

final class TerminalAccessoryBar: UIInputView {
    var onKey: ((String) -> Void)? {
        get { model.onKey }
        set { model.onKey = newValue }
    }

    var onModifierToggle: ((Bool) -> Void)? {
        get { model.onModifierToggle }
        set { model.onModifierToggle = newValue }
    }

    var onModifierChange: ((TerminalModifier) -> Void)? {
        get { model.onModifierChange }
        set { model.onModifierChange = newValue }
    }

    var onKeyboardToggle: (() -> Void)? {
        get { model.onKeyboardToggle }
        set { model.onKeyboardToggle = newValue }
    }

    func setKeyboardVisible(_ visible: Bool) {
        model.keyboardVisible = visible
    }

    private let model = TerminalAccessoryModel()
    private let hostingController: UIHostingController<TerminalAccessoryView>

    init() {
        hostingController = UIHostingController(rootView: TerminalAccessoryView(model: model))
        super.init(
            frame: CGRect(x: 0, y: 0, width: 0, height: 72),
            inputViewStyle: .keyboard
        )
        autoresizingMask = [.flexibleWidth]
        allowsSelfSizing = true
        setupHostingView()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupHostingView() {
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false
        hostingController.view.backgroundColor = .clear
        hostingController.sizingOptions = .preferredContentSize
        addSubview(hostingController.view)
        NSLayoutConstraint.activate([
            hostingController.view.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostingController.view.topAnchor.constraint(equalTo: topAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: bottomAnchor),
            heightAnchor.constraint(equalToConstant: 72),
        ])
    }

    func applyTheme(_ theme: ConnectionManager.DeviceTheme?) {
        model.theme = theme
        overrideUserInterfaceStyle = (theme?.isDark ?? true) ? .dark : .light
    }

    func setModifierArmed(_ armed: Bool) {
        model.setModifierArmed(armed)
    }
}

struct TerminalAccessoryView: View {
    @ObservedObject var model: TerminalAccessoryModel

    private var fg: Color { model.theme?.fgColor ?? .white }
    private var accent: Color { model.theme?.fgColor ?? .white }

    var body: some View {
        HStack(spacing: 10) {
            keyPill
            Spacer(minLength: 6)
            keyboardButton
            DPadControl(tint: fg) { payload in
                model.onKey?(payload)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }

    private var keyPill: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                key("esc", payload: "\u{1B}")
                modifierKey
                key("tab", payload: "\t")
                key("~", payload: "~")
                key("|", payload: "|")
                key("/", payload: "/")
                key("-", payload: "-")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
        }
        .frame(height: 44)
        .glassEffect(.regular, in: Capsule())
    }

    private func key(_ title: String, payload: String) -> some View {
        Button {
            model.onKey?(payload)
        } label: {
            Text(title)
                .font(.system(size: 14, weight: .medium, design: .monospaced))
                .foregroundStyle(fg)
                .frame(minWidth: 32)
                .padding(.vertical, 4)
        }
        .buttonStyle(.plain)
    }

    private var modifierKey: some View {
        Menu {
            Picker("Modifier", selection: modifierSelection) {
                ForEach(TerminalModifier.allCases) { modifier in
                    Text("\(modifier.glyph)  \(modifier.displayName)")
                        .tag(modifier)
                }
            }
        } label: {
            modifierLabel
        } primaryAction: {
            model.toggleModifier()
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
    }

    private var modifierSelection: Binding<TerminalModifier> {
        Binding(
            get: { model.activeModifier },
            set: { model.selectModifier($0) }
        )
    }

    private var modifierLabel: some View {
        HStack(spacing: 4) {
            Text(model.activeModifier.title)
                .font(.system(size: 14, weight: .medium, design: .monospaced))
            Text(model.activeModifier.glyph)
                .font(.system(size: 11, weight: .semibold))
                .opacity(0.7)
        }
        .foregroundStyle(model.modifierArmed ? (model.theme?.bgColor ?? .black) : fg)
        .frame(minWidth: 52)
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background {
            if model.modifierArmed {
                Capsule().fill(fg)
            }
        }
    }

    private var keyboardButton: some View {
        Button {
            model.onKeyboardToggle?()
        } label: {
            Image(systemName: model.keyboardVisible ? "keyboard.chevron.compact.down" : "keyboard")
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(fg)
                .frame(width: 44, height: 44)
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.interactive(), in: Circle())
    }
}

struct DPadControl: View {
    let tint: Color
    let onDirection: (String) -> Void

    private let outerSize: CGFloat = 44
    private let thumbSize: CGFloat = 18
    private let deadZone: CGFloat = 5

    @State private var thumbOffset: CGSize = .zero
    @State private var activeDirection: Direction?
    @State private var repeatTask: Task<Void, Never>?

    private enum Direction {
        case up
        case down
        case left
        case right

        var payload: String {
            switch self {
            case .up: "\u{1B}[A"
            case .down: "\u{1B}[B"
            case .left: "\u{1B}[D"
            case .right: "\u{1B}[C"
            }
        }

        var unit: CGSize {
            switch self {
            case .up: .init(width: 0, height: -1)
            case .down: .init(width: 0, height: 1)
            case .left: .init(width: -1, height: 0)
            case .right: .init(width: 1, height: 0)
            }
        }
    }

    var body: some View {
        ZStack {
            Circle()
                .fill(Color.black.opacity(0.35))
            Circle()
                .fill(tint.opacity(0.55))
                .frame(width: thumbSize, height: thumbSize)
                .offset(thumbOffset)
                .animation(.interactiveSpring(response: 0.18, dampingFraction: 0.8), value: thumbOffset)
        }
        .frame(width: outerSize, height: outerSize)
        .contentShape(Circle())
        .glassEffect(.regular.interactive(), in: Circle())
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    handleDrag(translation: value.translation)
                }
                .onEnded { _ in
                    resetThumb()
                    stopRepeating()
                }
        )
    }

    private func handleDrag(translation: CGSize) {
        let dx = translation.width
        let dy = translation.height
        let magnitude = hypot(dx, dy)
        guard magnitude > deadZone else {
            if activeDirection != nil {
                stopRepeating()
                activeDirection = nil
            }
            thumbOffset = .zero
            return
        }
        let direction: Direction = abs(dx) > abs(dy)
            ? (dx > 0 ? .right : .left)
            : (dy > 0 ? .down : .up)

        let maxReach = (outerSize - thumbSize) / 2 - 2
        thumbOffset = CGSize(
            width: direction.unit.width * maxReach,
            height: direction.unit.height * maxReach
        )

        guard direction != activeDirection else { return }
        activeDirection = direction
        startRepeating(direction: direction)
    }

    private func resetThumb() {
        activeDirection = nil
        thumbOffset = .zero
    }

    private func startRepeating(direction: Direction) {
        stopRepeating()
        onDirection(direction.payload)
        repeatTask = Task {
            try? await Task.sleep(for: .milliseconds(300))
            while !Task.isCancelled {
                onDirection(direction.payload)
                try? await Task.sleep(for: .milliseconds(60))
            }
        }
    }

    private func stopRepeating() {
        repeatTask?.cancel()
        repeatTask = nil
    }
}
