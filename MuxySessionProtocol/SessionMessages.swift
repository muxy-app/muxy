public enum SessionProtocolVersion {
    public static let current: UInt16 = 1
}

public struct SessionAttachRequest: Equatable, Sendable {
    public let version: UInt16
    public let identifier: SessionIdentifier
    public let columns: UInt16
    public let rows: UInt16
    public let workingDirectory: String
    public let command: String
    public let shell: String
    public let resourcesDirectory: String
    public let environment: [SessionEnvironmentEntry]
    public let metadata: [SessionEnvironmentEntry]

    public init(
        version: UInt16 = SessionProtocolVersion.current,
        identifier: SessionIdentifier,
        columns: UInt16,
        rows: UInt16,
        workingDirectory: String,
        command: String,
        shell: String,
        resourcesDirectory: String,
        environment: [SessionEnvironmentEntry],
        metadata: [SessionEnvironmentEntry] = []
    ) {
        self.version = version
        self.identifier = identifier
        self.columns = columns
        self.rows = rows
        self.workingDirectory = workingDirectory
        self.command = command
        self.shell = shell
        self.resourcesDirectory = resourcesDirectory
        self.environment = environment
        self.metadata = metadata
    }

    public func encoded() -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeUInt16(version)
        writer.writeIdentifier(identifier)
        writer.writeUInt16(columns)
        writer.writeUInt16(rows)
        writer.writeString(workingDirectory)
        writer.writeString(command)
        writer.writeString(shell)
        writer.writeString(resourcesDirectory)
        writer.writeEntries(environment)
        writer.writeEntries(metadata)
        return writer.bytes
    }

    public static func decode(_ payload: [UInt8]) throws -> SessionAttachRequest {
        var reader = SessionByteReader(payload)
        return try SessionAttachRequest(
            version: reader.readUInt16(),
            identifier: reader.readIdentifier(),
            columns: reader.readUInt16(),
            rows: reader.readUInt16(),
            workingDirectory: reader.readString(),
            command: reader.readString(),
            shell: reader.readString(),
            resourcesDirectory: reader.readString(),
            environment: reader.readEntries(),
            metadata: reader.readEntries()
        )
    }
}

public struct SessionEnvironmentEntry: Equatable, Sendable {
    public let key: String
    public let value: String

    public init(key: String, value: String) {
        self.key = key
        self.value = value
    }
}

public struct SessionAttachAccepted: Equatable, Sendable {
    public let created: Bool
    public let shellProcessID: Int32
    public let ttyDevice: UInt64

    public init(created: Bool, shellProcessID: Int32, ttyDevice: UInt64) {
        self.created = created
        self.shellProcessID = shellProcessID
        self.ttyDevice = ttyDevice
    }

    public func encoded() -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeUInt8(created ? 1 : 0)
        writer.writeInt32(shellProcessID)
        writer.writeUInt64(ttyDevice)
        return writer.bytes
    }

    public static func decode(_ payload: [UInt8]) throws -> SessionAttachAccepted {
        var reader = SessionByteReader(payload)
        return try SessionAttachAccepted(
            created: reader.readUInt8() == 1,
            shellProcessID: reader.readInt32(),
            ttyDevice: reader.readUInt64()
        )
    }
}

public struct SessionDescriptor: Equatable, Sendable {
    public let identifier: SessionIdentifier
    public let shellProcessID: Int32
    public let ttyDevice: UInt64
    public let workingDirectory: String
    public let isAttached: Bool
    public let metadata: [SessionEnvironmentEntry]

    public init(
        identifier: SessionIdentifier,
        shellProcessID: Int32,
        ttyDevice: UInt64,
        workingDirectory: String,
        isAttached: Bool,
        metadata: [SessionEnvironmentEntry] = []
    ) {
        self.identifier = identifier
        self.shellProcessID = shellProcessID
        self.ttyDevice = ttyDevice
        self.workingDirectory = workingDirectory
        self.isAttached = isAttached
        self.metadata = metadata
    }

    public func value(forMetadataKey key: String) -> String? {
        metadata.first { $0.key == key }?.value
    }

    public func encode(into writer: inout SessionByteWriter) {
        writer.writeIdentifier(identifier)
        writer.writeInt32(shellProcessID)
        writer.writeUInt64(ttyDevice)
        writer.writeString(workingDirectory)
        writer.writeUInt8(isAttached ? 1 : 0)
        writer.writeEntries(metadata)
    }

    public static func decode(from reader: inout SessionByteReader) throws -> SessionDescriptor {
        try SessionDescriptor(
            identifier: reader.readIdentifier(),
            shellProcessID: reader.readInt32(),
            ttyDevice: reader.readUInt64(),
            workingDirectory: reader.readString(),
            isAttached: reader.readUInt8() == 1,
            metadata: reader.readEntries()
        )
    }

    public static func encodeList(_ descriptors: [SessionDescriptor]) -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeUInt32(UInt32(descriptors.count))
        for descriptor in descriptors {
            descriptor.encode(into: &writer)
        }
        return writer.bytes
    }

    public static func decodeList(_ payload: [UInt8]) throws -> [SessionDescriptor] {
        var reader = SessionByteReader(payload)
        let count = try Int(reader.readUInt32())
        var descriptors: [SessionDescriptor] = []
        descriptors.reserveCapacity(count)
        for _ in 0 ..< count {
            try descriptors.append(SessionDescriptor.decode(from: &reader))
        }
        return descriptors
    }
}

public enum SessionResizePayload {
    public static func encode(columns: UInt16, rows: UInt16) -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeUInt16(columns)
        writer.writeUInt16(rows)
        return writer.bytes
    }

    public static func decode(_ payload: [UInt8]) throws -> (columns: UInt16, rows: UInt16) {
        var reader = SessionByteReader(payload)
        return try (reader.readUInt16(), reader.readUInt16())
    }
}

public enum SessionExitPayload {
    public static func encode(status: Int32) -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeInt32(status)
        return writer.bytes
    }

    public static func decode(_ payload: [UInt8]) throws -> Int32 {
        var reader = SessionByteReader(payload)
        return try reader.readInt32()
    }
}

public enum SessionTextPayload {
    public static func encode(_ text: String) -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeString(text)
        return writer.bytes
    }

    public static func decode(_ payload: [UInt8]) throws -> String {
        var reader = SessionByteReader(payload)
        return try reader.readString()
    }
}

public enum SessionIdentifierPayload {
    public static func encode(_ identifier: SessionIdentifier) -> [UInt8] {
        var writer = SessionByteWriter()
        writer.writeIdentifier(identifier)
        return writer.bytes
    }

    public static func decode(_ payload: [UInt8]) throws -> SessionIdentifier {
        var reader = SessionByteReader(payload)
        return try reader.readIdentifier()
    }
}
