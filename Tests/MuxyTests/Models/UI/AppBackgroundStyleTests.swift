import Testing

@testable import Muxy

@Suite("AppBackgroundStyle")
struct AppBackgroundStyleTests {
    @Test("resolves missing and invalid values to the default style")
    func resolvesFallbackStyle() {
        #expect(AppBackgroundStyle.resolve(nil) == .vibrant)
        #expect(AppBackgroundStyle.resolve("unknown") == .vibrant)
        #expect(AppBackgroundStyle.resolve("solid") == .solid)
    }

    @Test("uses vibrancy only when display conditions support it")
    func resolvesEffectiveVibrancy() {
        #expect(AppSidebarVibrancyPolicy.isActive(
            style: .vibrant,
            reduceTransparency: false,
            increaseContrast: false,
            isFullScreen: false
        ))
        #expect(!AppSidebarVibrancyPolicy.isActive(
            style: .solid,
            reduceTransparency: false,
            increaseContrast: false,
            isFullScreen: false
        ))
        #expect(!AppSidebarVibrancyPolicy.isActive(
            style: .vibrant,
            reduceTransparency: true,
            increaseContrast: false,
            isFullScreen: false
        ))
        #expect(!AppSidebarVibrancyPolicy.isActive(
            style: .vibrant,
            reduceTransparency: false,
            increaseContrast: true,
            isFullScreen: false
        ))
        #expect(!AppSidebarVibrancyPolicy.isActive(
            style: .vibrant,
            reduceTransparency: false,
            increaseContrast: false,
            isFullScreen: true
        ))
    }
}
