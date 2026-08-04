public struct SessionByteWriter: Sendable {
    public private(set) var bytes: [UInt8] = []

    public init() {}

    public mutating func writeUInt8(_ value: UInt8) {
        bytes.append(value)
    }

    public mutating func writeUInt16(_ value: UInt16) {
        bytes.append(UInt8(truncatingIfNeeded: value >> 8))
        bytes.append(UInt8(truncatingIfNeeded: value))
    }

    public mutating func writeUInt32(_ value: UInt32) {
        for shift in stride(from: 24, through: 0, by: -8) {
            bytes.append(UInt8(truncatingIfNeeded: value >> UInt32(shift)))
        }
    }

    public mutating func writeUInt64(_ value: UInt64) {
        for shift in stride(from: 56, through: 0, by: -8) {
            bytes.append(UInt8(truncatingIfNeeded: value >> UInt64(shift)))
        }
    }

    public mutating func writeInt32(_ value: Int32) {
        writeUInt32(UInt32(bitPattern: value))
    }

    public mutating func writeRaw(_ value: [UInt8]) {
        bytes.append(contentsOf: value)
    }

    public mutating func writeString(_ value: String) {
        let encoded = Array(value.utf8)
        writeUInt32(UInt32(encoded.count))
        bytes.append(contentsOf: encoded)
    }

    public mutating func writeIdentifier(_ value: SessionIdentifier) {
        bytes.append(contentsOf: value.bytes)
    }

    public mutating func writeEntries(_ entries: [SessionEnvironmentEntry]) {
        writeUInt32(UInt32(entries.count))
        for entry in entries {
            writeString(entry.key)
            writeString(entry.value)
        }
    }
}

public struct SessionByteReader {
    private let bytes: [UInt8]
    private var offset: Int

    public init(_ bytes: [UInt8]) {
        self.bytes = bytes
        offset = 0
    }

    public var isAtEnd: Bool { offset >= bytes.count }

    public mutating func readUInt8() throws -> UInt8 {
        try requireRemaining(1)
        defer { offset += 1 }
        return bytes[offset]
    }

    public mutating func readUInt16() throws -> UInt16 {
        try requireRemaining(2)
        defer { offset += 2 }
        return UInt16(bytes[offset]) << 8 | UInt16(bytes[offset + 1])
    }

    public mutating func readUInt32() throws -> UInt32 {
        try requireRemaining(4)
        defer { offset += 4 }
        var value: UInt32 = 0
        for index in 0 ..< 4 {
            value = value << 8 | UInt32(bytes[offset + index])
        }
        return value
    }

    public mutating func readUInt64() throws -> UInt64 {
        try requireRemaining(8)
        defer { offset += 8 }
        var value: UInt64 = 0
        for index in 0 ..< 8 {
            value = value << 8 | UInt64(bytes[offset + index])
        }
        return value
    }

    public mutating func readInt32() throws -> Int32 {
        try Int32(bitPattern: readUInt32())
    }

    public mutating func readRaw(_ count: Int) throws -> [UInt8] {
        try requireRemaining(count)
        defer { offset += count }
        return Array(bytes[offset ..< offset + count])
    }

    public mutating func readString() throws -> String {
        let length = try Int(readUInt32())
        let raw = try readRaw(length)
        return String(decoding: raw, as: UTF8.self)
    }

    public mutating func readIdentifier() throws -> SessionIdentifier {
        let raw = try readRaw(SessionIdentifier.byteCount)
        guard let identifier = SessionIdentifier(bytes: raw) else {
            throw SessionProtocolError.malformedPayload
        }
        return identifier
    }

    public mutating func readEntries() throws -> [SessionEnvironmentEntry] {
        let count = try Int(readUInt32())
        guard count >= 0, count <= bytes.count else { throw SessionProtocolError.malformedPayload }
        var entries: [SessionEnvironmentEntry] = []
        entries.reserveCapacity(count)
        for _ in 0 ..< count {
            try entries.append(SessionEnvironmentEntry(key: readString(), value: readString()))
        }
        return entries
    }

    public mutating func readRemaining() -> [UInt8] {
        defer { offset = bytes.count }
        return Array(bytes[offset...])
    }

    private func requireRemaining(_ count: Int) throws {
        guard count >= 0, bytes.count - offset >= count else {
            throw SessionProtocolError.malformedPayload
        }
    }
}

public enum SessionProtocolError: Error, Equatable, Sendable {
    case malformedPayload
    case unknownFrameKind(UInt8)
    case frameTooLarge(Int)
}
