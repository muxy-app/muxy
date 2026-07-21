import AppKit
import QuartzCore

@MainActor
enum QuickTerminalMaterialMask {
    static func image(opacity: Double) -> NSImage? {
        guard opacity > 0, opacity < 1 else { return nil }
        let color = NSColor.white.withAlphaComponent(CGFloat(opacity))
        let image = NSImage(size: NSSize(width: 1, height: 1), flipped: false) { bounds in
            color.setFill()
            NSBezierPath(rect: bounds).fill()
            return true
        }
        image.capInsets = NSEdgeInsets()
        image.resizingMode = .stretch
        return image
    }
}

@MainActor
final class QuickTerminalContentView: NSView {
    static let bridgeHeight: CGFloat = 34

    var onClose: (() -> Void)?
    var onOpenSettings: (() -> Void)?
    var shortcutSettingsProvider: (() -> QuickTerminalShortcutSettingsSnapshot)?
    var onShortcutChange: ((QuickTerminalShortcut) -> String?)?
    var onRequestInputMonitoringAccess: (() -> Bool)?

    private let revealMask = CAShapeLayer()
    private let terminalBackgroundView = NSVisualEffectView()
    private let terminalTintView = NSView()
    private let bridgeView = NSView()
    private let statusIndicator = NSView()
    private let statusLabel = NSTextField(labelWithString: "Ready")
    private let titleLabel = NSTextField(labelWithString: "Quick Terminal")
    private let shortcutButton = NSButton()
    private let settingsButton = NSButton()
    private let closeButton = NSButton()
    private let shortcutSettingsView = NSView()
    private let shortcutSettingsTitle = NSTextField(labelWithString: "Quick Terminal Shortcut")
    private let shortcutSettingsStatus = NSTextField(wrappingLabelWithString: "")
    private let doubleShiftButton = NSButton()
    private let customShortcutButton = NSButton()
    private let inputMonitoringButton = NSButton()
    private weak var terminalView: NSView?
    private var isRecordingShortcut = false
    private var isRevealed = true
    private var collapsedCutoutRect: NSRect?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        layer?.cornerRadius = 20
        layer?.cornerCurve = .continuous
        layer?.masksToBounds = true
        revealMask.fillColor = NSColor.black.cgColor
        layer?.mask = revealMask
        configureTerminalBackground()
        configureTerminalTint()
        configureBridge()
        configureShortcutSettings()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    override func layout() {
        super.layout()
        revealMask.frame = bounds
        bridgeView.frame = NSRect(
            x: 0,
            y: bounds.maxY - Self.bridgeHeight,
            width: bounds.width,
            height: Self.bridgeHeight
        )
        terminalBackgroundView.frame = NSRect(
            x: 0,
            y: 0,
            width: bounds.width,
            height: max(0, bounds.height - Self.bridgeHeight)
        )
        terminalTintView.frame = terminalBackgroundView.frame
        terminalView?.frame = NSRect(
            x: 0,
            y: 0,
            width: bounds.width,
            height: max(0, bounds.height - Self.bridgeHeight)
        )
        layoutBridgeControls()
        layoutShortcutSettings()
        if revealMask.animation(forKey: "reveal") == nil {
            revealMask.path = isRevealed ? expandedPath : collapsedPath
        }
    }

    func attach(surface: any QuickTerminalSurface) {
        let view = surface.quickTerminalView
        guard terminalView !== view else { return }
        terminalView?.removeFromSuperview()
        terminalView = view
        addSubview(view, positioned: .below, relativeTo: bridgeView)
        view.frame = NSRect(
            x: 0,
            y: 0,
            width: bounds.width,
            height: max(0, bounds.height - Self.bridgeHeight)
        )
        statusLabel.stringValue = "Ready"
        statusIndicator.layer?.backgroundColor = NSColor.systemGreen.cgColor
    }

    func applyAppearance(_ appearance: QuickTerminalAppearance) {
        terminalBackgroundView.isHidden = !appearance.showsBlur
        terminalBackgroundView.alphaValue = 1
        terminalBackgroundView.maskImage = QuickTerminalMaterialMask.image(opacity: appearance.blurFraction)
        terminalTintView.layer?.backgroundColor = MuxyTheme.nsBg
            .withAlphaComponent(CGFloat(appearance.backgroundOpacity))
            .cgColor
        shortcutSettingsView.layer?.backgroundColor = NSColor.black
            .withAlphaComponent(appearance.transparency == 0 ? 1 : 0.94)
            .cgColor
    }

    func clearTerminal(status: String) {
        terminalView?.removeFromSuperview()
        terminalView = nil
        statusLabel.stringValue = status
        statusIndicator.layer?.backgroundColor = NSColor.systemOrange.cgColor
    }

