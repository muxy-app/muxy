import Foundation
import Testing

@testable import Muxy

@Suite("Extension storage service")
@MainActor
struct ExtensionStorageServiceTests {
    private func uniqueID() -> String {
        "test-storage-\(UUID().uuidString)"
    }

    @Test("storage verbs are gated by storage permissions")
    func verbsAreGated() {
        let verbs = MuxyAPI.Permissions.verbNames
        #expect(verbs.contains("storage.get"))
        #expect(verbs.contains("storage.set"))
        #expect(verbs.contains("storage.delete"))
        #expect(verbs.contains("storage.keys"))
        #expect(MuxyAPI.Permissions.required(for: "storage.get") == .storageRead)
        #expect(MuxyAPI.Permissions.required(for: "storage.keys") == .storageRead)
        #expect(MuxyAPI.Permissions.required(for: "storage.set") == .storageWrite)
        #expect(MuxyAPI.Permissions.required(for: "storage.delete") == .storageWrite)
    }

    @Test("set then get round-trips a value")
    func roundTrip() throws {
        let id = uniqueID()
        defer { try? ExtensionStorageService.delete(extensionID: id, key: "k") }
        try ExtensionStorageService.set(extensionID: id, key: "k", value: ["a": 1, "b": "two"])
        let value = try ExtensionStorageService.get(extensionID: id, key: "k") as? [String: Any]
        #expect(value?["a"] as? Int == 1)
        #expect(value?["b"] as? String == "two")
    }

    @Test("get returns NSNull for a missing key")
    func missingKey() throws {
        let value = try ExtensionStorageService.get(extensionID: uniqueID(), key: "absent")
        #expect(value is NSNull)
    }

    @Test("keys lists stored keys and delete removes them")
    func keysAndDelete() throws {
        let id = uniqueID()
        defer {
            try? ExtensionStorageService.delete(extensionID: id, key: "one")
            try? ExtensionStorageService.delete(extensionID: id, key: "two")
        }
        try ExtensionStorageService.set(extensionID: id, key: "one", value: 1)
        try ExtensionStorageService.set(extensionID: id, key: "two", value: 2)
        #expect(try ExtensionStorageService.keys(extensionID: id) == ["one", "two"])

        try ExtensionStorageService.delete(extensionID: id, key: "one")
        #expect(try ExtensionStorageService.keys(extensionID: id) == ["two"])
    }

    @Test("storage is isolated per extension id")
    func isolation() throws {
        let a = uniqueID()
        let b = uniqueID()
        defer {
            try? ExtensionStorageService.delete(extensionID: a, key: "k")
        }
        try ExtensionStorageService.set(extensionID: a, key: "k", value: "secret")
        #expect(try ExtensionStorageService.get(extensionID: b, key: "k") is NSNull)
        #expect(try ExtensionStorageService.keys(extensionID: b).isEmpty)
    }

    @Test("an empty key is rejected")
    func rejectsEmptyKey() {
        #expect(throws: APIError.self) {
            try ExtensionStorageService.set(extensionID: uniqueID(), key: "", value: 1)
        }
    }

    @Test("an oversized value is rejected")
    func rejectsOversizedValue() {
        let big = String(repeating: "x", count: ExtensionStorageService.maxValueBytes + 1)
        #expect(throws: APIError.self) {
            try ExtensionStorageService.set(extensionID: uniqueID(), key: "k", value: big)
        }
    }
}
