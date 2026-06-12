import Darwin
import Foundation

public struct SSHTerminalSize: Equatable, Sendable {
    public let columns: Int
    public let rows: Int
    public let widthPixels: Int
    public let heightPixels: Int
    public static let fallback = SSHTerminalSize(columns: 80, rows: 24, widthPixels: 0, heightPixels: 0)

    public init(columns: Int, rows: Int, widthPixels: Int, heightPixels: Int) {
        self.columns = columns
        self.rows = rows
        self.widthPixels = widthPixels
        self.heightPixels = heightPixels
    }
}

public final class SSHFileDescriptorBridge {
    public let ghosttyReadFD: Int32
    public let ghosttyWriteFD: Int32
    public let sshReadFD: Int32
    public let sshWriteFD: Int32

    private var closed = false

    private init(
        ghosttyReadFD: Int32,
        ghosttyWriteFD: Int32,
        sshReadFD: Int32,
        sshWriteFD: Int32
    ) {
        self.ghosttyReadFD = ghosttyReadFD
        self.ghosttyWriteFD = ghosttyWriteFD
        self.sshReadFD = sshReadFD
        self.sshWriteFD = sshWriteFD
    }

    public static func make() throws -> SSHFileDescriptorBridge {
        var terminalToSSH: [Int32] = [-1, -1]
        var sshToTerminal: [Int32] = [-1, -1]

        guard socketpair(AF_UNIX, SOCK_STREAM, 0, &terminalToSSH) == 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        guard socketpair(AF_UNIX, SOCK_STREAM, 0, &sshToTerminal) == 0 else {
            closeIfOpen(terminalToSSH[0])
            closeIfOpen(terminalToSSH[1])
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        do {
            try setNonBlocking(terminalToSSH[0])
            try setNonBlocking(terminalToSSH[1])
            try setNonBlocking(sshToTerminal[0])
            try setNonBlocking(sshToTerminal[1])
        } catch {
            closeIfOpen(terminalToSSH[0])
            closeIfOpen(terminalToSSH[1])
            closeIfOpen(sshToTerminal[0])
            closeIfOpen(sshToTerminal[1])
            throw error
        }

        return SSHFileDescriptorBridge(
            ghosttyReadFD: sshToTerminal[0],
            ghosttyWriteFD: terminalToSSH[1],
            sshReadFD: terminalToSSH[0],
            sshWriteFD: sshToTerminal[1]
        )
    }

    public func closeSSHSide() {
        guard !closed else { return }
        closed = true
        closeIfOpen(sshReadFD)
        closeIfOpen(sshWriteFD)
    }

    public func closeAllBeforeSurfaceCreation() {
        guard !closed else { return }
        closed = true
        closeIfOpen(ghosttyReadFD)
        closeIfOpen(ghosttyWriteFD)
        closeIfOpen(sshReadFD)
        closeIfOpen(sshWriteFD)
    }

    deinit {
        closeSSHSide()
    }
}

extension SSHFileDescriptorBridge: @unchecked Sendable {}

private func closeIfOpen(_ fd: Int32) {
    guard fd >= 0 else { return }
    close(fd)
}

private func setNonBlocking(_ fd: Int32) throws {
    let flags = fcntl(fd, F_GETFL)
    guard flags >= 0, fcntl(fd, F_SETFL, flags | O_NONBLOCK) == 0 else {
        throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
    }
}
