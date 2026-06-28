import Foundation
import Testing

@testable import Muxy

@Suite("CodableFileStore")
struct CodableFileStoreTests {
    private struct Fixture: Codable, Equatable {
        let name: String
        let count: Int
    }

    private func tempURL() -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("CodableFileStoreTests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("value.json")
    }

    @Test("load returns nil when file does not exist")
    func loadMissingReturnsNil() throws {
        let store = CodableFileStore<Fixture>(fileURL: tempURL())
        #expect(try store.load() == nil)
    }

    @Test("save then load round-trips value")
    func roundTrip() throws {
        let url = tempURL()
        let store = CodableFileStore<Fixture>(fileURL: url)
        let original = Fixture(name: "thing", count: 7)
        try store.save(original)
        #expect(try store.load() == original)
    }

    @Test("save overwrites existing file atomically")
    func saveOverwrites() throws {
        let url = tempURL()
        let store = CodableFileStore<Fixture>(fileURL: url)
        try store.save(Fixture(name: "first", count: 1))
        try store.save(Fixture(name: "second", count: 2))
        #expect(try store.load() == Fixture(name: "second", count: 2))
    }

    @Test("pretty-printed option produces multi-line JSON")
    func prettyPrinted() throws {
        let url = tempURL()
        let store = CodableFileStore<Fixture>(fileURL: url, options: .pretty)
        try store.save(Fixture(name: "a", count: 1))
        let text = try String(contentsOf: url, encoding: .utf8)
        #expect(text.contains("\n"))
    }

    @Test("sortedKeys option orders keys alphabetically")
    func sortedKeys() throws {
        let url = tempURL()
        let store = CodableFileStore<Fixture>(fileURL: url, options: .prettySorted)
        try store.save(Fixture(name: "a", count: 1))
        let text = try String(contentsOf: url, encoding: .utf8)
        let countIndex = text.range(of: "\"count\"")?.lowerBound
        let nameIndex = text.range(of: "\"name\"")?.lowerBound
        #expect(countIndex != nil && nameIndex != nil)
        if let countIndex, let nameIndex {
            #expect(countIndex < nameIndex)
        }
    }

    @Test("filePermissions option applies to saved file")
    func filePermissions() throws {
        let url = tempURL()
        let store = CodableFileStore<Fixture>(
            fileURL: url,
            options: CodableFileStoreOptions(filePermissions: 0o600)
        )
        try store.save(Fixture(name: "a", count: 1))
        let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
        let perms = attrs[.posixPermissions] as? NSNumber
        #expect(perms?.intValue == 0o600)
    }

    @Test("remove deletes existing file")
    func removeExisting() throws {
        let url = tempURL()
        let store = CodableFileStore<Fixture>(fileURL: url)
        try store.save(Fixture(name: "a", count: 1))
        #expect(FileManager.default.fileExists(atPath: url.path))
        try store.remove()
        #expect(!FileManager.default.fileExists(atPath: url.path))
    }

    @Test("remove on missing file is a no-op")
    func removeMissing() throws {
        let store = CodableFileStore<Fixture>(fileURL: tempURL())
        try store.remove()
    }

    @Test("load throws on corrupt JSON")
    func loadCorrupt() throws {
        let url = tempURL()
        try "not json".data(using: .utf8)!.write(to: url)
        let store = CodableFileStore<Fixture>(fileURL: url)
        #expect(throws: (any Error).self) {
            _ = try store.load()
        }
    }
}
