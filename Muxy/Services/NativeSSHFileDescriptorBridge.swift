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

    static func make() throws -> NativeSSHFileDescriptorBridge {
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
            try setNonBlocking(sshToTerminal[1])
        } catch {
            closeIfOpen(terminalToSSH[0])
            closeIfOpen(terminalToSSH[1])
            closeIfOpen(sshToTerminal[0])
            closeIfOpen(sshToTerminal[1])
            throw error
        }

        return NativeSSHFileDescriptorBridge(
            ghosttyReadFD: sshToTerminal[0],
            ghosttyWriteFD: terminalToSSH[1],
            sshReadFD: terminalToSSH[0],
            sshWriteFD: sshToTerminal[1]
        )
    }

    func closeSSHSide() {
        guard !closed else { return }
        closed = true
        closeIfOpen(sshReadFD)
        closeIfOpen(sshWriteFD)
    }

    func closeAllBeforeSurfaceCreation() {
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

extension NativeSSHFileDescriptorBridge: @unchecked Sendable {}

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
