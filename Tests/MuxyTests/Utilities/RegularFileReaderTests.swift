import Darwin
import Foundation
import Testing

@testable import Muxy

@Suite("RegularFileReader")
struct RegularFileReaderTests {
    private let maximumByteCount = 1024

    @Test("reads a regular file in full")
    func readsRegularFile() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("payload.bin")
        let payload = Data(repeating: 7, count: 512)
        try payload.write(to: url)

        #expect(try RegularFileReader.data(contentsOf: url, maximumByteCount: maximumByteCount) == payload)
    }

    @Test("rejects special-file metadata without opening the file")
    func rejectsSpecialFileMetadata() {
        #expect(throws: RegularFileReadError.unsupportedFileType) {
            try RegularFileReader.validateMetadata(
                isRegularFile: false,
                fileSize: nil,
                maximumByteCount: maximumByteCount
            )
        }
    }

    @Test("rejects an oversized file before reading it")
    func rejectsOversizedFile() {
        #expect(throws: RegularFileReadError.tooLarge(maximumByteCount: maximumByteCount)) {
            try RegularFileReader.validateMetadata(
                isRegularFile: true,
                fileSize: maximumByteCount + 1,
                maximumByteCount: maximumByteCount
            )
        }
    }

    @Test("reads an empty regular file")
    func readsEmptyFile() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("empty.bin")
        #expect(FileManager.default.createFile(atPath: url.path, contents: nil))

        #expect(try RegularFileReader.data(contentsOf: url, maximumByteCount: maximumByteCount).isEmpty)
    }

    @Test("rejects a directory")
    func rejectsDirectory() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }

        #expect(throws: RegularFileReadError.unsupportedFileType) {
            try RegularFileReader.data(contentsOf: directory, maximumByteCount: maximumByteCount)
        }
    }

    @Test("rejects a FIFO without waiting for a writer")
    func rejectsFIFO() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("payload.pipe")
        let result = url.withUnsafeFileSystemRepresentation { path in
            guard let path else { return Int32(-1) }
            return Darwin.mkfifo(path, S_IRUSR | S_IWUSR)
        }
        #expect(result == 0)
        let clock = ContinuousClock()
        let start = clock.now

        #expect(throws: RegularFileReadError.self) {
            try RegularFileReader.data(contentsOf: url, maximumByteCount: maximumByteCount)
        }

        #expect(start.duration(to: clock.now) < .seconds(1))
    }

    private func temporaryDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: false
        )
        return directory
    }
}
