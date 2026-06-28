import Foundation

struct CodableFileStoreOptions {
    var prettyPrinted: Bool = false
    var sortedKeys: Bool = false
    var filePermissions: Int?

    static let standard = Self()
    static let pretty = Self(prettyPrinted: true)
    static let prettySorted = Self(prettyPrinted: true, sortedKeys: true)
}

struct CodableFileStore<Value: Codable> {
    let fileURL: URL
    let options: CodableFileStoreOptions

    init(fileURL: URL, options: CodableFileStoreOptions = .standard) {
        self.fileURL = fileURL
        self.options = options
    }

    func load() throws -> Value? {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return nil }
        let data = try Data(contentsOf: fileURL)
        return try JSONDecoder().decode(Value.self, from: data)
    }

    func save(_ value: Value) throws {
        let encoder = JSONEncoder()
        var formatting: JSONEncoder.OutputFormatting = []
        if options.prettyPrinted { formatting.insert(.prettyPrinted) }
        if options.sortedKeys { formatting.insert(.sortedKeys) }
        encoder.outputFormatting = formatting

        let data = try encoder.encode(value)
        try data.write(to: fileURL, options: .atomic)

        if let permissions = options.filePermissions {
            try FileManager.default.setAttributes(
                [.posixPermissions: permissions],
                ofItemAtPath: fileURL.path
            )
        }
    }

    func remove() throws {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return }
        try FileManager.default.removeItem(at: fileURL)
    }
}
