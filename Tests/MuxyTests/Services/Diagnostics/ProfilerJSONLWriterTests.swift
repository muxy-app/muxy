import Foundation
import Testing

@testable import Muxy

@Suite("Profiler JSONL writer")
struct ProfilerJSONLWriterTests {
    @Test("records append as valid newline-delimited JSON with private permissions")
    func appendsJSONLines() throws {
        let context = try makeContext()
        defer { try? FileManager.default.removeItem(at: context.directoryURL) }
        let writer = ProfilerJSONLWriter(fileURL: context.fileURL)
        let session = makeSession()

        try writer.startSession(session)
        try writer.append(makeSample(footprint: 123), session: session)

        let data = try Data(contentsOf: context.fileURL)
        let lines = data.split(separator: 0x0A)
        #expect(data.last == 0x0A)
        #expect(lines.count == 2)
        for line in lines {
            #expect(throws: Never.self) {
                try JSONSerialization.jsonObject(with: Data(line))
            }
        }

        let directoryPermissions = try permissions(at: context.directoryURL)
        let filePermissions = try permissions(at: context.fileURL)
        #expect(directoryPermissions == 0o700)
        #expect(filePermissions == 0o600)
    }

    @Test("append repairs a missing newline before the next record")
    func repairsMissingNewline() throws {
        let context = try makeContext()
        defer { try? FileManager.default.removeItem(at: context.directoryURL) }
        try FileManager.default.createDirectory(at: context.directoryURL, withIntermediateDirectories: true)
        try Data("{\"existing\":true}".utf8).write(to: context.fileURL)
        let writer = ProfilerJSONLWriter(fileURL: context.fileURL)

        try writer.startSession(makeSession())

        let text = try String(contentsOf: context.fileURL, encoding: .utf8)
        #expect(text.contains("{\"existing\":true}\n{"))
        #expect(text.hasSuffix("\n"))
    }

    @Test("append removes a truncated trailing record")
    func removesTruncatedTrailingRecord() throws {
        let context = try makeContext()
        defer { try? FileManager.default.removeItem(at: context.directoryURL) }
        let writer = ProfilerJSONLWriter(fileURL: context.fileURL)
        let session = makeSession()
        try writer.startSession(session)
        let validSessionData = try Data(contentsOf: context.fileURL)
        var interruptedData = validSessionData
        interruptedData.append(Data("{\"record_type\":\"sam".utf8))
        try interruptedData.write(to: context.fileURL)

        try writer.append(makeSample(footprint: 321), session: session)

        let data = try Data(contentsOf: context.fileURL)
        let lines = data.split(separator: 0x0A)
        #expect(lines.count == 2)
        for line in lines {
            #expect(throws: Never.self) {
                try JSONSerialization.jsonObject(with: Data(line))
            }
        }
        let sample = try #require(JSONSerialization.jsonObject(with: Data(lines[1])) as? [String: Any])
        #expect(sample["physical_footprint_bytes"] as? Int == 321)
    }

    @Test("rollover keeps the current session header and newest sample")
    func rolloverRetainsSessionAndNewestRecord() throws {
        let context = try makeContext()
        defer { try? FileManager.default.removeItem(at: context.directoryURL) }
        let maximumFileSize = 700
        let writer = ProfilerJSONLWriter(fileURL: context.fileURL, maximumFileSize: maximumFileSize)
        let session = makeSession()
        let newestSample = makeSample(footprint: 999)
        let sessionLine = try ProfilerRecordEncoder.encode(session)
        let sampleLine = try ProfilerRecordEncoder.encode(newestSample)
        var existingData = Data()
        existingData.append(sessionLine)
        existingData.append(0x0A)
        while existingData.count + sampleLine.count + 1 <= maximumFileSize {
            existingData.append(sampleLine)
            existingData.append(0x0A)
        }
        #expect(existingData.count <= maximumFileSize)
        #expect(existingData.count + sampleLine.count + 1 > maximumFileSize)
        try FileManager.default.createDirectory(at: context.directoryURL, withIntermediateDirectories: true)
        try existingData.write(to: context.fileURL)

        try writer.append(newestSample, session: session)

        let data = try Data(contentsOf: context.fileURL)
        let lines = data.split(separator: 0x0A)
        #expect(lines.count == 2)
        let header = try #require(JSONSerialization.jsonObject(with: Data(lines[0])) as? [String: Any])
        let sample = try #require(JSONSerialization.jsonObject(with: Data(lines[1])) as? [String: Any])
        #expect(header["record_type"] as? String == "session")
        #expect(header["app_version"] as? String == "1.2.3")
        #expect(sample["record_type"] as? String == "sample")
        #expect(sample["physical_footprint_bytes"] as? Int == 999)
        #expect(data.last == 0x0A)
    }

    private func makeContext() throws -> (directoryURL: URL, fileURL: URL) {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-profiler-tests-\(UUID().uuidString)", isDirectory: true)
        return (directoryURL, directoryURL.appendingPathComponent("profiler.jsonl"))
    }

    private func permissions(at url: URL) throws -> Int {
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        return try #require((attributes[.posixPermissions] as? NSNumber)?.intValue) & 0o777
    }

    private func makeSession() -> ProfilerSessionRecord {
        ProfilerSessionRecord(
            timestamp: Date(timeIntervalSince1970: 1_000),
            appVersion: "1.2.3",
            appBuild: "456",
            macOSVersion: "15.4.0",
            architecture: "arm64",
            samplingIntervalSeconds: 60
        )
    }

    private func makeSample(footprint: UInt64) -> ProfilerSampleRecord {
        ProfilerSampleRecord(
            timestamp: Date(timeIntervalSince1970: 1_060),
            profilerUptimeSeconds: 60,
            cpuPercent: 12.5,
            physicalFootprintBytes: footprint
        )
    }
}
