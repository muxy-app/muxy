import Foundation
import Testing

@testable import Muxy

@Suite("BackupService")
struct BackupServiceTests {
    private func tempDirectory() -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("BackupServiceTests-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    private func write(_ data: Data, named name: String, in directory: URL) throws {
        try data.write(to: directory.appendingPathComponent(name), options: .atomic)
    }

    private func seedSource() throws -> URL {
        let source = tempDirectory()
        try write(Data("[]".utf8), named: "projects.json", in: source)
        try write(Data(#"{"muxy.showStatusBar":true,"mobile.approvedDevices":[{"id":"x","tokenHash":"secret"}]}"#.utf8), named: "settings.json", in: source)

        let device = RemoteDevice(
            name: "Box",
            ssh: SSHWorkspaceData(host: "example.com", environment: ["SECRET_TOKEN": "abc", "TERM": "xterm-256color"])
        )
        try write(try JSONEncoder().encode([device]), named: "remote-devices.json", in: source)

        let logos = source.appendingPathComponent("logos", isDirectory: true)
        try FileManager.default.createDirectory(at: logos, withIntermediateDirectories: true)
        try write(Data("png".utf8), named: "logo.png", in: logos)
        return source
    }

    @Test("export then import round-trips files into a clean target")
    func roundTrip() throws {
        let source = try seedSource()
        let archive = tempDirectory().appendingPathComponent("backup.muxy")
        try BackupService(baseDirectory: source).export(to: archive, appVersion: "1.0", createdAt: Date())
        #expect(FileManager.default.fileExists(atPath: archive.path))

        let target = tempDirectory()
        try BackupService(baseDirectory: target).importBackup(from: archive, backupStamp: "stamp")

        #expect(FileManager.default.fileExists(atPath: target.appendingPathComponent("projects.json").path))
        let logo = target.appendingPathComponent("logos/logo.png")
        #expect(try Data(contentsOf: logo) == Data("png".utf8))
    }

    @Test("export strips SSH environment secrets")
    func stripsSSHEnvironment() throws {
        let source = try seedSource()
        let archive = tempDirectory().appendingPathComponent("backup.muxy")
        try BackupService(baseDirectory: source).export(to: archive, appVersion: "1.0", createdAt: Date())

        let target = tempDirectory()
        try BackupService(baseDirectory: target).importBackup(from: archive, backupStamp: "stamp")

        let data = try Data(contentsOf: target.appendingPathComponent("remote-devices.json"))
        let devices = try JSONDecoder().decode([RemoteDevice].self, from: data)
        #expect(devices.first?.ssh.environment == SSHEnvironmentVariables.default)
    }

    @Test("export empties approved devices from settings")
    func stripsApprovedDevices() throws {
        let source = try seedSource()
        let archive = tempDirectory().appendingPathComponent("backup.muxy")
        try BackupService(baseDirectory: source).export(to: archive, appVersion: "1.0", createdAt: Date())

        let target = tempDirectory()
        try BackupService(baseDirectory: target).importBackup(from: archive, backupStamp: "stamp")

        let data = try Data(contentsOf: target.appendingPathComponent("settings.json"))
        let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let approved = object?["mobile.approvedDevices"] as? [Any]
        #expect(approved?.isEmpty == true)
    }

    @Test("approved-devices file is never included in the archive")
    func excludesApprovedDevicesFile() throws {
        let source = try seedSource()
        try write(Data("[]".utf8), named: "approved-devices.json", in: source)
        let archive = tempDirectory().appendingPathComponent("backup.muxy")
        try BackupService(baseDirectory: source).export(to: archive, appVersion: "1.0", createdAt: Date())

        let target = tempDirectory()
        try BackupService(baseDirectory: target).importBackup(from: archive, backupStamp: "stamp")
        #expect(!FileManager.default.fileExists(atPath: target.appendingPathComponent("approved-devices.json").path))
    }

    @Test("import backs up existing data before replacing")
    func backsUpExistingData() throws {
        let source = try seedSource()
        let archive = tempDirectory().appendingPathComponent("backup.muxy")
        try BackupService(baseDirectory: source).export(to: archive, appVersion: "1.0", createdAt: Date())

        let target = tempDirectory()
        try write(Data(#"["old"]"#.utf8), named: "projects.json", in: target)
        let backupDirectory = try BackupService(baseDirectory: target).importBackup(from: archive, backupStamp: "stamp")

        let preserved = backupDirectory.appendingPathComponent("projects.json")
        #expect(try Data(contentsOf: preserved) == Data(#"["old"]"#.utf8))
    }

    @Test("import rejects an archive without a manifest")
    func rejectsMissingManifest() throws {
        let staging = tempDirectory()
        try write(Data("[]".utf8), named: "projects.json", in: staging)
        let archive = tempDirectory().appendingPathComponent("invalid.muxy")
        try BackupArchive.zip(directory: staging, to: archive)

        let target = tempDirectory()
        #expect(throws: BackupArchiveError.self) {
            try BackupService(baseDirectory: target).importBackup(from: archive, backupStamp: "stamp")
        }
    }

    @Test("import ignores entries not in the allowlist")
    func ignoresUnexpectedEntries() throws {
        let source = try seedSource()
        let archive = tempDirectory().appendingPathComponent("backup.muxy")
        try BackupService(baseDirectory: source).export(to: archive, appVersion: "1.0", createdAt: Date())

        let staging = tempDirectory()
        try BackupArchive.unzip(archiveURL: archive, to: staging)
        try write(Data("evil".utf8), named: "passwd", in: staging)

        let manifestURL = staging.appendingPathComponent(BackupManifest.filename)
        var manifest = try JSONDecoder.iso8601.decode(BackupManifest.self, from: Data(contentsOf: manifestURL))
        manifest.files.append("../passwd")
        try JSONEncoder.iso8601.encode(manifest).write(to: manifestURL, options: .atomic)

        let repacked = tempDirectory().appendingPathComponent("repacked.muxy")
        try BackupArchive.zip(directory: staging, to: repacked)

        let target = tempDirectory()
        try BackupService(baseDirectory: target).importBackup(from: repacked, backupStamp: "stamp")
        #expect(!FileManager.default.fileExists(atPath: target.appendingPathComponent("passwd").path))
    }
}

private extension JSONDecoder {
    static var iso8601: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }
}

private extension JSONEncoder {
    static var iso8601: JSONEncoder {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        return encoder
    }
}
