import CryptoKit
import Foundation
import NIOSSH

enum SSHOpenSSHPrivateKeyParser {
    private static let beginMarker = "-----BEGIN OPENSSH PRIVATE KEY-----"
    private static let endMarker = "-----END OPENSSH PRIVATE KEY-----"
    private static let opensshMagic = Array("openssh-key-v1\0".utf8)
    private static let keyType = "ssh-ed25519"
    private static let blockSize = 8

    static func parse(_ text: String) throws -> NIOSSHPrivateKey {
        let payload = try payload(from: text)
        guard let data = Data(base64Encoded: payload) else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        var reader = SSHBinaryReader(data: data)
        guard try reader.readBytes(count: Self.opensshMagic.count) == Self.opensshMagic else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        let cipherName = try reader.readString()
        let kdfName = try reader.readString()
        _ = try reader.readDataString()

        guard cipherName == "none", kdfName == "none" else {
            throw SSHConnectionFailure.encryptedPrivateKey
        }

        guard try reader.readUInt32() == 1 else {
            throw SSHConnectionFailure.unsupportedPrivateKey
        }

        let publicKey = try readPublicKey(from: reader.readDataString())
        let privateKey = try readPrivateKey(from: reader.readDataString(), expectedPublicKey: publicKey)

        guard reader.isAtEnd else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        return privateKey
    }

    private static func payload(from text: String) throws -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(Self.beginMarker), trimmed.hasSuffix(Self.endMarker) else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        return trimmed
            .split(separator: "\n")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty && !$0.hasPrefix("-----") }
            .joined()
    }

    private static func readPublicKey(from data: Data) throws -> Data {
        var reader = SSHBinaryReader(data: data)
        guard try reader.readString() == Self.keyType else {
            throw SSHConnectionFailure.unsupportedKeyType
        }

        let publicKey = try reader.readDataString()
        guard publicKey.count == 32, reader.isAtEnd else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }
        return publicKey
    }

    private static func readPrivateKey(from data: Data, expectedPublicKey: Data) throws -> NIOSSHPrivateKey {
        var reader = SSHBinaryReader(data: data)
        let check = try reader.readUInt32()
        guard try reader.readUInt32() == check else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        guard try reader.readString() == Self.keyType else {
            throw SSHConnectionFailure.unsupportedKeyType
        }

        let embeddedPublicKey = try reader.readDataString()
        guard embeddedPublicKey == expectedPublicKey else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        let privateAndPublic = try reader.readDataString()
        guard privateAndPublic.count == 64 else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        let privateKeyData = privateAndPublic.prefix(32)
        let publicKeyData = privateAndPublic.suffix(32)
        guard Data(publicKeyData) == expectedPublicKey else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        _ = try reader.readString()
        try validatePadding(reader.remainingBytes())

        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyData)
        guard privateKey.publicKey.rawRepresentation == expectedPublicKey else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        return NIOSSHPrivateKey(ed25519Key: privateKey)
    }

    private static func validatePadding(_ padding: [UInt8]) throws {
        guard padding.count < Self.blockSize else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }

        for (index, byte) in padding.enumerated() {
            guard byte == UInt8(index + 1) else {
                throw SSHConnectionFailure.privateKeyLoadFailed
            }
        }
    }
}

struct SSHBinaryReader {
    private let data: Data
    private var offset = 0

    init(data: Data) {
        self.data = data
    }

    var isAtEnd: Bool {
        offset == data.count
    }

    mutating func readUInt32() throws -> UInt32 {
        let bytes = try readBytes(count: 4)
        return bytes.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
    }

    mutating func readString() throws -> String {
        let data = try readDataString()
        guard let string = String(data: data, encoding: .utf8) else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }
        return string
    }

    mutating func readDataString() throws -> Data {
        let count = try Int(readUInt32())
        return try Data(readBytes(count: count))
    }

    mutating func remainingBytes() -> [UInt8] {
        let bytes = Array(data[offset...])
        offset = data.count
        return bytes
    }

    mutating func readBytes(count: Int) throws -> [UInt8] {
        guard count > -1, offset + count <= data.count else {
            throw SSHConnectionFailure.privateKeyLoadFailed
        }
        defer { offset += count }
        return Array(data[offset ..< offset + count])
    }
}
