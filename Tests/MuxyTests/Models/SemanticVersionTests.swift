import Testing

@testable import Muxy

@Suite("SemanticVersion")
struct SemanticVersionTests {
    @Test("orders by numeric component")
    func ordersByComponent() {
        #expect(SemanticVersion("1.2.0")! < SemanticVersion("1.10.0")!)
        #expect(SemanticVersion("2.0.0")! > SemanticVersion("1.9.9")!)
    }

    @Test("treats missing components as zero")
    func paddingShorterVersion() {
        #expect(SemanticVersion("1.2")! < SemanticVersion("1.2.1")!)
        #expect(SemanticVersion("1.2")! == SemanticVersion("1.2.0")!)
    }

    @Test("ignores prerelease and build metadata")
    func stripsSuffixes() {
        #expect(SemanticVersion("1.4.2-beta.1")!.components == [1, 4, 2])
        #expect(SemanticVersion("1.4.2+build9")!.components == [1, 4, 2])
    }

    @Test("rejects malformed versions")
    func rejectsMalformed() {
        #expect(SemanticVersion("") == nil)
        #expect(SemanticVersion("abc") == nil)
        #expect(SemanticVersion("1.x.0") == nil)
    }

    @Test("isUpdate only when available exceeds installed")
    func updateDetection() {
        #expect(SemanticVersion.isUpdate(installed: "1.0.0", available: "1.0.1"))
        #expect(!SemanticVersion.isUpdate(installed: "1.0.1", available: "1.0.1"))
        #expect(!SemanticVersion.isUpdate(installed: "2.0.0", available: "1.9.9"))
        #expect(!SemanticVersion.isUpdate(installed: "1.0.0", available: "garbage"))
    }
}
