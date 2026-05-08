import AppKit
import GhosttyKit

final class GhosttyTerminalNSView: NSView {
    nonisolated(unsafe) private(set) var surface: ghostty_surface_t?
    private let workingDirectory: String
    private let command: String?
    private let commandInteractive: Bool
    var envVars: [(key: String, value: String)] = []
    var onTitleChange: ((String) -> Void)?
    var onWorkingDirectoryChange: ((String) -> Void)?
    var onFocus: (() -> Void)?
    var onExternalDragHoverChange: ((Bool) -> Void)?
    var onProcessExit: (() -> Void)?
    var onSplitRequest: ((SplitDirection, SplitPosition) -> Void)?
    var onSearchStart: ((String?) -> Void)?
    var onSearchEnd: (() -> Void)?
    var onSearchTotal: ((Int?) -> Void)?
    var onSearchSelected: ((Int?) -> Void)?
    var onProgressReport: ((TerminalProgress?) -> Void)?
    var onCmdClickFile: ((String) -> Void)?
    var resolveCmdHoverFile: ((String) -> Bool)?
    var onOpenURL: ((URL) -> Bool)?
    private var isShowingHandCursor = false
    private var fileHoverUnderlineLayer: CAShapeLayer?
    private var lastMouseTopDownPoint: CGPoint?
    var hasOSC8LinkUnderCursor: Bool = false
    var isFocused: Bool = false
    var overlayActive: Bool = false

    var processExitHandled = false

    private var isPaneVisible = true
    private var isWindowVisible = true
    nonisolated(unsafe) private var occlusionObserver: NSObjectProtocol?

    var closesOnCommandExit: Bool {
        command != nil
    }

    private var _markedText: String = ""
    private var _markedRange: NSRange = .init(location: NSNotFound, length: 0)
    private var _selectedRange: NSRange = .init(location: NSNotFound, length: 0)

    private var keyTextAccumulator: [String] = []
    private var currentKeyEvent: NSEvent?
    private var commandSelectorCalled = false