    func setShortcutLabel(_ label: String) {
        shortcutButton.title = label
        shortcutButton.sizeToFit()
        needsLayout = true
    }

    func setCollapsedCutoutRect(_ rect: NSRect?) {
        collapsedCutoutRect = rect
        needsLayout = true
    }

    func handleKeyDown(_ event: NSEvent) -> Bool {
        guard isRecordingShortcut else { return false }
        if event.keyCode == 53 {
            isRecordingShortcut = false
            refreshShortcutSettings()
            return true
        }
        let modifiers = event.modifierFlags.intersection(KeyCombo.supportedModifierMask)
        let requiredModifiers: NSEvent.ModifierFlags = [.command, .control, .option]
        guard !modifiers.isDisjoint(with: requiredModifiers) else {
            shortcutSettingsStatus.stringValue = "Include Command, Control, or Option."
            return true
        }
        let key = KeyCombo.normalized(
            key: event.charactersIgnoringModifiers ?? "",
            keyCode: event.keyCode
        )
        let shortcut = QuickTerminalShortcut.keyCombo(
            KeyCombo(key: key, modifiers: modifiers.rawValue),
            virtualKeyCode: event.keyCode
        )
        guard shortcut.isValid else {
            shortcutSettingsStatus.stringValue = "That key cannot be used as a global shortcut."
            return true
        }
        if let message = onShortcutChange?(shortcut) {
            shortcutSettingsStatus.stringValue = message
            return true
        }
        isRecordingShortcut = false
        refreshShortcutSettings()
        return true
    }

    func hideShortcutSettings() {
        shortcutSettingsView.isHidden = true
        isRecordingShortcut = false
    }

    func setRevealProgress(_ revealed: Bool) {
        guard let layer else { return }
        isRevealed = revealed
        revealMask.removeAllAnimations()
        layer.removeAnimation(forKey: "opacity")
        revealMask.path = revealed ? expandedPath : collapsedPath
        layer.opacity = revealed ? 1 : 0
    }

    func animateReveal(_ revealed: Bool, duration: TimeInterval) {
        guard let layer else { return }
        let currentPath = revealMask.presentation()?.path ?? revealMask.path ?? (revealed ? collapsedPath : expandedPath)
        let currentOpacity = layer.presentation()?.opacity ?? layer.opacity
        let targetPath = revealed ? expandedPath : collapsedPath
        let targetOpacity: Float = revealed ? 1 : 0
        isRevealed = revealed

        revealMask.removeAllAnimations()
        layer.removeAnimation(forKey: "opacity")

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        revealMask.path = targetPath
        layer.opacity = targetOpacity
        CATransaction.commit()

        guard duration > 0 else { return }

        let maskAnimation = CABasicAnimation(keyPath: "path")
        maskAnimation.fromValue = currentPath
        maskAnimation.toValue = targetPath
        maskAnimation.duration = duration
        maskAnimation.timingFunction = CAMediaTimingFunction(
            controlPoints: revealed ? 0.18 : 0.4,
            revealed ? 0.88 : 0,
            revealed ? 0.24 : 1,
            1
        )
        revealMask.add(maskAnimation, forKey: "reveal")

        let opacityAnimation = CABasicAnimation(keyPath: "opacity")
        opacityAnimation.fromValue = currentOpacity
        opacityAnimation.toValue = targetOpacity
        opacityAnimation.duration = duration
        opacityAnimation.timingFunction = maskAnimation.timingFunction
        layer.add(opacityAnimation, forKey: "opacity")
    }

    private var expandedPath: CGPath {
        CGPath(
            roundedRect: bounds,
            cornerWidth: 20,
            cornerHeight: 20,
            transform: nil
        )
    }

    private var collapsedPath: CGPath {
        if let collapsedCutoutRect {
            let rect = collapsedCutoutRect.intersection(bounds)
            let target = rect.isEmpty ? fallbackCollapsedRect : rect
            let radius = min(12, target.height / 2)
            return CGPath(roundedRect: target, cornerWidth: radius, cornerHeight: radius, transform: nil)
        }
        return CGPath(
            roundedRect: fallbackCollapsedRect,
            cornerWidth: 14,
            cornerHeight: 14,
            transform: nil
        )
    }

    private var fallbackCollapsedRect: NSRect {
        let width = min(180, bounds.width)
        return NSRect(
            x: bounds.midX - width / 2,
            y: max(bounds.minY, bounds.maxY - Self.bridgeHeight),
            width: width,
            height: min(Self.bridgeHeight, bounds.height)
        )
    }

