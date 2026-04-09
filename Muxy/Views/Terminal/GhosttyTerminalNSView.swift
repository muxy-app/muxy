import AppKit
import GhosttyKit
import QuartzCore

final class GhosttyTerminalNSView: NSView {
    nonisolated(unsafe) private(set) var surface: ghostty_surface_t?
    private let workingDirectory: String
    var onTitleChange: ((String) -> Void)?
    var onFocus: (() -> Void)?
    var onProcessExit: (() -> Void)?
    var onSplitRequest: ((SplitDirection, SplitPosition) -> Void)?
    var onSearchStart: ((String?) -> Void)?
    var onSearchEnd: (() -> Void)?
    var onSearchTotal: ((Int?) -> Void)?
    var onSearchSelected: ((Int?) -> Void)?
    var isFocused: Bool = false

    private var _markedRange: NSRange = .init(location: NSNotFound, length: 0)
    private var _selectedRange: NSRange = .init(location: NSNotFound, length: 0)

    private var keyTextAccumulator: [String] = []
    private var currentKeyEvent: NSEvent?

    init(workingDirectory: String) {
        self.workingDirectory = workingDirectory
        super.init(frame: .zero)
        wantsLayer = true
        setupTrackingArea()
        registerForDraggedTypes([.fileURL])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override func makeBackingLayer() -> CALayer {
        let metalLayer = CAMetalLayer()
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.isOpaque = false
        metalLayer.framebufferOnly = false
        metalLayer.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        metalLayer.needsDisplayOnBoundsChange = true
        metalLayer.presentsWithTransaction = false
        return metalLayer
    }

    override var wantsUpdateLayer: Bool { true }

    private var pendingSurfaceCreation = false

    func createSurface() {
        guard surface == nil, let app = GhosttyService.shared.app else { return }

        let backingSize = convertToBacking(bounds).size
        guard backingSize.width > 0, backingSize.height > 0 else {
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

        workingDirectory.withCString { cwd in
            config.working_directory = cwd
            surface = ghostty_surface_new(app, &config)
        }

        guard let surface else { return }

        let scale = Double(window?.backingScaleFactor ?? 2.0)
        ghostty_surface_set_content_scale(surface, scale, scale)

        let w = UInt32(backingSize.width)
        let h = UInt32(backingSize.height)
        ghostty_surface_set_size(surface, w, h)

        let isDark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        ghostty_surface_set_color_scheme(surface, isDark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)

        if let screen = window?.screen ?? NSScreen.main,
           let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32
        {
            ghostty_surface_set_display_id(surface, displayID)
        }

        ghostty_surface_set_focus(surface, isFocused)
    }

    func destroySurface() {
        if let surface {
            ghostty_surface_free(surface)
        }
        surface = nil
    }

    deinit {
        if let surface {
            ghostty_surface_free(surface)
        }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil, surface == nil {
            createSurface()
        }
        if window != nil {
            updateMetalLayerSize()
        }
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if pendingSurfaceCreation {
            createSurface()
        }
        updateMetalLayerSize()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateMetalLayerSize()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        guard let surface else { return }
        let isDark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        ghostty_surface_set_color_scheme(surface, isDark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
    }

    private func updateMetalLayerSize() {
        guard let surface, window != nil else { return }

        let scaledSize = convertToBacking(bounds).size
        guard scaledSize.width > 0, scaledSize.height > 0 else { return }

        let scale = Double(window?.backingScaleFactor ?? 2.0)

        if let metalLayer = layer as? CAMetalLayer {
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            metalLayer.contentsScale = CGFloat(scale)
            CATransaction.commit()
        }

        ghostty_surface_set_content_scale(surface, scale, scale)

        let w = UInt32(scaledSize.width)
        let h = UInt32(scaledSize.height)
        ghostty_surface_set_size(surface, w, h)
    }

    private func isAppShortcut(_ event: NSEvent) -> Bool {
        let key = KeyCombo.normalized(key: event.charactersIgnoringModifiers ?? "", keyCode: event.keyCode)
        let modifiers = event.modifierFlags.intersection(KeyCombo.supportedModifierMask)
        if modifiers == .command, Self.systemShortcutKeys.contains(key) {
            return true
        }
        let scopes = ShortcutContext.activeScopes(for: window)
        return KeyBindingStore.shared.isRegisteredShortcut(event: event, scopes: scopes)
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

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        if result {
            ghostty_surface_set_focus(surface, true)
            onFocus?()
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

        if flags.contains(.control) && !flags.contains(.command) && !flags.contains(.option) && !hasMarkedText() {
            if isAppShortcut(event) { return }
            var keyEvent = buildKeyEvent(from: event, action: action)
            let text = event.charactersIgnoringModifiers ?? event.characters ?? ""
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
        interpretKeyEvents([event])
        currentKeyEvent = nil

        var keyEvent = buildKeyEvent(from: event, action: action)
        keyEvent.consumed_mods = consumedModsFromFlags(flags)
        keyEvent.composing = hasMarkedText() || hadMarkedText

        if !keyTextAccumulator.isEmpty, !keyEvent.composing {
            for text in keyTextAccumulator {
                text.withCString { ptr in
                    keyEvent.text = ptr
                    _ = ghostty_surface_key(surface, keyEvent)
                }
            }
        } else if !hasMarkedText() {
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

    override func doCommand(by selector: Selector) {}

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
        var keyEvent = buildKeyEvent(from: event, action: isFlagPress(event) ? GHOSTTY_ACTION_PRESS : GHOSTTY_ACTION_RELEASE)
        keyEvent.text = nil
        _ = ghostty_surface_key(surface, keyEvent)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if isAppShortcut(event) { return false }
        guard window?.firstResponder === self || window?.firstResponder === inputContext else { return false }
        guard event.type == .keyDown, let surface else { return false }

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let hasActionModifier = flags.contains(.command) || flags.contains(.control) || flags.contains(.option)
        guard hasActionModifier else { return false }

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
            onFocus?()
        }
        let pt = mousePoint(from: event)
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT, modsFromEvent(event))
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
        ghostty_surface_mouse_pos(surface, pt.x, pt.y, modsFromEvent(event))
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
        guard let text = NSPasteboard.general.string(forType: .string), !text.isEmpty else { return }
        insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
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
        keyEvent.keycode = UInt32(event.keyCode)
        keyEvent.mods = modsFromEvent(event)
        keyEvent.consumed_mods = GHOSTTY_MODS_NONE
        keyEvent.composing = false
        keyEvent.text = nil
        keyEvent.unshifted_codepoint = unshiftedCodepoint(from: event)
        return keyEvent
    }

    private func consumedModsFromFlags(_ flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.option) { mods |= GHOSTTY_MODS_ALT.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
    }

    private func modsFromEvent(_ event: NSEvent) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        let flags = event.modifierFlags
        if flags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.control) { mods |= GHOSTTY_MODS_CTRL.rawValue }
        if flags.contains(.option) { mods |= GHOSTTY_MODS_ALT.rawValue }
        if flags.contains(.command) { mods |= GHOSTTY_MODS_SUPER.rawValue }
        if flags.contains(.capsLock) { mods |= GHOSTTY_MODS_CAPS.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
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

    private func filterSpecialCharacters(_ text: String) -> String {
        guard let scalar = text.unicodeScalars.first else { return "" }
        let value = scalar.value
        if value < 0x20 || (0xF700 ... 0xF8FF).contains(value) { return "" }
        return text
    }

    private func unshiftedCodepoint(from event: NSEvent) -> UInt32 {
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

    enum SearchDirection: String {
        case next
        case previous
    }
}

extension GhosttyTerminalNSView {
    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        guard sender.draggingPasteboard.canReadObject(forClasses: [NSURL.self]) else { return [] }
        return .copy
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        guard let urls = sender.draggingPasteboard.readObjects(forClasses: [NSURL.self]) as? [URL],
              !urls.isEmpty
        else { return false }

        let paths = urls.map { Self.shellEscapedPath($0.path) }
        let text = paths.joined(separator: " ")
        insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
        return true
    }

    private static func shellEscapedPath(_ path: String) -> String {
        let needsQuoting = path.contains(" ") || path.contains("(") || path.contains(")")
            || path.contains("'") || path.contains("\"") || path.contains("\\")
            || path.contains("&") || path.contains("|") || path.contains(";")
            || path.contains("$") || path.contains("`") || path.contains("!")
        guard needsQuoting else { return path }
        return "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }
}

extension GhosttyTerminalNSView: @preconcurrency NSTextInputClient {
    func insertText(_ string: Any, replacementRange: NSRange) {
        let text = (string as? String) ?? (string as? NSAttributedString)?.string ?? ""
        guard !text.isEmpty else { return }

        _markedRange = NSRange(location: NSNotFound, length: 0)
        if let surface {
            ghostty_surface_preedit(surface, nil, 0)
        }

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
        guard let surface else { return }
        let text = (string as? String) ?? (string as? NSAttributedString)?.string ?? ""
        _markedRange = text.isEmpty ? NSRange(location: NSNotFound, length: 0) : NSRange(location: 0, length: text.count)
        _selectedRange = selectedRange

        text.withCString { ptr in
            ghostty_surface_preedit(surface, ptr, UInt(text.count))
        }
    }

    func unmarkText() {
        guard let surface else { return }
        _markedRange = NSRange(location: NSNotFound, length: 0)
        ghostty_surface_preedit(surface, nil, 0)
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
