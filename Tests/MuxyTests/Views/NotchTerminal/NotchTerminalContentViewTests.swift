import AppKit
import Testing

@testable import Muxy

@MainActor
@Suite("Notch terminal content view")
struct NotchTerminalContentViewTests {
    @Test("composes native material, theme tint, terminal, and solid bridge")
    func glassComposition() throws {
        let contentView = NotchTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        let surface = NotchTerminalContentTestSurface()

        contentView.attach(surface: surface)
        contentView.layout()

        let glassView = try #require(contentView.subviews.compactMap { $0 as? NSVisualEffectView }.first)
        let bridgeView = try #require(contentView.subviews.first { view in
            view.frame.height == NotchTerminalContentView.bridgeHeight && view.frame.maxY == contentView.bounds.maxY
        })
        let glassIndex = try #require(contentView.subviews.firstIndex(of: glassView))
        let terminalIndex = try #require(contentView.subviews.firstIndex(of: surface.notchTerminalView))
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
        let contentView = NotchTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        let surface = NotchTerminalContentTestSurface()
        contentView.attach(surface: surface)
        contentView.layout()
        let glassView = try #require(contentView.subviews.compactMap { $0 as? NSVisualEffectView }.first)
        let glassIndex = try #require(contentView.subviews.firstIndex(of: glassView))
        let tintView = contentView.subviews[glassIndex + 1]

        contentView.applyAppearance(NotchTerminalAppearance(transparency: 24, blurIntensity: 35))

        #expect(!glassView.isHidden)
        #expect(glassView.alphaValue == 1)
        let mask = try #require(glassView.maskImage)
        #expect(mask.size == NSSize(width: 1, height: 1))
        #expect(mask.resizingMode == .stretch)
        let tintColor = try #require(tintView.layer?.backgroundColor.flatMap(NSColor.init(cgColor:)))
        #expect(abs(tintColor.alphaComponent - 0.76) < 0.000_1)
        #expect(colorsMatch(tintColor, MuxyTheme.nsBg.withAlphaComponent(0.76)))

        contentView.applyAppearance(NotchTerminalAppearance(transparency: 24, blurIntensity: 100))

        #expect(!glassView.isHidden)
        #expect(glassView.alphaValue == 1)
        #expect(glassView.maskImage == nil)

        contentView.applyAppearance(NotchTerminalAppearance(transparency: 24, blurIntensity: 0))

        #expect(glassView.isHidden)
        #expect(glassView.alphaValue == 1)
        #expect(glassView.maskImage == nil)
    }

    @Test("accessibility appearance is opaque and unblurred")
    func accessibilityAppearance() throws {
        let contentView = NotchTerminalContentView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
        let glassView = try #require(contentView.subviews.compactMap { $0 as? NSVisualEffectView }.first)
        let glassIndex = try #require(contentView.subviews.firstIndex(of: glassView))
        let tintView = contentView.subviews[glassIndex + 1]

        contentView.applyAppearance(
            NotchTerminalAppearance(transparency: 24, blurIntensity: 100)
                .resolvingReduceTransparency(true)
        )

        #expect(glassView.isHidden)
        let tintColor = try #require(tintView.layer?.backgroundColor.flatMap(NSColor.init(cgColor:)))
        #expect(tintColor.alphaComponent == 1)
        #expect(colorsMatch(tintColor, MuxyTheme.nsBg))
    }

    @Test("material masks reserve endpoints for off and full intensity")
    func materialMaskEndpoints() {
        #expect(NotchTerminalMaterialMask.image(opacity: 0) == nil)
        #expect(NotchTerminalMaterialMask.image(opacity: 1) == nil)
        #expect(NotchTerminalMaterialMask.image(opacity: 0.01) != nil)
        #expect(NotchTerminalMaterialMask.image(opacity: 0.99) != nil)
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
private final class NotchTerminalContentTestSurface: NotchTerminalSurface {
    let notchTerminalView = NSView()
    var onProcessExit: (() -> Void)?

    func applyAppearance(_: NotchTerminalAppearance) {}
    func setVisible(_: Bool) {}
    func setFocused(_: Bool) {}
    func notifySurfaceUnfocused() {}
    func tearDown() {}
}
