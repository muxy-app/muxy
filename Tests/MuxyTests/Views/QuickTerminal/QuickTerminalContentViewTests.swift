import AppKit
import Testing

@testable import Muxy

@MainActor
@Suite("Quick terminal content view")
struct QuickTerminalContentViewTests {
    @Test("composes native material, theme tint, terminal, and solid bridge")
    func glassComposition() throws {
        let contentView = QuickTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        let surface = QuickTerminalContentTestSurface()

        contentView.attach(surface: surface)
        contentView.layout()

        let glassView = try #require(contentView.subviews.compactMap { $0 as? NSVisualEffectView }.first)
        let bridgeView = try #require(contentView.subviews.first { view in
            view.frame.height == QuickTerminalContentView.bridgeHeight && view.frame.maxY == contentView.bounds.maxY
        })
        let glassIndex = try #require(contentView.subviews.firstIndex(of: glassView))
        let terminalIndex = try #require(contentView.subviews.firstIndex(of: surface.quickTerminalView))
        let bridgeIndex = try #require(contentView.subviews.firstIndex(of: bridgeView))
        let tintIndex = glassIndex + 1
        let tintView = contentView.subviews[tintIndex]

        #expect(contentView.layer?.backgroundColor?.alpha == 0)
        #expect(bridgeView.layer?.backgroundColor?.alpha == 1)
        #expect(glassView.blendingMode == .behindWindow)
        #expect(glassView.material == .underWindowBackground)
        #expect(glassView.state == .active)
        #expect(glassView.alphaValue == 1)
        #expect(tintView.frame == glassView.frame)
        #expect(glassIndex < tintIndex)
        #expect(tintIndex < terminalIndex)
        #expect(terminalIndex < bridgeIndex)
    }

    @Test("updates continuous material intensity without fading the effect view")
    func glassAppearance() throws {
        let contentView = QuickTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        let surface = QuickTerminalContentTestSurface()
        contentView.attach(surface: surface)
        contentView.layout()
        let glassView = try #require(contentView.subviews.compactMap { $0 as? NSVisualEffectView }.first)
        let glassIndex = try #require(contentView.subviews.firstIndex(of: glassView))
        let tintView = contentView.subviews[glassIndex + 1]

        contentView.applyAppearance(QuickTerminalAppearance(transparency: 24, blurIntensity: 35))

        #expect(!glassView.isHidden)
        #expect(glassView.alphaValue == 1)
        let mask = try #require(glassView.maskImage)
        #expect(mask.size == NSSize(width: 1, height: 1))
        #expect(mask.resizingMode == .stretch)
        let tintColor = try #require(tintView.layer?.backgroundColor.flatMap(NSColor.init(cgColor:)))
        #expect(abs(tintColor.alphaComponent - 0.76) < 0.000_1)
        #expect(colorsMatch(tintColor, MuxyTheme.nsBg.withAlphaComponent(0.76)))

        contentView.applyAppearance(QuickTerminalAppearance(transparency: 24, blurIntensity: 100))

        #expect(!glassView.isHidden)
        #expect(glassView.alphaValue == 1)
        #expect(glassView.maskImage == nil)

        contentView.applyAppearance(QuickTerminalAppearance(transparency: 24, blurIntensity: 0))

        #expect(glassView.isHidden)
        #expect(glassView.alphaValue == 1)
        #expect(glassView.maskImage == nil)
    }

    @Test("accessibility appearance is opaque and unblurred")
    func accessibilityAppearance() throws {
        let contentView = QuickTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        let glassView = try #require(contentView.subviews.compactMap { $0 as? NSVisualEffectView }.first)
        let glassIndex = try #require(contentView.subviews.firstIndex(of: glassView))
        let tintView = contentView.subviews[glassIndex + 1]

        contentView.applyAppearance(
            QuickTerminalAppearance(transparency: 24, blurIntensity: 100)
                .resolvingReduceTransparency(true)
        )

        #expect(glassView.isHidden)
        let tintColor = try #require(tintView.layer?.backgroundColor.flatMap(NSColor.init(cgColor:)))
        #expect(tintColor.alphaComponent == 1)
        #expect(colorsMatch(tintColor, MuxyTheme.nsBg))
    }

    @Test("material masks reserve endpoints for off and full intensity")
    func materialMaskEndpoints() {
        #expect(QuickTerminalMaterialMask.image(opacity: 0) == nil)
        #expect(QuickTerminalMaterialMask.image(opacity: 1) == nil)
        #expect(QuickTerminalMaterialMask.image(opacity: 0.01) != nil)
        #expect(QuickTerminalMaterialMask.image(opacity: 0.99) != nil)
    }

    @Test("gear toggles the settings popover and hides the shortcut popover")
    func settingsPopoverIsMutuallyExclusive() throws {
        let contentView = QuickTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        contentView.quickSettingsProvider = {
            QuickTerminalQuickSettings(transparency: 20, blurIntensity: 60, width: 800, height: 500)
        }
        contentView.layout()
        let settingsPopover = try #require(popover("quickTerminalSettingsPopover", in: contentView))
        let shortcutPopover = try #require(popover("quickTerminalShortcutPopover", in: contentView))

        #expect(settingsPopover.isHidden)

        contentView.toggleSettingsPopover()
        #expect(!settingsPopover.isHidden)
        #expect(shortcutPopover.isHidden)

        contentView.toggleSettingsPopover()
        #expect(settingsPopover.isHidden)
    }

    @Test("dismissing overlays hides the settings popover")
    func hidesSettingsPopoverOnDismiss() throws {
        let contentView = QuickTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        contentView.quickSettingsProvider = {
            QuickTerminalQuickSettings(transparency: 20, blurIntensity: 60, width: 800, height: 500)
        }
        let settingsPopover = try #require(popover("quickTerminalSettingsPopover", in: contentView))

        contentView.toggleSettingsPopover()
        #expect(!settingsPopover.isHidden)

        contentView.hideConfigurationOverlays()
        #expect(settingsPopover.isHidden)
    }

    @Test("reset restores default appearance and size through the callbacks")
    func resetAppliesDefaults() {
        let contentView = QuickTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        var appearance: (transparency: Int, blurIntensity: Int)?
        var size: (width: Int, height: Int)?
        contentView.onAppearanceSettingsChange = { appearance = ($0, $1) }
        contentView.onSizeSettingsChange = { size = ($0, $1) }

        contentView.resetSettingsPopover()

        #expect(appearance?.transparency == QuickTerminalAppearancePreferences.defaultTransparency)
        #expect(appearance?.blurIntensity == QuickTerminalAppearancePreferences.defaultBlurIntensity)
        #expect(size?.width == QuickTerminalSizePreferences.defaultWidth)
        #expect(size?.height == QuickTerminalSizePreferences.defaultHeight)
    }

    private func popover(_ identifier: String, in view: NSView) -> NSView? {
        view.subviews.first { $0.accessibilityIdentifier() == identifier }
    }

    private func colorsMatch(_ lhs: NSColor, _ rhs: NSColor) -> Bool {
        guard let left = lhs.usingColorSpace(.sRGB),
              let right = rhs.usingColorSpace(.sRGB)
        else { return false }
        return abs(left.redComponent - right.redComponent) < 0.000_1
            && abs(left.greenComponent - right.greenComponent) < 0.000_1
            && abs(left.blueComponent - right.blueComponent) < 0.000_1
            && abs(left.alphaComponent - right.alphaComponent) < 0.000_1
    }
}

@MainActor
private final class QuickTerminalContentTestSurface: QuickTerminalSurface {
    let quickTerminalView = NSView()
    var onProcessExit: (() -> Void)?

    func applyQuickTerminalConfiguration() {}
    func setVisible(_: Bool) {}
    func setFocused(_: Bool) {}
    func notifySurfaceUnfocused() {}
    func tearDown() {}
}
