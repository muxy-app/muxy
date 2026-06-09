import CryptoKit
import Foundation
import NIOSSH

enum NativeSSHOpenSSHPrivateKeyParser {
    static func parse(_ text: String) throws -> NIOSSHPrivateKey {
        let body = text
            .split(separator: "\n")
            .filter { !$0.hasPrefix("-----") }
            .joined()
        guard let data = Data(base64Encoded: body) else {
            throw NativeSSHConnectionFailure.privateKeyLoadFailed
        }

        var reader = NativeSSHBinaryReader(data: data)
        guard try reader.readBytes(count: 15) == Array("openssh-key-v1\0".utf8) else {
            throw NativeSSHConnectionFailure.privateKeyLoadFailed
        }

        let cipherName = try reader.readString()
        let kdfName = try reader.readString()
        _ = try reader.readString()
        guard cipherName == "none", kdfName == "none" else {
            throw NativeSSHConnectionFailure.unsupportedPrivateKey
        }
        guard try reader.readUInt32() == 1 else {
            throw NativeSSHConnectionFailure.unsupportedPrivateKey
        }
        _ = try reader.readDataString()

        var privateReader = try NativeSSHBinaryReader(data: reader.readDataString())
        let check = try privateReader.readUInt32()
        guard try privateReader.readUInt32() == check else {
            throw NativeSSHConnectionFailure.privateKeyLoadFailed
        }
        guard try privateReader.readString() == "ssh-ed25519" else {
            throw NativeSSHConnectionFailure.unsupportedPrivateKey
        }
        _ = try privateReader.readDataString()
        let privateAndPublic = try privateReader.readBytesString()
        guard privateAndPublic.count == 64 else {
            throw NativeSSHConnectionFailure.privateKeyLoadFailed
        }

        let privateKeyBytes = privateAndPublic.prefix(32)
        return try NIOSSHPrivateKey(ed25519Key: Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyBytes))
    }
}

struct NativeSSHBinaryReader {
    private let data: Data
    private var offset = 0

    init(data: Data) {
        self.data = data
    }

    mutating func readUInt32() throws -> UInt32 {
        let bytes = try readBytes(count: 4)
        return bytes.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
    }

    mutating func readString() throws -> String {
        let data = try readDataString()
        guard let string = String(data: data, encoding: .utf8) else {
            throw NativeSSHConnectionFailure.privateKeyLoadFailed
        }
        return string
    }

    mutating func readDataString() throws -> Data {
        let count = try Int(readUInt32())
        return try Data(readBytes(count: count))
    }

    mutating func readBytesString() throws -> [UInt8] {
        let count = try Int(readUInt32())
        return try readBytes(count: count)
    }

    mutating func readBytes(count: Int) throws -> [UInt8] {
        guard count > -1, offset + count <= data.count else {
            throw NativeSSHConnectionFailure.privateKeyLoadFailed
        }
        defer { offset += count }
        return Array(data[offset ..< offset + count])
    }
}