    init(workingDirectory: String, command: String? = nil, commandInteractive: Bool = false) {
        self.workingDirectory = workingDirectory
        self.command = command
        self.commandInteractive = commandInteractive
        super.init(frame: .zero)
        wantsLayer = true
        setupTrackingArea()
        registerForDraggedTypes([.fileURL, .string])
        setAccessibilityRole(.textArea)
        setAccessibilityRoleDescription("Terminal")
        let directoryName = URL(fileURLWithPath: workingDirectory).lastPathComponent
        let label = directoryName.isEmpty ? "Terminal" : "Terminal — \(directoryName)"
        setAccessibilityLabel(label)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override func accessibilitySelectedText() -> String? {
        readSelectionText()
    }

    private func readSelectionText() -> String? {
        guard let surface, ghostty_surface_has_selection(surface) else { return nil }
        var text = ghostty_text_s()
        guard ghostty_surface_read_selection(surface, &text) else { return nil }
        defer { ghostty_surface_free_text(surface, &text) }
        return extractString(from: text)
    }

    private func extractString(from text: ghostty_text_s) -> String? {
        guard let ptr = text.text, text.text_len > 0 else { return nil }
        let len = Int(text.text_len)
        return ptr.withMemoryRebound(to: UInt8.self, capacity: len) { rawPtr in
            String(bytes: UnsafeBufferPointer(start: rawPtr, count: len), encoding: .utf8)
        }
    }

    private var pendingSurfaceCreation = false

    func createSurface() {
        guard surface == nil, let app = GhosttyService.shared.app else { return }

        guard let backingSize = backingPixelSize() else {
            pendingSurfaceCreation = true
            return
        }
        pendingSurfaceCreation = false

        var config = ghostty_surface_config_new()
        config.platform_tag = GHOSTTY_PLATFORM_MACOS
        config.platform = ghostty_platform_u(
            macos: ghostty_platform_macos_s(nsview: Unmanaged.passUnretained(self).toOpaque())
        )
        config.userdata = Unmanaged.passUnretained(self).toOpaque()
        config.scale_factor = Double(window?.backingScaleFactor ?? 2.0)
        config.context = GHOSTTY_SURFACE_CONTEXT_SPLIT

        var cStrings: [UnsafeMutablePointer<CChar>] = []
        defer { cStrings.forEach { free($0) } }

        if let command, let loginWrapped = strdup(Self.loginShellCommand(command, interactive: commandInteractive)) {
            cStrings.append(loginWrapped)
            config.command = UnsafePointer(loginWrapped)
            config.wait_after_command = false
        }

        var cEnvVars: [ghostty_env_var_s] = []
        for pair in envVars {
            guard let ck = strdup(pair.key), let cv = strdup(pair.value) else { continue }
            cStrings.append(contentsOf: [ck, cv])
            cEnvVars.append(ghostty_env_var_s(key: ck, value: cv))
        }

        workingDirectory.withCString { cwd in
            config.working_directory = cwd
            if !cEnvVars.isEmpty {
                cEnvVars.withUnsafeMutableBufferPointer { buffer in
                    config.env_vars = buffer.baseAddress
                    config.env_var_count = buffer.count
                    surface = ghostty_surface_new(app, &config)
                }
            } else {
                surface = ghostty_surface_new(app, &config)
            }
        }

        guard let surface else { return }

        let scale = Double(window?.backingScaleFactor ?? 2.0)
        ghostty_surface_set_content_scale(surface, scale, scale)

        ghostty_surface_set_size(surface, backingSize.width, backingSize.height)

        applyColorScheme(isDark: ThemeService.isCurrentAppearanceDark())

        if let screen = window?.screen ?? NSScreen.main,
           let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32
        {
            ghostty_surface_set_display_id(surface, displayID)
        }

        ghostty_surface_set_focus(surface, isFocused)

        if let paneID = TerminalViewRegistry.shared.paneID(for: self) {
            RemoteTerminalStreamer.shared.attach(paneID: paneID, surface: surface)
        }

        applyOcclusionState()
    }

    func destroySurface() {
        if let surface {
            if let paneID = TerminalViewRegistry.shared.paneID(for: self) {
                RemoteTerminalStreamer.shared.detach(paneID: paneID, surface: surface)
            }
            ghostty_surface_free(surface)
        }
        surface = nil
    }

    func tearDown() {
        setHandCursor(false)
        onOpenURL = nil
        onCmdClickFile = nil
        resolveCmdHoverFile = nil
        onTitleChange = nil
        onFocus = nil
        onExternalDragHoverChange = nil
        onProcessExit = nil
        onSplitRequest = nil
        onSearchStart = nil
        onSearchEnd = nil
        onSearchTotal = nil
        onSearchSelected = nil
        onProgressReport = nil
        if let observer = screenChangeObserver {
            NotificationCenter.default.removeObserver(observer)
            screenChangeObserver = nil
        }
        if let observer = occlusionObserver {
            NotificationCenter.default.removeObserver(observer)
            occlusionObserver = nil
        }
        delayedResizeWorkItem?.cancel()
        delayedResizeWorkItem = nil
        destroySurface()
        removeFromSuperview()
    }

    deinit {
        screenChangeObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        occlusionObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        delayedResizeWorkItem?.cancel()
        if let surface {
            ghostty_surface_free(surface)
        }
    }

    nonisolated(unsafe) private var screenChangeObserver: NSObjectProtocol?
    nonisolated(unsafe) private var delayedResizeWorkItem: DispatchWorkItem?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()

        screenChangeObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        screenChangeObserver = nil
        occlusionObserver.flatMap { NotificationCenter.default.removeObserver($0) }
        occlusionObserver = nil
        delayedResizeWorkItem?.cancel()
        delayedResizeWorkItem = nil

        guard let window else { return }

        if surface == nil {
            createSurface()
        }

        screenChangeObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeScreenNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.updateMetalLayerSize(deferred: true)
            }
        }

        occlusionObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeOcclusionStateNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.updateWindowVisibility()
            }
        }

        updateWindowVisibility()
        updateMetalLayerSize(deferred: true)
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if pendingSurfaceCreation {
            createSurface()
        }
        updateMetalLayerSize(deferred: false)
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateMetalLayerSize(deferred: true)
    }

    func setVisible(_ visible: Bool) {
        guard isPaneVisible != visible else { return }
        isPaneVisible = visible
        applyOcclusionState()
    }

    private func applyOcclusionState() {
        guard let surface else { return }
        ghostty_surface_set_occlusion(surface, isPaneVisible && isWindowVisible)
    }

    private func updateWindowVisibility() {
        let visible = window?.occlusionState.contains(.visible) ?? true
        guard isWindowVisible != visible else { return }
        isWindowVisible = visible
        applyOcclusionState()
    }

    func applyColorScheme(isDark: Bool) {
        guard let surface else { return }
        ghostty_surface_set_color_scheme(surface, isDark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
    }

    private func updateMetalLayerSize(deferred: Bool) {
        if deferred {
            delayedResizeWorkItem?.cancel()
            DispatchQueue.main.async { [weak self] in
                self?.updateMetalLayerSize(deferred: false)
            }
            let workItem = DispatchWorkItem { [weak self] in
                self?.updateMetalLayerSize(deferred: false)
            }
            delayedResizeWorkItem = workItem
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1, execute: workItem)
            return
        }

        guard let surface, let window else { return }
        layer?.contentsScale = window.backingScaleFactor
        layoutSubtreeIfNeeded()

        guard let backingSize = backingPixelSize() else { return }

        let scale = Double(window.backingScaleFactor)

        ghostty_surface_set_content_scale(surface, scale, scale)

        if let screen = window.screen,
           let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32
        {
            ghostty_surface_set_display_id(surface, displayID)
        }

        if let paneID = TerminalViewRegistry.shared.paneID(for: self),
           TerminalViewRegistry.shared.isOwnedByRemote(paneID)
        {
            return
        }

        ghostty_surface_set_size(surface, backingSize.width, backingSize.height)
    }

    func remoteOwnershipDidChange() {
        updateMetalLayerSize(deferred: false)
    }

    func materializeHeadless() {
        guard surface == nil else { return }
        if frame.size.width <= 0 || frame.size.height <= 0 {
            setFrameSize(NSSize(width: 1, height: 1))
        }
        createSurface()
    }

    private func backingPixelSize() -> (width: UInt32, height: UInt32)? {
        let size = convertToBacking(bounds).size
        let width = Int(floor(size.width))
        let height = Int(floor(size.height))
        guard width > 0, height > 0 else { return nil }
        return (UInt32(width), UInt32(height))
    }

    private func isAppShortcut(_ event: NSEvent) -> Bool {
        let key = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        let modifiers = event.modifierFlags.intersection(KeyCombo.supportedModifierMask)
        if modifiers == .command, Self.systemShortcutKeys.contains(key) {
            return true
        }
        let scopes = ShortcutContext.activeScopes(for: window)
        return KeyBindingStore.shared.isRegisteredShortcut(event: event, scopes: scopes)
            || CommandShortcutStore.shared.isRegisteredShortcut(event: event, scopes: scopes)
    }

    private static let systemShortcutKeys: Set<String> = ["q", "h", "m", ","]

    func needsConfirmQuit() -> Bool {
        guard let surface else { return false }
        return ghostty_surface_needs_confirm_quit(surface)
    }

    func notifySurfaceFocused() {
        guard let surface else { return }
        ghostty_surface_set_focus(surface, true)
    }

    func notifySurfaceUnfocused() {
        guard let surface else { return }
        ghostty_surface_set_focus(surface, false)
    }

    override var acceptsFirstResponder: Bool { !overlayActive }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        if result {
            ghostty_surface_set_focus(surface, true)
            if !isFocused {
                DispatchQueue.main.async { [weak self] in
                    self?.onFocus?()
                }
            }
        }
        return result
    }

    override func resignFirstResponder() -> Bool {
        let result = super.resignFirstResponder()
        if result {
            ghostty_surface_set_focus(surface, false)
        }
        return result
    }

    private var currentTrackingArea: NSTrackingArea?

    private func setupTrackingArea() {
        if let existing = currentTrackingArea { removeTrackingArea(existing) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .mouseMoved, .activeInKeyWindow, .inVisibleRect],
            owner: self
        )
        addTrackingArea(area)
        currentTrackingArea = area
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        setupTrackingArea()
    }

    override func keyDown(with event: NSEvent) {
        guard let surface else { super.keyDown(with: event)
            return
        }

        let action: ghostty_input_action_e = event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let optionAsAlt = translatedOptionAsAlt(for: event)

        if flags.contains(.control), !flags.contains(.command), !flags.contains(.option), !hasMarkedText() {
            if isAppShortcut(event) { return }
            var keyEvent = buildKeyEvent(from: event, action: action)
            let text = shortcutText(from: event)
            if text.isEmpty {
                keyEvent.text = nil
                _ = ghostty_surface_key(surface, keyEvent)
            } else {
                text.withCString { ptr in
                    keyEvent.text = ptr
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            }
            return
        }

        if flags.contains(.command) {
            if isAppShortcut(event) { return }
            var keyEvent = buildKeyEvent(from: event, action: action)
            keyEvent.text = nil
            _ = ghostty_surface_key(surface, keyEvent)
            return
        }

        let hadMarkedText = hasMarkedText()
        currentKeyEvent = event
        keyTextAccumulator = []
        commandSelectorCalled = false
        let interpretEvent = optionAsAlt ? eventStrippingOption(event) : event
        interpretKeyEvents([interpretEvent])
        currentKeyEvent = nil

        syncPreedit(clearIfNeeded: hadMarkedText)

        let commandWasCalled = commandSelectorCalled

        if !keyTextAccumulator.isEmpty {
            for text in keyTextAccumulator {
                var keyEvent = buildKeyEvent(from: event, action: action)
                keyEvent.consumed_mods = commandWasCalled ? GHOSTTY_MODS_NONE : consumedModsFromFlags(
                    flags,
                    consumeOption: !optionAsAlt
                )
                text.withCString { ptr in
                    keyEvent.text = ptr
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            }
        } else {
            var keyEvent = buildKeyEvent(from: event, action: action)
            keyEvent.consumed_mods = commandWasCalled ? GHOSTTY_MODS_NONE : consumedModsFromFlags(
                flags,
                consumeOption: !optionAsAlt
            )
            keyEvent.composing = hasMarkedText() || hadMarkedText

            let text = filterSpecialCharacters(event.characters ?? "")
            if !text.isEmpty, !keyEvent.composing {
                text.withCString { ptr in
                    keyEvent.text = ptr
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            } else {
                keyEvent.consumed_mods = GHOSTTY_MODS_NONE
                keyEvent.text = nil
                _ = ghostty_surface_key(surface, keyEvent)
            }
        }
    }

    override func doCommand(by selector: Selector) {
        commandSelectorCalled = true
    }

    override func insertText(_ insertString: Any) {
        insertText(insertString, replacementRange: NSRange(location: NSNotFound, length: 0))
    }

    override func keyUp(with event: NSEvent) {
        guard let surface else { return }
        var keyEvent = buildKeyEvent(from: event, action: GHOSTTY_ACTION_RELEASE)
        keyEvent.text = nil
        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func flagsChanged(with event: NSEvent) {
        guard let surface else { return }
        if hasMarkedText() { return }
        let action: ghostty_input_action_e = isFlagPress(event) ? GHOSTTY_ACTION_PRESS : GHOSTTY_ACTION_RELEASE
        var keyEvent = buildKeyEvent(from: event, action: action)
        keyEvent.text = nil
        _ = ghostty_surface_key(surface, keyEvent)
        updateCmdHoverCursor(modifierFlags: event.modifierFlags)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if isAppShortcut(event) { return false }
        guard window?.firstResponder === self || window?.firstResponder === inputContext else { return false }
        guard event.type == .keyDown, let surface else { return false }

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let hasActionModifier = flags.contains(.command) || flags.contains(.control) || flags.contains(.option)
        guard hasActionModifier else { return false }

        if isPasteShortcut(event, flags: flags), pasteboardHasImage() {
            sendRemoteBytes(Data([0x16]))
            return true
        }

        var keyEvent = buildKeyEvent(from: event, action: event.isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS)
        keyEvent.text = nil
        if ghostty_surface_key_is_binding(surface, keyEvent, nil) {
            _ = ghostty_surface_key(surface, keyEvent)
            return true
        }
        return false
    }

    private func mousePoint(from event: NSEvent) -> NSPoint {
        let local = convert(event.locationInWindow, from: nil)
        return NSPoint(x: local.x, y: bounds.height - local.y)
    }

    override func mouseDown(with event: NSEvent) {
        guard let surface else { return }
        let alreadyFirstResponder = window?.firstResponder === self
        window?.makeFirstResponder(self)
        if alreadyFirstResponder {
            ghostty_surface_set_focus(surface, true)
            DispatchQueue.main.async { [weak self] in
                self?.onFocus?()
            }
        }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        if event.modifierFlags.contains(.command), !hasOSC8LinkUnderCursor, let word = readWordUnderMouse() {
            onCmdClickFile?(word)
            return
        }
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT, modsFromEvent(event))
    }

    private func readWordUnderMouse() -> String? {
        guard let surface else { return nil }
        var text = ghostty_text_s()
        guard ghostty_surface_quicklook_word(surface, &text) else { return nil }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let value = extractString(from: text) else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    override func mouseUp(with event: NSEvent) {
        guard let surface else { return }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_LEFT, modsFromEvent(event))
    }

    override func mouseDragged(with event: NSEvent) {
        mouseMoved(with: event)
    }

    override func rightMouseDragged(with event: NSEvent) {
        mouseMoved(with: event)
    }

    override func otherMouseDragged(with event: NSEvent) {
        mouseMoved(with: event)
    }

    override func mouseMoved(with event: NSEvent) {
        guard let surface else { return }
        let pt = mousePoint(from: event)
        lastMouseTopDownPoint = pt
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        updateCmdHoverCursor(modifierFlags: event.modifierFlags)
    }

    override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        setHandCursor(false)
        hideFileHoverUnderline()
    }

    private func updateCmdHoverCursor(modifierFlags: NSEvent.ModifierFlags) {
        guard modifierFlags.contains(.command) else {
            setHandCursor(false)
            hideFileHoverUnderline()
            return
        }
        if hasOSC8LinkUnderCursor {
            setHandCursor(true)
            hideFileHoverUnderline()
            return
        }
        guard let word = readQuicklookWordUnderMouse(), resolveCmdHoverFile?(word.text) == true else {
            setHandCursor(false)
            hideFileHoverUnderline()
            return
        }
        showFileHoverUnderline(for: word)
        setHandCursor(true)
    }

    func refreshCmdHoverCursor() {
        updateCmdHoverCursor(modifierFlags: NSEvent.modifierFlags)
    }

    private func setHandCursor(_ on: Bool) {
        guard on != isShowingHandCursor else { return }
        isShowingHandCursor = on
        if on {
            NSCursor.pointingHand.push()
        } else {
            NSCursor.pop()
        }
    }

    private struct QuicklookWord {
        let text: String
        let topLeftPoints: CGPoint
    }

    private func readQuicklookWordUnderMouse() -> QuicklookWord? {
        guard let surface else { return nil }
        var text = ghostty_text_s()
        guard ghostty_surface_quicklook_word(surface, &text) else { return nil }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let value = extractString(from: text) else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return QuicklookWord(
            text: trimmed,
            topLeftPoints: CGPoint(x: text.tl_px_x, y: text.tl_px_y)
        )
    }

    private func showFileHoverUnderline(for word: QuicklookWord) {
        guard let layer else { return }
        let underlineLayer = fileHoverUnderlineLayer ?? CAShapeLayer()
        if fileHoverUnderlineLayer == nil {
            underlineLayer.fillColor = nil
            underlineLayer.isGeometryFlipped = true
            layer.addSublayer(underlineLayer)
            fileHoverUnderlineLayer = underlineLayer
        }

        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        let font = quicklookFont()
        let textSize = (word.text as NSString).size(withAttributes: [.font: font])
        let x = word.topLeftPoints.x
        let rowHeight = terminalRowHeight() ?? max(textSize.height, font.ascender - font.descender + font.leading)
        let mouseY = lastMouseTopDownPoint?.y ?? word.topLeftPoints.y
        let rowTopY = floor(mouseY / rowHeight) * rowHeight
        let y = rowTopY + rowHeight - max(2, font.underlineThickness)
        let width = max(textSize.width, 1)
        let thickness = max(font.underlineThickness, 1 / scale)

        let path = CGMutablePath()
        path.move(to: CGPoint(x: x, y: y))
        path.addLine(to: CGPoint(x: x + width, y: y))

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        underlineLayer.frame = bounds
        underlineLayer.contentsScale = scale
        underlineLayer.path = path
        underlineLayer.strokeColor = NSColor.controlAccentColor.cgColor
        underlineLayer.lineWidth = thickness
        underlineLayer.isHidden = false
        CATransaction.commit()
    }

    private func hideFileHoverUnderline() {
        fileHoverUnderlineLayer?.isHidden = true
    }

    private func quicklookFont() -> NSFont {
        guard let surface,
              let fontPtr = ghostty_surface_quicklook_font(surface)
        else {
            return .monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
        }
        return Unmanaged<NSFont>.fromOpaque(fontPtr).takeUnretainedValue()
    }

    private func terminalRowHeight() -> CGFloat? {
        guard let surface else { return nil }
        var cells = ghostty_cells_s()
        guard ghostty_surface_read_cells(surface, &cells) else { return nil }
        defer { ghostty_surface_free_cells(surface, &cells) }
        guard cells.rows > 0 else { return nil }
        return bounds.height / CGFloat(cells.rows)
    }

    override func rightMouseDown(with event: NSEvent) {
        guard let surface else { return }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        let consumed = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT, modsFromEvent(event))
        if !consumed {
            presentContextMenu(with: event)
        }
    }

    override func rightMouseUp(with event: NSEvent) {
        guard let surface else { return }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        let consumed = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_RELEASE, GHOSTTY_MOUSE_RIGHT, modsFromEvent(event))
        if !consumed {
            super.rightMouseUp(with: event)
        }
    }

    private func presentContextMenu(with event: NSEvent) {
        let menu = NSMenu(title: "Terminal")

        let paste = NSMenuItem(title: "Paste", action: #selector(handleContextPaste(_:)), keyEquivalent: "")
        paste.target = self
        paste.isEnabled = NSPasteboard.general.string(forType: .string).map { !$0.isEmpty } ?? false
        menu.addItem(paste)

        menu.addItem(.separator())

        menu.addItem(contextSplitMenuItem(title: "Split Right", direction: .horizontal, position: .second))
        menu.addItem(contextSplitMenuItem(title: "Split Left", direction: .horizontal, position: .first))
        menu.addItem(contextSplitMenuItem(title: "Split Down", direction: .vertical, position: .second))
        menu.addItem(contextSplitMenuItem(title: "Split Up", direction: .vertical, position: .first))

        NSMenu.popUpContextMenu(menu, with: event, for: self)
    }

    private func contextSplitMenuItem(title: String, direction: SplitDirection, position: SplitPosition) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: #selector(handleContextSplit(_:)), keyEquivalent: "")
        item.target = self
        item.representedObject = ContextSplit(direction: direction, position: position)
        return item
    }

    @objc
    private func handleContextPaste(_: Any?) {
        window?.makeFirstResponder(self)
        if pasteboardHasImage() {
            sendRemoteBytes(Data([0x16]))
            return
        }
        guard let text = NSPasteboard.general.string(forType: .string), !text.isEmpty else { return }
        insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
    }

    private func isPasteShortcut(_ event: NSEvent, flags: NSEvent.ModifierFlags) -> Bool {
        guard flags.contains(.command), !flags.contains(.control), !flags.contains(.option) else { return false }
        return event.keyCode == 9
    }

    private func pasteboardHasImage() -> Bool {
        let pb = NSPasteboard.general
        if pb.string(forType: .string) != nil { return false }
        return pb.canReadObject(forClasses: [NSImage.self], options: nil)
    }

    @objc
    func paste(_ sender: Any?) {
        handleContextPaste(sender)
    }

    @objc
    private func handleContextSplit(_ sender: NSMenuItem) {
        guard let split = sender.representedObject as? ContextSplit else { return }
        onSplitRequest?(split.direction, split.position)
    }

    private final class ContextSplit: NSObject {
        let direction: SplitDirection
        let position: SplitPosition

        init(direction: SplitDirection, position: SplitPosition) {
            self.direction = direction
            self.position = position
        }
    }

    override func scrollWheel(with event: NSEvent) {
        guard let surface else { return }
        var mods: ghostty_input_scroll_mods_t = 0
        if event.hasPreciseScrollingDeltas { mods |= 1 }
        ghostty_surface_mouse_scroll(surface, event.scrollingDeltaX, event.scrollingDeltaY, mods)
    }

    private func buildKeyEvent(from event: NSEvent, action: ghostty_input_action_e) -> ghostty_input_key_s {
        var keyEvent = ghostty_input_key_s()
        keyEvent.action = action

        let normalized = event.type == .keyDown || event.type == .keyUp
            ? KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
            : KeyCombo.normalized(key: "", keyCode: event.keyCode)
        if let mappedCode = KeyCombo.keyCode(for: normalized) {
            keyEvent.keycode = UInt32(mappedCode)
        } else {
            keyEvent.keycode = UInt32(event.keyCode)
        }

        keyEvent.mods = modsFromEvent(event)
        keyEvent.consumed_mods = GHOSTTY_MODS_NONE
        keyEvent.composing = false
        keyEvent.text = nil
        keyEvent.unshifted_codepoint = unshiftedCodepoint(from: event)
        return keyEvent
    }

    private func consumedModsFromFlags(
        _ flags: NSEvent.ModifierFlags,
        consumeOption: Bool = true
    ) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if consumeOption, flags.contains(.option) { mods |= GHOSTTY_MODS_ALT.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
    }

    private enum RightModifierMask {
        static let shift: UInt = 0x04
        static let control: UInt = 0x2000
        static let option: UInt = 0x40
        static let command: UInt = 0x10
    }

    private func modsFromEvent(_ event: NSEvent) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        let flags = event.modifierFlags
        let raw = flags.rawValue
        if flags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.control) { mods |= GHOSTTY_MODS_CTRL.rawValue }
        if flags.contains(.option) { mods |= GHOSTTY_MODS_ALT.rawValue }
        if flags.contains(.command) { mods |= GHOSTTY_MODS_SUPER.rawValue }
        if flags.contains(.capsLock) { mods |= GHOSTTY_MODS_CAPS.rawValue }
        if raw & RightModifierMask.shift != 0 { mods |= GHOSTTY_MODS_SHIFT_RIGHT.rawValue }
        if raw & RightModifierMask.control != 0 { mods |= GHOSTTY_MODS_CTRL_RIGHT.rawValue }
        if raw & RightModifierMask.option != 0 { mods |= GHOSTTY_MODS_ALT_RIGHT.rawValue }
        if raw & RightModifierMask.command != 0 { mods |= GHOSTTY_MODS_SUPER_RIGHT.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
    }

    private func translatedOptionAsAlt(for event: NSEvent) -> Bool {
        guard let surface else { return false }
        let flags = event.modifierFlags
        guard flags.contains(.option) else { return false }
        let original = modsFromEvent(event)
        let translated = ghostty_surface_key_translation_mods(surface, original)
        return translated.rawValue & GHOSTTY_MODS_ALT.rawValue == 0
    }

    private func eventStrippingOption(_ event: NSEvent) -> NSEvent {
        let stripped = event.modifierFlags.subtracting(.option)
        let synthetic = NSEvent.keyEvent(
            with: event.type,
            location: event.locationInWindow,
            modifierFlags: stripped,
            timestamp: event.timestamp,
            windowNumber: event.windowNumber,
            context: nil,
            characters: event.charactersIgnoringModifiers ?? "",
            charactersIgnoringModifiers: event.charactersIgnoringModifiers ?? "",
            isARepeat: event.isARepeat,
            keyCode: event.keyCode
        )
        return synthetic ?? event
    }

    private func isFlagPress(_ event: NSEvent) -> Bool {
        let flags = event.modifierFlags
        switch event.keyCode {
        case 56,
             60: return flags.contains(.shift)
        case 58,
             61: return flags.contains(.option)
        case 59,
             62: return flags.contains(.control)
        case 55,
             54: return flags.contains(.command)
        case 57: return flags.contains(.capsLock)
        default: return false
        }
    }

    private func syncPreedit(clearIfNeeded: Bool = true) {
        guard let surface else { return }

        if hasMarkedText(), !_markedText.isEmpty {
            let byteCount = _markedText.utf8.count
            _markedText.withCString { ptr in
                ghostty_surface_preedit(surface, ptr, UInt(byteCount))
            }
        } else if clearIfNeeded {
            ghostty_surface_preedit(surface, nil, 0)
        }
    }

    private func filterSpecialCharacters(_ text: String) -> String {
        guard let scalar = text.unicodeScalars.first else { return "" }
        let value = scalar.value
        if value < 0x20 || (0xF700 ... 0xF8FF).contains(value) { return "" }
        return text
    }

    private func shortcutText(from event: NSEvent) -> String {
        let normalized = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        if normalized.unicodeScalars.count == 1,
           let scalar = normalized.unicodeScalars.first,
           scalar.isASCII, scalar.value >= 32, scalar.value <= 126
        {
            return normalized
        }
        if let scalar = KeyCombo.scalar(for: event.keyCode) {
            return String(scalar)
        }
        return event.charactersIgnoringModifiers ?? event.characters ?? ""
    }

    private func unshiftedCodepoint(from event: NSEvent) -> UInt32 {
        guard event.type == .keyDown || event.type == .keyUp else { return 0 }
        let normalized = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        if let scalar = normalized.unicodeScalars.first, normalized.unicodeScalars.count == 1 {
            return scalar.value
        }
        if let scalar = KeyCombo.scalar(for: event.keyCode) {
            return scalar.value
        }
        guard let chars = event.characters(byApplyingModifiers: []),
              let scalar = chars.unicodeScalars.first
        else { return 0 }
        return scalar.value
    }

    func sendSearchQuery(_ needle: String) {
        guard let surface else { return }
        let action = "search:\(needle)"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func navigateSearch(direction: SearchDirection) {
        guard let surface else { return }
        let action = "navigate_search:\(direction.rawValue)"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func endSearch() {
        guard let surface else { return }
        let action = "end_search"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func startSearch() {
        guard let surface else { return }
        let action = "start_search"
        ghostty_surface_binding_action(surface, action, UInt(action.utf8.count))
    }

    func sendText(_ text: String) {
        guard let surface else { return }
        text.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
    }

    func sendReturnKey() {
        sendKeyPress(codepoint: 13, keycode: 36)
    }

    func sendRemoteBytes(_ bytes: Data) {
        guard let surface, !bytes.isEmpty else { return }
        bytes.withUnsafeBytes { raw in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return }
            ghostty_surface_send_input_raw(surface, base, UInt(bytes.count))
        }
    }

    func sendKeyPress(codepoint: UInt32, keycode: UInt32 = 0, mods: ghostty_input_mods_e = GHOSTTY_MODS_NONE) {
        guard let surface else { return }
        var press = ghostty_input_key_s()
        press.action = GHOSTTY_ACTION_PRESS
        press.keycode = keycode
        press.mods = mods
        press.consumed_mods = mods
        press.composing = false
        press.text = nil
        press.unshifted_codepoint = codepoint
        _ = ghostty_surface_key(surface, press)

        var release = press
        release.action = GHOSTTY_ACTION_RELEASE
        _ = ghostty_surface_key(surface, release)
    }

    var hasLiveSurface: Bool {
        surface != nil
    }

    private static func loginShellCommand(_ command: String, interactive: Bool) -> String {
        let shell = userShell()
        let escaped = command.replacingOccurrences(of: "'", with: "'\\''")
        let flags = interactive ? "-l -i" : "-l"
        return "\(shell) \(flags) -c '\(escaped)'"
    }

    private static func userShell() -> String {
        if let shell = ProcessInfo.processInfo.environment["SHELL"], !shell.isEmpty {
            return shell
        }
        guard let pw = getpwuid(getuid()), let shellPtr = pw.pointee.pw_shell else {
            return "/bin/zsh"
        }
        return String(cString: shellPtr)
    }

    enum SearchDirection: String {
        case next
        case previous
    }
}

extension GhosttyTerminalNSView {
    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        guard !droppedPaths(from: sender).isEmpty else { return [] }
        onExternalDragHoverChange?(true)
        return .copy
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        droppedPaths(from: sender).isEmpty ? [] : .copy
    }

    override func draggingExited(_: (any NSDraggingInfo)?) {
        onExternalDragHoverChange?(false)
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        onExternalDragHoverChange?(false)
        let paths = droppedPaths(from: sender)
        guard !paths.isEmpty else { return false }
        let text = paths.map { ShellEscaper.escape($0) }.joined(separator: " ")
        scheduleFocusAndInsertAfterDrop(text: text)
        return true
    }

    private func scheduleFocusAndInsertAfterDrop(text: String) {
        RunLoop.main.perform(inModes: [.default]) { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                NSApp.activate()
                self.window?.makeKeyAndOrderFront(nil)
                self.window?.makeFirstResponder(self)
                self.notifySurfaceFocused()
                self.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
            }
        }
    }

    private func droppedPaths(from sender: any NSDraggingInfo) -> [String] {
        let pasteboard = sender.draggingPasteboard
        let urls = (pasteboard.readObjects(forClasses: [NSURL.self]) as? [URL]) ?? []
        return DroppedPathsParser.parse(fileURLs: urls, plainString: pasteboard.string(forType: .string))
    }
}

