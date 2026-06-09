import Darwin
import Foundation

struct NativeSSHTerminalSize: Equatable {
    let columns: Int
    let rows: Int
    let widthPixels: Int
    let heightPixels: Int

    static let fallback = NativeSSHTerminalSize(columns: 80, rows: 24, widthPixels: 0, heightPixels: 0)
}

final class NativeSSHFileDescriptorBridge {
    let ghosttyReadFD: Int32
    let ghosttyWriteFD: Int32
    let sshReadFD: Int32
    let sshWriteFD: Int32

    private let directoryURL: URL
    private let terminalToSSHURL: URL
    private let sshToTerminalURL: URL
    private var closed = false

    private init(
        ghosttyReadFD: Int32,
        ghosttyWriteFD: Int32,
        sshReadFD: Int32,
        sshWriteFD: Int32,
        directoryURL: URL,
        terminalToSSHURL: URL,
        sshToTerminalURL: URL
    ) {
        self.ghosttyReadFD = ghosttyReadFD
        self.ghosttyWriteFD = ghosttyWriteFD
        self.sshReadFD = sshReadFD
        self.sshWriteFD = sshWriteFD
        self.directoryURL = directoryURL
        self.terminalToSSHURL = terminalToSSHURL
        self.sshToTerminalURL = sshToTerminalURL
    }

    static func make() throws -> NativeSSHFileDescriptorBridge {
        let directoryURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-native-ssh-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(
            at: directoryURL,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        let terminalToSSHURL = directoryURL.appendingPathComponent("terminal-to-ssh")
        let sshToTerminalURL = directoryURL.appendingPathComponent("ssh-to-terminal")

        guard mkfifo(terminalToSSHURL.path, mode_t(FilePermissions.privateFile)) == 0 else {
            try? FileManager.default.removeItem(at: directoryURL)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        guard mkfifo(sshToTerminalURL.path, mode_t(FilePermissions.privateFile)) == 0 else {
            try? FileManager.default.removeItem(at: directoryURL)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        let sshReadFD = open(terminalToSSHURL.path, O_RDWR | O_NONBLOCK)
        guard sshReadFD >= 0 else {
            try? FileManager.default.removeItem(at: directoryURL)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        let sshWriteFD = open(sshToTerminalURL.path, O_RDWR | O_NONBLOCK)
        guard sshWriteFD >= 0 else {
            close(sshReadFD)
            try? FileManager.default.removeItem(at: directoryURL)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        return NativeSSHFileDescriptorBridge(
            ghosttyReadFD: -1,
            ghosttyWriteFD: -1,
            sshReadFD: sshReadFD,
            sshWriteFD: sshWriteFD,
            directoryURL: directoryURL,
            terminalToSSHURL: terminalToSSHURL,
            sshToTerminalURL: sshToTerminalURL
        )
    }

    func closeSSHSide() {
        guard !closed else { return }
        closed = true
        closeIfOpen(sshReadFD)
        closeIfOpen(sshWriteFD)
        removeBridgeFiles()
    }

    func closeAllBeforeSurfaceCreation() {
        guard !closed else { return }
        closed = true
        closeIfOpen(ghosttyReadFD)
        closeIfOpen(ghosttyWriteFD)
        closeIfOpen(sshReadFD)
        closeIfOpen(sshWriteFD)
        removeBridgeFiles()
    }

    var terminalBridgeCommand: String {
        let outputPath = ShellEscaper.escape(sshToTerminalURL.path)
        let inputPath = ShellEscaper.escape(terminalToSSHURL.path)
        let script = "cat \(outputPath) & cat > \(inputPath); wait"
        return "/bin/sh -c \(ShellEscaper.escape(script))"
    }

    private func removeBridgeFiles() {
        try? FileManager.default.removeItem(at: directoryURL)
    }

    deinit {
        closeSSHSide()
    }
}

extension NativeSSHFileDescriptorBridge: @unchecked Sendable {}

private func closeIfOpen(_ fd: Int32) {
    guard fd >= 0 else { return }
    close(fd)
}
