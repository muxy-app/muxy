import Foundation
import Testing

@testable import Muxy

@Suite("Terminal pane surface background")
struct TerminalPaneGhosttyConfigTests {
    @Test("a client theme keeps the surface untouched")
    func clientThemeWins() {
        let background = TerminalPaneSurfaceBackground.resolve(
            hasClientTheme: true,
            appearance: BackgroundAppearance(transparency: 30, blurIntensity: 70)
        )

        #expect(background == .clientThemed)
    }

    @Test("transparency above zero makes the surface transparent")
    func transparentSurface() {
        let background = TerminalPaneSurfaceBackground.resolve(
            hasClientTheme: false,
            appearance: BackgroundAppearance(transparency: 1, blurIntensity: 0)
        )

        #expect(background == .transparent)
    }

    @Test("zero transparency keeps the surface opaque")
    func opaqueSurface() {
        let background = TerminalPaneSurfaceBackground.resolve(
            hasClientTheme: false,
            appearance: BackgroundAppearance(transparency: 0, blurIntensity: 100)
        )

        #expect(background == .opaque)
    }

    @Test("Reduce Transparency resolves to an opaque surface")
    func reduceTransparencyResolvesOpaque() {
        let appearance = BackgroundAppearance(transparency: 40, blurIntensity: 80)
            .resolvingReduceTransparency(true)

        let background = TerminalPaneSurfaceBackground.resolve(
            hasClientTheme: false,
            appearance: appearance
        )

        #expect(background == .opaque)
    }
}
