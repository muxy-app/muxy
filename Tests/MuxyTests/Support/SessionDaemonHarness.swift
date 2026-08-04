import Darwin
import Foundation
import MuxySessionProtocol

final class SessionDaemonHarness {
    static let binaryURL: URL? = {
        let root = RepositoryRoot.find()
        let candidates = [
            ProcessInfo.processInfo.environment["MUXY_SESSION_BINARY"].map { URL(fileURLWithPath: $0) },
            root.appendingPathComponent(".build/debug/muxy-session"),
            root.appendingPathComponent(".build/release/muxy-session"),
        ].compactMap(\.self)
        return candidates.first { FileManager.default.isExecutableFile(atPath: $0.path) }
    }()

    let directory: URL
    let socketPath: String

    private var daemon: Process?

    init() throws {
        let name = "mxs-" + UUID().uuidString.prefix(8)
        directory = URL(fileURLWithPath: "/tmp", isDirectory: true).appendingPathComponent(name, isDirectory: true)
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: false,
            attributes: [.posixPermissions: 0o700]
        )
        socketPath = directory.appendingPathComponent("c.sock").path
    }

    func start(idleTimeoutMilliseconds: Int? = nil) throws {
        guard let binaryURL = Self.binaryURL else { return }
        let process = Process()
        process.executableURL = binaryURL
        process.arguments = ["daemon", "--socket", socketPath]
            + (idleTimeoutMilliseconds.map { ["--idle-timeout", String($0)] } ?? [])
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        daemon = process

        let deadline = Date().addingTimeInterval(5)
        while Date() < deadline {
            if FileManager.default.fileExists(atPath: socketPath), let probe = SessionTestConnection(socketPath: socketPath) {
                probe.close()
                return
            }
            usleep(20_000)
        }
    }

    func stop() {
        if let connection = SessionTestConnection(socketPath: socketPath) {
            connection.send(SessionFrame(kind: .killAll))
            _ = connection.waitForFrame(timeout: 2) { $0.kind == .acknowledged }
            connection.close()
        }
        daemon?.terminate()
        daemon?.waitUntilExit()
        daemon = nil
        try? FileManager.default.removeItem(at: directory)
    }

    struct AttachClientResult {
        let status: Int32
        let output: String
    }

    func runAttachClient(
        identifier: SessionIdentifier,
        command: String,
        timeout: TimeInterval,
        daemonBinaryPath: String? = nil
    ) throws -> AttachClientResult {
        guard let binaryURL = Self.binaryURL else {
            return AttachClientResult(status: -1, output: "")
        }
        let process = Process()
        process.executableURL = binaryURL
        process.arguments = ["attach"]
        process.environment = [
            "PATH": "/usr/bin:/bin",
            "MUXY_SESSION_ID": identifier.uuidString,
            "MUXY_SESSION_SOCKET": socketPath,
            "MUXY_SESSION_BINARY": daemonBinaryPath ?? binaryURL.path,
            "MUXY_SESSION_SHELL": "/bin/sh",
            "MUXY_SESSION_CWD": "/tmp",
            "MUXY_SESSION_COMMAND": command,
        ]

        let outputURL = directory.appendingPathComponent("attach-\(identifier.uuidString).log")
        FileManager.default.createFile(atPath: outputURL.path, contents: nil)
        let outputHandle = try FileHandle(forWritingTo: outputURL)
        let input = Pipe()
        process.standardInput = input
        process.standardOutput = outputHandle
        process.standardError = outputHandle
        try process.run()

        let deadline = Date().addingTimeInterval(timeout)
        while process.isRunning, Date() < deadline {
            usleep(20_000)
        }
        if process.isRunning {
            process.terminate()
        }
        process.waitUntilExit()
        input.fileHandleForWriting.closeFile()
        try? outputHandle.close()

        return AttachClientResult(
            status: process.terminationStatus,
            output: (try? String(contentsOf: outputURL, encoding: .utf8)) ?? ""
        )
    }

    func attachRequest(
        identifier: SessionIdentifier,
        command: String,
        shell: String = "/bin/sh",
        columns: UInt16 = 80,
        rows: UInt16 = 24,
        environment: [SessionEnvironmentEntry] = [],
        metadata: [SessionEnvironmentEntry] = [],
        version: UInt16 = SessionProtocolVersion.current
    ) -> SessionFrame {
        SessionFrame(
            kind: .attach,
            payload: SessionAttachRequest(
                version: version,
                identifier: identifier,
                columns: columns,
                rows: rows,
                workingDirectory: "/tmp",
                command: command,
                shell: shell,
                resourcesDirectory: "",
                environment: [SessionEnvironmentEntry(key: "PATH", value: "/usr/bin:/bin")] + environment,
                metadata: metadata
            ).encoded()
        )
    }
}

final class SessionTestConnection {
    private let descriptor: Int32
    private var decoder = SessionFrameDecoder()
    private var pending: [SessionFrame] = []

    init?(socketPath: String) {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        let bytes = Array(socketPath.utf8)
        guard bytes.count < capacity else { return nil }
        withUnsafeMutablePointer(to: &address.sun_path) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: capacity) { destination in
                for (index, byte) in bytes.enumerated() {
                    destination[index] = CChar(bitPattern: byte)
                }
                destination[bytes.count] = 0
            }
        }
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)

        let socketDescriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard socketDescriptor >= 0 else { return nil }
        let connected = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { casted in
                Darwin.connect(socketDescriptor, casted, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard connected == 0 else {
            Darwin.close(socketDescriptor)
            return nil
        }
        descriptor = socketDescriptor
    }

    func send(_ frame: SessionFrame) {
        let bytes = frame.encoded()
        var offset = 0
        bytes.withUnsafeBytes { pointer in
            guard let base = pointer.baseAddress else { return }
            while offset < bytes.count {
                let written = Darwin.write(descriptor, base.advanced(by: offset), bytes.count - offset)
                guard written > 0 else { return }
                offset += written
            }
        }
    }

    func waitForFrame(timeout: TimeInterval, matching predicate: (SessionFrame) -> Bool) -> SessionFrame? {
        let deadline = Date().addingTimeInterval(timeout)
        while true {
            if let index = pending.firstIndex(where: predicate) {
                return pending.remove(at: index)
            }
            let remaining = deadline.timeIntervalSinceNow
            guard remaining > 0, readMore(timeout: remaining) else { return nil }
        }
    }

    func collectOutput(timeout: TimeInterval, until predicate: (String) -> Bool) -> String {
        var text = ""
        let deadline = Date().addingTimeInterval(timeout)
        while true {
            while let index = pending.firstIndex(where: { $0.kind == .output }) {
                let frame = pending.remove(at: index)
                text += String(decoding: frame.payload, as: UTF8.self)
            }
            if predicate(text) { return text }
            let remaining = deadline.timeIntervalSinceNow
            guard remaining > 0, readMore(timeout: remaining) else { return text }
        }
    }

    private func readMore(timeout: TimeInterval) -> Bool {
        var descriptors = [pollfd(fd: descriptor, events: Int16(POLLIN), revents: 0)]
        let ready = poll(&descriptors, 1, Int32(timeout * 1000))
        guard ready > 0 else { return false }

        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        let count = buffer.withUnsafeMutableBytes { pointer in
            Darwin.read(descriptor, pointer.baseAddress, 64 * 1024)
        }
        guard count > 0 else { return false }
        decoder.push(Array(buffer[0 ..< count]))
        while let frame = try? decoder.next() {
            pending.append(frame)
        }
        return true
    }

    func close() {
        Darwin.close(descriptor)
    }
}
