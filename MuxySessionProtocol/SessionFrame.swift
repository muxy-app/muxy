public enum SessionFrameKind: UInt8, Sendable, CaseIterable {
    case attach = 0x01
    case input = 0x02
    case resize = 0x03

    case attached = 0x11
    case output = 0x12
    case exited = 0x13
    case failure = 0x14

    case list = 0x21
    case info = 0x22
    case kill = 0x23
    case killAll = 0x24

    case sessions = 0x31
    case acknowledged = 0x32
}

public struct SessionFrame: Equatable, Sendable {
    public static let headerSize = 5
    public static let maximumPayloadSize = 1 << 20

    public let kind: SessionFrameKind
    public let payload: [UInt8]

    public init(kind: SessionFrameKind, payload: [UInt8] = []) {
        self.kind = kind
        self.payload = payload
    }

    public func encoded() -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeUInt8(kind.rawValue)
        writer.writeUInt32(UInt32(payload.count))
        writer.writeRaw(payload)
        return writer.bytes
    }
}

public struct SessionFrameDecoder: Sendable {
    private var buffer: [UInt8] = []
    private var consumed = 0

    public init() {}

    public var bufferedByteCount: Int { buffer.count - consumed }

    public mutating func push(_ bytes: [UInt8]) {
        buffer.append(contentsOf: bytes)
    }

    public mutating func push(_ bytes: UnsafeBufferPointer<UInt8>) {
        buffer.append(contentsOf: bytes)
    }

    public mutating func next() throws -> SessionFrame? {
        guard buffer.count - consumed >= SessionFrame.headerSize else {
            compact()
            return nil
        }
        let rawKind = buffer[consumed]
        var length = 0
        for index in 1 ..< SessionFrame.headerSize {
            length = length << 8 | Int(buffer[consumed + index])
        }
        guard length <= SessionFrame.maximumPayloadSize else {
            throw SessionProtocolError.frameTooLarge(length)
        }
        guard buffer.count - consumed - SessionFrame.headerSize >= length else {
            compact()
            return nil
        }
        guard let kind = SessionFrameKind(rawValue: rawKind) else {
            throw SessionProtocolError.unknownFrameKind(rawKind)
        }
        let start = consumed + SessionFrame.headerSize
        let payload = Array(buffer[start ..< start + length])
        consumed = start + length
        compact()
        return SessionFrame(kind: kind, payload: payload)
    }

    private mutating func compact() {
        guard consumed > 0 else { return }
        if consumed == buffer.count {
            buffer.removeAll(keepingCapacity: true)
        } else if consumed >= 1 << 16 {
            buffer.removeFirst(consumed)
        } else {
            return
        }
        consumed = 0
    }
}
