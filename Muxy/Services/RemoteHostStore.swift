import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "RemoteHostStore")

@MainActor
@Observable
final class RemoteHostStore {
    static let shared = RemoteHostStore()

    private(set) var hosts: [RemoteHost] = []
    private let persistence: CodableFileStore<[RemoteHost]>

    private static var storageURL: URL {
        let configDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/muxy")
        return configDir.appendingPathComponent("remote-hosts.json")
    }

    init() {
        persistence = CodableFileStore(fileURL: Self.storageURL, options: .prettySorted)
        load()
    }

    private func load() {
        do {
            hosts = try persistence.load() ?? []
        } catch {
            logger.error("Failed to load remote hosts: \(error)")
            hosts = []
        }
    }

    private func save() {
        do {
            let configDir = Self.storageURL.deletingLastPathComponent()
            if !FileManager.default.fileExists(atPath: configDir.path) {
                try FileManager.default.createDirectory(at: configDir, withIntermediateDirectories: true)
            }
            try persistence.save(hosts)
        } catch {
            logger.error("Failed to save remote hosts: \(error)")
        }
    }

    func add(_ host: RemoteHost) {
        hosts.append(host)
        save()
    }

    func update(_ host: RemoteHost) {
        guard let index = hosts.firstIndex(where: { $0.id == host.id }) else { return }
        var updated = host
        updated.updatedAt = Date()
        hosts[index] = updated
        save()
    }

    func remove(id: UUID) {
        hosts.removeAll { $0.id == id }
        save()
    }

    func find(byID id: UUID) -> RemoteHost? {
        hosts.first { $0.id == id }
    }

    func importFromSSHConfig() -> [RemoteHost] {
        let parsed = SSHConfigParser.parse()
        var imported: [RemoteHost] = []

        for entry in parsed {
            guard !hosts.contains(where: { $0.host == entry.hostName && $0.user == (entry.user ?? NSUserName()) }) else {
                continue
            }

            let host = RemoteHost(
                name: entry.name,
                host: entry.hostName,
                port: entry.port,
                user: entry.user ?? NSUserName(),
                identityFile: entry.identityFile
            )
            hosts.append(host)
            imported.append(host)
        }

        if !imported.isEmpty {
            save()
        }

        return imported
    }

    func discoverSSHConfigHosts() -> [SSHConfigParser.ParsedHost] {
        SSHConfigParser.parse()
    }

    func ensureControlDir() {
        let controlDir = RemoteHost.controlPathBase()
        let controlURL = URL(fileURLWithPath: controlDir)
        guard !FileManager.default.fileExists(atPath: controlDir) else { return }
        do {
            try FileManager.default.createDirectory(at: controlURL, withIntermediateDirectories: true)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o700],
                ofItemAtPath: controlDir
            )
        } catch {
            logger.error("Failed to create control dir: \(error)")
        }
    }
}
