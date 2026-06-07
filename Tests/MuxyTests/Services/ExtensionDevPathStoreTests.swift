import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionDevPathStore")
struct ExtensionDevPathStoreTests {
    @Test("adds and lists paths in insertion order")
    func addsAndLists() {
        let defaults = makeDefaults()
        ExtensionDevPathStore.add("/tmp/a", defaults: defaults)
        ExtensionDevPathStore.add("/tmp/b", defaults: defaults)

        #expect(ExtensionDevPathStore.paths(defaults: defaults) == ["/tmp/a", "/tmp/b"])
    }

    @Test("ignores duplicate paths")
    func ignoresDuplicates() {
        let defaults = makeDefaults()
        ExtensionDevPathStore.add("/tmp/a", defaults: defaults)
        ExtensionDevPathStore.add("/tmp/a/", defaults: defaults)

        #expect(ExtensionDevPathStore.paths(defaults: defaults) == ["/tmp/a"])
    }

    @Test("removes a path")
    func removes() {
        let defaults = makeDefaults()
        ExtensionDevPathStore.add("/tmp/a", defaults: defaults)
        ExtensionDevPathStore.add("/tmp/b", defaults: defaults)
        ExtensionDevPathStore.remove("/tmp/a", defaults: defaults)

        #expect(ExtensionDevPathStore.paths(defaults: defaults) == ["/tmp/b"])
    }

    private func makeDefaults() -> UserDefaults {
        let defaults = UserDefaults(suiteName: "dev-paths-\(UUID().uuidString)")!
        defaults.removePersistentDomain(forName: "dev-paths")
        return defaults
    }
}
