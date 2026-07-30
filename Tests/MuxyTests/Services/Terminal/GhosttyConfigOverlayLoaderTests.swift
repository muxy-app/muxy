import GhosttyKit
import Testing

@testable import Muxy

@MainActor
@Suite("Ghostty config overlay loader")
struct GhosttyConfigOverlayLoaderTests {
    @Test("clones the loaded config and applies the override without finalizing")
    func loadOrder() throws {
        let base = try #require(ghostty_config_t(bitPattern: 1))
        let clone = try #require(ghostty_config_t(bitPattern: 2))
        var events: [String] = []
        let loader = GhosttyConfigOverlayLoader(
            clone: { clonedConfig in
                #expect(clonedConfig == base)
                events.append("clone")
                return clone
            },
            loadFile: { loadedConfig, file in
                #expect(loadedConfig == clone)
                events.append("load:\(file)")
            }
        )

        let result = loader.load(base: base, overridesFilePath: "quick.conf")

        #expect(result == clone)
        #expect(events == ["clone", "load:quick.conf"])
    }

    @Test("does not load the override when cloning fails")
    func cloneFailure() throws {
        let base = try #require(ghostty_config_t(bitPattern: 1))
        var didLoad = false
        let loader = GhosttyConfigOverlayLoader(
            clone: { _ in nil },
            loadFile: { _, _ in didLoad = true }
        )

        #expect(loader.load(base: base, overridesFilePath: "quick.conf") == nil)
        #expect(!didLoad)
    }
}
