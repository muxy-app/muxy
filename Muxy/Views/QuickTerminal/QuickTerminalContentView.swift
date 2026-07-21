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
    var quickSettingsProvider: (() -> QuickTerminalQuickSettings)?
    var onAppearanceSettingsChange: ((_ transparency: Int, _ blurIntensity: Int) -> Void)?
    var onSizeSettingsChange: ((_ width: Int, _ height: Int) -> Void)?

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
    private let settingsPopover = NSView()
    private let settingsPopoverTitle = NSTextField(labelWithString: "Quick Terminal")
    private let transparencyTitle = NSTextField(labelWithString: "Transparency")
    private let transparencySlider = NSSlider()
    private let transparencyValue = NSTextField(labelWithString: "")
    private let vibrancyTitle = NSTextField(labelWithString: "Vibrancy")
    private let vibrancySlider = NSSlider()
    private let vibrancyValue = NSTextField(labelWithString: "")
    private let widthTitle = NSTextField(labelWithString: "Width")
    private let widthSlider = NSSlider()
    private let widthValue = NSTextField(labelWithString: "")
    private let heightTitle = NSTextField(labelWithString: "Height")
    private let heightSlider = NSSlider()
    private let heightValue = NSTextField(labelWithString: "")
    private let settingsResetButton = NSButton()
    private let openFullSettingsButton = NSButton()
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
        configureSettingsPopover()
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
        layoutSettingsPopover()
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
        settingsPopover.layer?.backgroundColor = NSColor.black
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
        if !settingsPopover.isHidden, event.keyCode == 53 {
            settingsPopover.isHidden = true
            return true
        }
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

    func hideConfigurationOverlays() {
        shortcutSettingsView.isHidden = true
        settingsPopover.isHidden = true
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

        configureButton(settingsButton, title: "", symbolName: "gearshape", action: #selector(toggleSettingsPopover))
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
        shortcutSettingsView.setAccessibilityIdentifier("quickTerminalShortcutPopover")
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

    private func configureSettingsPopover() {
        settingsPopover.wantsLayer = true
        settingsPopover.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.94).cgColor
        settingsPopover.layer?.borderColor = NSColor.white.withAlphaComponent(0.14).cgColor
        settingsPopover.layer?.borderWidth = 1
        settingsPopover.layer?.cornerRadius = 12
        settingsPopover.layer?.shadowColor = NSColor.black.cgColor
        settingsPopover.layer?.shadowOpacity = 0.4
        settingsPopover.layer?.shadowRadius = 16
        settingsPopover.isHidden = true
        settingsPopover.setAccessibilityIdentifier("quickTerminalSettingsPopover")
        addSubview(settingsPopover)

        settingsPopoverTitle.textColor = .white
        settingsPopoverTitle.font = .systemFont(ofSize: 12, weight: .semibold)
        settingsPopover.addSubview(settingsPopoverTitle)

        configureSettingsRow(title: transparencyTitle, value: transparencyValue)
        configureSettingsRow(title: vibrancyTitle, value: vibrancyValue)
        configureSettingsRow(title: widthTitle, value: widthValue)
        configureSettingsRow(title: heightTitle, value: heightValue)

        configureSettingsSlider(
            transparencySlider,
            range: QuickTerminalAppearancePreferences.transparencyRange,
            continuous: true,
            action: #selector(appearanceSlidersChanged)
        )
        configureSettingsSlider(
            vibrancySlider,
            range: QuickTerminalAppearancePreferences.blurIntensityRange,
            continuous: true,
            action: #selector(appearanceSlidersChanged)
        )
        configureSettingsSlider(
            widthSlider,
            range: QuickTerminalSizePreferences.widthRange,
            continuous: true,
            action: #selector(sizeSlidersChanged)
        )
        configureSettingsSlider(
            heightSlider,
            range: QuickTerminalSizePreferences.heightRange,
            continuous: true,
            action: #selector(sizeSlidersChanged)
        )

        configureSettingsPopoverButton(settingsResetButton, title: "Reset", action: #selector(resetSettingsPopover))
        configureSettingsPopoverButton(openFullSettingsButton, title: "Open Settings…", action: #selector(openFullSettings))
    }

    private func configureSettingsRow(title: NSTextField, value: NSTextField) {
        title.textColor = NSColor.white.withAlphaComponent(0.72)
        title.font = .systemFont(ofSize: 11, weight: .medium)
        value.textColor = NSColor.white.withAlphaComponent(0.55)
        value.font = .monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        value.alignment = .right
        settingsPopover.addSubview(title)
        settingsPopover.addSubview(value)
    }

    private func configureSettingsSlider(
        _ slider: NSSlider,
        range: ClosedRange<Int>,
        continuous: Bool,
        action: Selector
    ) {
        slider.minValue = Double(range.lowerBound)
        slider.maxValue = Double(range.upperBound)
        slider.isContinuous = continuous
        slider.controlSize = .small
        slider.target = self
        slider.action = action
        settingsPopover.addSubview(slider)
    }

    private func configureSettingsPopoverButton(_ button: NSButton, title: String, action: Selector) {
        button.title = title
        button.bezelStyle = .rounded
        button.controlSize = .small
        button.font = .systemFont(ofSize: 11, weight: .medium)
        button.target = self
        button.action = action
        settingsPopover.addSubview(button)
    }

    private func layoutSettingsPopover() {
        let size = NSSize(width: 300, height: 244)
        settingsPopover.frame = NSRect(
            x: max(12, bounds.maxX - size.width - 12),
            y: max(12, bounds.maxY - Self.bridgeHeight - size.height - 8),
            width: size.width,
            height: size.height
        )
        settingsPopoverTitle.frame = NSRect(x: 16, y: size.height - 32, width: size.width - 32, height: 18)
        layoutSettingsRow(title: transparencyTitle, slider: transparencySlider, value: transparencyValue, sliderY: size.height - 66)
        layoutSettingsRow(title: vibrancyTitle, slider: vibrancySlider, value: vibrancyValue, sliderY: size.height - 98)
        layoutSettingsRow(title: widthTitle, slider: widthSlider, value: widthValue, sliderY: size.height - 130)
        layoutSettingsRow(title: heightTitle, slider: heightSlider, value: heightValue, sliderY: size.height - 162)
        settingsResetButton.frame = NSRect(x: 16, y: 14, width: 72, height: 24)
        let openWidth: CGFloat = 132
        openFullSettingsButton.frame = NSRect(x: size.width - openWidth - 16, y: 14, width: openWidth, height: 24)
    }

    private func layoutSettingsRow(title: NSTextField, slider: NSSlider, value: NSTextField, sliderY: CGFloat) {
        let width = settingsPopover.frame.width
        slider.frame = NSRect(x: 98, y: sliderY, width: width - 98 - 64, height: 20)
        title.frame = NSRect(x: 16, y: sliderY + 2, width: 78, height: 16)
        value.frame = NSRect(x: width - 16 - 42, y: sliderY + 2, width: 42, height: 16)
    }

    private func refreshSettingsPopover() {
        guard let snapshot = quickSettingsProvider?() else { return }
        transparencySlider.integerValue = snapshot.transparency
        vibrancySlider.integerValue = snapshot.blurIntensity
        widthSlider.integerValue = snapshot.width
        heightSlider.integerValue = snapshot.height
        updateSettingsValueLabels()
    }

    private func updateSettingsValueLabels() {
        transparencyValue.stringValue = "\(transparencySlider.integerValue)%"
        vibrancyValue.stringValue = "\(vibrancySlider.integerValue)%"
        widthValue.stringValue = "\(widthSlider.integerValue)"
        heightValue.stringValue = "\(heightSlider.integerValue)"
    }

    @objc
    private func appearanceSlidersChanged() {
        updateSettingsValueLabels()
        onAppearanceSettingsChange?(transparencySlider.integerValue, vibrancySlider.integerValue)
    }

    @objc
    private func sizeSlidersChanged() {
        updateSettingsValueLabels()
        guard NSApp.currentEvent?.type == .leftMouseUp else { return }
        onSizeSettingsChange?(widthSlider.integerValue, heightSlider.integerValue)
    }

    @objc
    func resetSettingsPopover() {
        transparencySlider.integerValue = QuickTerminalAppearancePreferences.defaultTransparency
        vibrancySlider.integerValue = QuickTerminalAppearancePreferences.defaultBlurIntensity
        widthSlider.integerValue = QuickTerminalSizePreferences.defaultWidth
        heightSlider.integerValue = QuickTerminalSizePreferences.defaultHeight
        updateSettingsValueLabels()
        onAppearanceSettingsChange?(transparencySlider.integerValue, vibrancySlider.integerValue)
        onSizeSettingsChange?(widthSlider.integerValue, heightSlider.integerValue)
    }

    @objc
    private func close() {
        onClose?()
    }

    @objc
    private func openFullSettings() {
        onOpenSettings?()
    }

    @objc
    private func toggleShortcutSettings() {
        settingsPopover.isHidden = true
        shortcutSettingsView.isHidden.toggle()
        isRecordingShortcut = false
        refreshShortcutSettings()
    }

    @objc
    func toggleSettingsPopover() {
        if settingsPopover.isHidden {
            shortcutSettingsView.isHidden = true
            isRecordingShortcut = false
            refreshSettingsPopover()
            settingsPopover.isHidden = false
        } else {
            settingsPopover.isHidden = true
        }
        needsLayout = true
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

struct QuickTerminalQuickSettings {
    let transparency: Int
    let blurIntensity: Int
    let width: Int
    let height: Int
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