    private func configureBridge() {
        bridgeView.wantsLayer = true
        bridgeView.layer?.backgroundColor = NSColor.black.cgColor
        addSubview(bridgeView)

        statusIndicator.wantsLayer = true
        statusIndicator.layer?.backgroundColor = NSColor.systemGreen.cgColor
        statusIndicator.layer?.cornerRadius = 3
        bridgeView.addSubview(statusIndicator)

        titleLabel.textColor = .white
        titleLabel.font = .systemFont(ofSize: 12, weight: .semibold)
        statusLabel.textColor = NSColor.white.withAlphaComponent(0.58)
        statusLabel.font = .systemFont(ofSize: 11, weight: .medium)
        bridgeView.addSubview(titleLabel)
        bridgeView.addSubview(statusLabel)

        configureButton(shortcutButton, title: "⇧ ⇧", symbolName: nil, action: #selector(toggleShortcutSettings))
        shortcutButton.font = .monospacedSystemFont(ofSize: 10, weight: .medium)
        shortcutButton.wantsLayer = true
        shortcutButton.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.1).cgColor
        shortcutButton.layer?.cornerRadius = 5
        shortcutButton.setAccessibilityLabel("Change quick terminal shortcut")

        configureButton(settingsButton, title: "", symbolName: "gearshape", action: #selector(openSettings))
        settingsButton.setAccessibilityLabel("Open quick terminal settings")
        configureButton(closeButton, title: "", symbolName: "xmark", action: #selector(close))
        closeButton.setAccessibilityLabel("Close quick terminal")
    }

    private func configureTerminalBackground() {
        terminalBackgroundView.blendingMode = .behindWindow
        terminalBackgroundView.material = .underWindowBackground
        terminalBackgroundView.state = .active
        addSubview(terminalBackgroundView)
    }

    private func configureTerminalTint() {
        terminalTintView.wantsLayer = true
        terminalTintView.layer?.backgroundColor = NSColor.clear.cgColor
        addSubview(terminalTintView)
    }

    private func configureShortcutSettings() {
        shortcutSettingsView.wantsLayer = true
        shortcutSettingsView.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.94).cgColor
        shortcutSettingsView.layer?.borderColor = NSColor.white.withAlphaComponent(0.14).cgColor
        shortcutSettingsView.layer?.borderWidth = 1
        shortcutSettingsView.layer?.cornerRadius = 12
        shortcutSettingsView.layer?.shadowColor = NSColor.black.cgColor
        shortcutSettingsView.layer?.shadowOpacity = 0.4
        shortcutSettingsView.layer?.shadowRadius = 16
        shortcutSettingsView.isHidden = true
        addSubview(shortcutSettingsView)

        shortcutSettingsTitle.textColor = .white
        shortcutSettingsTitle.font = .systemFont(ofSize: 12, weight: .semibold)
        shortcutSettingsStatus.textColor = NSColor.white.withAlphaComponent(0.62)
        shortcutSettingsStatus.font = .systemFont(ofSize: 10.5, weight: .regular)
        shortcutSettingsStatus.maximumNumberOfLines = 2
        shortcutSettingsView.addSubview(shortcutSettingsTitle)
        shortcutSettingsView.addSubview(shortcutSettingsStatus)

        configureSettingsChoice(doubleShiftButton, title: "Double Shift", action: #selector(selectDoubleShift))
        configureSettingsChoice(customShortcutButton, title: "Record Custom…", action: #selector(recordCustomShortcut))
        configureSettingsChoice(
            inputMonitoringButton,
            title: "Enable Input Monitoring",
            action: #selector(requestInputMonitoring)
        )
    }

    private func configureSettingsChoice(_ button: NSButton, title: String, action: Selector) {
        button.title = title
        button.setButtonType(.pushOnPushOff)
        button.target = self
        button.action = action
        button.bezelStyle = .rounded
        button.controlSize = .small
        button.font = .systemFont(ofSize: 11, weight: .medium)
        shortcutSettingsView.addSubview(button)
    }

    private func configureButton(_ button: NSButton, title: String, symbolName: String?, action: Selector) {
        button.title = title
        if let symbolName {
            button.image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil)
        }
        button.target = self
        button.action = action
        button.isBordered = false
        button.contentTintColor = NSColor.white.withAlphaComponent(0.72)
        button.imagePosition = symbolName == nil ? .noImage : .imageOnly
        bridgeView.addSubview(button)
    }

    private func layoutBridgeControls() {
        let centerY = bridgeView.bounds.midY
        statusIndicator.frame = NSRect(x: 14, y: centerY - 3, width: 6, height: 6)
        titleLabel.sizeToFit()
        titleLabel.frame.origin = NSPoint(x: 27, y: centerY - titleLabel.frame.height / 2)
        statusLabel.sizeToFit()
        statusLabel.frame.origin = NSPoint(
            x: titleLabel.frame.maxX + 10,
            y: centerY - statusLabel.frame.height / 2
        )

        let buttonSize = NSSize(width: 28, height: 26)
        closeButton.frame = NSRect(
            x: bridgeView.bounds.maxX - buttonSize.width - 7,
            y: centerY - buttonSize.height / 2,
            width: buttonSize.width,
            height: buttonSize.height
        )
        settingsButton.frame = NSRect(
            x: closeButton.frame.minX - buttonSize.width,
            y: closeButton.frame.minY,
            width: buttonSize.width,
            height: buttonSize.height
        )
        let shortcutWidth = max(44, shortcutButton.intrinsicContentSize.width + 14)
        shortcutButton.frame = NSRect(
            x: settingsButton.frame.minX - shortcutWidth - 4,
            y: centerY - 11,
            width: shortcutWidth,
            height: 22
        )
    }

    private func layoutShortcutSettings() {
        let size = NSSize(width: 272, height: inputMonitoringButton.isHidden ? 142 : 174)
        shortcutSettingsView.frame = NSRect(
            x: max(12, bounds.maxX - size.width - 12),
            y: max(12, bounds.maxY - Self.bridgeHeight - size.height - 8),
            width: size.width,
            height: size.height
        )
        shortcutSettingsTitle.frame = NSRect(x: 14, y: size.height - 32, width: size.width - 28, height: 18)
        shortcutSettingsStatus.frame = NSRect(x: 14, y: size.height - 59, width: size.width - 28, height: 24)
        doubleShiftButton.frame = NSRect(x: 14, y: size.height - 91, width: size.width - 28, height: 26)
        customShortcutButton.frame = NSRect(x: 14, y: size.height - 123, width: size.width - 28, height: 26)
        inputMonitoringButton.frame = NSRect(x: 14, y: 14, width: size.width - 28, height: 26)
    }

    private func refreshShortcutSettings() {
        guard let snapshot = shortcutSettingsProvider?() else { return }
        setShortcutLabel(snapshot.shortcut.displayString)
        doubleShiftButton.state = snapshot.shortcut == .doubleShift ? .on : .off
        if case let .keyCombo(combo, _) = snapshot.shortcut {
            customShortcutButton.title = combo.displayString
            customShortcutButton.state = .on
        } else {
            customShortcutButton.title = isRecordingShortcut ? "Press shortcut…" : "Record Custom…"
            customShortcutButton.state = .off
        }
        if isRecordingShortcut {
            customShortcutButton.title = "Press shortcut…"
            shortcutSettingsStatus.stringValue = "Press a global shortcut, or Escape to cancel."
        } else if let errorMessage = snapshot.errorMessage {
            shortcutSettingsStatus.stringValue = errorMessage
        } else {
            shortcutSettingsStatus.stringValue = snapshot.statusText
        }
        inputMonitoringButton.isHidden = !snapshot.needsInputMonitoringAccess
        needsLayout = true
    }

    @objc
    private func close() {
        onClose?()
    }

    @objc
    private func openSettings() {
        onOpenSettings?()
    }

    @objc
    private func toggleShortcutSettings() {
        shortcutSettingsView.isHidden.toggle()
        isRecordingShortcut = false
        refreshShortcutSettings()
    }

    @objc
    private func selectDoubleShift() {
        if let message = onShortcutChange?(.doubleShift) {
            shortcutSettingsStatus.stringValue = message
            return
        }
        isRecordingShortcut = false
        refreshShortcutSettings()
    }

    @objc
    private func recordCustomShortcut() {
        isRecordingShortcut = true
        refreshShortcutSettings()
    }

    @objc
    private func requestInputMonitoring() {
        _ = onRequestInputMonitoringAccess?()
        refreshShortcutSettings()
    }
}

struct QuickTerminalShortcutSettingsSnapshot {
    let shortcut: QuickTerminalShortcut
    let monitoringState: QuickTerminalShortcutMonitoringState
    let errorMessage: String?

    var needsInputMonitoringAccess: Bool {
        shortcut == .doubleShift && monitoringState != .systemWide
    }

    var statusText: String {
        switch monitoringState {
        case .systemWide,
             .carbonHotKey:
            "Active system-wide"
        case .localOnly:
            "Active in Muxy. Input Monitoring is needed system-wide."
        case .stopped:
            "Shortcut listener is inactive."
        }
    }
}
