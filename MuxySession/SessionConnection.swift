import Darwin
import MuxySessionProtocol

final class SessionConnection {
    let descriptor: Int32
    var decoder = SessionFrameDecoder()
    var attachedSession: SessionIdentifier?
    var closesAfterFlush = false

    private var outbox: [UInt8] = []
    private var offset = 0

    init(descriptor: Int32) {
        self.descriptor = descriptor
    }

    var pendingByteCount: Int { outbox.count - offset }
    var hasPendingOutput: Bool { pendingByteCount > 0 }

    func enqueue(_ frame: SessionFrame) {
        outbox.append(contentsOf: frame.encoded())
    }

    func flush() -> Bool {
        while offset < outbox.count {
            switch SessionIO.write(descriptor, outbox, from: offset) {
            case let .wrote(count):
                offset += count
            case .wouldBlock:
                compact()
                return true
            case .failed:
                return false
            }
        }
        outbox.removeAll(keepingCapacity: true)
        offset = 0
        return true
    }

    private func compact() {
        guard offset > 0 else { return }
        if offset == outbox.count {
            outbox.removeAll(keepingCapacity: true)
            offset = 0
        } else if offset >= 1 << 16 {
            outbox.removeFirst(offset)
            offset = 0
        }
    }
}

final class PTYSession {
    let identifier: SessionIdentifier
    let processID: pid_t
    let ttyDevice: UInt64
    let workingDirectory: String

    var masterDescriptor: Int32
    var replay: SessionReplayBuffer
    var clientDescriptor: Int32?
    var isEnding = false
    var metadata: [SessionEnvironmentEntry] = []

    private var pendingInput: [UInt8] = []
    private var inputOffset = 0

    init(
        identifier: SessionIdentifier,
        process: SessionProcess,
        workingDirectory: String,
        replayCapacity: Int
    ) {
        self.identifier = identifier
        processID = process.processID
        ttyDevice = process.ttyDevice
        masterDescriptor = process.masterDescriptor
        self.workingDirectory = workingDirectory
        replay = SessionReplayBuffer(capacity: replayCapacity)
    }

    var hasPendingInput: Bool { inputOffset < pendingInput.count }

    var descriptor: SessionDescriptor {
        SessionDescriptor(
            identifier: identifier,
            shellProcessID: processID,
            ttyDevice: ttyDevice,
            workingDirectory: workingDirectory,
            isAttached: clientDescriptor != nil,
            metadata: metadata
        )
    }

    func appendInput(_ bytes: [UInt8], limit: Int) {
        guard pendingInput.count - inputOffset + bytes.count <= limit else {
            SessionLog.write("dropped \(bytes.count) input byte(s): the session is not draining its terminal")
            return
        }
        pendingInput.append(contentsOf: bytes)
    }

    func flushInput() {
        guard masterDescriptor >= 0 else { return }
        while inputOffset < pendingInput.count {
            switch SessionIO.write(masterDescriptor, pendingInput, from: inputOffset) {
            case let .wrote(count):
                inputOffset += count
            case .wouldBlock:
                compactInput()
                return
            case .failed:
                pendingInput.removeAll(keepingCapacity: true)
                inputOffset = 0
                return
            }
        }
        pendingInput.removeAll(keepingCapacity: true)
        inputOffset = 0
    }

    private func compactInput() {
        guard inputOffset >= 1 << 16 else { return }
        pendingInput.removeFirst(inputOffset)
        inputOffset = 0
    }
}