extension GhosttyTerminalNSView: @preconcurrency NSTextInputClient {
    func insertText(_ string: Any, replacementRange: NSRange) {
        let text = (string as? String) ?? (string as? NSAttributedString)?.string ?? ""

        unmarkText()

        guard !text.isEmpty else { return }

        if currentKeyEvent != nil {
            keyTextAccumulator.append(text)
        } else if let surface {
            text.withCString { ptr in
                var keyEvent = ghostty_input_key_s()
                keyEvent.action = GHOSTTY_ACTION_PRESS
                keyEvent.keycode = 0
                keyEvent.mods = GHOSTTY_MODS_NONE
                keyEvent.consumed_mods = GHOSTTY_MODS_NONE
                keyEvent.composing = false
                keyEvent.text = ptr
                _ = ghostty_surface_key(surface, keyEvent)
            }
        }
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        let text = (string as? String) ?? (string as? NSAttributedString)?.string ?? ""
        _markedText = text
        _markedRange = text.isEmpty ? NSRange(location: NSNotFound, length: 0) : NSRange(location: 0, length: text.count)
        _selectedRange = selectedRange

        if currentKeyEvent == nil {
            syncPreedit()
        }
    }

    func unmarkText() {
        guard hasMarkedText() else { return }
        _markedText = ""
        _markedRange = NSRange(location: NSNotFound, length: 0)
        syncPreedit()
    }

    func selectedRange() -> NSRange {
        _selectedRange
    }

    func markedRange() -> NSRange {
        _markedRange
    }

    func hasMarkedText() -> Bool {
        _markedRange.location != NSNotFound
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        nil
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        [.underlineStyle, .backgroundColor]
    }

    func characterIndex(for point: NSPoint) -> Int {
        NSNotFound
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        guard let surface else { return .zero }
        var x: Double = 0, y: Double = 0, w: Double = 0, h: Double = 0
        ghostty_surface_ime_point(surface, &x, &y, &w, &h)
        let viewPt = NSPoint(x: x, y: bounds.height - y)
        let screenPt = window?.convertPoint(toScreen: convert(viewPt, to: nil)) ?? viewPt
        return NSRect(x: screenPt.x, y: screenPt.y - h, width: w, height: h)
    }
}
