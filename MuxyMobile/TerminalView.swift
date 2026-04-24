import MuxyShared
import SwiftTerm
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
}

struct TerminalView: View {
    let paneID: UUID
    @Environment(ConnectionManager.self) private var connection
    @State private var autoTakenPaneID: UUID?
    @State private var takeOverInFlight = false
    @State private var reportedCols: UInt32?
    @State private var reportedRows: UInt32?

    private var themeBg: SwiftUI.Color {
        connection.deviceTheme?.bgColor ?? .black
    }

    private var isOwnedBySelf: Bool {
        connection.paneIsOwnedBySelf(paneID)
    }

    var body: some View {
        ZStack {
            SwiftTermRepresentable(paneID: paneID) { cols, rows in
                reportedCols = cols
                reportedRows = rows
            }
            .opacity(isOwnedBySelf ? 1 : 0)
            .allowsHitTesting(isOwnedBySelf)

            if !isOwnedBySelf, !takeOverInFlight {
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
        .onAppear { attemptAutoTakeOver() }
        .onDisappear {
            Task { await connection.releasePane(paneID: paneID) }
        }
        .onChange(of: paneID) { _, _ in
            takeOverInFlight = false
            autoTakenPaneID = nil
            attemptAutoTakeOver()
        }
        .onChange(of: reportedCols) { _, _ in attemptAutoTakeOver() }
        .onChange(of: reportedRows) { _, _ in attemptAutoTakeOver() }
    }

    private var ownerDisplayName: String {
        if case let .mac(name) = connection.paneOwner(for: paneID) { return name }
        if case let .remote(_, name) = connection.paneOwner(for: paneID) { return name }
        return "Mac"
    }

    private func takeOverCurrentPane() {
        guard let cols = reportedCols, let rows = reportedRows else { return }
        takeOverInFlight = true
        Task {
            await connection.takeOverPane(paneID: paneID, cols: cols, rows: rows)
            takeOverInFlight = false
        }
    }

    private func attemptAutoTakeOver() {
        guard let cols = reportedCols, let rows = reportedRows else { return }
        guard autoTakenPaneID != paneID else { return }
        autoTakenPaneID = paneID
        takeOverInFlight = true
        Task {
            await connection.takeOverPane(paneID: paneID, cols: cols, rows: rows)
            takeOverInFlight = false
        }
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

    private var accentColor: SwiftUI.Color { theme?.fgColor ?? .white }
    private var primaryColor: SwiftUI.Color { theme?.fgColor ?? .white }
    private var secondaryColor: SwiftUI.Color { (theme?.fgColor ?? .white).opacity(0.7) }
    private var buttonForeground: SwiftUI.Color { theme?.bgColor ?? .black }
    private var panelBackground: SwiftUI.Color { (theme?.fgColor ?? .white).opacity(0.08) }
}

private struct SwiftTermRepresentable: UIViewRepresentable {
    let paneID: UUID
    let onSize: (UInt32, UInt32) -> Void
    @Environment(ConnectionManager.self) private var connection

    func makeUIView(context: Context) -> MuxySwiftTermView {
        let view = MuxySwiftTermView(frame: .zero, font: TerminalFont.regular(size: TerminalFont.fontSize))
        view.paneID = paneID
        view.connection = connection
        view.terminalDelegate = context.coordinator
        view.backspaceSendsControlH = false
        view.allowMouseReporting = false
        applyTheme(to: view)
        context.coordinator.bind(view: view, paneID: paneID, connection: connection, onSize: onSize)
        subscribe(view: view, paneID: paneID)
        return view
    }

    func updateUIView(_ uiView: MuxySwiftTermView, context: Context) {
        if let previousPaneID = uiView.paneID, previousPaneID != paneID {
            context.coordinator.unbind()
            connection.unsubscribeTerminalBytes(paneID: previousPaneID)
            Task { await connection.releasePane(paneID: previousPaneID) }
            uiView.getTerminal().resetToInitialState()
            uiView.paneID = paneID
            context.coordinator.bind(view: uiView, paneID: paneID, connection: connection, onSize: onSize)
            subscribe(view: uiView, paneID: paneID)
        } else {
            context.coordinator.updateOnSize(onSize)
        }
        applyTheme(to: uiView)
    }

    static func dismantleUIView(_ uiView: MuxySwiftTermView, coordinator: Coordinator) {
        if let paneID = uiView.paneID {
            coordinator.connection?.unsubscribeTerminalBytes(paneID: paneID)
        }
        coordinator.unbind()
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    private func subscribe(view: MuxySwiftTermView, paneID: UUID) {
        connection.subscribeTerminalBytes(paneID: paneID) { [weak view] data in
            guard let view else { return }
            let bytes = [UInt8](data)
            view.feedPreservingScroll(byteArray: bytes[...])
        }
    }

    private func applyTheme(to view: MuxySwiftTermView) {
        view.applyMuxyTheme(connection.deviceTheme)
    }

    @MainActor
    final class Coordinator: NSObject, TerminalViewDelegate {
        weak var view: MuxySwiftTermView?
        weak var connection: ConnectionManager?
        var paneID: UUID?
        private var onSize: ((UInt32, UInt32) -> Void)?
        private var lastReportedCols: Int = 0
        private var lastReportedRows: Int = 0
        private var isReady: Bool = false

        func bind(view: MuxySwiftTermView, paneID: UUID, connection: ConnectionManager, onSize: @escaping (UInt32, UInt32) -> Void) {
            self.view = view
            self.paneID = paneID
            self.connection = connection
            self.onSize = onSize
            lastReportedCols = 0
            lastReportedRows = 0
            isReady = true
        }

        func updateOnSize(_ onSize: @escaping (UInt32, UInt32) -> Void) {
            self.onSize = onSize
        }

        func unbind() {
            isReady = false
            view = nil
            paneID = nil
            connection = nil
            onSize = nil
        }

        nonisolated func send(source _: SwiftTerm.TerminalView, data: ArraySlice<UInt8>) {
            MainActor.assumeIsolated {
                guard let paneID, let connection, let view else { return }
                let bytes = view.accessoryTransformedBytes(data)
                guard !bytes.isEmpty else { return }
                connection.sendTerminalInput(paneID: paneID, bytes: bytes)
            }
        }

        nonisolated func sizeChanged(source _: SwiftTerm.TerminalView, newCols: Int, newRows: Int) {
            MainActor.assumeIsolated {
                guard isReady else { return }
                guard newCols > 0, newRows > 0 else { return }
                if newCols == lastReportedCols, newRows == lastReportedRows { return }
                lastReportedCols = newCols
                lastReportedRows = newRows
                let cols = UInt32(newCols)
                let rows = UInt32(newRows)
                onSize?(cols, rows)
                guard let paneID, let connection else { return }
                Task { await connection.resizeTerminal(paneID: paneID, cols: cols, rows: rows) }
            }
        }

        nonisolated func setTerminalTitle(source _: SwiftTerm.TerminalView, title _: String) {}
        nonisolated func hostCurrentDirectoryUpdate(source _: SwiftTerm.TerminalView, directory _: String?) {}
        nonisolated func scrolled(source _: SwiftTerm.TerminalView, position _: Double) {}
        nonisolated func requestOpenLink(source _: SwiftTerm.TerminalView, link _: String, params _: [String: String]) {}
        nonisolated func rangeChanged(source _: SwiftTerm.TerminalView, startY _: Int, endY _: Int) {}
        nonisolated func clipboardCopy(source _: SwiftTerm.TerminalView, content: Data) {
            MainActor.assumeIsolated {
                if let text = String(data: content, encoding: .utf8) {
                    UIPasteboard.general.string = text
                }
            }
        }
    }
}

final class MuxySwiftTermView: SwiftTerm.TerminalView {
    var paneID: UUID?
    weak var connection: ConnectionManager?

    private let muxyAccessoryBar: TerminalAccessoryBar = .init()

    private var keyboardHidden = false
    private var wheelAccumulatedDelta: CGFloat = 0
    private static let wheelPointsPerTick: CGFloat = 16
    private static let wheelMaxTicksPerFrame: Int = 2

    private var userDetachedFromBottom = false
    private static let bottomStickThreshold: CGFloat = 2

    private let hiddenKeyboardPlaceholder: UIView = {
        let view = UIView(frame: .zero)
        view.isHidden = true
        return view
    }()

    override init(frame: CGRect, font: UIFont?) {
        super.init(frame: frame, font: font)
        muxyAccessoryBar.onKey = { [weak self] text in self?.sendAccessoryKey(text) }
        muxyAccessoryBar.onPaste = { [weak self] in self?.pasteFromClipboard() }
        muxyAccessoryBar.onCopy = { [weak self] in self?.copySelectionToClipboard() }
        muxyAccessoryBar.onKeyboardToggle = { [weak self] in self?.toggleKeyboard() }
        inputAccessoryView = muxyAccessoryBar
        setupWheelGesture()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var contentOffset: CGPoint {
        didSet {
            if isTracking || isDragging || isDecelerating {
                updateUserDetachedFromBottom()
            }
        }
    }

    func feedPreservingScroll(byteArray: ArraySlice<UInt8>) {
        preserveDetachedScrollPosition {
            feed(byteArray: byteArray)
        }
    }

    func applyAccessoryTheme(_ theme: ConnectionManager.DeviceTheme?) {
        muxyAccessoryBar.applyTheme(theme)
    }

    private var lastAppliedFg: UInt32?
    private var lastAppliedBg: UInt32?
    private var lastAppliedPalette: [UInt32]?

    func applyMuxyTheme(_ theme: ConnectionManager.DeviceTheme?) {
        let fgRGB = theme?.fg ?? 0xFFFFFF
        let bgRGB = theme?.bg ?? 0x000000
        if fgRGB != lastAppliedFg || bgRGB != lastAppliedBg {
            lastAppliedFg = fgRGB
            lastAppliedBg = bgRGB
            let terminal = getTerminal()
            setForegroundColor(source: terminal, color: Self.swiftTermColor(fgRGB))
            setBackgroundColor(source: terminal, color: Self.swiftTermColor(bgRGB))
        }
        if let palette = theme?.palette, palette.count == 16, palette != lastAppliedPalette {
            lastAppliedPalette = palette
            installColors(palette.map(Self.swiftTermColor))
        }
        caretColor = UIColor(theme?.fgColor ?? .white)
        overrideUserInterfaceStyle = (theme?.isDark ?? true) ? .dark : .light
        applyAccessoryTheme(theme)
    }

    private static func swiftTermColor(_ rgb: UInt32) -> SwiftTerm.Color {
        SwiftTerm.Color(
            red: UInt16((rgb >> 16) & 0xFF) * 0x0101,
            green: UInt16((rgb >> 8) & 0xFF) * 0x0101,
            blue: UInt16(rgb & 0xFF) * 0x0101
        )
    }

    var modifierIsArmed: Bool { muxyAccessoryBar.modifierArmed }
    var activeAccessoryModifier: TerminalModifier { muxyAccessoryBar.activeModifier }

    func clearArmedModifier() {
        muxyAccessoryBar.setModifierArmed(false)
    }

    func accessoryTransformedBytes(_ slice: ArraySlice<UInt8>) -> Data {
        if modifierIsArmed,
           let text = String(bytes: slice, encoding: .utf8),
           let transformed = Self.transform(text, with: activeAccessoryModifier)
        {
            clearArmedModifier()
            return Data(transformed.utf8)
        }
        return Data(slice)
    }

    private func sendAccessoryKey(_ text: String) {
        sendBytes(Data(text.utf8))
    }

    private func sendBytes(_ bytes: Data) {
        guard !bytes.isEmpty, let paneID, let connection else { return }
        connection.sendTerminalInput(paneID: paneID, bytes: bytes)
    }

    private func pasteFromClipboard() {
        guard let text = UIPasteboard.general.string, !text.isEmpty else { return }
        sendBytes(Data(text.utf8))
    }

    private func copySelectionToClipboard() {
        guard let text = getSelection(), !text.isEmpty else { return }
        UIPasteboard.general.string = text
    }

    private func toggleKeyboard() {
        keyboardHidden.toggle()
        muxyAccessoryBar.setKeyboardVisible(!keyboardHidden)
        inputView = keyboardHidden ? hiddenKeyboardPlaceholder : nil
        if !isFirstResponder { _ = becomeFirstResponder() }
        reloadInputViews()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        guard window != nil else { return }
        _ = becomeFirstResponder()
    }

    private func setupWheelGesture() {
        let gesture = UIPanGestureRecognizer(target: self, action: #selector(handleWheelPan(_:)))
        gesture.minimumNumberOfTouches = 1
        gesture.maximumNumberOfTouches = 1
        gesture.delegate = wheelGestureDelegate
        addGestureRecognizer(gesture)
    }

    private func preserveDetachedScrollPosition(_ update: () -> Void) {
        let wasDetached = userDetachedFromBottom
        let preservedOffset = contentOffset
        update()
        guard wasDetached else {
            updateUserDetachedFromBottom()
            return
        }
        let maxOffsetY = max(0, contentSize.height - bounds.height)
        let restoredOffset = CGPoint(x: preservedOffset.x, y: min(preservedOffset.y, maxOffsetY))
        if contentOffset != restoredOffset {
            setContentOffset(restoredOffset, animated: false)
        }
        updateUserDetachedFromBottom()
    }

    private func updateUserDetachedFromBottom() {
        let maxOffsetY = max(0, contentSize.height - bounds.height)
        let distanceFromBottom = maxOffsetY - contentOffset.y
        userDetachedFromBottom = distanceFromBottom > Self.bottomStickThreshold
    }

    private lazy var wheelGestureDelegate: WheelGestureDelegate = {
        let d = WheelGestureDelegate()
        d.shouldFire = { [weak self] in
            self?.getTerminal().mouseMode != .off
        }
        return d
    }()

    @objc
    private func handleWheelPan(_ gesture: UIPanGestureRecognizer) {
        let terminal = getTerminal()
        guard terminal.mouseMode != .off else { return }

        switch gesture.state {
        case .began:
            wheelAccumulatedDelta = 0
            gesture.setTranslation(.zero, in: self)
        case .changed:
            let translation = gesture.translation(in: self)
            gesture.setTranslation(.zero, in: self)
            wheelAccumulatedDelta += translation.y
            let baseTicks = Int((wheelAccumulatedDelta / Self.wheelPointsPerTick).rounded(.towardZero))
            guard baseTicks != 0 else { return }
            wheelAccumulatedDelta -= CGFloat(baseTicks) * Self.wheelPointsPerTick
            let clamped = max(-Self.wheelMaxTicksPerFrame, min(Self.wheelMaxTicksPerFrame, baseTicks))
            guard clamped != 0 else { return }
            emitWheelTicks(clamped, terminal: terminal, location: gesture.location(in: self))
        case .ended,
             .cancelled,
             .failed:
            wheelAccumulatedDelta = 0
        default:
            break
        }
    }

    private func emitWheelTicks(_ ticks: Int, terminal: Terminal, location _: CGPoint) {
        let col = max(0, terminal.cols / 2)
        let row = max(0, terminal.rows / 2)
        let button = ticks > 0 ? 4 : 5
        let count = abs(ticks)
        let encoded = terminal.encodeButton(button: button, release: false, shift: false, meta: false, control: false)
        for _ in 0 ..< count {
            terminal.sendEvent(buttonFlags: encoded, x: col, y: row)
        }
    }

    private static func transform(_ text: String, with modifier: TerminalModifier) -> String? {
        switch modifier {
        case .ctrl: ctrlTransform(text)
        case .shift: text.uppercased()
        case .alt: "\u{1B}" + text
        case .cmd: text
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

private final class WheelGestureDelegate: NSObject, UIGestureRecognizerDelegate {
    var shouldFire: (() -> Bool)?

    func gestureRecognizerShouldBegin(_: UIGestureRecognizer) -> Bool {
        shouldFire?() ?? false
    }

    func gestureRecognizer(_: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith _: UIGestureRecognizer) -> Bool {
        true
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

@MainActor
final class TerminalAccessoryModel: ObservableObject {
    @Published var theme: ConnectionManager.DeviceTheme?
    @Published var modifierArmed: Bool = false
    @Published var activeModifier: TerminalModifier = .ctrl
    @Published var keyboardVisible: Bool = true
    @Published var canCopySelection: Bool = false

    var onKey: ((String) -> Void)?
    var onModifierToggle: ((Bool) -> Void)?
    var onModifierChange: ((TerminalModifier) -> Void)?
    var onKeyboardToggle: (() -> Void)?
    var onPaste: (() -> Void)?
    var onCopy: (() -> Void)?

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
        if modifierArmed { setModifierArmed(false) }
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

    var onPaste: (() -> Void)? {
        get { model.onPaste }
        set { model.onPaste = newValue }
    }

    var onCopy: (() -> Void)? {
        get { model.onCopy }
        set { model.onCopy = newValue }
    }

    var modifierArmed: Bool { model.modifierArmed }
    var activeModifier: TerminalModifier { model.activeModifier }
    var canCopySelection: Bool { model.canCopySelection }

    func setKeyboardVisible(_ visible: Bool) {
        model.keyboardVisible = visible
    }

    func setCanCopySelection(_ enabled: Bool) {
        model.canCopySelection = enabled
    }

    func setModifierArmed(_ armed: Bool) {
        model.setModifierArmed(armed)
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
}

struct TerminalAccessoryView: View {
    @ObservedObject var model: TerminalAccessoryModel

    private var fg: SwiftUI.Color { model.theme?.fgColor ?? .white }

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
                actionIcon("doc.on.clipboard", label: "Paste", action: { model.onPaste?() })
                actionIcon("doc.on.doc", label: "Copy", enabled: model.canCopySelection, action: { model.onCopy?() })
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

    private func actionIcon(
        _ systemName: String,
        label: String,
        enabled: Bool = true,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(fg)
                .frame(width: 32, height: 28)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .opacity(enabled ? 1 : 0.45)
        .accessibilityLabel(label)
    }

    private var modifierKey: some View {
        ModifierKeyButton(
            active: model.activeModifier,
            armed: model.modifierArmed,
            fg: fg,
            bg: model.theme?.bgColor ?? .black,
            onTap: { model.toggleModifier() },
            onSelect: { model.selectModifier($0) }
        )
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

struct ModifierKeyButton: UIViewRepresentable {
    let active: TerminalModifier
    let armed: Bool
    let fg: SwiftUI.Color
    let bg: SwiftUI.Color
    let onTap: () -> Void
    let onSelect: (TerminalModifier) -> Void

    func makeUIView(context _: Context) -> ModifierKeyHostView {
        let view = ModifierKeyHostView()
        view.configure(
            active: active,
            armed: armed,
            fg: UIColor(fg),
            bg: UIColor(bg),
            onTap: onTap,
            onSelect: onSelect
        )
        return view
    }

    func updateUIView(_ uiView: ModifierKeyHostView, context _: Context) {
        uiView.configure(
            active: active,
            armed: armed,
            fg: UIColor(fg),
            bg: UIColor(bg),
            onTap: onTap,
            onSelect: onSelect
        )
    }
}

final class ModifierKeyHostView: UIView {
    private let label = UILabel()
    private let chevron = UIImageView()
    private let stack = UIStackView()
    private let background = UIView()

    private var activeModifier: TerminalModifier = .ctrl
    private var armed: Bool = false
    private var fgColor: UIColor = .white
    private var bgColor: UIColor = .black
    private var onTap: (() -> Void)?
    private var onSelect: ((TerminalModifier) -> Void)?

    private var pickerView: ModifierPickerView?
    private var didCommitSelection = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        setupViews()
        setupGestures()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupViews() {
        setContentHuggingPriority(.required, for: .horizontal)
        setContentHuggingPriority(.required, for: .vertical)
        setContentCompressionResistancePriority(.required, for: .horizontal)
        setContentCompressionResistancePriority(.required, for: .vertical)

        background.translatesAutoresizingMaskIntoConstraints = false
        background.isUserInteractionEnabled = false
        background.layer.cornerCurve = .continuous
        addSubview(background)

        label.font = UIFont.monospacedSystemFont(ofSize: 14, weight: .medium)
        chevron.image = UIImage(systemName: "chevron.up", withConfiguration: UIImage.SymbolConfiguration(pointSize: 9, weight: .semibold))
        chevron.contentMode = .scaleAspectFit
        chevron.alpha = 0.6

        stack.axis = .horizontal
        stack.spacing = 4
        stack.alignment = .center
        stack.isUserInteractionEnabled = false
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.addArrangedSubview(label)
        stack.addArrangedSubview(chevron)
        addSubview(stack)

        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: centerYAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            background.topAnchor.constraint(equalTo: topAnchor),
            background.bottomAnchor.constraint(equalTo: bottomAnchor),
            background.leadingAnchor.constraint(equalTo: leadingAnchor),
            background.trailingAnchor.constraint(equalTo: trailingAnchor),
        ])
    }

    override var intrinsicContentSize: CGSize {
        let stackSize = stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        let width = max(stackSize.width + 16, 52)
        let height = max(stackSize.height + 8, 32)
        return CGSize(width: width, height: height)
    }

    private func setupGestures() {
        let longPress = UILongPressGestureRecognizer(target: self, action: #selector(handleLongPress(_:)))
        longPress.minimumPressDuration = 0.3
        longPress.allowableMovement = .greatestFiniteMagnitude
        addGestureRecognizer(longPress)

        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTap))
        tap.require(toFail: longPress)
        addGestureRecognizer(tap)
    }

    func configure(
        active: TerminalModifier,
        armed: Bool,
        fg: UIColor,
        bg: UIColor,
        onTap: @escaping () -> Void,
        onSelect: @escaping (TerminalModifier) -> Void
    ) {
        activeModifier = active
        self.armed = armed
        fgColor = fg
        bgColor = bg
        self.onTap = onTap
        self.onSelect = onSelect
        refreshAppearance()
    }

    private func refreshAppearance() {
        label.text = activeModifier.title
        let textColor = armed ? bgColor : fgColor
        label.textColor = textColor
        chevron.tintColor = textColor
        background.backgroundColor = armed ? fgColor : .clear
        background.layer.cornerRadius = background.bounds.height / 2
        invalidateIntrinsicContentSize()
        setNeedsLayout()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        background.layer.cornerRadius = background.bounds.height / 2
    }

    @objc
    private func handleTap() {
        onTap?()
    }

    @objc
    private func handleLongPress(_ gesture: UILongPressGestureRecognizer) {
        switch gesture.state {
        case .began:
            presentPicker()
        case .changed:
            guard let pickerView else { return }
            let location = gesture.location(in: pickerView)
            pickerView.updateHover(at: location)
        case .ended:
            commitSelectionIfNeeded()
            dismissPicker()
        case .cancelled,
             .failed:
            dismissPicker()
        default:
            break
        }
    }

    private func presentPicker() {
        guard pickerView == nil,
              let window
        else { return }

        didCommitSelection = false
        let picker = ModifierPickerView(active: activeModifier, fg: fgColor)
        picker.translatesAutoresizingMaskIntoConstraints = false
        window.addSubview(picker)

        let buttonFrame = convert(bounds, to: window)
        let pickerSize = picker.intrinsicContentSize
        var originX = buttonFrame.midX - pickerSize.width / 2
        let minX: CGFloat = 8
        let maxX = window.bounds.width - pickerSize.width - 8
        originX = min(max(originX, minX), maxX)
        let originY = buttonFrame.minY - pickerSize.height - 8

        NSLayoutConstraint.activate([
            picker.leadingAnchor.constraint(equalTo: window.leadingAnchor, constant: originX),
            picker.topAnchor.constraint(equalTo: window.topAnchor, constant: originY),
            picker.widthAnchor.constraint(equalToConstant: pickerSize.width),
            picker.heightAnchor.constraint(equalToConstant: pickerSize.height),
        ])

        picker.alpha = 0
        picker.transform = CGAffineTransform(scaleX: 0.9, y: 0.9)
        UIView.animate(withDuration: 0.18, delay: 0, options: [.curveEaseOut]) {
            picker.alpha = 1
            picker.transform = .identity
        }

        pickerView = picker
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
    }

    private func commitSelectionIfNeeded() {
        guard let pickerView,
              let selection = pickerView.currentHoveredModifier,
              selection != activeModifier
        else { return }
        didCommitSelection = true
        onSelect?(selection)
        UISelectionFeedbackGenerator().selectionChanged()
    }

    private func dismissPicker() {
        guard let picker = pickerView else { return }
        pickerView = nil
        UIView.animate(
            withDuration: 0.15,
            delay: 0,
            options: [.curveEaseIn],
            animations: {
                picker.alpha = 0
                picker.transform = CGAffineTransform(scaleX: 0.9, y: 0.9)
            },
            completion: { _ in picker.removeFromSuperview() }
        )
    }
}

final class ModifierPickerView: UIView {
    private let rowHeight: CGFloat = 44
    private let horizontalPadding: CGFloat = 16
    private let verticalPadding: CGFloat = 6
    private let pickerWidth: CGFloat = 180
    private let arrowHeight: CGFloat = 8

    private let active: TerminalModifier
    private let fgColor: UIColor
    private let containerView = UIView()
    private var rowViews: [ModifierPickerRow] = []
    private(set) var currentHoveredModifier: TerminalModifier?

    init(active: TerminalModifier, fg: UIColor) {
        self.active = active
        fgColor = fg
        super.init(frame: .zero)
        backgroundColor = .clear
        isOpaque = false
        setupViews()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var intrinsicContentSize: CGSize {
        let rows = CGFloat(TerminalModifier.allCases.count)
        let height = rows * rowHeight + verticalPadding * 2 + arrowHeight
        return CGSize(width: pickerWidth, height: height)
    }

    private func setupViews() {
        containerView.translatesAutoresizingMaskIntoConstraints = false
        containerView.backgroundColor = UIColor.black.withAlphaComponent(0.7)
        containerView.layer.cornerRadius = 18
        containerView.layer.cornerCurve = .continuous
        containerView.layer.borderWidth = 0.5
        containerView.layer.borderColor = fgColor.withAlphaComponent(0.12).cgColor
        addSubview(containerView)

        let blur = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterialDark))
        blur.translatesAutoresizingMaskIntoConstraints = false
        blur.layer.cornerRadius = 18
        blur.layer.cornerCurve = .continuous
        blur.clipsToBounds = true
        containerView.insertSubview(blur, at: 0)

        let rows = TerminalModifier.allCases
        var previousAnchor: NSLayoutYAxisAnchor = containerView.topAnchor
        var topInset: CGFloat = verticalPadding
        for (index, modifier) in rows.enumerated() {
            let row = ModifierPickerRow(modifier: modifier, fg: fgColor, disabled: modifier == active)
            row.translatesAutoresizingMaskIntoConstraints = false
            containerView.addSubview(row)
            rowViews.append(row)

            NSLayoutConstraint.activate([
                row.topAnchor.constraint(equalTo: previousAnchor, constant: topInset),
                row.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
                row.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
                row.heightAnchor.constraint(equalToConstant: rowHeight),
            ])

            if index < rows.count - 1 {
                let divider = UIView()
                divider.translatesAutoresizingMaskIntoConstraints = false
                divider.backgroundColor = fgColor.withAlphaComponent(0.08)
                containerView.addSubview(divider)
                NSLayoutConstraint.activate([
                    divider.topAnchor.constraint(equalTo: row.bottomAnchor),
                    divider.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: horizontalPadding),
                    divider.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -horizontalPadding),
                    divider.heightAnchor.constraint(equalToConstant: 0.5),
                ])
            }

            previousAnchor = row.bottomAnchor
            topInset = 0
        }

        NSLayoutConstraint.activate([
            containerView.topAnchor.constraint(equalTo: topAnchor),
            containerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            containerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            containerView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -arrowHeight),
            blur.topAnchor.constraint(equalTo: containerView.topAnchor),
            blur.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            blur.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            blur.bottomAnchor.constraint(equalTo: containerView.bottomAnchor),
        ])
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        setNeedsDisplay()
    }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        let arrowWidth: CGFloat = 18
        let midX = rect.midX
        let topY = rect.maxY - arrowHeight
        ctx.beginPath()
        ctx.move(to: CGPoint(x: midX - arrowWidth / 2, y: topY))
        ctx.addLine(to: CGPoint(x: midX + arrowWidth / 2, y: topY))
        ctx.addLine(to: CGPoint(x: midX, y: rect.maxY))
        ctx.closePath()
        ctx.setFillColor(UIColor.black.withAlphaComponent(0.7).cgColor)
        ctx.fillPath()
    }

    func updateHover(at location: CGPoint) {
        var hovered: TerminalModifier?
        for row in rowViews {
            let frameInSelf = row.convert(row.bounds, to: self)
            if frameInSelf.contains(location), !row.isDisabled {
                hovered = row.modifier
                row.setHighlighted(true)
            } else {
                row.setHighlighted(false)
            }
        }
        currentHoveredModifier = hovered
    }
}

final class ModifierPickerRow: UIView {
    let modifier: TerminalModifier
    let isDisabled: Bool
    private let glyphLabel = UILabel()
    private let titleLabel = UILabel()
    private let highlight = UIView()

    init(modifier: TerminalModifier, fg: UIColor, disabled: Bool) {
        self.modifier = modifier
        isDisabled = disabled
        super.init(frame: .zero)

        highlight.translatesAutoresizingMaskIntoConstraints = false
        highlight.backgroundColor = fg.withAlphaComponent(0.18)
        highlight.alpha = 0
        highlight.isUserInteractionEnabled = false
        addSubview(highlight)

        glyphLabel.font = UIFont.systemFont(ofSize: 15, weight: .semibold)
        glyphLabel.textAlignment = .center
        glyphLabel.text = modifier.glyph
        glyphLabel.textColor = disabled ? fg.withAlphaComponent(0.4) : fg
        glyphLabel.translatesAutoresizingMaskIntoConstraints = false

        titleLabel.font = UIFont.monospacedSystemFont(ofSize: 14, weight: .medium)
        titleLabel.text = modifier.displayName.lowercased()
        titleLabel.textColor = disabled ? fg.withAlphaComponent(0.4) : fg
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        addSubview(glyphLabel)
        addSubview(titleLabel)

        NSLayoutConstraint.activate([
            highlight.topAnchor.constraint(equalTo: topAnchor),
            highlight.bottomAnchor.constraint(equalTo: bottomAnchor),
            highlight.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            highlight.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            glyphLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            glyphLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            glyphLabel.widthAnchor.constraint(equalToConstant: 20),
            titleLabel.leadingAnchor.constraint(equalTo: glyphLabel.trailingAnchor, constant: 14),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        highlight.layer.cornerRadius = 10
        highlight.layer.cornerCurve = .continuous
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setHighlighted(_ active: Bool) {
        let target: CGFloat = active && !isDisabled ? 1 : 0
        guard highlight.alpha != target else { return }
        UIView.animate(withDuration: 0.08) { self.highlight.alpha = target }
    }
}

struct DPadControl: View {
    let tint: SwiftUI.Color
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
                .onChanged { value in handleDrag(translation: value.translation) }
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
